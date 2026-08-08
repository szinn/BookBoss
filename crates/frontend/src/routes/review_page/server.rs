use dioxus::prelude::*;
// ── Server-only imports
// ───────────────────────────────────────────────────────
#[cfg(feature = "server")]
use {
    super::types::SeriesOption,
    crate::routes::server_helpers::{require_any_capability, require_capability, to_server_err},
    crate::server::{AuthSession, AuthUser, BackendSessionPool},
    axum::http::Method,
    axum_session_auth::{Auth, Rights},
    base64::{Engine, engine::general_purpose::STANDARD as B64},
    bb_core::{
        CoreServices,
        book::{AuthorToken, BookToken, FileRole, IdentifierType, PublisherToken, SeriesToken},
        collection::BookEdit,
        import::ImportJobToken,
        pipeline::ProviderBook,
        types::Capability,
        user::UserId,
    },
    rust_decimal::Decimal,
    std::{str::FromStr, sync::Arc},
};

use super::types::{BookEditFields, BookReviewData, IdentifierMap, PicklistData, ProviderResult};

// ── Helpers (server only)
// ─────────────────────────────────────────────────────

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
        let w = u32::from(u16::from_le_bytes([data[6], data[7]]));
        let h = u32::from(u16::from_le_bytes([data[8], data[9]]));
        return Some((w, h));
    }
    if data.len() >= 30 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
        match &data[12..16] {
            b"VP8 " => {
                let w = u32::from(u16::from_le_bytes([data[26], data[27]]) & 0x3FFF);
                let h = u32::from(u16::from_le_bytes([data[28], data[29]]) & 0x3FFF);
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
                let h = u32::from(u16::from_be_bytes([data[i + 5], data[i + 6]]));
                let w = u32::from(u16::from_be_bytes([data[i + 7], data[i + 8]]));
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
fn provider_book_to_result(pb: &ProviderBook) -> ProviderResult {
    let meta = &pb.metadata;
    let title = meta.title.clone().unwrap_or_default();
    let description = meta.description.clone().unwrap_or_default();
    let published_date = meta.published_date.map(|y| y.to_string()).unwrap_or_default();
    let language = meta.language.clone().unwrap_or_default();
    let series_name = meta.series_name.clone().unwrap_or_default();
    let series_number = meta.series_number.as_ref().map(std::string::ToString::to_string).unwrap_or_default();
    let publisher_name = meta.publisher.clone().unwrap_or_default();
    let authors = meta.authors.as_deref().unwrap_or(&[]).iter().map(|a| a.name.clone()).collect::<Vec<_>>();
    let identifiers = meta
        .identifiers
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|i| (i.identifier_type.form_key().to_string(), i.value.clone()))
        .collect();
    // Provider cover bytes take priority; fall back to embedded cover in metadata.
    let cover_bytes = pb.cover_bytes.as_deref().or(meta.cover_bytes.as_deref());
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
        authors,
        identifiers,
        cover_thumbnail,
        cover_dimensions,
    }
}

// ── Review server functions
// ────────────────────────────────────────────────────

