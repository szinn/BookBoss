use std::collections::HashMap;

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

// ── Shared data structures
// ────────────────────────────────────────────────────

/// All identifiers are represented as a map from `IdentifierType` serde name
/// (e.g. `"Isbn13"`, `"Hardcover"`) to value string.
pub(crate) type IdentifierMap = HashMap<String, String>;

/// All data needed to populate the review page on initial load.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct BookReviewData {
    pub job_token: String,
    pub book_token: String,
    pub title: String,
    pub description: String,
    pub published_date: String,
    pub language: String,
    pub series_name: String,
    pub series_number: String,
    pub publisher_name: String,
    pub page_count: String,
    /// Comma-separated author names in sort order.
    pub authors_csv: String,
    pub identifiers: IdentifierMap,
    /// Provider names in priority order (for rendering provider buttons).
    pub provider_names: Vec<String>,
    /// Pixel dimensions (width, height) of the stored cover, if any.
    pub cover_dimensions: Option<(u32, u32)>,
}

/// Metadata returned by a single provider fetch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ProviderResult {
    pub title: String,
    pub description: String,
    pub published_date: String,
    pub language: String,
    pub series_name: String,
    pub series_number: String,
    pub publisher_name: String,
    pub page_count: String,
    pub authors_csv: String,
    pub identifiers: IdentifierMap,
    /// Base64 encoded cover from the provider, if any.
    pub cover_thumbnail: Option<String>,
    /// Pixel dimensions (width, height) of the provider cover, if any.
    pub cover_dimensions: Option<(u32, u32)>,
}

/// All edit fields submitted to the server on approval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct BookEditFields {
    pub job_token: String,
    pub title: String,
    pub description: String,
    pub published_date: String,
    pub language: String,
    pub series_name: String,
    pub series_number: String,
    pub publisher_name: String,
    pub page_count: String,
    pub authors_csv: String,
    pub identifiers: IdentifierMap,
    pub use_fetched_cover: bool,
}

// ── Server-only imports
// ───────────────────────────────────────────────────────

#[cfg(feature = "server")]
use {
    crate::server::{AuthSession, AuthUser, BackendSessionPool},
    axum::http::Method,
    axum_session_auth::{Auth, Rights},
    base64::{Engine as _, engine::general_purpose::STANDARD as B64},
    bb_core::{
        CoreServices,
        book::{AuthorToken, BookToken, IdentifierType, PublisherToken, SeriesToken},
        import::ImportJobToken,
        pipeline::{BookEdit, ProviderBook},
        types::Capability,
        user::UserId,
    },
    rust_decimal::Decimal,
    std::{str::FromStr, sync::Arc},
};

// ── Helpers (server only)
// ─────────────────────────────────────────────────────

#[cfg(feature = "server")]
pub(crate) fn identifier_type_key(t: &IdentifierType) -> &'static str {
    match t {
        IdentifierType::Isbn13 => "Isbn13",
        IdentifierType::Isbn10 => "Isbn10",
        IdentifierType::Asin => "Asin",
        IdentifierType::GoogleBooks => "GoogleBooks",
        IdentifierType::OpenLibrary => "OpenLibrary",
        IdentifierType::Hardcover => "Hardcover",
    }
}

#[cfg(feature = "server")]
fn key_to_identifier_type(key: &str) -> Option<IdentifierType> {
    match key {
        "Isbn13" => Some(IdentifierType::Isbn13),
        "Isbn10" => Some(IdentifierType::Isbn10),
        "Asin" => Some(IdentifierType::Asin),
        "GoogleBooks" => Some(IdentifierType::GoogleBooks),
        "OpenLibrary" => Some(IdentifierType::OpenLibrary),
        "Hardcover" => Some(IdentifierType::Hardcover),
        _ => None,
    }
}

#[cfg(feature = "server")]
fn cover_to_base64(bytes: &[u8]) -> String {
    let mime = if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "image/png"
    } else if bytes.starts_with(&[0x47, 0x49, 0x46]) {
        "image/gif"
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        "image/jpeg"
    };
    format!("data:{};base64,{}", mime, B64.encode(bytes))
}

#[cfg(feature = "server")]
pub(crate) fn image_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) && data.len() >= 24 {
        let w = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let h = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
        return Some((w, h));
    }
    if (data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a")) && data.len() >= 10 {
        let w = u16::from_le_bytes([data[6], data[7]]) as u32;
        let h = u16::from_le_bytes([data[8], data[9]]) as u32;
        return Some((w, h));
    }
    if data.len() >= 30 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
        match &data[12..16] {
            b"VP8 " => {
                let w = (u16::from_le_bytes([data[26], data[27]]) & 0x3FFF) as u32;
                let h = (u16::from_le_bytes([data[28], data[29]]) & 0x3FFF) as u32;
                return Some((w, h));
            }
            b"VP8L" if data.len() >= 25 => {
                let bits = u32::from_le_bytes([data[21], data[22], data[23], data[24]]);
                return Some(((bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1));
            }
            b"VP8X" => {
                let w = u32::from_le_bytes([data[24], data[25], data[26], 0]) + 1;
                let h = u32::from_le_bytes([data[27], data[28], data[29], 0]) + 1;
                return Some((w, h));
            }
            _ => {}
        }
    }
    if data.starts_with(&[0xFF, 0xD8]) {
        let mut i = 2usize;
        while i + 3 < data.len() {
            if data[i] != 0xFF {
                break;
            }
            let marker = data[i + 1];
            if matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) && i + 8 < data.len() {
                let h = u16::from_be_bytes([data[i + 5], data[i + 6]]) as u32;
                let w = u16::from_be_bytes([data[i + 7], data[i + 8]]) as u32;
                return Some((w, h));
            }
            let len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
            if len < 2 {
                break;
            }
            i += 2 + len;
        }
    }
    None
}

