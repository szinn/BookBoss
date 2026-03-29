use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    Route,
    components::{BookGrid, BookGridContext, SelectionActionBar, ShelfBar, filter_books_by_search},
    routes::{
        book_detail_page::ReadingStateDto,
        shelf_page::{ShelfSummary, list_all_accessible_shelves},
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
    pub cover_path: Option<String>,
    pub authors: Vec<AuthorLink>,
    pub series_token: Option<String>,
    pub series_name: Option<String>,
    pub series_number: Option<String>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    pub reading_state: Option<ReadingStateDto>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ListBooksResponse {
    pub books: Vec<BookSummary>,
    pub can_delete_books: bool,
    /// Books with `Reading` status, sorted by `last_progress_at` desc.
    pub currently_reading: Vec<BookSummary>,
}

#[cfg(feature = "server")]
use {
    crate::components::to_core_sort,
    crate::routes::{book_detail_page::to_reading_state_dto, server_helpers::authenticated_user},
    crate::server::{AuthSession, AuthUser, BackendSessionPool},
    axum::http::Method,
    axum_session_auth::{Auth, Rights},
    bb_core::{
        CoreServices,
        book::{AuthorToken, Book, BookQuery, SeriesToken},
        reading::ReadStatus,
        types::Capability,
    },
    std::sync::Arc,
};

/// Hydrates a slice of `Book`s into `BookSummary` view-models.
///
/// For each book this fetches the linked authors and series names in two
/// batched passes (one per unique ID), then assembles `BookSummary`.
/// `reading_map` is an optional pre-built map of `book_id → ReadingStateDto`
/// used to attach per-user reading state; pass `None` for read-only contexts.
#[cfg(feature = "server")]
pub(crate) async fn hydrate_books(
    books: &[Book],
    core_services: &CoreServices,
    reading_map: Option<&std::collections::HashMap<u64, ReadingStateDto>>,
) -> Result<Vec<BookSummary>, ServerFnError> {
    use std::collections::{HashMap, HashSet};

    let book_service = &core_services.book_service;

    // Gather per-book author links and collect unique author IDs.
    let mut book_author_pairs: Vec<Vec<(i32, u64)>> = Vec::with_capacity(books.len());
    let mut all_author_ids: HashSet<u64> = HashSet::new();
    for book in books {
        let authors = book_service.authors_for_book(book.id).await.map_err(|e| ServerFnError::new(e.to_string()))?;
        let pairs: Vec<(i32, u64)> = authors.iter().map(|ba| (ba.sort_order, ba.author_id)).collect();
        for &(_, aid) in &pairs {
            all_author_ids.insert(aid);
        }
        book_author_pairs.push(pairs);
    }

    // Batch-load unique authors (token + name).
    let mut author_map: HashMap<u64, (String, String)> = HashMap::new();
    for author_id in all_author_ids {
        if let Some(a) = book_service
            .find_author_by_token(AuthorToken::new(author_id))
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
        {
            author_map.insert(author_id, (a.token.to_string(), a.name));
        }
    }

    // Batch-load unique series (token + name).
    let unique_series: HashSet<u64> = books.iter().filter_map(|b| b.series_id).collect();
    let mut series_map: HashMap<u64, (String, String)> = HashMap::new();
    for series_id in unique_series {
        if let Some(s) = book_service
            .find_series_by_token(SeriesToken::new(series_id))
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
        {
            series_map.insert(series_id, (s.token.to_string(), s.name));
        }
    }

    // Load genres and tags per book.
    let mut book_genres: Vec<Vec<String>> = Vec::with_capacity(books.len());
    let mut book_tags: Vec<Vec<String>> = Vec::with_capacity(books.len());
    for book in books {
        let genres = book_service.genres_for_book(book.id).await.map_err(|e| ServerFnError::new(e.to_string()))?;
        book_genres.push(genres.into_iter().map(|g| g.name).collect());

        let tags = book_service.tags_for_book(book.id).await.map_err(|e| ServerFnError::new(e.to_string()))?;
        book_tags.push(tags.into_iter().map(|t| t.name).collect());
    }

    let summaries = books
        .iter()
        .zip(book_author_pairs.iter())
        .enumerate()
        .map(|(idx, (book, author_pairs))| {
            let mut sorted = author_pairs.clone();
            sorted.sort_by_key(|&(order, _)| order);
            let authors = sorted
                .iter()
                .filter_map(|&(_, aid)| {
                    author_map.get(&aid).map(|(token, name)| AuthorLink {
                        token: token.clone(),
                        name: name.clone(),
                    })
                })
                .collect();
            let (series_token, series_name) = book
                .series_id
                .and_then(|sid| series_map.get(&sid))
                .map_or((None, None), |(tok, name)| (Some(tok.clone()), Some(name.clone())));
            BookSummary {
                token: book.token.to_string(),
                title: book.title.clone(),
                cover_path: book.cover_path.clone(),
                authors,
                series_token,
                series_name,
                series_number: book.series_number.as_ref().map(std::string::ToString::to_string),
                genres: book_genres[idx].clone(),
                tags: book_tags[idx].clone(),
                reading_state: reading_map.and_then(|m| m.get(&book.id).cloned()),
                created_at: book.created_at.to_rfc3339(),
            }
        })
        .collect();

    Ok(summaries)
}

#[post("/api/v1/books", auth_session: axum::Extension<AuthSession>, core_services: axum::Extension<Arc<CoreServices>>)]
async fn list_books(sort: crate::components::SortOrder) -> Result<ListBooksResponse, ServerFnError> {
    use std::collections::HashMap;

    let current_user = authenticated_user(&auth_session)?;
    let user_id = current_user.id();

    let can_delete_books = Auth::<AuthUser, _, BackendSessionPool>::build([Method::POST], true)
        .requires(Rights::any([Rights::permission(Capability::DeleteBook.as_str())]))
        .validate(&current_user, &Method::POST, None)
        .await;

    let filter = BookQuery {
        sort: Some(to_core_sort(sort)),
        ..Default::default()
    };
    let books = core_services
        .book_service
        .list_books(&filter, None, None)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Load per-user reading state scoped to the fetched books (avoids page limits).
    let book_ids: Vec<bb_core::book::BookId> = books.iter().map(|b| b.id).collect();
    let reading_metas = core_services
        .reading_service
        .list_for_user_and_books(user_id, &book_ids)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let reading_map: HashMap<u64, ReadingStateDto> = reading_metas
        .iter()
        .filter(|m| m.read_status != ReadStatus::Unread)
        .map(|m| (m.book_id, to_reading_state_dto(m)))
        .collect();

    let summaries = hydrate_books(&books, &core_services, Some(&reading_map)).await?;

    // Build the "Currently Reading" list: Reading books sorted by last_progress_at
    // desc.
    let book_id_to_idx: HashMap<u64, usize> = books.iter().enumerate().map(|(i, b)| (b.id, i)).collect();
    let mut reading_now: Vec<_> = reading_metas
        .iter()
        .filter(|m| matches!(m.read_status, ReadStatus::Reading | ReadStatus::Rereading | ReadStatus::Paused))
        .collect();
    reading_now.sort_by_key(|b| std::cmp::Reverse(b.last_progress_at));
    let currently_reading: Vec<BookSummary> = reading_now
        .iter()
        .filter_map(|m| book_id_to_idx.get(&m.book_id).map(|&idx| summaries[idx].clone()))
        .collect();

    Ok(ListBooksResponse {
        books: summaries,
        can_delete_books,
        currently_reading,
    })
}

#[component]
pub(crate) fn BooksPage() -> Element {
    use_context_provider(|| Signal::new(None::<String>)); // DraggedBookToken
    let mut books_refresh = use_signal(|| 0u32);
    let mut page_data = use_server_future(move || {
        let sort = crate::components::SORT_ORDER();
        let _ = books_refresh();
        list_books(sort)
    })?;
    let mut shelves_resource = use_server_future(list_all_accessible_shelves)?;
    let shelves: Vec<ShelfSummary> = shelves_resource().and_then(std::result::Result::ok).unwrap_or_default();
    rsx! {
        match page_data() {
            None => rsx! {
                div { class: "flex-1 flex items-center justify-center text-gray-400 text-sm",
                    "Loading…"
                }
            },
            Some(Err(e)) => rsx! {
                div { class: "flex-1 flex items-center justify-center text-red-600 text-sm",
                    "Failed to load books: {e}"
                }
            },
            Some(Ok(ListBooksResponse { books, can_delete_books, currently_reading })) => {
                let query = crate::components::SEARCH_TEXT();
                let filtered_books = filter_books_by_search(books, &query);
                let filtered_reading = filter_books_by_search(currently_reading, &query);
                let has_search = !query.trim().is_empty();
                let book_tokens: Vec<String> = filtered_books.iter().map(|b| b.token.clone()).collect();
                rsx! {
                    div { class: "flex-1 flex flex-col overflow-hidden",
                        ShelfBar {
                            shelves,
                            current_shelf_token: None,
                            on_edit_shelf: |()| {},
                            on_delete_shelf: |()| {},
                        }
                        CurrentlyReadingSection { books: filtered_reading }
                        if filtered_books.is_empty() && has_search {
                            div { class: "flex-1 flex items-center justify-center text-gray-400 text-sm",
                                "No books match your search."
                            }
                        } else {
                            BookGrid {
                                books: filtered_books,
                                context: BookGridContext::AllBooks { can_delete: can_delete_books },
                                on_action: move |()| page_data.restart(),
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
        div { class: "shrink-0 border-b border-gray-100 bg-gray-50 px-4 pt-3 pb-2",
            h2 { class: "text-xs font-semibold uppercase tracking-wider text-gray-500 mb-2",
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
                                    src: "/api/v1/covers/{book.token}",
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