#[post(
    "/api/v1/incoming/review",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(super) async fn get_review_data(job_token: String) -> Result<BookReviewData, ServerFnError> {
    require_capability(&auth_session, Capability::ApproveImports, Method::POST).await?;

    let token: ImportJobToken = job_token.parse().map_err(|_| ServerFnError::new("Invalid token"))?;
    let import_service = &core_services.import_job_service;
    let book_service = &core_services.book_service;
    let pipeline_service = &core_services.pipeline_service;

    let job = import_service
        .find_by_token(token)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("Job not found"))?;

    let book_id = job.candidate_book_id.ok_or_else(|| ServerFnError::new("No candidate book"))?;
    let book = book_service
        .find_book_by_token(BookToken::new(book_id))
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("Book not found"))?;

    // Authors sorted by sort_order
    let book_author_links = {
        let mut links = book_service.authors_for_book(book.id).await.map_err(to_server_err)?;
        links.sort_by_key(|a| a.sort_order);
        links
    };
    let mut author_names = Vec::with_capacity(book_author_links.len());
    for ba in &book_author_links {
        if let Some(author) = book_service.find_author_by_token(AuthorToken::new(ba.author_id)).await.map_err(to_server_err)? {
            author_names.push(author.name);
        }
    }

    // Series name
    let series_name = if let Some(sid) = book.series_id {
        book_service
            .find_series_by_token(SeriesToken::new(sid))
            .await
            .map_err(to_server_err)?
            .map(|s| s.name)
            .unwrap_or_default()
    } else {
        String::new()
    };

    // Publisher name
    let publisher_name = if let Some(pid) = book.publisher_id {
        book_service
            .find_publisher_by_token(PublisherToken::new(pid))
            .await
            .map_err(to_server_err)?
            .map(|p| p.name)
            .unwrap_or_default()
    } else {
        String::new()
    };

    // Identifiers
    let raw_identifiers = book_service.identifiers_for_book(book.id).await.map_err(to_server_err)?;
    let identifiers: IdentifierMap = raw_identifiers
        .iter()
        .map(|i| (i.identifier_type.form_key().to_string(), i.value.clone()))
        .collect();

    let provider_names = pipeline_service
        .list_provider_names()
        .into_iter()
        .map(std::string::ToString::to_string)
        .collect();

    // Check whether the original source file is still on disk.
    let book_files = book_service.files_for_book(book.id).await.map_err(to_server_err)?;
    let original_missing = {
        let original = book_files.iter().find(|f| f.file_role == FileRole::Original);
        match original {
            Some(f) => !core_services.file_store.resolve(&f.path).exists(),
            None => true,
        }
    };

    // Read cover file to determine dimensions.
    let cover_dimensions = if book.has_cover {
        let path = core_services.file_store.cover_path(book.token);
        tokio::fs::read(&path).await.ok().and_then(|b| image_dimensions(&b))
    } else {
        None
    };

    let genres = book_service
        .genres_for_book(book.id)
        .await
        .map_err(to_server_err)?
        .into_iter()
        .map(|g| g.name)
        .collect();
    let tags = book_service
        .tags_for_book(book.id)
        .await
        .map_err(to_server_err)?
        .into_iter()
        .map(|t| t.name)
        .collect();

    Ok(BookReviewData {
        job_token: job.token.to_string(),
        book_token: book.token.to_string(),
        updated_at: book.updated_at.timestamp().to_string(),
        title: book.title,
        description: book.description.unwrap_or_default(),
        published_date: book.published_date.map(|y| y.to_string()).unwrap_or_default(),
        language: book.language.unwrap_or_default(),
        series_name,
        series_number: book.series_number.as_ref().map(std::string::ToString::to_string).unwrap_or_default(),
        publisher_name,
        page_count: book.page_count.map(|p| p.to_string()).unwrap_or_default(),
        authors: author_names,
        genres,
        tags,
        identifiers,
        provider_names,
        cover_dimensions,
        original_missing,
    })
}

#[post(
    "/api/v1/incoming/review/fetch",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(super) async fn fetch_provider_metadata(
    job_token: String,
    provider_name: String,
    title: String,
    authors: Vec<String>,
    identifiers: IdentifierMap,
) -> Result<Option<ProviderResult>, ServerFnError> {
    require_capability(&auth_session, Capability::ApproveImports, Method::POST).await?;

    let token: ImportJobToken = job_token.parse().map_err(|_| ServerFnError::new("Invalid token"))?;
    let temp_dir = std::env::temp_dir();

    let parsed_identifiers: Vec<(IdentifierType, String)> = identifiers
        .into_iter()
        .filter(|(_, v)| !v.is_empty())
        .filter_map(|(k, v)| IdentifierType::from_form_key(&k).map(|t| (t, v)))
        .collect();

    let title = if title.is_empty() { None } else { Some(title) };
    let result = core_services
        .metadata_service
        .fetch_from_provider(&provider_name, title, authors, parsed_identifiers)
        .await
        .map_err(to_server_err)?;

    if let Some(pb) = &result
        && let Some(cover) = &pb.cover_bytes
    {
        let cover_dir = temp_dir.join("bookboss-covers");
        tokio::fs::create_dir_all(&cover_dir).await.map_err(to_server_err)?;
        let pending_name = format!("{token}-provider");
        tokio::fs::write(cover_dir.join(pending_name), cover).await.map_err(to_server_err)?;
    }

    Ok(result.as_ref().map(provider_book_to_result))
}

