use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{Route, components::BookGrid, routes::books_page::BookSummary};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct AuthorPageData {
    pub token: String,
    pub name: String,
    pub bio: Option<String>,
    pub books: Vec<BookSummary>,
}

#[cfg(feature = "server")]
use {
    crate::server::AuthSession,
    bb_core::CoreServices,
    bb_core::book::{AuthorToken, BookFilter, BookStatus, SeriesToken},
    std::collections::{HashMap, HashSet},
    std::str::FromStr,
    std::sync::Arc,
};

#[post("/api/v1/author", auth_session: axum::Extension<AuthSession>, core_services: axum::Extension<Arc<CoreServices>>)]
async fn get_author(token: String) -> Result<AuthorPageData, ServerFnError> {
    auth_session
        .current_user
        .as_ref()
        .filter(|u| !u.username.is_empty())
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;

    let book_service = &core_services.book_service;

    let author_token = AuthorToken::from_str(&token).map_err(|_| ServerFnError::new("Invalid author token"))?;

    let author = book_service
        .find_author_by_token(&author_token)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("Author not found"))?;

    let filter = BookFilter {
        author_id: Some(author.id),
        status: Some(BookStatus::Available),
        ..Default::default()
    };
    let books = book_service
        .list_books(&filter, None, None)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Gather per-book author links and collect unique author IDs
    let mut book_authors: Vec<Vec<(i32, u64)>> = Vec::with_capacity(books.len());
    let mut all_author_ids: HashSet<u64> = HashSet::new();
    for book in &books {
        let authors = book_service.authors_for_book(book.id).await.map_err(|e| ServerFnError::new(e.to_string()))?;
        let pairs: Vec<(i32, u64)> = authors.iter().map(|ba| (ba.sort_order, ba.author_id)).collect();
        for &(_, aid) in &pairs {
            all_author_ids.insert(aid);
        }
        book_authors.push(pairs);
    }

    // Fetch each unique author once
    let mut author_name_map: HashMap<u64, String> = HashMap::new();
    for author_id in all_author_ids {
        if let Some(a) = book_service
            .find_author_by_token(&AuthorToken::new(author_id))
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
        {
            author_name_map.insert(author_id, a.name);
        }
    }

    // Fetch each unique series once
    let unique_series: HashSet<u64> = books.iter().filter_map(|b| b.series_id).collect();
    let mut series_map: HashMap<u64, String> = HashMap::new();
    for series_id in unique_series {
        if let Some(s) = book_service
            .find_series_by_token(&SeriesToken::new(series_id))
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
        {
            series_map.insert(series_id, s.name);
        }
    }

    // Assemble BookSummary list
    let book_summaries = books
        .iter()
        .zip(book_authors.iter())
        .map(|(book, author_pairs)| {
            let mut sorted = author_pairs.clone();
            sorted.sort_by_key(|&(order, _)| order);
            let author_names = sorted.iter().filter_map(|&(_, aid)| author_name_map.get(&aid).cloned()).collect();
            BookSummary {
                token: book.token.to_string(),
                title: book.title.clone(),
                cover_path: book.cover_path.clone(),
                author_names,
                series_name: book.series_id.and_then(|sid| series_map.get(&sid).cloned()),
                series_number: book.series_number.as_ref().map(|n| n.to_string()),
            }
        })
        .collect();

    Ok(AuthorPageData {
        token: author.token.to_string(),
        name: author.name,
        bio: author.bio,
        books: book_summaries,
    })
}

#[component]
pub(crate) fn AuthorDetailPage(token: String) -> Element {
    let author = use_server_future(move || get_author(token.clone()))?;

    rsx! {
        div { class: "flex-1 overflow-auto p-6",
            match author() {
                None => rsx! {
                    div { class: "flex items-center justify-center h-full text-gray-400 text-sm",
                        "Loading…"
                    }
                },
                Some(Err(e)) => rsx! {
                    div { class: "text-red-600 text-sm", "Failed to load author: {e}" }
                },
                Some(Ok(author)) => rsx! {
                    Link {
                        to: Route::BooksPage {},
                        class: "inline-flex items-center gap-1 text-sm text-indigo-600 hover:text-indigo-800 mb-6",
                        "← Library"
                    }

                    h1 { class: "text-2xl font-bold text-gray-900 mb-2", "{author.name}" }

                    if let Some(ref bio) = author.bio {
                        p { class: "text-sm text-gray-600 leading-relaxed mb-6 max-w-prose", "{bio}" }
                    }

                    if !author.books.is_empty() {
                        h2 { class: "text-xs font-semibold uppercase tracking-wider text-gray-500 mb-4",
                            "Books"
                        }
                        BookGrid { books: author.books }
                    }
                },
            }
        }
    }
}