#[cfg(feature = "server")]
fn provider_book_to_result(pb: ProviderBook) -> ProviderResult {
    let meta = &pb.metadata;
    let title = meta.title.clone().unwrap_or_default();
    let description = meta.description.clone().unwrap_or_default();
    let published_date = meta.published_date.map(|y| y.to_string()).unwrap_or_default();
    let language = meta.language.clone().unwrap_or_default();
    let series_name = meta.series_name.clone().unwrap_or_default();
    let series_number = meta.series_number.as_ref().map(|n| n.to_string()).unwrap_or_default();
    let publisher_name = meta.publisher.clone().unwrap_or_default();
    let authors_csv = meta
        .authors
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|a| a.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    let identifiers = meta
        .identifiers
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|i| (identifier_type_key(&i.identifier_type).to_string(), i.value.clone()))
        .collect();
    // Provider cover bytes take priority; fall back to embedded cover in metadata.
    let cover_bytes = pb.cover_bytes.as_deref().or_else(|| meta.cover_bytes.as_deref());
    let cover_dimensions = cover_bytes.and_then(image_dimensions);
    let cover_thumbnail = cover_bytes.map(cover_to_base64);
    ProviderResult {
        title,
        description,
        published_date,
        language,
        series_name,
        series_number,
        publisher_name,
        page_count: String::new(),
        authors_csv,
        identifiers,
        cover_thumbnail,
        cover_dimensions,
    }
}

// ── Server functions
// ──────────────────────────────────────────────────────────

#[post(
    "/api/v1/incoming/review",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn get_review_data(job_token: String) -> Result<BookReviewData, ServerFnError> {
    let current_user = auth_session.current_user.clone().unwrap_or_default();
    if !Auth::<AuthUser, UserId, BackendSessionPool>::build([Method::POST], true)
        .requires(Rights::any([Rights::permission(Capability::ApproveImports.as_str())]))
        .validate(&current_user, &Method::POST, None)
        .await
    {
        return Err(ServerFnError::new("Forbidden"));
    }

    let token: ImportJobToken = job_token.parse().map_err(|_| ServerFnError::new("Invalid token"))?;
    let import_service = &core_services.import_job_service;
    let book_service = &core_services.book_service;
    let pipeline_service = &core_services.pipeline_service;

    let job = import_service
        .find_by_token(&token)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("Job not found"))?;

    let book_id = job.candidate_book_id.ok_or_else(|| ServerFnError::new("No candidate book"))?;
    let book = book_service
        .find_book_by_token(&BookToken::new(book_id))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("Book not found"))?;

    // Authors sorted by sort_order
    let book_author_links = {
        let mut links = book_service.authors_for_book(book.id).await.map_err(|e| ServerFnError::new(e.to_string()))?;
        links.sort_by_key(|a| a.sort_order);
        links
    };
    let mut author_names = Vec::with_capacity(book_author_links.len());
    for ba in &book_author_links {
        if let Some(author) = book_service
            .find_author_by_token(&AuthorToken::new(ba.author_id))
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
        {
            author_names.push(author.name);
        }
    }

    // Series name
    let series_name = if let Some(sid) = book.series_id {
        book_service
            .find_series_by_token(&SeriesToken::new(sid))
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .map(|s| s.name)
            .unwrap_or_default()
    } else {
        String::new()
    };

    // Publisher name
    let publisher_name = if let Some(pid) = book.publisher_id {
        book_service
            .find_publisher_by_token(&PublisherToken::new(pid))
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .map(|p| p.name)
            .unwrap_or_default()
    } else {
        String::new()
    };

    // Identifiers
    let raw_identifiers = book_service
        .identifiers_for_book(book.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let identifiers: IdentifierMap = raw_identifiers
        .iter()
        .map(|i| (identifier_type_key(&i.identifier_type).to_string(), i.value.clone()))
        .collect();

    let provider_names = pipeline_service.list_provider_names().into_iter().map(|s| s.to_string()).collect();

    // Read cover file to determine dimensions.
    let cover_dimensions = if let Some(filename) = &book.cover_path {
        let path = core_services.library_store.cover_path(&book.token, filename);
        tokio::fs::read(&path).await.ok().and_then(|b| image_dimensions(&b))
    } else {
        None
    };

    Ok(BookReviewData {
        job_token: job.token.to_string(),
        book_token: book.token.to_string(),
        title: book.title,
        description: book.description.unwrap_or_default(),
        published_date: book.published_date.map(|y| y.to_string()).unwrap_or_default(),
        language: book.language.unwrap_or_default(),
        series_name,
        series_number: book.series_number.as_ref().map(|n| n.to_string()).unwrap_or_default(),
        publisher_name,
        page_count: book.page_count.map(|p| p.to_string()).unwrap_or_default(),
        authors_csv: author_names.join(", "),
        identifiers,
        provider_names,
        cover_dimensions,
    })
}

#[post(
    "/api/v1/incoming/review/fetch",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn fetch_provider_metadata(
    job_token: String,
    provider_name: String,
    title: String,
    identifiers: IdentifierMap,
) -> Result<Option<ProviderResult>, ServerFnError> {
    let current_user = auth_session.current_user.clone().unwrap_or_default();
    if !Auth::<AuthUser, UserId, BackendSessionPool>::build([Method::POST], true)
        .requires(Rights::any([Rights::permission(Capability::ApproveImports.as_str())]))
        .validate(&current_user, &Method::POST, None)
        .await
    {
        return Err(ServerFnError::new("Forbidden"));
    }

    let token: ImportJobToken = job_token.parse().map_err(|_| ServerFnError::new("Invalid token"))?;
    let temp_dir = std::env::temp_dir();

    let parsed_identifiers: Vec<(IdentifierType, String)> = identifiers
        .into_iter()
        .filter(|(_, v)| !v.is_empty())
        .filter_map(|(k, v)| key_to_identifier_type(&k).map(|t| (t, v)))
        .collect();

    let title = if title.is_empty() { None } else { Some(title) };
    let result = core_services
        .pipeline_service
        .fetch_from_provider(&provider_name, title, parsed_identifiers, &token.to_string(), &temp_dir)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(result.map(provider_book_to_result))
}

