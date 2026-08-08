use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    Route,
    components::{
        BookFilter, BookGrid, BookGridContext, FilterBuilder, FilterEntityOptions, SORT_ORDER, SelectionActionBar, ShelfBar, default_book_filter,
        filter_books_by_search, freshen_entity_labels, sort_books_client_side,
    },
    routes::books_page::BookSummary,
};

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

/// Paginated book result for a shelf.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ShelfBooksPage {
    pub books: Vec<BookSummary>,
    /// Offset for the next page. `None` when there are no more results.
    pub next_offset: Option<u64>,
}

/// Lightweight shelf descriptor returned by list and create operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ShelfSummary {
    pub id: i64,
    pub token: String,
    pub name: String,
    /// `true` if the current user owns this shelf.
    pub is_own: bool,
    /// `true` if this is a smart (filter-based) shelf.
    pub is_smart: bool,
    /// `true` if this shelf is managed by a device (delete is disabled).
    pub is_device_shelf: bool,
    /// Serialized `BookFilter` JSON — present only for smart shelves owned by
    /// the current user.
    pub filter_json: Option<String>,
    /// Matching book count — `Some(n)` when the shelf has books, `None` for
    /// empty shelves or count errors.
    pub count: Option<u64>,
    /// Token of the library this shelf belongs to. Present for own shelves.
    pub library_token: Option<String>,
}

// ---------------------------------------------------------------------------
// Server-only imports
// ---------------------------------------------------------------------------

#[cfg(feature = "server")]
use {
    crate::components::to_core_sort,
    crate::routes::{
        book_detail_page::{ReadingStateDto, to_reading_state_dto},
        books_page::hydrate_books,
        server_helpers::{authenticated_user, to_server_err},
    },
    crate::server::AuthSession,
    bb_core::{
        CoreServices,
        book::{Book, BookToken},
        filter::BookFilter as CoreBookFilter,
        reading::ReadStatus,
        shelf::{ShelfToken, ShelfType},
    },
    std::sync::Arc,
};

// ---------------------------------------------------------------------------
// Helpers (server only)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Server functions
// ---------------------------------------------------------------------------

/// Returns all shelves belonging to the authenticated user.
#[get(
    "/api/v1/shelves",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn list_my_shelves() -> Result<Vec<ShelfSummary>, ServerFnError> {
    let user_id = authenticated_user(&auth_session)?.id();

    let shelves = core_services.shelf_service.list_shelves_for_user(user_id).await.map_err(to_server_err)?;

    let mut result = Vec::with_capacity(shelves.len());
    for s in &shelves {
        let is_smart = s.shelf_type == ShelfType::Smart;
        let filter_json = if is_smart {
            s.filter_criteria.as_ref().and_then(|f| serde_json::to_string(f).ok())
        } else {
            None
        };
        let count = core_services
            .shelf_service
            .count_for_filter(s.token, user_id)
            .await
            .inspect_err(|e| tracing::warn!(shelf_token = %s.token, error = %e, "count_for_filter failed"))
            .ok()
            .filter(|&n| n > 0);
        result.push(ShelfSummary {
            id: s.id as i64,
            token: s.token.to_string(),
            name: s.name.clone(),
            is_own: true,
            is_smart,
            is_device_shelf: s.device_id.is_some(),
            filter_json,
            count,
            library_token: Some(bb_core::library::LibraryToken::new(s.library_id).to_string()),
        });
    }
    Ok(result)
}

/// Creates a new manual shelf and returns its token.
#[post(
    "/api/v1/shelves",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn create_shelf(name: String) -> Result<String, ServerFnError> {
    let user_id = authenticated_user(&auth_session)?.id();

    let token = core_services.shelf_service.create_manual_shelf(user_id, name).await.map_err(to_server_err)?;

    Ok(token.to_string())
}