#[put(
    "/api/v1/incoming/review/cover/accept",
    auth_session: axum::Extension<AuthSession>
)]
pub(super) async fn accept_incoming_provider_cover(job_token: String) -> Result<(), ServerFnError> {
    require_capability(&auth_session, Capability::ApproveImports, Method::PUT).await?;

    let cover_dir = std::env::temp_dir().join("bookboss-covers");
    tokio::fs::create_dir_all(&cover_dir).await.map_err(to_server_err)?;
    let pending = cover_dir.join(format!("{job_token}-provider"));
    let committed = cover_dir.join(&job_token);
    tokio::fs::rename(&pending, &committed).await.map_err(to_server_err)?;

    Ok(())
}

#[put(
    "/api/v1/incoming/review/approve",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(super) async fn approve_book(fields: BookEditFields) -> Result<(), ServerFnError> {
    require_capability(&auth_session, Capability::ApproveImports, Method::PUT).await?;
    let current_user = auth_session.current_user.clone().unwrap_or_default();

    let token: ImportJobToken = fields.job_token.parse().map_err(|_| ServerFnError::new("Invalid token"))?;

    let authors = fields.authors;

    let identifiers: Vec<(IdentifierType, String)> = fields
        .identifiers
        .into_iter()
        .filter(|(_, v)| !v.is_empty())
        .filter_map(|(k, v)| IdentifierType::from_form_key(&k).map(|t| (t, v)))
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
        genres: fields.genres,
        tags: fields.tags,
    };

    let temp_dir = std::env::temp_dir();
    core_services
        .collection_service
        .approve_book(token, current_user.id(), edit, &temp_dir)
        .await
        .map_err(to_server_err)?;

    // Clean up any unaccepted provider cover
    let _ = tokio::fs::remove_file(temp_dir.join("bookboss-covers").join(format!("{token}-provider"))).await;

    Ok(())
}

#[put(
    "/api/v1/incoming/review/reject",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(super) async fn reject_review_book(job_token: String) -> Result<(), ServerFnError> {
    require_capability(&auth_session, Capability::ApproveImports, Method::PUT).await?;

    let token: ImportJobToken = job_token.parse().map_err(|_| ServerFnError::new("Invalid token"))?;
    let temp_dir = std::env::temp_dir();

    // Remove any temp covers that may have been staged or fetched
    let cover_dir = temp_dir.join("bookboss-covers");
    let _ = tokio::fs::remove_file(cover_dir.join(&job_token)).await;
    let _ = tokio::fs::remove_file(cover_dir.join(format!("{job_token}-provider"))).await;

    core_services.collection_service.reject_book(token).await.map_err(to_server_err)?;

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
    require_capability(&auth_session, Capability::EditBook, Method::POST).await?;

    let book_service = &core_services.book_service;
    let pipeline_service = &core_services.pipeline_service;

    let token = BookToken::from_str(&book_token).map_err(|_| ServerFnError::new("Invalid book token"))?;
    let book = book_service
        .find_book_by_token(token)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("Book not found"))?;

    // Authors sorted by sort_order
    let book_author_links = {
        let mut links = book_service.authors_for_book(book.id).await.map_err(to_server_err)?;
        links.sort_by_key(|a| a.sort_order);
        links
    };
    let mut author_names = Vec::with_capacity(book_author_links.len());
    for ba in &book_author_links {
        if let Some(author) = book_service.find_author_by_token(AuthorToken::new(ba.author_id)).await.map_err(to_server_err)? {
            author_names.push(author.name);
        }
    }

    // Series name
    let series_name = if let Some(sid) = book.series_id {
        book_service
            .find_series_by_token(SeriesToken::new(sid))
            .await
            .map_err(to_server_err)?
            .map(|s| s.name)
            .unwrap_or_default()
    } else {
        String::new()
    };

    // Publisher name
    let publisher_name = if let Some(pid) = book.publisher_id {
        book_service
            .find_publisher_by_token(PublisherToken::new(pid))
            .await
            .map_err(to_server_err)?
            .map(|p| p.name)
            .unwrap_or_default()
    } else {
        String::new()
    };

    // Identifiers
    let raw_identifiers = book_service.identifiers_for_book(book.id).await.map_err(to_server_err)?;
    let identifiers: IdentifierMap = raw_identifiers
        .iter()
        .map(|i| (i.identifier_type.form_key().to_string(), i.value.clone()))
        .collect();

    let provider_names = pipeline_service
        .list_provider_names()
        .into_iter()
        .map(std::string::ToString::to_string)
        .collect();

    // Cover dimensions
    let cover_dimensions = if book.has_cover {
        let path = core_services.file_store.cover_path(book.token);
        tokio::fs::read(&path).await.ok().and_then(|b| image_dimensions(&b))
    } else {
        None
    };

    let genres = book_service
        .genres_for_book(book.id)
        .await
        .map_err(to_server_err)?
        .into_iter()
        .map(|g| g.name)
        .collect();
    let tags = book_service
        .tags_for_book(book.id)
        .await
        .map_err(to_server_err)?
        .into_iter()
        .map(|t| t.name)
        .collect();

    Ok(BookReviewData {
        job_token: String::new(),
        book_token: book.token.to_string(),
        updated_at: book.updated_at.timestamp().to_string(),
        title: book.title,
        description: book.description.unwrap_or_default(),
        published_date: book.published_date.map(|y| y.to_string()).unwrap_or_default(),
        language: book.language.unwrap_or_default(),
        series_name,
        series_number: book.series_number.as_ref().map(std::string::ToString::to_string).unwrap_or_default(),
        publisher_name,
        page_count: book.page_count.map(|p| p.to_string()).unwrap_or_default(),
        authors: author_names,
        genres,
        tags,
        identifiers,
        provider_names,
        cover_dimensions,
        original_missing: false,
    })
}

