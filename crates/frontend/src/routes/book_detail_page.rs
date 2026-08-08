use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::Route;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct AuthorDetail {
    pub token: String,
    pub name: String,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct FileDetail {
    pub format: String,
    pub file_size: i64,
    /// First 8 hex chars of the file hash, used as a cache-busting query
    /// parameter.
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct IdentifierDetail {
    pub identifier_type: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ReadingStateDto {
    pub status: String,
    pub progress_pct: Option<u8>,
    pub personal_rating: Option<u8>,
    pub times_read: u32,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct BookDetail {
    pub token: String,
    pub title: String,
    pub description: Option<String>,
    pub published_date: Option<i32>,
    pub language: Option<String>,
    pub page_count: Option<i32>,
    pub series_token: Option<String>,
    pub series_name: Option<String>,
    pub series_number: Option<String>,
    pub authors: Vec<AuthorDetail>,
    pub files: Vec<FileDetail>,
    pub identifiers: Vec<IdentifierDetail>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    pub can_edit: bool,
    pub can_delete: bool,
    pub reading_state: Option<ReadingStateDto>,
    pub updated_at: String,
}

#[cfg(feature = "server")]
use {
    crate::routes::server_helpers::{authenticated_user, require_capability, to_server_err},
    crate::server::{AuthSession, AuthUser, BackendSessionPool},
    axum::http::Method,
    axum_session_auth::{Auth, Rights},
    bb_core::book::{AuthorToken, BookToken, FileRole, SeriesToken},
    bb_core::reading::{ReadStatus, UserBookMetadata},
    bb_core::{CoreServices, types::Capability, user::UserId},
    std::str::FromStr,
    std::sync::Arc,
};

#[cfg(feature = "server")]
pub(crate) fn to_reading_state_dto(meta: &UserBookMetadata) -> ReadingStateDto {
    ReadingStateDto {
        status: match meta.read_status {
            ReadStatus::Unread => "Unread",
            ReadStatus::Reading => "Reading",
            ReadStatus::Paused => "Paused",
            ReadStatus::Rereading => "Rereading",
            ReadStatus::Read => "Read",
            ReadStatus::Abandoned => "Abandoned",
        }
        .to_string(),
        #[expect(clippy::cast_possible_truncation, reason = "bps / 100 gives 0–100 percentage; always fits u8")]
        progress_pct: meta.progress_percentage.map(|bps| (bps / 100) as u8),
        personal_rating: meta.personal_rating,
        times_read: meta.times_read,
        notes: meta.notes.clone(),
    }
}

// ---------------------------------------------------------------------------
// Server functions
// ---------------------------------------------------------------------------

#[post("/api/v1/book", auth_session: axum::Extension<AuthSession>, core_services: axum::Extension<Arc<CoreServices>>)]
async fn get_book(token: String) -> Result<BookDetail, ServerFnError> {
    let user_id = authenticated_user(&auth_session)?.id();
    let book_service = &core_services.book_service;

    let book_token = BookToken::from_str(&token).map_err(|_| ServerFnError::new("Invalid book token"))?;

    let book = book_service
        .find_book_by_token(book_token)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("Book not found"))?;

    let book_author_links = book_service.authors_for_book(book.id).await.map_err(to_server_err)?;

    let book_files = book_service.files_for_book(book.id).await.map_err(to_server_err)?;

    let book_identifiers = book_service.identifiers_for_book(book.id).await.map_err(to_server_err)?;

    // Fetch unique authors (token + name)
    let mut author_map: std::collections::HashMap<u64, (String, String)> = std::collections::HashMap::new();
    for ba in &book_author_links {
        if let std::collections::hash_map::Entry::Vacant(e) = author_map.entry(ba.author_id)
            && let Some(author) = book_service.find_author_by_token(AuthorToken::new(ba.author_id)).await.map_err(to_server_err)?
        {
            e.insert((author.token.to_string(), author.name));
        }
    }

    // Build author details sorted by sort_order
    let mut sorted_authors = book_author_links.clone();
    sorted_authors.sort_by_key(|ba| ba.sort_order);
    let authors: Vec<AuthorDetail> = sorted_authors
        .iter()
        .filter_map(|ba| {
            author_map.get(&ba.author_id).map(|(token, name)| AuthorDetail {
                token: token.clone(),
                name: name.clone(),
                role: ba.role.display_name().to_string(),
            })
        })
        .collect();

    // Fetch series name and token if needed
    let (series_token, series_name) = if let Some(series_id) = book.series_id {
        let series = book_service.find_series_by_token(SeriesToken::new(series_id)).await.map_err(to_server_err)?;
        match series {
            Some(s) => (Some(s.token.to_string()), Some(s.name)),
            None => (None, None),
        }
    } else {
        (None, None)
    };

    // Deduplicate by format: Enriched takes priority over Original for display.
    // This ensures one download badge per format even when both roles exist.
    let mut format_map: std::collections::HashMap<String, (i64, String)> = std::collections::HashMap::new();
    for f in &book_files {
        let fmt_str = f.format.display_name().to_string();
        let entry = format_map
            .entry(fmt_str)
            .or_insert_with(|| (f.file_size, f.file_hash[..8.min(f.file_hash.len())].to_string()));
        if f.file_role == FileRole::Enriched {
            *entry = (f.file_size, f.file_hash[..8.min(f.file_hash.len())].to_string());
        }
    }
    let mut files: Vec<FileDetail> = format_map
        .into_iter()
        .map(|(format, (file_size, version))| FileDetail { format, file_size, version })
        .collect();
    files.sort_by(|a, b| a.format.cmp(&b.format));

    let identifiers: Vec<IdentifierDetail> = book_identifiers
        .iter()
        .map(|i| IdentifierDetail {
            identifier_type: i.identifier_type.display_name().to_string(),
            value: i.value.clone(),
        })
        .collect();

    let genres: Vec<String> = book_service
        .genres_for_book(book.id)
        .await
        .map_err(to_server_err)?
        .into_iter()
        .map(|g| g.name)
        .collect();

    let tags: Vec<String> = book_service
        .tags_for_book(book.id)
        .await
        .map_err(to_server_err)?
        .into_iter()
        .map(|t| t.name)
        .collect();

    let current_user = auth_session.current_user.clone().unwrap_or_default();
    let can_edit = Auth::<AuthUser, UserId, BackendSessionPool>::build([Method::POST], true)
        .requires(Rights::any([Rights::permission(Capability::EditBook.as_str())]))
        .validate(&current_user, &Method::POST, None)
        .await;
    let can_delete = Auth::<AuthUser, UserId, BackendSessionPool>::build([Method::POST], true)
        .requires(Rights::any([Rights::permission(Capability::DeleteBook.as_str())]))
        .validate(&current_user, &Method::POST, None)
        .await;

    let reading_state = core_services
        .reading_service
        .get_reading_state(user_id, book.id)
        .await
        .map_err(to_server_err)?
        .as_ref()
        .map(to_reading_state_dto);

    Ok(BookDetail {
        token: book.token.to_string(),
        title: book.title.clone(),
        description: book.description.clone(),
        published_date: book.published_date,
        language: book.language.clone(),
        page_count: book.page_count,
        series_token,
        series_name,
        series_number: book.series_number.as_ref().map(std::string::ToString::to_string),
        authors,
        files,
        identifiers,
        genres,
        tags,
        can_edit,
        can_delete,
        reading_state,
        updated_at: book.updated_at.timestamp().to_string(),
    })
}

#[post(
    "/api/v1/book/delete",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn delete_library_book(token: String) -> Result<(), ServerFnError> {
    require_capability(&auth_session, Capability::DeleteBook, Method::POST).await?;

    let book_token = BookToken::from_str(&token).map_err(|_| ServerFnError::new("Invalid book token"))?;

    core_services.collection_service.delete_book(book_token).await.map_err(to_server_err)
}

#[post(
    "/api/v1/book/reading/status",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn set_reading_status(token: String, status: String) -> Result<ReadingStateDto, ServerFnError> {
    let user_id = authenticated_user(&auth_session)?.id();
    let book_token = BookToken::from_str(&token).map_err(|_| ServerFnError::new("Invalid book token"))?;
    let book = core_services
        .book_service
        .find_book_by_token(book_token)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("Book not found"))?;

    let new_status: ReadStatus = status.parse().map_err(|e: String| ServerFnError::new(e))?;

    let meta = core_services
        .reading_service
        .set_status(user_id, book.id, new_status)
        .await
        .map_err(to_server_err)?;

    Ok(to_reading_state_dto(&meta))
}

#[post(
    "/api/v1/book/reading/progress",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn update_reading_progress(token: String, progress_pct: u8) -> Result<ReadingStateDto, ServerFnError> {
    let user_id = authenticated_user(&auth_session)?.id();

    if progress_pct > 100 {
        return Err(ServerFnError::new("Progress must be between 0 and 100"));
    }
    let book_token = BookToken::from_str(&token).map_err(|_| ServerFnError::new("Invalid book token"))?;
    let book = core_services
        .book_service
        .find_book_by_token(book_token)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("Book not found"))?;

    let progress_bps = u16::from(progress_pct) * 100;
    let meta = core_services
        .reading_service
        .update_progress(user_id, book.id, progress_bps, None)
        .await
        .map_err(to_server_err)?;

    Ok(to_reading_state_dto(&meta))
}

#[post(
    "/api/v1/book/reading/rating",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn set_personal_rating(token: String, rating: u8) -> Result<ReadingStateDto, ServerFnError> {
    let user_id = authenticated_user(&auth_session)?.id();
    let book_token = BookToken::from_str(&token).map_err(|_| ServerFnError::new("Invalid book token"))?;
    let book = core_services
        .book_service
        .find_book_by_token(book_token)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("Book not found"))?;

    let meta = core_services
        .reading_service
        .set_rating(user_id, book.id, rating)
        .await
        .map_err(to_server_err)?;

    Ok(to_reading_state_dto(&meta))
}

