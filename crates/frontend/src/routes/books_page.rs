use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    components::{BookGrid, BookTable, DetailPanel, TreeExplorer},
    settings::BookDisplayView,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct BookSummary {
    pub token: String,
    pub title: String,
    pub cover_path: Option<String>,
    pub author_names: Vec<String>,
    pub series_name: Option<String>,
    pub series_number: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TreeCategory {
    pub name: String,
    pub items: Vec<String>,
}

fn sample_categories() -> Vec<TreeCategory> {
    vec![
        TreeCategory {
            name: "Genres".into(),
            items: vec!["Fantasy".into(), "Science Fiction".into(), "Non-fiction".into()],
        },
        TreeCategory {
            name: "Authors".into(),
            items: vec![],
        },
        TreeCategory {
            name: "Series".into(),
            items: vec![],
        },
    ]
}

#[cfg(feature = "server")]
use {
    bb_core::CoreServices,
    bb_core::book::{AuthorToken, BookFilter, BookStatus, SeriesToken},
    std::sync::Arc,
};

#[get("/api/v1/books", core_services: axum::Extension<Arc<CoreServices>>)]
#[tracing::instrument(level = "trace", skip(core_services))]
async fn list_books() -> Result<Vec<BookSummary>, ServerFnError> {
    use std::collections::{HashMap, HashSet};

    let book_service = &core_services.book_service;

    let filter = BookFilter {
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
    let mut author_map: HashMap<u64, String> = HashMap::new();
    for author_id in all_author_ids {
        if let Some(author) = book_service
            .find_author_by_token(&AuthorToken::new(author_id))
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
        {
            author_map.insert(author_id, author.name);
        }
    }

    // Fetch each unique series once
    let unique_series: HashSet<u64> = books.iter().filter_map(|b| b.series_id).collect();
    let mut series_map: HashMap<u64, String> = HashMap::new();
    for series_id in unique_series {
        if let Some(series) = book_service
            .find_series_by_token(&SeriesToken::new(series_id))
            .await
            .map_err(|e| ServerFnError::new(e.to_string()))?
        {
            series_map.insert(series_id, series.name);
        }
    }

    // Assemble view models
    let summaries = books
        .iter()
        .zip(book_authors.iter())
        .map(|(book, author_pairs)| {
            let mut sorted = author_pairs.clone();
            sorted.sort_by_key(|&(order, _)| order);
            let author_names = sorted.iter().filter_map(|&(_, aid)| author_map.get(&aid).cloned()).collect();
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

    Ok(summaries)
}

#[component]
pub(crate) fn BooksPage() -> Element {
    let selected_book: Signal<Option<BookSummary>> = use_signal(|| None);
    use_context_provider(|| selected_book);

    let view: Signal<BookDisplayView> = use_context();
    let books = use_server_future(list_books)?;
    let categories = sample_categories();

    rsx! {
        match books() {
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
            Some(Ok(books)) => rsx! {
                match *view.read() {
                    BookDisplayView::GridView => rsx! {
                        BookGrid { books }
                    },
                    BookDisplayView::TableView => rsx! {
                        TreeExplorer { categories }
                        BookTable { books }
                        DetailPanel {}
                    },
                }
            },
        }
    }
}