#[post(
    "/api/v1/books/edit/fetch",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(super) async fn fetch_provider_for_edit(
    book_token: String,
    provider_name: String,
    title: String,
    authors: Vec<String>,
    identifiers: IdentifierMap,
) -> Result<Option<ProviderResult>, ServerFnError> {
    require_capability(&auth_session, Capability::EditBook, Method::POST).await?;

    let temp_dir = std::env::temp_dir();

    let parsed_identifiers: Vec<(IdentifierType, String)> = identifiers
        .into_iter()
        .filter(|(_, v)| !v.is_empty())
        .filter_map(|(k, v)| IdentifierType::from_form_key(&k).map(|t| (t, v)))
        .collect();

    let title = if title.is_empty() { None } else { Some(title) };
    let result = core_services
        .metadata_service
        .fetch_from_provider(&provider_name, title, authors, parsed_identifiers)
        .await
        .map_err(to_server_err)?;

    if let Some(pb) = &result
        && let Some(cover) = &pb.cover_bytes
    {
        let cover_dir = temp_dir.join("bookboss-covers");
        tokio::fs::create_dir_all(&cover_dir).await.map_err(to_server_err)?;
        let pending_name = format!("{book_token}-provider");
        tokio::fs::write(cover_dir.join(pending_name), cover).await.map_err(to_server_err)?;
    }

    Ok(result.as_ref().map(provider_book_to_result))
}

#[put(
    "/api/v1/books/cover/accept",
    auth_session: axum::Extension<AuthSession>
)]
pub(super) async fn accept_library_provider_cover(book_token: String) -> Result<(), ServerFnError> {
    require_capability(&auth_session, Capability::EditBook, Method::PUT).await?;

    let cover_dir = std::env::temp_dir().join("bookboss-covers");
    tokio::fs::create_dir_all(&cover_dir).await.map_err(to_server_err)?;
    let pending = cover_dir.join(format!("{book_token}-provider"));
    let committed = cover_dir.join(&book_token);
    tokio::fs::rename(&pending, &committed).await.map_err(to_server_err)?;

    Ok(())
}