/// Creates a new smart shelf from a serialized `BookFilter` JSON.
///
/// `filter_json` must be a valid JSON-encoded `BookFilter`.  Returns the new
/// shelf token.
#[post(
    "/api/v1/shelves/smart",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn create_smart_shelf(name: String, filter_json: String) -> Result<String, ServerFnError> {
    let user_id = authenticated_user(&auth_session)?.id();

    let filter: CoreBookFilter = serde_json::from_str(&filter_json).map_err(|e| ServerFnError::new(format!("Invalid filter: {e}")))?;

    let token = core_services
        .shelf_service
        .create_smart_shelf(user_id, name, filter)
        .await
        .map_err(to_server_err)?;

    Ok(token.to_string())
}

/// Updates the filter on an existing smart shelf. Only the owner may update.
#[put(
    "/api/v1/shelves/smart/filter",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn update_smart_shelf_filter(token: String, filter_json: String) -> Result<(), ServerFnError> {
    let user_id = authenticated_user(&auth_session)?.id();

    let shelf_token: ShelfToken = token.parse().map_err(|_| ServerFnError::new("Invalid shelf token"))?;
    let filter: CoreBookFilter = serde_json::from_str(&filter_json).map_err(|e| ServerFnError::new(format!("Invalid filter: {e}")))?;

    core_services
        .shelf_service
        .update_shelf_filter(shelf_token, filter, user_id)
        .await
        .map_err(to_server_err)
}

/// Deletes a shelf. Only the owner may delete.
#[delete(
    "/api/v1/shelves",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn delete_shelf(token: String) -> Result<(), ServerFnError> {
    let user_id = authenticated_user(&auth_session)?.id();

    let shelf_token: ShelfToken = token.parse().map_err(|_| ServerFnError::new("Invalid shelf token"))?;

    core_services.shelf_service.delete_shelf(shelf_token, user_id).await.map_err(to_server_err)
}

/// Adds a book to a shelf. Only the owner may add books.
#[post(
    "/api/v1/shelves/books",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn add_book_to_shelf(shelf_token: String, book_token: String) -> Result<(), ServerFnError> {
    let user_id = authenticated_user(&auth_session)?.id();

    let shelf: ShelfToken = shelf_token.parse().map_err(|_| ServerFnError::new("Invalid shelf token"))?;
    let book: BookToken = book_token.parse().map_err(|_| ServerFnError::new("Invalid book token"))?;

    core_services.shelf_service.add_book_to_shelf(shelf, book, user_id).await.map_err(to_server_err)
}

/// Adds a book to a manual shelf AND to the shelf's parent library.
/// Idempotent — if the book is already in either, no error.
#[post(
    "/api/v1/shelves/books/drop",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn add_book_to_shelf_with_library(shelf_token: String, book_token: String) -> Result<(), ServerFnError> {
    let user_id = authenticated_user(&auth_session)?.id();

    let shelf: ShelfToken = shelf_token.parse().map_err(|_| ServerFnError::new("Invalid shelf token"))?;
    let book: BookToken = book_token.parse().map_err(|_| ServerFnError::new("Invalid book token"))?;

    // Add to shelf (service validates ownership)
    core_services
        .shelf_service
        .add_book_to_shelf(shelf, book, user_id)
        .await
        .map_err(to_server_err)?;

    // Find the shelf to get its library_id, then add book to that library
    let shelf_info = core_services.shelf_service.get_shelf(shelf, user_id).await.map_err(to_server_err)?;
    let lib_token = bb_core::library::LibraryToken::new(shelf_info.library_id);
    core_services
        .library_service
        .add_book_to_library(lib_token, book)
        .await
        .map_err(to_server_err)?;

    Ok(())
}

/// Removes a book from a shelf. Only the owner may remove books.
#[delete(
    "/api/v1/shelves/books",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn remove_book_from_shelf(shelf_token: String, book_token: String) -> Result<(), ServerFnError> {
    let user_id = authenticated_user(&auth_session)?.id();

    let shelf: ShelfToken = shelf_token.parse().map_err(|_| ServerFnError::new("Invalid shelf token"))?;
    let book: BookToken = book_token.parse().map_err(|_| ServerFnError::new("Invalid book token"))?;

    core_services
        .shelf_service
        .remove_book_from_shelf(shelf, book, user_id)
        .await
        .map_err(to_server_err)
}