#[post(
    "/api/v1/book/reading/rating/clear",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn clear_personal_rating(token: String) -> Result<ReadingStateDto, ServerFnError> {
    let user_id = authenticated_user(&auth_session)?.id();
    let book_token = BookToken::from_str(&token).map_err(|_| ServerFnError::new("Invalid book token"))?;
    let book = core_services
        .book_service
        .find_book_by_token(book_token)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("Book not found"))?;

    let meta = core_services.reading_service.clear_rating(user_id, book.id).await.map_err(to_server_err)?;

    Ok(to_reading_state_dto(&meta))
}

#[post(
    "/api/v1/book/reading/notes",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn save_reading_notes(token: String, notes: String) -> Result<ReadingStateDto, ServerFnError> {
    let user_id = authenticated_user(&auth_session)?.id();
    let book_token = BookToken::from_str(&token).map_err(|_| ServerFnError::new("Invalid book token"))?;
    let book = core_services
        .book_service
        .find_book_by_token(book_token)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("Book not found"))?;

    let meta = core_services.reading_service.set_notes(user_id, book.id, notes).await.map_err(to_server_err)?;

    Ok(to_reading_state_dto(&meta))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn format_file_size(bytes: i64) -> String {
    const KB: i64 = 1024;
    const MB: i64 = 1024 * 1024;
    if bytes < KB {
        format!("{bytes} B")
    } else if bytes < MB {
        format!("{} KB", bytes / KB)
    } else {
        #[expect(clippy::cast_precision_loss, reason = "precision loss acceptable for human-readable file size display")]
        let mb = bytes as f64 / MB as f64;
        format!("{mb:.1} MB")
    }
}