#[put(
    "/api/v1/books/edit",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(super) async fn save_library_book(book_token: String, fields: BookEditFields) -> Result<(), ServerFnError> {
    require_capability(&auth_session, Capability::EditBook, Method::PUT).await?;

    let token = BookToken::from_str(&book_token).map_err(|_| ServerFnError::new("Invalid book token"))?;

    let authors = fields.authors;

    let identifiers: Vec<(IdentifierType, String)> = fields
        .identifiers
        .into_iter()
        .filter(|(_, v)| !v.is_empty())
        .filter_map(|(k, v)| IdentifierType::from_form_key(&k).map(|t| (t, v)))
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
        genres: fields.genres,
        tags: fields.tags,
    };

    let temp_dir = std::env::temp_dir();
    core_services
        .collection_service
        .edit_book(token, edit, &book_token, &temp_dir)
        .await
        .map_err(to_server_err)?;

    // Clean up any unaccepted provider cover
    let _ = tokio::fs::remove_file(temp_dir.join("bookboss-covers").join(format!("{book_token}-provider"))).await;

    Ok(())
}

// ── Picklist server function
// ──────────────────────────────────────────────────

#[post(
    "/api/v1/books/picklist",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn get_picklist_data((): ()) -> Result<PicklistData, ServerFnError> {
    let current_user = auth_session.current_user.clone().unwrap_or_default();
    if !Auth::<AuthUser, UserId, BackendSessionPool>::build([Method::POST], true)
        .requires(Rights::any([
            Rights::permission(Capability::ApproveImports.as_str()),
            Rights::permission(Capability::EditBook.as_str()),
        ]))
        .validate(&current_user, &Method::POST, None)
        .await
    {
        return Err(ServerFnError::new("Forbidden"));
    }
    let book_service = &core_services.book_service;

    let authors = book_service
        .list_all_authors()
        .await
        .map_err(to_server_err)?
        .into_iter()
        .map(|a| a.name)
        .collect();

    let genres = book_service
        .list_all_genres()
        .await
        .map_err(to_server_err)?
        .into_iter()
        .map(|g| g.name)
        .collect();

    let tags = book_service.list_all_tags().await.map_err(to_server_err)?.into_iter().map(|t| t.name).collect();

    let all_series = book_service.list_all_series().await.map_err(to_server_err)?;

    let mut series_options = Vec::with_capacity(all_series.len());
    for s in all_series {
        let next = book_service.series_next_number(&s.name).await.map_err(to_server_err)?;
        series_options.push(SeriesOption {
            name: s.name,
            next_number: next,
        });
    }

    let publishers = book_service
        .list_all_publishers()
        .await
        .map_err(to_server_err)?
        .into_iter()
        .map(|p| p.name)
        .collect();

    Ok(PicklistData {
        authors,
        genres,
        tags,
        series: series_options,
        publishers,
    })
}

// ── Library membership server functions
// ─────────────────────────────────────

/// Returns the tokens of non-system libraries the book currently belongs to.
#[post(
    "/api/v1/books/libraries/list",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn get_book_libraries(book_token: String) -> Result<Vec<String>, ServerFnError> {
    require_capability(&auth_session, Capability::EditBook, Method::POST).await?;

    let book_token_parsed = BookToken::from_str(&book_token).map_err(|_| ServerFnError::new("Invalid book token"))?;
    let book = core_services
        .book_service
        .find_book_by_token(book_token_parsed)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("Book not found"))?;

    // Get all library IDs the book belongs to.
    let book_library_ids = core_services.library_service.library_ids_for_book(book.id).await.map_err(to_server_err)?;

    // Load all libraries, filter to non-system, and intersect with book's
    // libraries.
    let all_libraries = core_services.library_service.list_libraries().await.map_err(to_server_err)?;
    let tokens: Vec<String> = all_libraries
        .into_iter()
        .filter(|e| !e.library.is_system && book_library_ids.contains(&e.library.id))
        .map(|e| e.library.token.to_string())
        .collect();

    Ok(tokens)
}

/// Returns all non-system libraries (token + name) for the picker.
#[post(
    "/api/v1/books/libraries/non-system",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn list_non_system_libraries() -> Result<Vec<(String, String)>, ServerFnError> {
    require_any_capability(&auth_session, &[Capability::EditBook, Capability::ApproveImports], Method::GET).await?;

    let all_libraries = core_services.library_service.list_libraries().await.map_err(to_server_err)?;
    let result: Vec<(String, String)> = all_libraries
        .into_iter()
        .filter(|e| !e.library.is_system)
        .map(|e| (e.library.token.to_string(), e.library.name))
        .collect();

    Ok(result)
}

