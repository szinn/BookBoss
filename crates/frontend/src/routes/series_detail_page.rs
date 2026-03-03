use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{Route, components::BookGrid, routes::books_page::BookSummary};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct SeriesPageData {
    pub token: String,
    pub name: String,
    pub description: Option<String>,
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

#[post("/api/v1/series", auth_session: axum::Extension<AuthSession>, core_services: axum::Extension<Arc<CoreServices>>)]
#[tracing::instrument(level = "trace", skip(auth_session, core_services))]
async fn get_series(token: String) -> Result<SeriesPageData, ServerFnError> {
    auth_session
        .current_user
        .as_ref()
        .filter(|u| !u.username.is_empty())
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;

    let book_service = &core_services.book_service;

    let series_token = SeriesToken::from_str(&token).map_err(|_| ServerFnError::new("Invalid series token"))?;

    let series = book_service
        .find_series_by_token(&series_token)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("Series not found"))?;

    let filter = BookFilter {
        series_id: Some(series.id),
        status: Some(BookStatus::Available),
        ..Default::default()
    };
    let mut books = book_service
        .list_books(&filter, None, None)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    // Sort books by series_number ascending (None sorts last)
    books.sort_by(|a, b| match (&a.series_number, &b.series_number) {
        (Some(a), Some(b)) => a.cmp(b),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });

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

    // Assemble BookSummary list (series_name/number already known from series)
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
                series_name: Some(series.name.clone()),
                series_number: book.series_number.as_ref().map(|n| n.to_string()),
            }
        })
        .collect();

    Ok(SeriesPageData {
        token: series.token.to_string(),
        name: series.name,
        description: series.description,
        books: book_summaries,
    })
}

#[component]
pub(crate) fn SeriesDetailPage(token: String) -> Element {
    let series = use_server_future(move || get_series(token.clone()))?;

    rsx! {
        div { class: "flex-1 overflow-auto p-6",
            match series() {
                None => rsx! {
                    div { class: "flex items-center justify-center h-full text-gray-400 text-sm",
                        "Loading…"
                    }
                },
                Some(Err(e)) => rsx! {
                    div { class: "text-red-600 text-sm", "Failed to load series: {e}" }
                },
                Some(Ok(series)) => rsx! {
                    Link {
                        to: Route::BooksPage {},
                        class: "inline-flex items-center gap-1 text-sm text-indigo-600 hover:text-indigo-800 mb-6",
                        "← Library"
                    }

                    h1 { class: "text-2xl font-bold text-gray-900 mb-2", "{series.name}" }

                    if let Some(ref desc) = series.description {
                        p { class: "text-sm text-gray-600 leading-relaxed mb-6 max-w-prose", "{desc}" }
                    }

                    if !series.books.is_empty() {
                        h2 { class: "text-xs font-semibold uppercase tracking-wider text-gray-500 mb-4",
                            "Books"
                        }
                        BookGrid { books: series.books }
                    }
                },
            }
        }
    }
}