/// Updates the name of a shelf. Only the owner may update.
#[put(
    "/api/v1/shelves/update",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn update_shelf(token: String, name: String) -> Result<(), ServerFnError> {
    let user_id = authenticated_user(&auth_session)?.id();

    let shelf_token: ShelfToken = token.parse().map_err(|_| ServerFnError::new("Invalid shelf token"))?;

    core_services
        .shelf_service
        .update_shelf(shelf_token, name, user_id)
        .await
        .map_err(to_server_err)
}

/// Returns all entity options needed by the filter builder (authors, series,
/// genres, tags, publishers), each as `(id, name)` pairs for use as `EntityRef`
/// values.
#[get(
    "/api/v1/shelves/filter-entities",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn get_filter_entity_options() -> Result<FilterEntityOptions, ServerFnError> {
    let user_id = authenticated_user(&auth_session)?.id();
    let current_user = auth_session.current_user.clone().unwrap_or_default();
    let is_admin = !current_user.username.is_empty() && (current_user.permissions.contains("Admin") || current_user.permissions.contains("SuperAdmin"));

    let book_service = &core_services.book_service;

    let authors = book_service
        .list_all_authors()
        .await
        .map_err(to_server_err)?
        .into_iter()
        .map(|a| (a.id as i64, a.name))
        .collect();

    let series = book_service
        .list_all_series()
        .await
        .map_err(to_server_err)?
        .into_iter()
        .map(|s| (s.id as i64, s.name))
        .collect();

    let genres = book_service
        .list_all_genres()
        .await
        .map_err(to_server_err)?
        .into_iter()
        .map(|g| (g.id as i64, g.name))
        .collect();

    let tags = book_service
        .list_all_tags()
        .await
        .map_err(to_server_err)?
        .into_iter()
        .map(|t| (t.id as i64, t.name))
        .collect();

    let publishers = book_service
        .list_all_publishers()
        .await
        .map_err(to_server_err)?
        .into_iter()
        .map(|p| (p.id as i64, p.name))
        .collect();

    let shelves = core_services
        .shelf_service
        .list_shelves_for_user(user_id)
        .await
        .map_err(to_server_err)?
        .into_iter()
        .filter(|s| s.shelf_type == ShelfType::Manual)
        .map(|s| (s.id as i64, s.name))
        .collect();

    let libraries: Vec<(i64, String)> = if is_admin {
        core_services
            .library_service
            .list_libraries()
            .await
            .map_err(to_server_err)?
            .into_iter()
            .map(|e| (e.library.id as i64, e.library.name))
            .collect()
    } else {
        vec![]
    };

    Ok(FilterEntityOptions {
        authors,
        series,
        genres,
        tags,
        publishers,
        shelves,
        libraries,
    })
}