/// Syncs non-system library memberships for a book.
/// NEVER removes the book from system libraries (All Books stays always).
#[put(
    "/api/v1/books/libraries/set",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn set_book_libraries(book_token: String, library_tokens: Vec<String>) -> Result<(), ServerFnError> {
    require_any_capability(&auth_session, &[Capability::EditBook, Capability::ApproveImports], Method::PUT).await?;

    let book_token_parsed = BookToken::from_str(&book_token).map_err(|_| ServerFnError::new("Invalid book token"))?;
    let book = core_services
        .book_service
        .find_book_by_token(book_token_parsed)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("Book not found"))?;

    // Load all non-system libraries.
    let all_libraries = core_services.library_service.list_libraries().await.map_err(to_server_err)?;
    let non_system: Vec<_> = all_libraries.into_iter().filter(|e| !e.library.is_system).collect();

    // Current library IDs for the book.
    let current_ids = core_services.library_service.library_ids_for_book(book.id).await.map_err(to_server_err)?;

    for entry in &non_system {
        let lib = &entry.library;
        let token_str = lib.token.to_string();
        let desired = library_tokens.contains(&token_str);
        let has = current_ids.contains(&lib.id);

        if desired && !has {
            core_services
                .library_service
                .add_book_to_library(lib.token, book_token_parsed)
                .await
                .map_err(to_server_err)?;
        } else if !desired && has {
            core_services
                .library_service
                .remove_book_from_library(lib.token, book_token_parsed)
                .await
                .map_err(to_server_err)?;
        }
    }

    Ok(())
}

// ── Cover staging server functions
// ─────────────────────────────────
// Uploaded cover bytes are written to the same temp-dir location used by
// fetch_provider_metadata / fetch_provider_for_edit. The editor sets
// use_fetched_cover = true so the staged file is committed on Save/Approve.
// Cancelling the editor discards the staged file without touching the DB.

#[put(
    "/api/v1/incoming/review/cover",
    auth_session: axum::Extension<AuthSession>
)]
pub(super) async fn stage_incoming_cover(job_token: String, data_base64: String) -> Result<(), ServerFnError> {
    require_capability(&auth_session, Capability::ApproveImports, Method::PUT).await?;

    let cover_bytes = B64.decode(&data_base64).map_err(|_| ServerFnError::new("Invalid base64"))?;
    let cover_dir = std::env::temp_dir().join("bookboss-covers");
    tokio::fs::create_dir_all(&cover_dir).await.map_err(to_server_err)?;
    tokio::fs::write(cover_dir.join(&job_token), &cover_bytes).await.map_err(to_server_err)?;

    Ok(())
}

