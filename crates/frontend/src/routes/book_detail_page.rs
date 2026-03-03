use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::Route;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct AuthorDetail {
    pub name: String,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct FileDetail {
    pub format: String,
    pub file_size: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct IdentifierDetail {
    pub identifier_type: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct BookDetail {
    pub token: String,
    pub title: String,
    pub description: Option<String>,
    pub published_date: Option<i32>,
    pub language: Option<String>,
    pub page_count: Option<i32>,
    pub cover_path: Option<String>,
    pub series_name: Option<String>,
    pub series_number: Option<String>,
    pub authors: Vec<AuthorDetail>,
    pub files: Vec<FileDetail>,
    pub identifiers: Vec<IdentifierDetail>,
}

#[cfg(feature = "server")]
use {
    crate::server::AuthSession,
    bb_core::CoreServices,
    bb_core::book::{AuthorRole, AuthorToken, BookToken, FileFormat, IdentifierType, SeriesToken},
    std::str::FromStr,
    std::sync::Arc,
};

#[get("/api/v1/book", auth_session: axum::Extension<AuthSession>, core_services: axum::Extension<Arc<CoreServices>>)]
#[tracing::instrument(level = "trace", skip(auth_session, core_services))]
async fn get_book(token: String) -> Result<BookDetail, ServerFnError> {
    auth_session
        .current_user
        .as_ref()
        .filter(|u| !u.username.is_empty())
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;

    let book_service = &core_services.book_service;

    let book_token = BookToken::from_str(&token).map_err(|_| ServerFnError::new("Invalid book token"))?;

    let book = book_service
        .find_book_by_token(&book_token)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("Book not found"))?;

    let book_author_links = book_service.authors_for_book(book.id).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let book_files = book_service.files_for_book(book.id).await.map_err(|e| ServerFnError::new(e.to_string()))?;

    let book_identifiers = book_service
        .identifiers_for_book(book.id)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Fetch unique author names
    let mut author_name_map = std::collections::HashMap::new();
    for ba in &book_author_links {
        if !author_name_map.contains_key(&ba.author_id) {
            if let Some(author) = book_service
                .find_author_by_token(&AuthorToken::new(ba.author_id))
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?
            {
                author_name_map.insert(ba.author_id, author.name);
            }
        }
    }

    // Build author details sorted by sort_order
    let mut sorted_authors = book_author_links.clone();
    sorted_authors.sort_by_key(|ba| ba.sort_order);
    let authors: Vec<AuthorDetail> = sorted_authors
        .iter()
        .filter_map(|ba| {
            author_name_map.get(&ba.author_id).map(|name| AuthorDetail {
                name: name.clone(),
                role: match &ba.role {
                    AuthorRole::Author => "Author",
                    AuthorRole::Editor => "Editor",
                    AuthorRole::Translator => "Translator",
                    AuthorRole::Illustrator => "Illustrator",
                }
                .to_string(),
            })
        })
        .collect();

    // Fetch series name if needed
    let series_name = if let Some(series_id) = book.series_id {
        book_service
            .find_series_by_token(&SeriesToken::new(series_id))
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
            .map(|s| s.name)
    } else {
        None
    };

    let files: Vec<FileDetail> = book_files
        .iter()
        .map(|f| FileDetail {
            format: match &f.format {
                FileFormat::Epub => "EPUB",
                FileFormat::Mobi => "MOBI",
                FileFormat::Azw3 => "AZW3",
                FileFormat::Pdf => "PDF",
                FileFormat::Cbz => "CBZ",
            }
            .to_string(),
            file_size: f.file_size,
        })
        .collect();

    let identifiers: Vec<IdentifierDetail> = book_identifiers
        .iter()
        .map(|i| IdentifierDetail {
            identifier_type: match &i.identifier_type {
                IdentifierType::Isbn13 => "ISBN-13",
                IdentifierType::Isbn10 => "ISBN-10",
                IdentifierType::Asin => "ASIN",
                IdentifierType::GoogleBooks => "Google Books",
                IdentifierType::OpenLibrary => "Open Library",
                IdentifierType::Hardcover => "Hardcover",
            }
            .to_string(),
            value: i.value.clone(),
        })
        .collect();

    Ok(BookDetail {
        token: book.token.to_string(),
        title: book.title.clone(),
        description: book.description.clone(),
        published_date: book.published_date,
        language: book.language.clone(),
        page_count: book.page_count,
        cover_path: book.cover_path.clone(),
        series_name,
        series_number: book.series_number.as_ref().map(|n| n.to_string()),
        authors,
        files,
        identifiers,
    })
}

fn format_file_size(bytes: i64) -> String {
    const KB: i64 = 1024;
    const MB: i64 = 1024 * 1024;
    if bytes < KB {
        format!("{bytes} B")
    } else if bytes < MB {
        format!("{} KB", bytes / KB)
    } else {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    }
}

#[component]
pub(crate) fn BookDetailPage(token: String) -> Element {
    let book = use_server_future(move || get_book(token.clone()))?;

    rsx! {
        div { class: "flex-1 overflow-auto p-6",
            match book() {
                None => rsx! {
                    div { class: "flex items-center justify-center h-full text-gray-400 text-sm",
                        "Loading…"
                    }
                },
                Some(Err(e)) => rsx! {
                    div { class: "text-red-600 text-sm", "Failed to load book: {e}" }
                },
                Some(Ok(book)) => rsx! {
                    // Back link
                    Link {
                        to: Route::BooksPage {},
                        class: "inline-flex items-center gap-1 text-sm text-indigo-600 hover:text-indigo-800 mb-6",
                        "← Library"
                    }

                    div { class: "flex gap-8",
                        // Cover
                        div { class: "shrink-0",
                            match book.cover_path {
                                Some(ref path) => rsx! {
                                    img {
                                        src: "{path}",
                                        alt: "{book.title}",
                                        class: "w-36 rounded shadow-md",
                                        style: "aspect-ratio: 2/3; object-fit: cover",
                                    }
                                },
                                None => rsx! {
                                    img {
                                        src: asset!("/assets/BlankCover.png"),
                                        alt: "{book.title}",
                                        class: "w-36 rounded shadow-md",
                                        style: "aspect-ratio: 2/3; object-fit: cover",
                                    }
                                },
                            }
                        }

                        // Main info
                        div { class: "flex-1 min-w-0",
                            h1 { class: "text-2xl font-bold text-gray-900 mb-2", "{book.title}" }

                            // Authors
                            if !book.authors.is_empty() {
                                div { class: "flex flex-wrap gap-2 mb-3",
                                    for author in &book.authors {
                                        span { class: "text-sm text-gray-700",
                                            "{author.name}"
                                            if author.role != "Author" {
                                                span { class: "text-gray-400 ml-1", "({author.role})" }
                                            }
                                        }
                                    }
                                }
                            }

                            // Series
                            if let Some(ref series_name) = book.series_name {
                                p { class: "text-sm text-indigo-600 mb-3",
                                    match &book.series_number {
                                        Some(num) => rsx! { "{series_name} #{num}" },
                                        None => rsx! { "{series_name}" },
                                    }
                                }
                            }

                            // Metadata row
                            div { class: "flex flex-wrap gap-4 text-sm text-gray-500 mb-4 pb-4 border-b border-gray-200",
                                if let Some(year) = book.published_date {
                                    span { "Published: {year}" }
                                }
                                if let Some(pages) = book.page_count {
                                    span { "{pages} pages" }
                                }
                                if let Some(ref lang) = book.language {
                                    span { "Language: {lang}" }
                                }
                            }

                            // Description
                            if let Some(ref desc) = book.description {
                                p { class: "text-sm text-gray-700 leading-relaxed mb-6", "{desc}" }
                            }

                            // Files
                            if !book.files.is_empty() {
                                div { class: "mb-4",
                                    h2 { class: "text-xs font-semibold uppercase tracking-wider text-gray-500 mb-2",
                                        "Formats"
                                    }
                                    div { class: "flex flex-wrap gap-2",
                                        for file in &book.files {
                                            {
                                                let size_str = format_file_size(file.file_size);
                                                rsx! {
                                                    span { class: "inline-flex items-center gap-1.5 px-2.5 py-1 rounded bg-gray-100 text-xs text-gray-700",
                                                        span { class: "font-medium", "{file.format}" }
                                                        span { class: "text-gray-400", "{size_str}" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // Identifiers
                            if !book.identifiers.is_empty() {
                                div {
                                    h2 { class: "text-xs font-semibold uppercase tracking-wider text-gray-500 mb-2",
                                        "Identifiers"
                                    }
                                    dl { class: "space-y-1",
                                        for id in &book.identifiers {
                                            div { class: "flex gap-2 text-sm",
                                                dt { class: "text-gray-500 w-28 shrink-0", "{id.identifier_type}" }
                                                dd { class: "text-gray-800 font-mono", "{id.value}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
            }
        }
    }
}