/// Returns a paginated list of books on a shelf, with author and series data
/// hydrated.
///
/// `offset` is the number of rows to skip. `page_size` defaults to 48 when
/// `None`. `sort` is applied server-side for smart shelves.
///
/// Smart shelves use the shelf's filter; manual shelves return all books
/// (client-side sorting handles manual shelves).
#[post(
    "/api/v1/shelves/books/list",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn books_for_shelf(
    token: String,
    offset: Option<u64>,
    page_size: Option<u64>,
    sort: Option<crate::components::SortOrder>,
) -> Result<ShelfBooksPage, ServerFnError> {
    const PAGE_SIZE: u64 = 48;

    let user_id = authenticated_user(&auth_session)?.id();

    let shelf_token: ShelfToken = token.parse().map_err(|_| ServerFnError::new("Invalid shelf token"))?;

    let shelf_service = &core_services.shelf_service;
    let book_service = &core_services.book_service;

    let effective_page_size = page_size.unwrap_or(PAGE_SIZE);

    // Load the shelf to determine if it's smart or manual.
    let shelf = shelf_service.get_shelf(shelf_token, user_id).await.map_err(to_server_err)?;

    // Fetch books: smart → filter query with server-side sort;
    // manual → all books (no pagination, client sorts).
    let (books, next_offset) = if shelf.shelf_type == ShelfType::Smart {
        let core_sort = sort.map(to_core_sort);
        let rows: Vec<Book> = shelf_service
            .books_for_filter(shelf_token, user_id, offset, Some(effective_page_size), core_sort)
            .await
            .map_err(to_server_err)?;

        let next = if rows.len() as u64 >= effective_page_size {
            Some(offset.unwrap_or(0) + rows.len() as u64)
        } else {
            None
        };
        (rows, next)
    } else {
        // Manual shelves: load all entries — they're small/curated.
        let shelf_entries = shelf_service.books_for_shelf(shelf_token, user_id, None, None).await.map_err(to_server_err)?;

        let mut books = Vec::with_capacity(shelf_entries.len());
        for entry in &shelf_entries {
            let book = book_service
                .find_book_by_token(BookToken::new(entry.book_id))
                .await
                .map_err(to_server_err)?
                .ok_or_else(|| ServerFnError::new("Book not found"))?;
            books.push(book);
        }
        (books, None)
    };

    if books.is_empty() {
        return Ok(ShelfBooksPage {
            books: vec![],
            next_offset: None,
        });
    }

    // Load per-user reading state for the displayed books only.
    // list_for_user has a pagination cap; list_for_user_and_books is ID-scoped with
    // no cap.
    let book_ids: Vec<bb_core::book::BookId> = books.iter().map(|b| b.id).collect();
    let reading_metas = core_services
        .reading_service
        .list_for_user_and_books(user_id, &book_ids)
        .await
        .map_err(to_server_err)?;
    let reading_map: std::collections::HashMap<u64, ReadingStateDto> = reading_metas
        .iter()
        .filter(|m| m.read_status != ReadStatus::Unread)
        .map(|m| (m.book_id, to_reading_state_dto(m)))
        .collect();

    let book_summaries = hydrate_books(&books, &core_services, Some(&reading_map)).await?;

    Ok(ShelfBooksPage {
        books: book_summaries,
        next_offset,
    })
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Shelf detail page — shows the `ShelfBar` and book grid, matching `BooksPage`
/// layout.
#[component]
pub(crate) fn ShelfPage(token: String) -> Element {
    let nav = use_navigator();

    use_context_provider(|| Signal::new(None::<String>)); // DraggedBookToken

    // Edit shelf modal state
    let mut show_edit = use_signal(|| false);
    let mut edit_name = use_signal(String::new);
    let mut edit_filter = use_signal(default_book_filter);
    let mut saving = use_signal(|| false);
    let mut edit_error: Signal<Option<String>> = use_signal(|| None);

    // Entity options for the smart shelf filter editor
    let entity_options_resource = use_resource(get_filter_entity_options);

    // Admin check for Library filter rule visibility
    let is_admin_resource = use_resource(crate::components::get_is_admin);

    // Delete shelf modal state
    let mut show_delete = use_signal(|| false);
    let mut deleting = use_signal(|| false);

    // Data loading
    let mut shelves_resource = use_server_future(list_my_shelves)?;

    // Sync `token` prop into a signal so `use_server_future` restarts reactively
    // when navigating between shelves (Dioxus re-renders in-place; plain captured
    // values won't trigger the future to re-run).
    let mut token_sig = use_signal(|| token.clone());
    if *token_sig.peek() != token {
        token_sig.set(token.clone());
    }
    let mut books_resource = use_server_future(move || {
        let tok = token_sig();
        let sort = SORT_ORDER();
        books_for_shelf(tok, None, None, Some(sort))
    })?;

    // Accumulated load-more books (first page comes from books_resource).
    let mut extra_books: Signal<Vec<BookSummary>> = use_signal(Vec::new);
    let mut next_offset: Signal<Option<u64>> = use_signal(|| None);
    let mut loading_more = use_signal(|| false);
    let mut load_more_error: Signal<Option<String>> = use_signal(|| None);

    // Sync offset from first page; reset accumulated state when token/data changes.
    use_effect(move || {
        let _ = token_sig(); // subscribe to token changes
        extra_books.set(vec![]);
        load_more_error.set(None);
        match books_resource() {
            Some(Ok(ref page)) => next_offset.set(page.next_offset),
            Some(Err(_)) | None => next_offset.set(None),
        }
    });

    // Derive current shelf info from the shelves list (avoids a separate get_shelf
    // call).
    let shelves: Vec<ShelfSummary> = shelves_resource().and_then(std::result::Result::ok).unwrap_or_default();
    let current_shelf = shelves.iter().find(|s| s.token == token).cloned();

    // Redirect if the shelf is not in the user's own shelves (not found or not
    // owner). Only redirect after the resource has finished loading (Some).
    if shelves_resource().is_some() && current_shelf.is_none() {
        nav.push(Route::BooksPage {});
    }

    let is_own = current_shelf.as_ref().is_some_and(|s| s.is_own);
    let current_name = current_shelf.as_ref().map(|s| s.name.clone()).unwrap_or_default();
    let current_is_smart = current_shelf.as_ref().is_some_and(|s| s.is_smart);
    let current_filter_json = current_shelf.as_ref().and_then(|s| s.filter_json.clone());

    let entity_options: FilterEntityOptions = entity_options_resource().and_then(std::result::Result::ok).unwrap_or_default();
    let is_admin = is_admin_resource().and_then(std::result::Result::ok).unwrap_or(false);

    // Smart shelves are read-only (no drag-drop, no remove button).
    let context = if is_own && !current_is_smart {
        BookGridContext::OwnShelf { shelf_token: token.clone() }
    } else {
        BookGridContext::ReadOnly {
            current_author_token: None,
            current_series_token: None,
        }
    };

    // Merged book list: first page + any load-more pages.
    // Manual shelves: client-side sort; smart shelves: already sorted server-side.
    let first_books = books_resource().and_then(Result::ok).map(|p| p.books).unwrap_or_default();
    let is_manual = current_shelf.as_ref().is_some_and(|s| !s.is_smart);
    let merged: Vec<BookSummary> = first_books.into_iter().chain(extra_books()).collect();
    let sorted = if is_manual { sort_books_client_side(merged, SORT_ORDER()) } else { merged };
    let query = crate::components::SEARCH_TEXT();
    let all_books = filter_books_by_search(sorted, &query);
    let has_search = !query.trim().is_empty();
    let has_more = next_offset().is_some();
    let book_tokens: Vec<String> = all_books.iter().map(|b| b.token.clone()).collect();

    rsx! {
        div { class: "flex-1 flex flex-col overflow-hidden",
            ShelfBar {
                shelves: shelves.clone(),
                current_shelf_token: Some(token.clone()),
                on_edit_shelf: {
                    let name_for_edit = current_name.clone();
                    let filter_json_for_edit = current_filter_json.clone();
                    let options_for_edit = entity_options.clone();
                    move |()| {
                        edit_name.set(name_for_edit.clone());
                        let mut parsed = filter_json_for_edit
                            .as_deref()
                            .and_then(|j| serde_json::from_str::<BookFilter>(j).ok())
                            .unwrap_or_else(default_book_filter);
                        freshen_entity_labels(&mut parsed, &options_for_edit);
                        edit_filter.set(parsed);
                        edit_error.set(None);
                        show_edit.set(true);
                    }
                },
                is_device_shelf: current_shelf.as_ref().is_some_and(|s| s.is_device_shelf),
                on_delete_shelf: move |()| show_delete.set(true),
            }

            match books_resource() {
                None => rsx! {
                    div { class: "flex-1 flex items-center justify-center text-gray-400 dark:text-slate-500 text-sm",
                        "Loading…"
                    }
                },
                Some(Err(e)) => rsx! {
                    div { class: "flex-1 flex items-center justify-center text-red-600 dark:text-red-400 text-sm",
                        "Failed to load books: {e}"
                    }
                },
                Some(Ok(_)) => {
                    if all_books.is_empty() {
                        rsx! {
                            div { class: "flex-1 flex flex-col items-center justify-center py-20 text-center",
                                p { class: "text-gray-400 dark:text-slate-500 text-sm",
                                    if has_search {
                                        "No books match your search."
                                    } else if current_is_smart {
                                        "No books match this filter."
                                    } else {
                                        "No books on this shelf yet."
                                    }
                                }
                                if !has_search && is_own && !current_is_smart {
                                    p { class: "text-gray-300 dark:text-slate-600 text-xs mt-1",
                                        "Drag a book here or open any book and use \"Add to Shelf\"."
                                    }
                                }
                            }
                        }
                    } else {
                        rsx! {
                            div { class: "flex-1 flex flex-col overflow-hidden",
                                BookGrid {
                                    books: all_books.clone(),
                                    context: context.clone(),
                                    on_action: move |()| books_resource.restart(),
                                }
                                if has_more {
                                    div { class: "flex flex-col items-center gap-2 py-4 shrink-0",
                                        if let Some(err) = load_more_error() {
                                            p { class: "text-red-600 dark:text-red-400 text-xs", {err} }
                                        }
                                        button {
                                            class: "px-6 py-2 text-sm font-medium rounded bg-indigo-600 text-white hover:bg-indigo-700 disabled:opacity-50",
                                            disabled: loading_more(),
                                            onclick: {
                                                let tok = token.clone();
                                                move |_| {
                                                    let tok = tok.clone();
                                                    let off = next_offset();
                                                    loading_more.set(true);
                                                    load_more_error.set(None);
                                                    spawn(async move {
                                                        match books_for_shelf(tok, off, None, Some(SORT_ORDER())).await {
                                                            Ok(page) => {
                                                                next_offset.set(page.next_offset);
                                                                extra_books.write().extend(page.books);
                                                                loading_more.set(false);
                                                            }
                                                            Err(e) => {
                                                                load_more_error.set(Some(e.to_string()));
                                                                loading_more.set(false);
                                                            }
                                                        }
                                                    });
                                                }
                                            },
                                            if loading_more() { "Loading…" } else { "Load more" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Edit shelf modal
        if show_edit() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/40",
                tabindex: -1,
                onkeydown: move |e| { if e.key() == Key::Escape { show_edit.set(false); } },
                div {
                    class: if current_is_smart {
                        "bg-white dark:bg-slate-800 rounded-lg shadow-xl p-6 w-full max-w-2xl mx-4 max-h-[85vh] overflow-y-auto"
                    } else {
                        "bg-white dark:bg-slate-800 rounded-lg shadow-xl p-6 w-full max-w-sm mx-4"
                    },
                    h2 { class: "text-lg font-semibold text-gray-900 dark:text-slate-100 mb-4", "Edit Shelf" }
                    form {
                        onsubmit: {
                            let tok = token.clone();
                            move |e: FormEvent| {
                                e.prevent_default();
                                let name = edit_name().trim().to_string();
                                if name.is_empty() {
                                    edit_error.set(Some("Shelf name is required.".into()));
                                    return;
                                }
                                let tok = tok.clone();
                                saving.set(true);
                                edit_error.set(None);
                                if current_is_smart {
                                    let filter_json = match serde_json::to_string(&edit_filter()) {
                                        Ok(j) => j,
                                        Err(err) => {
                                            saving.set(false);
                                            edit_error.set(Some(format!("Filter error: {err}")));
                                            return;
                                        }
                                    };
                                    spawn(async move {
                                        let r1 = update_shelf(tok.clone(), name).await;
                                        let r2 = update_smart_shelf_filter(tok, filter_json).await;
                                        match (r1, r2) {
                                            (Ok(()), Ok(())) => {
                                                show_edit.set(false);
                                                saving.set(false);
                                                shelves_resource.restart();
                                                books_resource.restart();
                                            }
                                            (Err(e), _) | (_, Err(e)) => {
                                                saving.set(false);
                                                edit_error.set(Some(e.to_string()));
                                            }
                                        }
                                    });
                                } else {
                                    spawn(async move {
                                        match update_shelf(tok, name).await {
                                            Ok(()) => {
                                                show_edit.set(false);
                                                saving.set(false);
                                                shelves_resource.restart();
                                            }
                                            Err(e) => {
                                                saving.set(false);
                                                edit_error.set(Some(e.to_string()));
                                            }
                                        }
                                    });
                                }
                            }
                        },

                        div { class: "mb-4",
                            label { class: "block text-sm font-medium text-gray-700 dark:text-slate-300 mb-1",
                                r#for: "edit-shelf-name",
                                "Shelf name"
                            }
                            input {
                                id: "edit-shelf-name",
                                class: "w-full px-3 py-2 border rounded text-sm outline-none focus:ring-1 bg-white dark:bg-slate-700 text-gray-900 dark:text-slate-100",
                                class: if edit_error().is_some() {
                                    "border-red-400 focus:border-red-500 focus:ring-red-500"
                                } else {
                                    "border-gray-300 dark:border-slate-600 focus:border-indigo-500 focus:ring-indigo-500"
                                },
                                r#type: "text",
                                autofocus: true,
                                value: edit_name(),
                                oninput: move |e| {
                                    edit_name.set(e.value());
                                    edit_error.set(None);
                                },
                                onkeydown: move |e: KeyboardEvent| {
                                    if e.key() == Key::Escape {
                                        show_edit.set(false);
                                    }
                                },
                            }
                            if let Some(msg) = edit_error() {
                                p { class: "mt-1 text-xs text-red-600 dark:text-red-400", {msg} }
                            }
                        }

                        if current_is_smart {
                            div { class: "mb-6",
                                p { class: "text-sm font-medium text-gray-700 dark:text-slate-300 mb-2", "Filter rules" }
                                FilterBuilder {
                                    filter: edit_filter,
                                    entity_options: entity_options.clone(),
                                    current_shelf_id: current_shelf.as_ref().map(|s| s.id),
                                    is_admin,
                                }
                            }
                        }

                        div { class: "flex gap-3 justify-end",
                            button {
                                r#type: "button",
                                class: "px-4 py-2 text-sm font-medium rounded border border-gray-300 dark:border-slate-600 text-gray-700 dark:text-slate-300 hover:bg-gray-50 dark:hover:bg-slate-700",
                                onclick: move |_| show_edit.set(false),
                                "Cancel"
                            }
                            button {
                                r#type: "submit",
                                class: "px-4 py-2 text-sm font-medium rounded bg-indigo-600 text-white hover:bg-indigo-700 disabled:opacity-50",
                                disabled: saving(),
                                if saving() { "Saving…" } else { "Save" }
                            }
                        }
                    }
                }
            }
        }

        SelectionActionBar {
            all_book_tokens: book_tokens,
            on_action: move |()| {
                books_resource.restart();
                shelves_resource.restart();
            },
        }

        // Delete shelf modal
        if show_delete() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/40",
                tabindex: -1,
                onkeydown: move |e| { if e.key() == Key::Escape { show_delete.set(false); } },
                div { class: "bg-white dark:bg-slate-800 rounded-lg shadow-xl p-6 w-full max-w-sm mx-4",
                    h2 { class: "text-lg font-semibold text-gray-900 dark:text-slate-100 mb-2", "Delete Shelf?" }
                    p { class: "text-sm text-gray-600 dark:text-slate-400 mb-6",
                        "This will permanently delete \"{current_name}\". Books will not be affected."
                    }
                    div { class: "flex gap-3 justify-end",
                        button {
                            class: "px-4 py-2 text-sm font-medium rounded border border-gray-300 dark:border-slate-600 text-gray-700 dark:text-slate-300 hover:bg-gray-50 dark:hover:bg-slate-700",
                            autofocus: true,
                            onclick: move |_| show_delete.set(false),
                            "Cancel"
                        }
                        button {
                            class: "px-4 py-2 text-sm font-medium rounded bg-red-600 text-white hover:bg-red-700 disabled:opacity-50",
                            disabled: deleting(),
                            onclick: {
                                let tok = token.clone();
                                move |_| {
                                    let tok = tok.clone();
                                    deleting.set(true);
                                    show_delete.set(false);
                                    spawn(async move {
                                        if let Ok(()) = delete_shelf(tok).await {
                                            nav.push(Route::BooksPage {});
                                        }
                                        deleting.set(false);
                                    });
                                }
                            },
                            if deleting() { "Deleting…" } else { "Yes, Delete" }
                        }
                    }
                }
            }
        }
    }
}