#[put(
    "/api/v1/books/cover",
    auth_session: axum::Extension<AuthSession>
)]
pub(super) async fn stage_library_cover(book_token: String, data_base64: String) -> Result<(), ServerFnError> {
    require_capability(&auth_session, Capability::EditBook, Method::PUT).await?;

    let cover_bytes = B64.decode(&data_base64).map_err(|_| ServerFnError::new("Invalid base64"))?;
    let cover_dir = std::env::temp_dir().join("bookboss-covers");
    tokio::fs::create_dir_all(&cover_dir).await.map_err(to_server_err)?;
    tokio::fs::write(cover_dir.join(&book_token), &cover_bytes).await.map_err(to_server_err)?;

    Ok(())
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use bb_core::{import::ImportSource, pipeline::ExtractedMetadata};

    use super::*;

    // ── image_dimensions helpers ──────────────────────────────────────────────

    /// Builds a minimal 24-byte PNG IHDR buffer with the given dimensions.
    fn png_bytes(w: u32, h: u32) -> Vec<u8> {
        let mut v = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]; // PNG magic (8)
        v.extend_from_slice(&[0u8; 8]); // 8 padding bytes before width offset
        v.extend_from_slice(&w.to_be_bytes()); // width at offset 16
        v.extend_from_slice(&h.to_be_bytes()); // height at offset 20
        v
    }

    /// Builds a minimal GIF89a buffer with the given dimensions.
    fn gif89a_bytes(w: u16, h: u16) -> Vec<u8> {
        let mut v = b"GIF89a".to_vec();
        v.extend_from_slice(&w.to_le_bytes());
        v.extend_from_slice(&h.to_le_bytes());
        v
    }

    /// Builds a minimal WebP VP8 (lossy) buffer. Width/height are 14-bit
    /// values.
    fn webp_vp8_bytes(w: u16, h: u16) -> Vec<u8> {
        let mut v = b"RIFF".to_vec();
        v.extend_from_slice(&[0u8; 4]); // file size (ignored)
        v.extend_from_slice(b"WEBP");
        v.extend_from_slice(b"VP8 ");
        v.extend_from_slice(&[0u8; 10]); // chunk size + VP8 bitstream header bytes 0-9
        // bytes 26-27: width & 0x3FFF, bytes 28-29: height & 0x3FFF
        v.extend_from_slice(&(w & 0x3FFF).to_le_bytes());
        v.extend_from_slice(&(h & 0x3FFF).to_le_bytes());
        v
    }

    /// Builds a minimal WebP VP8L (lossless) buffer.
    fn webp_vp8l_bytes(w: u32, h: u32) -> Vec<u8> {
        // VP8L stores (width-1) in bits 0-13 and (height-1) in bits 14-27 of a
        // little-endian u32 starting at byte 21.  The outer RIFF check requires
        // data.len() >= 30, so pad the buffer to 30 bytes.
        let bits: u32 = ((w - 1) & 0x3FFF) | (((h - 1) & 0x3FFF) << 14);
        let mut v = b"RIFF".to_vec();
        v.extend_from_slice(&[0u8; 4]); // file size (bytes 4-7)
        v.extend_from_slice(b"WEBP"); // bytes 8-11
        v.extend_from_slice(b"VP8L"); // bytes 12-15
        v.extend_from_slice(&[0u8; 4]); // chunk size (bytes 16-19)
        v.push(0x2F); // VP8L signature byte (byte 20)
        v.extend_from_slice(&bits.to_le_bytes()); // bits at bytes 21-24
        v.extend_from_slice(&[0u8; 5]); // padding to reach 30 bytes (bytes 25-29)
        v
    }

    /// Builds a minimal WebP VP8X buffer.
    fn webp_vp8x_bytes(w: u32, h: u32) -> Vec<u8> {
        // VP8X stores canvas_width_minus_one as 3 LE bytes at offset 24,
        // canvas_height_minus_one as 3 LE bytes at offset 27.
        let w_bytes = (w - 1).to_le_bytes();
        let h_bytes = (h - 1).to_le_bytes();
        let mut v = b"RIFF".to_vec();
        v.extend_from_slice(&[0u8; 4]); // file size (bytes 4-7)
        v.extend_from_slice(b"WEBP"); // bytes 8-11
        v.extend_from_slice(b"VP8X"); // bytes 12-15
        v.extend_from_slice(&[0u8; 4]); // chunk size (bytes 16-19)
        v.extend_from_slice(&[0u8; 4]); // flags (bytes 20-23)
        v.extend_from_slice(&w_bytes[..3]); // canvas_width_minus_one (bytes 24-26)
        v.extend_from_slice(&h_bytes[..3]); // canvas_height_minus_one (bytes 27-29)
        v
    }

    /// Builds a minimal JPEG with a SOF0 marker for the given dimensions.
    fn jpeg_sof0_bytes(w: u16, h: u16) -> Vec<u8> {
        let mut v = vec![0xFF, 0xD8]; // SOI
        // APP0 marker (skipped by the scanner)
        v.extend_from_slice(&[0xFF, 0xE0]); // APP0
        v.extend_from_slice(&[0x00, 0x10]); // length = 16 (including these 2 bytes)
        v.extend_from_slice(&[0u8; 14]); // 14 bytes of APP0 payload
        // SOF0 marker
        v.extend_from_slice(&[0xFF, 0xC0]); // SOF0
        v.extend_from_slice(&[0x00, 0x11]); // length = 17
        v.push(0x08); // precision
        v.extend_from_slice(&h.to_be_bytes()); // height at SOF0+5
        v.extend_from_slice(&w.to_be_bytes()); // width at SOF0+7
        v
    }

    // ── image_dimensions tests ────────────────────────────────────────────────

    #[test]
    fn image_dimensions_png() {
        assert_eq!(image_dimensions(&png_bytes(800, 600)), Some((800, 600)));
    }

    #[test]
    fn image_dimensions_gif89a() {
        assert_eq!(image_dimensions(&gif89a_bytes(320, 240)), Some((320, 240)));
    }

    #[test]
    fn image_dimensions_webp_vp8_lossy() {
        assert_eq!(image_dimensions(&webp_vp8_bytes(1024, 768)), Some((1024, 768)));
    }

    #[test]
    fn image_dimensions_webp_vp8l_lossless() {
        assert_eq!(image_dimensions(&webp_vp8l_bytes(640, 480)), Some((640, 480)));
    }

    #[test]
    fn image_dimensions_webp_vp8x() {
        assert_eq!(image_dimensions(&webp_vp8x_bytes(1920, 1080)), Some((1920, 1080)));
    }

    #[test]
    fn image_dimensions_jpeg_sof0() {
        assert_eq!(image_dimensions(&jpeg_sof0_bytes(1280, 720)), Some((1280, 720)));
    }

    #[test]
    fn image_dimensions_unrecognized() {
        assert_eq!(image_dimensions(b"not an image at all"), None);
    }

    #[test]
    fn image_dimensions_truncated() {
        // PNG magic bytes but only 8 bytes total (need >= 24)
        let truncated = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(image_dimensions(&truncated), None);
    }

    // ── provider_book_to_result helpers ──────────────────────────────────────

    fn make_provider_book(
        title: Option<&str>,
        authors: &[&str],
        language: Option<&str>,
        provider_cover: Option<Vec<u8>>,
        metadata_cover: Option<Vec<u8>>,
    ) -> ProviderBook {
        ProviderBook {
            metadata: ExtractedMetadata {
                title: title.map(String::from),
                authors: if authors.is_empty() {
                    None
                } else {
                    Some(
                        authors
                            .iter()
                            .enumerate()
                            .map(|(i, name)| bb_core::pipeline::ExtractedAuthor {
                                name: name.to_string(),
                                role: None,
                                sort_order: i as i32,
                            })
                            .collect(),
                    )
                },
                language: language.map(String::from),
                cover_bytes: metadata_cover,
                ..Default::default()
            },
            cover_bytes: provider_cover,
            source: ImportSource::Hardcover,
        }
    }

    // ── provider_book_to_result tests ─────────────────────────────────────────

    #[test]
    fn provider_book_to_result_maps_fields() {
        let pb = make_provider_book(Some("Dune"), &["Frank Herbert"], Some("en"), None, None);
        let result = provider_book_to_result(&pb);
        assert_eq!(result.title, "Dune");
        assert_eq!(result.authors, vec!["Frank Herbert"]);
        assert_eq!(result.language, "en");
        assert!(result.cover_thumbnail.is_none());
        assert!(result.cover_dimensions.is_none());
    }

    #[test]
    fn provider_book_to_result_provider_cover_takes_priority() {
        // Provider cover (PNG 10×20) and metadata cover (PNG 30×40) both present.
        // Provider cover must win.
        let provider_cover = png_bytes(10, 20);
        let metadata_cover = png_bytes(30, 40);
        let pb = make_provider_book(Some("Foundation"), &[], None, Some(provider_cover), Some(metadata_cover));
        let result = provider_book_to_result(&pb);
        // cover_dimensions should reflect the provider cover (10×20), not metadata
        // (30×40)
        assert_eq!(result.cover_dimensions, Some((10, 20)));
        assert!(result.cover_thumbnail.is_some());
    }

    #[test]
    fn provider_book_to_result_no_cover() {
        let pb = make_provider_book(Some("1984"), &[], None, None, None);
        let result = provider_book_to_result(&pb);
        assert!(result.cover_thumbnail.is_none());
        assert!(result.cover_dimensions.is_none());
    }
}