#[put(
    "/api/v1/incoming/review/approve",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn approve_book(fields: BookEditFields) -> Result<(), ServerFnError> {
    let current_user = auth_session.current_user.clone().unwrap_or_default();
    if !Auth::<AuthUser, UserId, BackendSessionPool>::build([Method::PUT], true)
        .requires(Rights::any([Rights::permission(Capability::ApproveImports.as_str())]))
        .validate(&current_user, &Method::PUT, None)
        .await
    {
        return Err(ServerFnError::new("Forbidden"));
    }

    let token: ImportJobToken = fields.job_token.parse().map_err(|_| ServerFnError::new("Invalid token"))?;

    let authors: Vec<String> = fields.authors_csv.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();

    let identifiers: Vec<(IdentifierType, String)> = fields
        .identifiers
        .into_iter()
        .filter(|(_, v)| !v.is_empty())
        .filter_map(|(k, v)| key_to_identifier_type(&k).map(|t| (t, v)))
        .collect();

    let edit = BookEdit {
        title: fields.title,
        description: if fields.description.is_empty() { None } else { Some(fields.description) },
        published_date: fields.published_date.parse::<i32>().ok(),
        language: if fields.language.is_empty() { None } else { Some(fields.language) },
        series_name: if fields.series_name.is_empty() { None } else { Some(fields.series_name) },
        series_number: Decimal::from_str(&fields.series_number).ok(),
        publisher_name: if fields.publisher_name.is_empty() {
            None
        } else {
            Some(fields.publisher_name)
        },
        page_count: fields.page_count.parse::<i32>().ok(),
        authors,
        identifiers,
        use_fetched_cover: fields.use_fetched_cover,
    };

    let temp_dir = std::env::temp_dir();
    core_services
        .pipeline_service
        .approve_job(token, edit, &temp_dir)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[put(
    "/api/v1/incoming/review/reject",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn reject_review_book(job_token: String) -> Result<(), ServerFnError> {
    let current_user = auth_session.current_user.clone().unwrap_or_default();
    if !Auth::<AuthUser, UserId, BackendSessionPool>::build([Method::PUT], true)
        .requires(Rights::any([Rights::permission(Capability::ApproveImports.as_str())]))
        .validate(&current_user, &Method::PUT, None)
        .await
    {
        return Err(ServerFnError::new("Forbidden"));
    }

    let token: ImportJobToken = job_token.parse().map_err(|_| ServerFnError::new("Invalid token"))?;
    let temp_dir = std::env::temp_dir();

    // Remove any temp cover that may have been fetched
    let cover_path = temp_dir.join("bookboss-covers").join(job_token);
    let _ = tokio::fs::remove_file(&cover_path).await;

    core_services
        .pipeline_service
        .reject_job(token)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

// ── Edit-metadata server functions
// ────────────────────────────────────────────

#[post(
    "/api/v1/books/edit/data",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn get_book_for_edit(book_token: String) -> Result<BookReviewData, ServerFnError> {
    let current_user = auth_session.current_user.clone().unwrap_or_default();
    if !Auth::<AuthUser, UserId, BackendSessionPool>::build([Method::POST], true)
        .requires(Rights::any([Rights::permission(Capability::EditBook.as_str())]))
        .validate(&current_user, &Method::POST, None)
        .await
    {
        return Err(ServerFnError::new("Forbidden"));
    }

    let book_service = &core_services.book_service;
    let pipeline_service = &core_services.pipeline_service;

    let token = BookToken::from_str(&book_token).map_err(|_| ServerFnError::new("Invalid book token"))?;
    let book = book_service
        .find_book_by_token(&token)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("Book not found"))?;

    // Authors sorted by sort_order
    let book_author_links = {
        let mut links = book_service.authors_for_book(book.id).await.map_err(|e| ServerFnError::new(e.to_string()))?;
        links.sort_by_key(|a| a.sort_order);
        links
    };
    let mut author_names = Vec::with_capacity(book_author_links.len());
    for ba in &book_author_links {
        if let Some(author) = book_service
            .find_author_by_token(&AuthorToken::new(ba.author_id))
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
        {
            author_names.push(author.name);
        }
    }

    // Series name
    let series_name = if let Some(sid) = book.series_id {
        book_service
            .find_series_by_token(&SeriesToken::new(sid))
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .map(|s| s.name)
            .unwrap_or_default()
    } else {
        String::new()
    };

    // Publisher name
    let publisher_name = if let Some(pid) = book.publisher_id {
        book_service
            .find_publisher_by_token(&PublisherToken::new(pid))
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .map(|p| p.name)
            .unwrap_or_default()
    } else {
        String::new()
    };

    // Identifiers
    let raw_identifiers = book_service
        .identifiers_for_book(book.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let identifiers: IdentifierMap = raw_identifiers
        .iter()
        .map(|i| (identifier_type_key(&i.identifier_type).to_string(), i.value.clone()))
        .collect();

    let provider_names = pipeline_service.list_provider_names().into_iter().map(|s| s.to_string()).collect();

    // Cover dimensions
    let cover_dimensions = if let Some(filename) = &book.cover_path {
        let path = core_services.library_store.cover_path(&book.token, filename);
        tokio::fs::read(&path).await.ok().and_then(|b| image_dimensions(&b))
    } else {
        None
    };

    Ok(BookReviewData {
        job_token: String::new(),
        book_token: book.token.to_string(),
        title: book.title,
        description: book.description.unwrap_or_default(),
        published_date: book.published_date.map(|y| y.to_string()).unwrap_or_default(),
        language: book.language.unwrap_or_default(),
        series_name,
        series_number: book.series_number.as_ref().map(|n| n.to_string()).unwrap_or_default(),
        publisher_name,
        page_count: book.page_count.map(|p| p.to_string()).unwrap_or_default(),
        authors_csv: author_names.join(", "),
        identifiers,
        provider_names,
        cover_dimensions,
    })
}

#[post(
    "/api/v1/books/edit/fetch",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn fetch_provider_for_edit(
    book_token: String,
    provider_name: String,
    title: String,
    identifiers: IdentifierMap,
) -> Result<Option<ProviderResult>, ServerFnError> {
    let current_user = auth_session.current_user.clone().unwrap_or_default();
    if !Auth::<AuthUser, UserId, BackendSessionPool>::build([Method::POST], true)
        .requires(Rights::any([Rights::permission(Capability::EditBook.as_str())]))
        .validate(&current_user, &Method::POST, None)
        .await
    {
        return Err(ServerFnError::new("Forbidden"));
    }

    let temp_dir = std::env::temp_dir();

    let parsed_identifiers: Vec<(IdentifierType, String)> = identifiers
        .into_iter()
        .filter(|(_, v)| !v.is_empty())
        .filter_map(|(k, v)| key_to_identifier_type(&k).map(|t| (t, v)))
        .collect();

    let title = if title.is_empty() { None } else { Some(title) };
    let result = core_services
        .pipeline_service
        .fetch_from_provider(&provider_name, title, parsed_identifiers, &book_token, &temp_dir)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(result.map(provider_book_to_result))
}

#[put(
    "/api/v1/books/edit",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn save_library_book(book_token: String, fields: BookEditFields) -> Result<(), ServerFnError> {
    let current_user = auth_session.current_user.clone().unwrap_or_default();
    if !Auth::<AuthUser, UserId, BackendSessionPool>::build([Method::PUT], true)
        .requires(Rights::any([Rights::permission(Capability::EditBook.as_str())]))
        .validate(&current_user, &Method::PUT, None)
        .await
    {
        return Err(ServerFnError::new("Forbidden"));
    }

    let token = BookToken::from_str(&book_token).map_err(|_| ServerFnError::new("Invalid book token"))?;

    let authors: Vec<String> = fields.authors_csv.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();

    let identifiers: Vec<(IdentifierType, String)> = fields
        .identifiers
        .into_iter()
        .filter(|(_, v)| !v.is_empty())
        .filter_map(|(k, v)| key_to_identifier_type(&k).map(|t| (t, v)))
        .collect();

    let edit = BookEdit {
        title: fields.title,
        description: if fields.description.is_empty() { None } else { Some(fields.description) },
        published_date: fields.published_date.parse::<i32>().ok(),
        language: if fields.language.is_empty() { None } else { Some(fields.language) },
        series_name: if fields.series_name.is_empty() { None } else { Some(fields.series_name) },
        series_number: Decimal::from_str(&fields.series_number).ok(),
        publisher_name: if fields.publisher_name.is_empty() {
            None
        } else {
            Some(fields.publisher_name)
        },
        page_count: fields.page_count.parse::<i32>().ok(),
        authors,
        identifiers,
        use_fetched_cover: fields.use_fetched_cover,
    };

    let temp_dir = std::env::temp_dir();
    core_services
        .pipeline_service
        .edit_book(&token, edit, &book_token, &temp_dir)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

// ── Identifier definitions (client + server)
// ──────────────────────────────────

const ALL_IDENTIFIER_TYPES: &[(&str, &str)] = &[
    ("Isbn13", "ISBN-13"),
    ("Isbn10", "ISBN-10"),
    ("Asin", "ASIN"),
    ("GoogleBooks", "Google Books"),
    ("OpenLibrary", "Open Library"),
    ("Hardcover", "Hardcover"),
];

// ── Component
// ─────────────────────────────────────────────────────────────────

#[component]
pub(crate) fn ReviewPage(token: String) -> Element {
    let nav = use_navigator();
    let review_data = use_server_future(move || get_review_data(token.clone()))?;

    match review_data() {
        None => rsx! {
            div { class: "flex-1 flex items-center justify-center text-gray-400 text-sm",
                "Loading…"
            }
        },
        Some(Err(e)) => rsx! {
            div { class: "flex-1 flex items-center justify-center text-red-600 text-sm",
                "Failed to load: {e}"
            }
        },
        Some(Ok(data)) => {
            rsx! {
                ReviewEditor {
                    data,
                    edit_mode: false,
                    on_back: move |_| {
                        nav.push(crate::Route::IncomingPage {});
                    },
                }
            }
        }
    }
}

// ── ReviewEditor sub-component
// ────────────────────────────────────────────────

#[component]
pub(crate) fn ReviewEditor(data: BookReviewData, edit_mode: bool, on_back: EventHandler<()>) -> Element {
    // ── Edit state ────────────────────────────────────────────────────────────
    let mut title = use_signal(|| data.title.clone());
    let mut description = use_signal(|| data.description.clone());
    let mut published_date = use_signal(|| data.published_date.clone());
    let mut language = use_signal(|| data.language.clone());
    let mut series_name = use_signal(|| data.series_name.clone());
    let mut series_number = use_signal(|| data.series_number.clone());
    let mut publisher_name = use_signal(|| data.publisher_name.clone());
    let mut page_count = use_signal(|| data.page_count.clone());
    let mut authors_csv = use_signal(|| data.authors_csv.clone());
    let mut identifiers: Signal<IdentifierMap> = use_signal(|| data.identifiers.clone());
    let mut use_fetched_cover = use_signal(|| false);
    let cover_url = format!("/api/v1/covers/{}", data.book_token);
    let mut current_cover = use_signal(|| cover_url);
    let mut current_cover_dimensions: Signal<Option<(u32, u32)>> = use_signal(|| data.cover_dimensions);

    // ── Provider fetch state ──────────────────────────────────────────────────
    let mut provider_result: Signal<Option<ProviderResult>> = use_signal(|| None);
    let mut fetching: Signal<Option<String>> = use_signal(|| None); // provider name being fetched
    let mut action_busy = use_signal(|| false);
    let mut error_msg: Signal<Option<String>> = use_signal(|| None);

    let job_token = data.job_token.clone();
    let book_token_for_edit = data.book_token.clone();
    // cover_key identifies the temp cover file: job token for review, book token
    // for edit.
    let cover_key = if edit_mode { data.book_token.clone() } else { data.job_token.clone() };

    rsx! {
        div { class: "flex-1 flex flex-col overflow-hidden",
            // ── Header ────────────────────────────────────────────────────────
            div { class: "px-6 py-4 border-b border-gray-200 flex items-center justify-between",
                div { class: "flex items-center gap-4",
                    button {
                        class: "text-sm text-indigo-600 hover:text-indigo-800 cursor-pointer",
                        onclick: move |_| on_back.call(()),
                        if edit_mode { "← Book" } else { "← Incoming" }
                    }
                    h1 { class: "text-xl font-semibold text-gray-900",
                        if edit_mode { "Edit Metadata" } else { "Review Book" }
                    }
                }
                // ── Action buttons ────────────────────────────────────────────
                div { class: "flex items-center gap-3",
                    {
                        let is_busy = *action_busy.read();
                        let cancel_class = if is_busy {
                            "px-4 py-2 rounded border border-gray-300 text-sm font-medium text-gray-500 opacity-40 cursor-not-allowed"
                        } else {
                            "px-4 py-2 rounded border border-gray-300 text-sm font-medium text-gray-600 hover:bg-gray-50 cursor-pointer"
                        };
                        rsx! {
                            button {
                                class: "{cancel_class}",
                                disabled: is_busy,
                                onclick: move |_| on_back.call(()),
                                "Cancel"
                            }
                        }
                    }
                    if !edit_mode {
                        {
                            let is_busy = *action_busy.read();
                            let reject_class = if is_busy {
                                "px-4 py-2 rounded border border-red-300 text-sm font-medium text-red-600 opacity-40 cursor-not-allowed"
                            } else {
                                "px-4 py-2 rounded border border-red-300 text-sm font-medium text-red-600 hover:bg-red-50 cursor-pointer"
                            };
                            let jt = job_token.clone();
                            rsx! {
                                button {
                                    class: "{reject_class}",
                                    disabled: is_busy,
                                    onclick: move |_| {
                                        let jt = jt.clone();
                                        action_busy.set(true);
                                        error_msg.set(None);
                                        spawn(async move {
                                            match reject_review_book(jt).await {
                                                Ok(()) => on_back.call(()),
                                                Err(e) => {
                                                    error_msg.set(Some(e.to_string()));
                                                    action_busy.set(false);
                                                }
                                            }
                                        });
                                    },
                                    "Reject"
                                }
                            }
                        }
                    }
                    {
                        let is_busy = *action_busy.read();
                        let primary_class = if is_busy {
                            "px-4 py-2 rounded bg-indigo-400 text-sm font-medium text-white cursor-not-allowed"
                        } else {
                            "px-4 py-2 rounded bg-indigo-600 text-sm font-medium text-white hover:bg-indigo-700 cursor-pointer"
                        };
                        let jt = job_token.clone();
                        let bk = book_token_for_edit.clone();
                        rsx! {
                            button {
                                class: "{primary_class}",
                                disabled: is_busy,
                                onclick: move |_| {
                                    let fields = BookEditFields {
                                        job_token: jt.clone(),
                                        title: title.read().clone(),
                                        description: description.read().clone(),
                                        published_date: published_date.read().clone(),
                                        language: language.read().clone(),
                                        series_name: series_name.read().clone(),
                                        series_number: series_number.read().clone(),
                                        publisher_name: publisher_name.read().clone(),
                                        page_count: page_count.read().clone(),
                                        authors_csv: authors_csv.read().clone(),
                                        identifiers: identifiers.read().clone(),
                                        use_fetched_cover: *use_fetched_cover.read(),
                                    };
                                    action_busy.set(true);
                                    error_msg.set(None);
                                    let bk = bk.clone();
                                    spawn(async move {
                                        let result = if edit_mode {
                                            save_library_book(bk, fields).await
                                        } else {
                                            approve_book(fields).await
                                        };
                                        match result {
                                            Ok(()) => on_back.call(()),
                                            Err(e) => {
                                                error_msg.set(Some(e.to_string()));
                                                action_busy.set(false);
                                            }
                                        }
                                    });
                                },
                                if edit_mode {
                                    if *action_busy.read() { "Saving…" } else { "Save" }
                                } else {
                                    if *action_busy.read() { "Approving…" } else { "Approve" }
                                }
                            }
                        }
                    }
                }
            }

            // ── Error banner ──────────────────────────────────────────────────
            if let Some(err) = error_msg.read().clone() {
                div { class: "mx-6 mt-3 px-4 py-2 bg-red-50 border border-red-200 rounded text-sm text-red-700",
                    "{err}"
                }
            }

            // ── 3-column metadata table ───────────────────────────────────────
            div { class: "flex-1 overflow-auto px-6 pb-6",
                table { class: "w-full text-sm table-fixed",
                    thead {
                        tr { class: "border-b border-gray-200",
                            th { class: "py-2 pr-4 text-left text-xs font-medium text-gray-500 uppercase tracking-wide w-36", "Field" }
                            th { class: "py-2 pr-4 text-left text-xs font-medium text-gray-500 uppercase tracking-wide w-[46%]", "Current" }
                            th { class: "py-2 pr-4 w-8" }
                            th { class: "py-2 text-left text-xs font-medium text-gray-500 uppercase tracking-wide w-[46%]",
                                div { class: "flex items-center gap-2",
                                    span { "Search" }
                                    for pname in data.provider_names.clone() {
                                        {
                                            let pname = pname.clone();
                                            let ck = cover_key.clone();
                                            let is_fetching_this =
                                                fetching.read().as_deref() == Some(pname.as_str());
                                            let is_busy_any =
                                                fetching.read().is_some() || *action_busy.read();
                                            let btn_class = if is_busy_any {
                                                "px-2 py-0.5 rounded border border-indigo-200 text-xs font-medium text-indigo-400 normal-case opacity-50 cursor-not-allowed tracking-normal"
                                            } else {
                                                "px-2 py-0.5 rounded border border-indigo-300 text-xs font-medium text-indigo-600 normal-case hover:bg-indigo-50 cursor-pointer tracking-normal"
                                            };
                                            rsx! {
                                                button {
                                                    key: "{pname}",
                                                    class: "{btn_class}",
                                                    disabled: is_busy_any,
                                                    onclick: move |_| {
                                                        let pname = pname.clone();
                                                        let ck = ck.clone();
                                                        fetching.set(Some(pname.clone()));
                                                        error_msg.set(None);
                                                        let current_ids = identifiers.read().clone();
                                                        let current_title = title.read().clone();
                                                        spawn(async move {
                                                            let result = if edit_mode {
                                                                fetch_provider_for_edit(
                                                                    ck,
                                                                    pname.clone(),
                                                                    current_title,
                                                                    current_ids,
                                                                )
                                                                .await
                                                            } else {
                                                                fetch_provider_metadata(
                                                                    ck,
                                                                    pname.clone(),
                                                                    current_title,
                                                                    current_ids,
                                                                )
                                                                .await
                                                            };
                                                            match result {
                                                                Ok(r) => provider_result.set(r),
                                                                Err(e) => {
                                                                    error_msg.set(Some(e.to_string()));
                                                                    provider_result.set(None);
                                                                }
                                                            }
                                                            fetching.set(None);
                                                        });
                                                    },
                                                    if is_fetching_this {
                                                        "{pname}…"
                                                    } else {
                                                        "{pname}"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    if fetching.read().is_some() {
                                        span { class: "text-xs text-gray-400 normal-case font-normal tracking-normal", "Fetching…" }
                                    }
                                }
                            }
                        }
                    }
                    tbody { class: "divide-y divide-gray-100",

                        // Title
                        tr {
                            td { class: "py-2 pr-4 text-gray-500 font-medium whitespace-nowrap", "Title" }
                            td { class: "py-2 pr-4",
                                input {
                                    class: "w-full border border-gray-300 rounded px-2 py-1 text-sm focus:outline-none focus:ring-1 focus:ring-indigo-400",
                                    value: "{title}",
                                    oninput: move |e| title.set(e.value()),
                                }
                            }
                            td { class: "py-2 pr-4 text-center",
                                if provider_result.read().is_some() {
                                    {
                                        let pv = provider_result.read().as_ref().map(|r| r.title.clone()).unwrap_or_default();
                                        rsx! {
                                            button {
                                                class: "text-indigo-500 hover:text-indigo-700 cursor-pointer text-xs font-bold",
                                                title: "Copy from provider",
                                                onclick: move |_| title.set(pv.clone()),
                                                "←"
                                            }
                                        }
                                    }
                                }
                            }
                            td { class: "py-2 text-gray-600",
                                if let Some(pr) = provider_result.read().as_ref() {
                                    "{pr.title}"
                                }
                            }
                        }

                        // Authors
                        tr {
                            td { class: "py-2 pr-4 text-gray-500 font-medium whitespace-nowrap", "Authors" }
                            td { class: "py-2 pr-4",
                                input {
                                    class: "w-full border border-gray-300 rounded px-2 py-1 text-sm focus:outline-none focus:ring-1 focus:ring-indigo-400",
                                    value: "{authors_csv}",
                                    placeholder: "Comma-separated names",
                                    oninput: move |e| authors_csv.set(e.value()),
                                }
                            }
                            td { class: "py-2 pr-4 text-center",
                                if provider_result.read().is_some() {
                                    {
                                        let pv = provider_result.read().as_ref().map(|r| r.authors_csv.clone()).unwrap_or_default();
                                        rsx! {
                                            button {
                                                class: "text-indigo-500 hover:text-indigo-700 cursor-pointer text-xs font-bold",
                                                title: "Copy from provider",
                                                onclick: move |_| authors_csv.set(pv.clone()),
                                                "←"
                                            }
                                        }
                                    }
                                }
                            }
                            td { class: "py-2 text-gray-600",
                                if let Some(pr) = provider_result.read().as_ref() {
                                    "{pr.authors_csv}"
                                }
                            }
                        }

                        // Description
                        tr {
                            td { class: "py-2 pr-4 text-gray-500 font-medium whitespace-nowrap align-top pt-3", "Description" }
                            td { class: "py-2 pr-4",
                                textarea {
                                    class: "w-full border border-gray-300 rounded px-2 py-1 text-sm focus:outline-none focus:ring-1 focus:ring-indigo-400 resize-y overflow-y-auto",
                                    rows: "20",
                                    value: "{description}",
                                    oninput: move |e| description.set(e.value()),
                                }
                            }
                            td { class: "py-2 pr-4 text-center align-top pt-3",
                                if provider_result.read().is_some() {
                                    {
                                        let pv = provider_result.read().as_ref().map(|r| r.description.clone()).unwrap_or_default();
                                        rsx! {
                                            button {
                                                class: "text-indigo-500 hover:text-indigo-700 cursor-pointer text-xs font-bold",
                                                title: "Copy from provider",
                                                onclick: move |_| description.set(pv.clone()),
                                                "←"
                                            }
                                        }
                                    }
                                }
                            }
                            td { class: "py-2 text-gray-600 text-xs max-w-xs overflow-hidden",
                                if let Some(pr) = provider_result.read().as_ref() {
                                    "{pr.description}"
                                }
                            }
                        }

                        // Publisher
                        tr {
                            td { class: "py-2 pr-4 text-gray-500 font-medium whitespace-nowrap", "Publisher" }
                            td { class: "py-2 pr-4",
                                input {
                                    class: "w-full border border-gray-300 rounded px-2 py-1 text-sm focus:outline-none focus:ring-1 focus:ring-indigo-400",
                                    value: "{publisher_name}",
                                    oninput: move |e| publisher_name.set(e.value()),
                                }
                            }
                            td { class: "py-2 pr-4 text-center",
                                if provider_result.read().is_some() {
                                    {
                                        let pv = provider_result.read().as_ref().map(|r| r.publisher_name.clone()).unwrap_or_default();
                                        rsx! {
                                            button {
                                                class: "text-indigo-500 hover:text-indigo-700 cursor-pointer text-xs font-bold",
                                                title: "Copy from provider",
                                                onclick: move |_| publisher_name.set(pv.clone()),
                                                "←"
                                            }
                                        }
                                    }
                                }
                            }
                            td { class: "py-2 text-gray-600",
                                if let Some(pr) = provider_result.read().as_ref() {
                                    "{pr.publisher_name}"
                                }
                            }
                        }

                        // Published year
                        tr {
                            td { class: "py-2 pr-4 text-gray-500 font-medium whitespace-nowrap", "Published" }
                            td { class: "py-2 pr-4",
                                input {
                                    r#type: "number",
                                    class: "w-32 border border-gray-300 rounded px-2 py-1 text-sm focus:outline-none focus:ring-1 focus:ring-indigo-400",
                                    value: "{published_date}",
                                    placeholder: "YYYY",
                                    oninput: move |e| published_date.set(e.value()),
                                }
                            }
                            td { class: "py-2 pr-4 text-center",
                                if provider_result.read().is_some() {
                                    {
                                        let pv = provider_result.read().as_ref().map(|r| r.published_date.clone()).unwrap_or_default();
                                        rsx! {
                                            button {
                                                class: "text-indigo-500 hover:text-indigo-700 cursor-pointer text-xs font-bold",
                                                title: "Copy from provider",
                                                onclick: move |_| published_date.set(pv.clone()),
                                                "←"
                                            }
                                        }
                                    }
                                }
                            }
                            td { class: "py-2 text-gray-600",
                                if let Some(pr) = provider_result.read().as_ref() {
                                    "{pr.published_date}"
                                }
                            }
                        }

                        // Language
                        tr {
                            td { class: "py-2 pr-4 text-gray-500 font-medium whitespace-nowrap", "Language" }
                            td { class: "py-2 pr-4",
                                input {
                                    class: "w-40 border border-gray-300 rounded px-2 py-1 text-sm focus:outline-none focus:ring-1 focus:ring-indigo-400",
                                    value: "{language}",
                                    placeholder: "e.g. en",
                                    oninput: move |e| language.set(e.value()),
                                }
                            }
                            td { class: "py-2 pr-4 text-center",
                                if provider_result.read().is_some() {
                                    {
                                        let pv = provider_result.read().as_ref().map(|r| r.language.clone()).unwrap_or_default();
                                        rsx! {
                                            button {
                                                class: "text-indigo-500 hover:text-indigo-700 cursor-pointer text-xs font-bold",
                                                title: "Copy from provider",
                                                onclick: move |_| language.set(pv.clone()),
                                                "←"
                                            }
                                        }
                                    }
                                }
                            }
                            td { class: "py-2 text-gray-600",
                                if let Some(pr) = provider_result.read().as_ref() {
                                    "{pr.language}"
                                }
                            }
                        }

                        // Series (name + number combined)
                        tr {
                            td { class: "py-2 pr-4 text-gray-500 font-medium whitespace-nowrap", "Series" }
                            td { class: "py-2 pr-4",
                                div { class: "flex items-center gap-2",
                                    input {
                                        class: "flex-1 border border-gray-300 rounded px-2 py-1 text-sm focus:outline-none focus:ring-1 focus:ring-indigo-400",
                                        value: "{series_name}",
                                        placeholder: "Series name",
                                        oninput: move |e| series_name.set(e.value()),
                                    }
                                    span { class: "text-gray-400 text-xs whitespace-nowrap", "Book" }
                                    input {
                                        class: "w-16 border border-gray-300 rounded px-2 py-1 text-sm focus:outline-none focus:ring-1 focus:ring-indigo-400",
                                        value: "{series_number}",
                                        placeholder: "#",
                                        oninput: move |e| series_number.set(e.value()),
                                    }
                                }
                            }
                            td { class: "py-2 pr-4 text-center",
                                if provider_result.read().is_some() {
                                    {
                                        let psn = provider_result.read().as_ref().map(|r| r.series_name.clone()).unwrap_or_default();
                                        let pnum = provider_result.read().as_ref().map(|r| r.series_number.clone()).unwrap_or_default();
                                        rsx! {
                                            button {
                                                class: "text-indigo-500 hover:text-indigo-700 cursor-pointer text-xs font-bold",
                                                title: "Copy from provider",
                                                onclick: move |_| {
                                                    series_name.set(psn.clone());
                                                    series_number.set(pnum.clone());
                                                },
                                                "←"
                                            }
                                        }
                                    }
                                }
                            }
                            td { class: "py-2 text-gray-600",
                                if let Some(pr) = provider_result.read().as_ref() {
                                    if !pr.series_name.is_empty() {
                                        span {
                                            "{pr.series_name}"
                                            if !pr.series_number.is_empty() {
                                                span { class: "text-gray-400 ml-1", "Book {pr.series_number}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Page count
                        tr {
                            td { class: "py-2 pr-4 text-gray-500 font-medium whitespace-nowrap", "Pages" }
                            td { class: "py-2 pr-4",
                                input {
                                    r#type: "number",
                                    class: "w-32 border border-gray-300 rounded px-2 py-1 text-sm focus:outline-none focus:ring-1 focus:ring-indigo-400",
                                    value: "{page_count}",
                                    placeholder: "",
                                    oninput: move |e| page_count.set(e.value()),
                                }
                            }
                            td { class: "py-2 pr-4 text-center",
                                if provider_result.read().is_some() {
                                    {
                                        let pv = provider_result.read().as_ref().map(|r| r.page_count.clone()).unwrap_or_default();
                                        rsx! {
                                            button {
                                                class: "text-indigo-500 hover:text-indigo-700 cursor-pointer text-xs font-bold",
                                                title: "Copy from provider",
                                                onclick: move |_| page_count.set(pv.clone()),
                                                "←"
                                            }
                                        }
                                    }
                                }
                            }
                            td { class: "py-2 text-gray-600",
                                if let Some(pr) = provider_result.read().as_ref() {
                                    "{pr.page_count}"
                                }
                            }
                        }

                        // One row per identifier type
                        for (type_key , label) in ALL_IDENTIFIER_TYPES {
                            {
                                let type_key = type_key.to_string();
                                let label = label.to_string();
                                let tk_copy = type_key.clone();
                                rsx! {
                                    tr { key: "{type_key}",
                                        td { class: "py-2 pr-4 text-gray-500 font-medium whitespace-nowrap", "{label}" }
                                        td { class: "py-2 pr-4",
                                            {
                                                let tk = type_key.clone();
                                                let cur_val = identifiers.read().get(&type_key).cloned().unwrap_or_default();
                                                rsx! {
                                                    input {
                                                        class: "w-full border border-gray-300 rounded px-2 py-1 text-sm font-mono focus:outline-none focus:ring-1 focus:ring-indigo-400",
                                                        value: "{cur_val}",
                                                        oninput: move |e| {
                                                            identifiers.write().insert(tk.clone(), e.value());
                                                        },
                                                    }
                                                }
                                            }
                                        }
                                        td { class: "py-2 pr-4 text-center",
                                            if provider_result.read().is_some() {
                                                {
                                                    let provider_val = provider_result
                                                        .read()
                                                        .as_ref()
                                                        .and_then(|r| r.identifiers.get(&tk_copy).cloned())
                                                        .unwrap_or_default();
                                                    if !provider_val.is_empty() {
                                                        let tk2 = tk_copy.clone();
                                                        let pv = provider_val.clone();
                                                        rsx! {
                                                            button {
                                                                class: "text-indigo-500 hover:text-indigo-700 cursor-pointer text-xs font-bold",
                                                                title: "Copy from provider",
                                                                onclick: move |_| {
                                                                    identifiers.write().insert(tk2.clone(), pv.clone());
                                                                },
                                                                "←"
                                                            }
                                                        }
                                                    } else {
                                                        rsx! {}
                                                    }
                                                }
                                            }
                                        }
                                        td { class: "py-2 text-gray-600 font-mono text-xs",
                                            if let Some(pr) = provider_result.read().as_ref() {
                                                if let Some(val) = pr.identifiers.get(&tk_copy) {
                                                    "{val}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Cover
                        tr {
                            td { class: "py-2 pr-4 text-gray-500 font-medium whitespace-nowrap align-top pt-3", "Cover" }
                            td { class: "py-2 pr-4",
                                div { class: "flex flex-col items-center gap-0.5",
                                    img {
                                        class: "max-h-32 max-w-24 object-contain rounded shadow-sm",
                                        src: "{current_cover}",
                                        alt: "Current cover",
                                    }
                                    if let Some((w, h)) = *current_cover_dimensions.read() {
                                        span { class: "text-gray-400 text-xs", "{w} × {h}" }
                                    }
                                }
                            }
                            td { class: "py-2 pr-4 text-center align-top pt-3",
                                if let Some(pr) = provider_result.read().as_ref() {
                                    if pr.cover_thumbnail.is_some() {
                                        button {
                                            class: "text-indigo-500 hover:text-indigo-700 cursor-pointer text-xs font-bold",
                                            title: "Use provider cover",
                                            onclick: move |_| {
                                                if let Some(pr) = provider_result.read().as_ref() {
                                                    if let Some(thumb) = pr.cover_thumbnail.clone() {
                                                        current_cover.set(thumb);
                                                        current_cover_dimensions.set(pr.cover_dimensions);
                                                        use_fetched_cover.set(true);
                                                    }
                                                }
                                            },
                                            "←"
                                        }
                                    }
                                }
                            }
                            td { class: "py-2 align-top",
                                if let Some(pr) = provider_result.read().as_ref() {
                                    if let Some(thumb) = &pr.cover_thumbnail {
                                        div { class: "flex flex-col items-center gap-0.5",
                                            img {
                                                class: "max-h-32 max-w-24 object-contain rounded shadow-sm",
                                                src: "{thumb}",
                                                alt: "Provider cover",
                                            }
                                            if let Some((w, h)) = pr.cover_dimensions {
                                                span { class: "text-gray-400 text-xs", "{w} × {h}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
