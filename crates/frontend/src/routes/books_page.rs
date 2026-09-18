use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    Route,
    components::{BookGrid, BookGridContext, SelectionActionBar, ShelfBar, filter_books_by_search},
    routes::{
        book_detail_page::ReadingStateDto,
        shelf_page::{ShelfSummary, list_my_shelves},
    },
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct AuthorLink {
    pub token: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct BookSummary {
    pub token: String,
    pub title: String,
    pub has_cover: bool,
    pub authors: Vec<AuthorLink>,
    pub series_token: Option<String>,
    pub series_name: Option<String>,
    pub series_number: Option<String>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    pub reading_state: Option<ReadingStateDto>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ListBooksResponse {
    pub books: Vec<BookSummary>,
    pub can_delete_books: bool,
    /// Books with `Reading` status, sorted by `last_progress_at` desc.
    pub currently_reading: Vec<BookSummary>,
    /// True when the active library is a system library (e.g. "All Books").
    /// In this case delete = physical delete and requires `DeleteBook`.
    pub library_is_system: bool,
    /// True when the current user owns the active library (personal library).
    /// Owner can remove books without `DeleteBook`.
    pub user_is_library_owner: bool,
    /// The active library token (for remove-from-library calls), or `None` for
    /// system / All Books.
    pub active_library_token: Option<String>,
}

#[cfg(feature = "server")]
use {
    crate::components::to_core_sort,
    crate::routes::{
        book_detail_page::to_reading_state_dto,
        server_helpers::{authenticated_user, to_server_err},
    },
    crate::server::{AuthSession, AuthUser, BackendSessionPool},
    axum::http::Method,
    axum_session_auth::{Auth, Rights},
    bb_core::{
        CoreServices,
        book::{Author, AuthorId, Book, BookAuthor, BookHydrationData, BookId, BookQuery, Series, SeriesId},
        library::{ALL_BOOKS_LIBRARY_TOKEN, LibraryToken},
        reading::ReadStatus,
        types::Capability,
    },
    std::sync::Arc,
};

/// Hydrates a slice of `Book`s into `BookSummary` view-models.
///
/// Issues O(1) DB queries via a single `fetch_hydration_data` call
/// regardless of library size.
/// `reading_map` is an optional pre-built map of `book_id → ReadingStateDto`
/// used to attach per-user reading state; pass `None` for read-only contexts.
#[cfg(feature = "server")]
pub(crate) async fn hydrate_books(
    books: &[Book],
    core_services: &CoreServices,
    reading_map: Option<&std::collections::HashMap<u64, ReadingStateDto>>,
) -> Result<Vec<BookSummary>, ServerFnError> {
    use std::collections::HashMap;

    if books.is_empty() {
        return Ok(vec![]);
    }

    let book_ids: Vec<BookId> = books.iter().map(|b| b.id).collect();
    let series_ids: Vec<SeriesId> = {
        let mut ids: Vec<SeriesId> = books.iter().filter_map(|b| b.series_id).collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    };

    let data: BookHydrationData = core_services
        .book_service
        .fetch_hydration_data(&book_ids, &series_ids)
        .await
        .map_err(to_server_err)?;

    // Build lookup maps for O(1) assembly.
    let author_map: HashMap<AuthorId, &Author> = data.authors.iter().map(|a| (a.id, a)).collect();
    let series_map: HashMap<SeriesId, &Series> = data.series.iter().map(|s| (s.id, s)).collect();

    let mut book_authors_map: HashMap<BookId, Vec<&BookAuthor>> = HashMap::new();
    for ba in &data.book_authors {
        book_authors_map.entry(ba.book_id).or_default().push(ba);
    }

    let mut genres_map: HashMap<BookId, Vec<String>> = HashMap::new();
    for (book_id, genre) in &data.genres {
        genres_map.entry(*book_id).or_default().push(genre.name.clone());
    }

    let mut tags_map: HashMap<BookId, Vec<String>> = HashMap::new();
    for (book_id, tag) in &data.tags {
        tags_map.entry(*book_id).or_default().push(tag.name.clone());
    }

    let summaries = books
        .iter()
        .map(|book| {
            let mut author_links = book_authors_map.get(&book.id).cloned().unwrap_or_default();
            author_links.sort_by_key(|ba| ba.sort_order);
            let authors = author_links
                .iter()
                .filter_map(|ba| {
                    author_map.get(&ba.author_id).map(|a| AuthorLink {
                        token: a.token.to_string(),
                        name: a.name.clone(),
                    })
                })
                .collect();

            let (series_token, series_name) = book
                .series_id
                .and_then(|sid| series_map.get(&sid))
                .map_or((None, None), |s| (Some(s.token.to_string()), Some(s.name.clone())));

            BookSummary {
                token: book.token.to_string(),
                title: book.title.clone(),
                has_cover: book.has_cover,
                authors,
                series_token,
                series_name,
                series_number: book.series_number.as_ref().map(std::string::ToString::to_string),
                genres: genres_map.get(&book.id).cloned().unwrap_or_default(),
                tags: tags_map.get(&book.id).cloned().unwrap_or_default(),
                reading_state: reading_map.and_then(|m| m.get(&book.id).cloned()),
                created_at: book.created_at.to_rfc3339(),
                updated_at: book.updated_at.timestamp().to_string(),
            }
        })
        .collect();

    Ok(summaries)
}

#[post("/api/v1/books", auth_session: axum::Extension<AuthSession>, core_services: axum::Extension<Arc<CoreServices>>)]
async fn list_books(sort: crate::components::SortOrder, library_token: Option<String>) -> Result<ListBooksResponse, ServerFnError> {
    use std::collections::HashMap;

    let current_user = authenticated_user(&auth_session)?;
    let user_id = current_user.id();

    let can_delete_books = Auth::<AuthUser, _, BackendSessionPool>::build([Method::POST], true)
        .requires(Rights::any([Rights::permission(Capability::DeleteBook.as_str())]))
        .validate(&current_user, &Method::POST, None)
        .await;

    // When no token is provided (initial SSR load or first client render before
    // LibraryInit has run), resolve the user's default library so the correct
    // content is shown immediately.
    let library_token = match library_token {
        None => Some(core_services.library_service.get_default_library_token(user_id).await.map_err(to_server_err)?),
        some => some,
    };

    // Resolve the active library, distinguishing system from non-system.
    let is_all_books = library_token.as_deref().is_none_or(|t| t == ALL_BOOKS_LIBRARY_TOKEN);
    let (library_id, library_is_system, user_is_library_owner, active_library_token) = if is_all_books {
        (None, true, false, None)
    } else {
        let token_str = library_token.as_deref().unwrap();
        let token = token_str.parse::<LibraryToken>().map_err(|_| ServerFnError::new("Invalid library token"))?;
        let lib_id = token.id();
        core_services
            .library_service
            .validate_user_library_access(user_id, lib_id)
            .await
            .map_err(|_| ServerFnError::new("Access denied"))?;
        // Fetch library to check is_system and owner_id.
        let lib = core_services
            .library_service
            .find_library_by_token(token)
            .await
            .map_err(to_server_err)?
            .ok_or_else(|| ServerFnError::new("Library not found"))?;
        let is_owner = lib.owner_id == Some(user_id);
        (Some(lib_id), lib.is_system, is_owner, Some(token_str.to_string()))
    };

    let filter = BookQuery {
        sort: Some(to_core_sort(sort)),
        ..Default::default()
    };
    let books = core_services
        .book_service
        .list_books(&filter, library_id, None, None)
        .await
        .map_err(to_server_err)?;

    // Load per-user reading state scoped to the fetched books (avoids page
    // limits).
    let book_ids: Vec<bb_core::book::BookId> = books.iter().map(|b| b.id).collect();
    let reading_metas = core_services
        .reading_service
        .list_for_user_and_books(user_id, &book_ids)
        .await
        .map_err(to_server_err)?;
    let reading_map: HashMap<u64, ReadingStateDto> = reading_metas
        .iter()
        .filter(|m| m.read_status != ReadStatus::Unread)
        .map(|m| (m.book_id, to_reading_state_dto(m)))
        .collect();

    let summaries = hydrate_books(&books, &core_services, Some(&reading_map)).await?;

    // Build the "Currently Reading" list only for libraries the user owns or
    // system libraries (e.g. "All Books").  When browsing another user's
    // library we deliberately omit it — reading state is personal and has no
    // relevance to a library you don't own.
    let currently_reading: Vec<BookSummary> = if library_is_system || user_is_library_owner {
        let book_id_to_idx: HashMap<u64, usize> = books.iter().enumerate().map(|(i, b)| (b.id, i)).collect();
        let mut reading_now: Vec<_> = reading_metas
            .iter()
            .filter(|m| matches!(m.read_status, ReadStatus::Reading | ReadStatus::Rereading | ReadStatus::Paused))
            .collect();
        reading_now.sort_by_key(|b| std::cmp::Reverse(b.last_progress_at));
        reading_now
            .iter()
            .filter_map(|m| book_id_to_idx.get(&m.book_id).map(|&idx| summaries[idx].clone()))
            .collect()
    } else {
        vec![]
    };

    Ok(ListBooksResponse {
        books: summaries,
        can_delete_books,
        currently_reading,
        library_is_system,
        user_is_library_owner,
        active_library_token,
    })
}

/// Removes a book from a non-system library. The caller must be either the
/// library owner or hold `DeleteBook` capability — enforced here so this
/// endpoint cannot be abused directly.
#[post(
    "/api/v1/books/remove-from-library",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn remove_book_from_library(library_token: String, book_token: String) -> Result<(), ServerFnError> {
    use crate::routes::server_helpers::to_server_err;

    let current_user = authenticated_user(&auth_session)?;
    let user_id = current_user.id();

    let lib_token = library_token.parse::<LibraryToken>().map_err(|_| ServerFnError::new("Invalid library token"))?;
    let lib = core_services
        .library_service
        .find_library_by_token(lib_token)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("Library not found"))?;

    if lib.is_system {
        return Err(ServerFnError::new("Cannot remove a book from a system library"));
    }

    let is_owner = lib.owner_id == Some(user_id);
    let has_delete = Auth::<AuthUser, _, BackendSessionPool>::build([Method::POST], true)
        .requires(Rights::any([Rights::permission(Capability::DeleteBook.as_str())]))
        .validate(&current_user, &Method::POST, None)
        .await;

    if !is_owner && !has_delete {
        return Err(ServerFnError::new("Forbidden"));
    }

    let bk_token = book_token
        .parse::<bb_core::book::BookToken>()
        .map_err(|_| ServerFnError::new("Invalid book token"))?;
    core_services
        .library_service
        .remove_book_from_library(lib_token, bk_token)
        .await
        .map_err(to_server_err)
}

#[component]
pub(crate) fn BooksPage() -> Element {
    use crate::components::ACTIVE_LIBRARY;
    use_context_provider(|| Signal::new(None::<String>)); // DraggedBookToken
    let mut books_refresh = use_signal(|| 0u32);

    // `explicit_library` drives the fetch.  It is initialised from the current
    // ACTIVE_LIBRARY (already set when navigating *back* to this page) but is
    // intentionally NOT a reactive subscriber of ACTIVE_LIBRARY.  This prevents
    // the initial LibraryInit write (None → Some) from triggering a redundant
    // re-fetch that causes the "All Books flash" on browser refresh.
    //
    // `initialized` tracks whether ACTIVE_LIBRARY has been set at least once
    // since this component mounted, so the effect can tell the difference
    // between "LibraryInit is just now setting the default for the first
    // time" (skip) and "the user explicitly switched libraries" (re-fetch).
    let mut explicit_library = use_signal(|| ACTIVE_LIBRARY.peek().clone());
    let mut initialized = use_signal(|| ACTIVE_LIBRARY.peek().is_some());

    // Keep `explicit_library` in sync with user-initiated library switches, but
    // ignore the first None → Some transition that comes from LibraryInit.
    use_effect(move || {
        let active = ACTIVE_LIBRARY();
        // Use peek() for `initialized` so reading it here does NOT create a
        // reactive subscription.  Without peek(), setting initialized to true
        // would re-fire this effect, fall into the "already initialized"
        // branch, and write to explicit_library — causing the very
        // re-fetch we want to prevent.
        if *initialized.peek() {
            // Already past initialization — this is a real user switch.
            if *explicit_library.peek() != active {
                *explicit_library.write() = active;
            }
        } else if active.is_some() {
            // First time we see Some: LibraryInit just set the default.
            // Mark as initialized but do NOT update explicit_library so the
            // server future doesn't re-run.
            initialized.set(true);
        }
    });

    // Apply any pending search set by external navigation (e.g. genre/tag
    // links). Ordering guarantee: this effect runs once on first render,
    // which happens in a separate render pass *after* AppLayout's
    // route-change effect has already cleared SEARCH_TEXT. BooksPage is a
    // fresh mount on navigation — it cannot render before AppLayout has
    // processed the route change that caused the mount.
    use_effect(move || {
        if let Some(search) = crate::components::PENDING_SEARCH.write().take() {
            *crate::components::SEARCH_TEXT.write() = search;
        }
    });

    let mut page_data = use_server_future(move || {
        let sort = crate::components::SORT_ORDER();
        let library_token = explicit_library();
        let _ = books_refresh();
        list_books(sort, library_token)
    })?;
    let mut shelves_resource = use_server_future(list_my_shelves)?;
    let shelves: Vec<ShelfSummary> = shelves_resource().and_then(std::result::Result::ok).unwrap_or_default();
    rsx! {
        match page_data() {
            None => rsx! {
                div { class: "flex-1 flex items-center justify-center text-gray-400 dark:text-slate-500 text-sm",
                    "Loading…"
                }
            },
            Some(Err(e)) => rsx! {
                div { class: "flex-1 flex items-center justify-center text-red-600 text-sm",
                    "Failed to load books: {e}"
                }
            },
            Some(Ok(ListBooksResponse { books, can_delete_books, currently_reading, library_is_system, user_is_library_owner, active_library_token })) => {
                let query = crate::components::SEARCH_TEXT();
                let filtered_books = filter_books_by_search(books, &query);
                let filtered_reading = filter_books_by_search(currently_reading, &query);
                let has_search = !query.trim().is_empty();
                let book_tokens: Vec<String> = filtered_books.iter().map(|b| b.token.clone()).collect();
                let grid_context = if library_is_system {
                    BookGridContext::SystemLibrary { can_delete: can_delete_books }
                } else {
                    let can_remove = user_is_library_owner || can_delete_books;
                    BookGridContext::NonSystemLibrary {
                        library_token: active_library_token.unwrap_or_default(),
                        can_remove,
                    }
                };
                rsx! {
                    div { class: "flex-1 flex flex-col overflow-hidden",
                        ShelfBar {
                            shelves,
                            current_shelf_token: None,
                            on_edit_shelf: |()| {},
                            on_delete_shelf: |()| {},
                        }
                        div { class: "flex-1 overflow-auto",
                            CurrentlyReadingSection { books: filtered_reading }
                            if filtered_books.is_empty() && has_search {
                                div { class: "p-8 text-center text-gray-400 dark:text-slate-500 text-sm",
                                    "No books match your search."
                                }
                            } else {
                                BookGrid {
                                    books: filtered_books,
                                    context: grid_context,
                                    on_action: move |()| page_data.restart(),
                                }
                            }
                        }
                    }
                    SelectionActionBar {
                        all_book_tokens: book_tokens,
                        on_action: move |()| {
                            *books_refresh.write() += 1;
                            shelves_resource.restart();
                        },
                    }
                }
            },
        }
    }
}

// ---------------------------------------------------------------------------
// CurrentlyReadingSection
// ---------------------------------------------------------------------------

#[component]
fn CurrentlyReadingSection(books: Vec<BookSummary>) -> Element {
    if books.is_empty() {
        return rsx! {};
    }

    let navigator = use_navigator();

    rsx! {
        div { class: "shrink-0 border-b border-gray-100 dark:border-slate-700 bg-gray-50 dark:bg-slate-900 px-4 pt-3 pb-2",
            h2 { class: "text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-slate-400 mb-2",
                "Currently Reading"
            }
            div { class: "flex gap-3 overflow-x-auto pb-1",
                for book in &books {
                    {
                        let tok = book.token.clone();
                        let pct = book.reading_state.as_ref().and_then(|s| s.progress_pct).unwrap_or(0);
                        rsx! {
                            div {
                                class: "relative flex-none w-20 cursor-pointer",
                                onclick: move |_| {
                                    navigator.push(Route::BookDetailPage { token: tok.clone() });
                                },
                                img {
                                    src: "/api/v1/covers/{book.token}?v={book.updated_at}",
                                    alt: "{book.title}",
                                    class: "w-full object-cover rounded shadow-sm",
                                    style: "aspect-ratio: 2/3",
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
                }
            }
        }
    }
}