// ---------------------------------------------------------------------------
// BookDetailPage
// ---------------------------------------------------------------------------

#[component]
pub(crate) fn BookDetailPage(token: String) -> Element {
    let nav = use_navigator();
    let book = use_server_future(move || get_book(token.clone()))?;
    let mut show_confirm = use_signal(|| false);
    let mut deleting = use_signal(|| false);

    rsx! {
        div { class: "flex-1 overflow-auto p-6",
            match book() {
                None => rsx! {
                    div { class: "flex items-center justify-center h-full text-gray-400 dark:text-slate-500 text-sm",
                        "Loading…"
                    }
                },
                Some(Err(e)) => rsx! {
                    div { class: "text-red-600 text-sm", "Failed to load book: {e}" }
                },
                Some(Ok(book)) => rsx! {
                    // Delete confirmation modal
                    if show_confirm() {
                        div {
                            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/40",
                            tabindex: -1,
                            onkeydown: move |e| { if e.key() == Key::Escape { show_confirm.set(false); } },
                            div { class: "bg-white dark:bg-slate-800 rounded-lg shadow-xl p-6 w-full max-w-sm mx-4",
                                h2 { class: "text-lg font-semibold text-gray-900 dark:text-slate-100 mb-2", "Delete Book?" }
                                p { class: "text-sm text-gray-600 dark:text-slate-300 mb-6",
                                    "This will permanently delete \"{book.title}\" and all its files. This cannot be undone."
                                }
                                div { class: "flex gap-3 justify-end",
                                    button {
                                        class: "px-4 py-2 text-sm font-medium rounded border border-gray-300 dark:border-slate-600 text-gray-700 dark:text-slate-300 hover:bg-gray-50 dark:hover:bg-slate-700",
                                        autofocus: true,
                                        onclick: move |_| show_confirm.set(false),
                                        "No, Keep It"
                                    }
                                    button {
                                        class: "px-4 py-2 text-sm font-medium rounded bg-red-600 text-white hover:bg-red-700 disabled:opacity-50",
                                        disabled: deleting(),
                                        onclick: {
                                            let bk = book.token.clone();
                                            move |_| {
                                                let bk = bk.clone();
                                                // Hide the modal and start deleting synchronously so
                                                // the modal closes immediately on confirm regardless
                                                // of when the async task's render cycle runs.
                                                show_confirm.set(false);
                                                deleting.set(true);
                                                spawn(async move {
                                                    if let Ok(()) = delete_library_book(bk).await {
                                                        let _ = nav.push(Route::BooksPage {});
                                                    } else {
                                                        deleting.set(false);
                                                    }
                                                });
                                            }
                                        },
                                        if deleting() { "Deleting…" } else { "Yes, Delete" }
                                    }
                                }
                            }
                        }
                    }

                    // Back link
                    Link {
                        to: Route::BooksPage {},
                        class: "inline-flex items-center gap-1 text-sm text-indigo-600 dark:text-indigo-400 hover:text-indigo-800 dark:hover:text-indigo-300 mb-6",
                        "← Library"
                    }

                    div { class: "flex gap-8",
                        // Cover
                        BookCover {
                            token: book.token.clone(),
                            updated_at: book.updated_at.clone(),
                            progress_pct: book.reading_state.as_ref()
                                .filter(|rs| matches!(rs.status.as_str(), "Reading" | "Rereading" | "Paused"))
                                .and_then(|rs| rs.progress_pct),
                        }

                        // Main info
                        div { class: "flex-1 min-w-0",
                            div { class: "flex items-start justify-between gap-4 mb-2",
                                div { class: "flex flex-wrap items-baseline gap-x-2 gap-y-1 min-w-0",
                                    h1 { class: "text-2xl font-bold text-gray-900 dark:text-slate-100", "{book.title}" }
                                    InlineStarRating {
                                        token: book.token.clone(),
                                        initial_rating: book.reading_state.as_ref().and_then(|s| s.personal_rating),
                                    }
                                }
                                div { class: "flex items-center gap-2 shrink-0",
                                    if book.can_edit {
                                        Link {
                                            to: Route::EditMetadataPage { token: book.token.clone() },
                                            class: "px-3 py-1 text-xs font-medium rounded border border-gray-300 dark:border-slate-600 text-gray-600 dark:text-slate-300 hover:bg-gray-50 dark:hover:bg-slate-700",
                                            "Edit"
                                        }
                                    }
                                    if book.can_delete {
                                        button {
                                            class: "px-3 py-1 text-xs font-medium rounded border border-red-300 text-red-600 hover:bg-red-50",
                                            onclick: move |_| show_confirm.set(true),
                                            "Delete"
                                        }
                                    }
                                }
                            }

                            // Authors
                            if !book.authors.is_empty() {
                                div { class: "flex flex-wrap gap-2 mb-3",
                                    for author in &book.authors {
                                        span { class: "text-sm text-gray-700 dark:text-slate-300",
                                            Link {
                                                to: Route::AuthorDetailPage { token: author.token.clone() },
                                                class: "text-indigo-600 dark:text-indigo-400 hover:text-indigo-800 dark:hover:text-indigo-300",
                                                "{author.name}"
                                            }
                                            if author.role != "Author" {
                                                span { class: "text-gray-400 dark:text-slate-500 ml-1", "({author.role})" }
                                            }
                                        }
                                    }
                                }
                            }

                            // Series
                            if let (Some(series_name), Some(series_token)) = (&book.series_name, &book.series_token) {
                                p { class: "text-sm mb-3",
                                    Link {
                                        to: Route::SeriesDetailPage { token: series_token.clone() },
                                        class: "text-indigo-600 dark:text-indigo-400 hover:text-indigo-800 dark:hover:text-indigo-300",
                                        match &book.series_number {
                                            Some(num) => rsx! { "{series_name} #{num}" },
                                            None => rsx! { {series_name.clone()} },
                                        }
                                    }
                                }
                            }

                            // Metadata row
                            div { class: "flex flex-wrap gap-4 text-sm text-gray-500 dark:text-slate-400 mb-4",
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

                            // Genres
                            if !book.genres.is_empty() {
                                div { class: "flex flex-wrap items-center gap-1 mb-3",
                                    svg {
                                        class: "w-3.5 h-3.5 text-gray-400 dark:text-slate-500 shrink-0 mr-0.5",
                                        xmlns: "http://www.w3.org/2000/svg",
                                        fill: "none",
                                        view_box: "0 0 24 24",
                                        stroke_width: "1.5",
                                        stroke: "currentColor",
                                        path {
                                            stroke_linecap: "round",
                                            stroke_linejoin: "round",
                                            d: "M3.75 6A2.25 2.25 0 0 1 6 3.75h2.25A2.25 2.25 0 0 1 10.5 6v2.25a2.25 2.25 0 0 1-2.25 2.25H6a2.25 2.25 0 0 1-2.25-2.25V6ZM3.75 15.75A2.25 2.25 0 0 1 6 13.5h2.25a2.25 2.25 0 0 1 2.25 2.25V18a2.25 2.25 0 0 1-2.25 2.25H6A2.25 2.25 0 0 1 3.75 18v-2.25ZM13.5 6a2.25 2.25 0 0 1 2.25-2.25H18A2.25 2.25 0 0 1 20.25 6v2.25A2.25 2.25 0 0 1 18 10.5h-2.25a2.25 2.25 0 0 1-2.25-2.25V6ZM13.5 15.75a2.25 2.25 0 0 1 2.25-2.25H18a2.25 2.25 0 0 1 2.25 2.25V18A2.25 2.25 0 0 1 18 20.25h-2.25A2.25 2.25 0 0 1 13.5 18v-2.25Z",
                                        }
                                    }
                                    for genre in &book.genres {
                                        span { class: "inline-flex items-center px-2 py-0.5 rounded-full text-xs bg-gray-100 dark:bg-slate-700 text-gray-700 dark:text-slate-300 border border-gray-200 dark:border-slate-600",
                                            {genre.clone()}
                                        }
                                    }
                                }
                            }

                            // Tags
                            if !book.tags.is_empty() {
                                div { class: "flex flex-wrap items-center gap-1 mb-3",
                                    svg {
                                        class: "w-3.5 h-3.5 text-gray-400 dark:text-slate-500 shrink-0 mr-0.5",
                                        xmlns: "http://www.w3.org/2000/svg",
                                        fill: "none",
                                        view_box: "0 0 24 24",
                                        stroke_width: "1.5",
                                        stroke: "currentColor",
                                        path {
                                            stroke_linecap: "round",
                                            stroke_linejoin: "round",
                                            d: "M9.568 3H5.25A2.25 2.25 0 0 0 3 5.25v4.318c0 .597.237 1.17.659 1.591l9.581 9.581c.699.699 1.78.872 2.607.33a18.095 18.095 0 0 0 5.223-5.223c.542-.827.369-1.908-.33-2.607L11.16 3.66A2.25 2.25 0 0 0 9.568 3Z",
                                        }
                                        path {
                                            stroke_linecap: "round",
                                            stroke_linejoin: "round",
                                            d: "M6 6h.008v.008H6V6Z",
                                        }
                                    }
                                    for tag in &book.tags {
                                        span { class: "inline-flex items-center px-2 py-0.5 rounded-full text-xs bg-indigo-50 text-indigo-700 border border-indigo-200 dark:bg-slate-700 dark:text-indigo-300 dark:border-slate-600",
                                            {tag.clone()}
                                        }
                                    }
                                }
                            }

                            // Files + reading status on one row
                            div { class: "flex flex-wrap items-center gap-2 mb-4",
                                for file in &book.files {
                                    {
                                        let size_str = format_file_size(file.file_size);
                                        let fmt_lower = file.format.to_lowercase();
                                        let href = format!("/api/v1/books/{}/download/{fmt_lower}?v={}", book.token, file.version);
                                        rsx! {
                                            a {
                                                href,
                                                target: "_blank",
                                                rel: "noopener noreferrer",
                                                class: "inline-flex items-center gap-1.5 px-2.5 py-1 rounded bg-gray-100 dark:bg-slate-700 text-xs text-gray-700 dark:text-slate-300 hover:bg-indigo-50 hover:text-indigo-700 transition-colors",
                                                span { class: "font-medium", "{file.format}" }
                                                span { class: "text-gray-400 dark:text-slate-500", "↓ {size_str}" }
                                            }
                                        }
                                    }
                                }
                                StatusPill {
                                    token: book.token.clone(),
                                    initial_state: book.reading_state.clone(),
                                }
                            }

                            // Description
                            if let Some(ref desc) = book.description {
                                p { class: "text-sm text-gray-700 dark:text-slate-300 leading-relaxed mb-6", {desc.clone()} }
                            }

                            // Identifiers
                            if !book.identifiers.is_empty() {
                                div {
                                    h2 { class: "text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-slate-400 mb-2",
                                        "Identifiers"
                                    }
                                    dl { class: "space-y-1",
                                        for id in &book.identifiers {
                                            div { class: "flex gap-2 text-sm",
                                                dt { class: "text-gray-500 dark:text-slate-400 w-28 shrink-0", "{id.identifier_type}" }
                                                dd { class: "text-gray-800 dark:text-slate-200 font-mono", "{id.value}" }
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

// ---------------------------------------------------------------------------
// BookCover
// ---------------------------------------------------------------------------

#[component]
fn BookCover(token: String, updated_at: String, progress_pct: Option<u8>) -> Element {
    let pct = progress_pct.unwrap_or(0);
    rsx! {
        div { class: "shrink-0 relative w-36 self-start",
            img {
                src: "/api/v1/covers/{token}?v={updated_at}&full=true",
                class: "w-full block rounded shadow-md",
                style: "aspect-ratio: 2/3; object-fit: cover",
            }
            if pct > 0 {
                div {
                    class: "absolute bottom-0 left-0 right-0 h-1 bg-black/20 rounded-b overflow-hidden",
                    div { class: "h-full bg-indigo-400", style: "width: {pct}%" }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// StatusPill
// ---------------------------------------------------------------------------

#[component]
fn StatusPill(token: String, initial_state: Option<ReadingStateDto>) -> Element {
    let mut status = use_signal(move || initial_state.map_or_else(|| "Unread".to_string(), |s| s.status));
    let mut show_dropdown = use_signal(|| false);
    let mut busy = use_signal(|| false);

    let pill_class = move || match status().as_str() {
        "Reading" | "Rereading" => {
            "inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium bg-indigo-100 text-indigo-700 dark:bg-indigo-900 dark:text-indigo-300"
        }
        "Paused" => {
            "inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium bg-yellow-100 text-yellow-700 dark:bg-yellow-900 dark:text-yellow-300"
        }
        "Read" => "inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300",
        "Abandoned" => "inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium bg-red-100 text-red-700 dark:bg-red-900 dark:text-red-300",
        _ => "inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium bg-gray-100 text-gray-600 dark:bg-slate-700 dark:text-slate-300",
    };

    rsx! {
        div { class: "relative inline-flex items-center gap-1",
            span { class: pill_class(), "{status()}" }
            button {
                class: "text-gray-400 dark:text-slate-500 hover:text-gray-600 dark:hover:text-slate-300 disabled:opacity-40",
                title: "Change reading status",
                disabled: busy(),
                onclick: move |_| show_dropdown.set(!show_dropdown()),
                svg {
                    class: "w-3.5 h-3.5",
                    xmlns: "http://www.w3.org/2000/svg",
                    fill: "none",
                    view_box: "0 0 24 24",
                    stroke_width: "2",
                    stroke: "currentColor",
                    path {
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        d: "M16.862 4.487l1.687-1.688a1.875 1.875 0 112.652 2.652L6.832 19.82a4.5 4.5 0 01-1.897 1.13l-2.685.8.8-2.685a4.5 4.5 0 011.13-1.897L16.863 4.487zm0 0L19.5 7.125",
                    }
                }
            }
            if show_dropdown() {
                div { class: "absolute top-full left-0 mt-1 z-10 bg-white dark:bg-slate-800 rounded-lg shadow-lg border border-gray-200 dark:border-slate-700 py-1 min-w-max",
                    for s in ["Unread", "Reading", "Paused", "Rereading", "Read", "Abandoned"] {
                        {
                            let tok = token.clone();
                            let s_owned = s.to_string();
                            rsx! {
                                button {
                                    class: "block w-full text-left px-3 py-1.5 text-xs text-gray-700 dark:text-slate-300 hover:bg-gray-50 dark:hover:bg-slate-700 disabled:opacity-40",
                                    disabled: busy(),
                                    onclick: move |_| {
                                        let tok = tok.clone();
                                        let s = s_owned.clone();
                                        show_dropdown.set(false);
                                        busy.set(true);
                                        spawn(async move {
                                            if let Ok(new_state) = set_reading_status(tok, s).await {
                                                status.set(new_state.status);
                                            }
                                            busy.set(false);
                                        });
                                    },
                                    {s}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// InlineStarRating
// ---------------------------------------------------------------------------

#[component]
fn InlineStarRating(token: String, initial_rating: Option<u8>) -> Element {
    let mut rating = use_signal(move || initial_rating.unwrap_or(0));
    let mut busy = use_signal(|| false);

    rsx! {
        div { class: "flex items-center gap-0.5",
            for star in 1u8..=5u8 {
                {
                    let filled = star <= rating();
                    let tok = token.clone();
                    rsx! {
                        button {
                            class: if filled {
                                "text-yellow-400 text-lg leading-none hover:scale-110 transition-transform disabled:cursor-default"
                            } else {
                                "text-gray-300 dark:text-slate-600 text-lg leading-none hover:text-yellow-400 transition-colors disabled:cursor-default"
                            },
                            disabled: busy(),
                            onclick: move |_| {
                                let tok = tok.clone();
                                busy.set(true);
                                spawn(async move {
                                    if let Ok(s) = set_personal_rating(tok, star).await {
                                        rating.set(s.personal_rating.unwrap_or(0));
                                    }
                                    busy.set(false);
                                });
                            },
                            if filled { "★" } else { "☆" }
                        }
                    }
                }
            }
            if rating() > 0 {
                {
                    let tok = token.clone();
                    rsx! {
                        button {
                            class: "ml-1 text-gray-400 dark:text-slate-500 hover:text-gray-600 dark:hover:text-slate-300 text-sm leading-none transition-colors disabled:cursor-default",
                            title: "Clear rating",
                            disabled: busy(),
                            onclick: move |_| {
                                let tok = tok.clone();
                                busy.set(true);
                                spawn(async move {
                                    if let Ok(s) = clear_personal_rating(tok).await {
                                        rating.set(s.personal_rating.unwrap_or(0));
                                    }
                                    busy.set(false);
                                });
                            },
                            "✕"
                        }
                    }
                }
            }
        }
    }
}
