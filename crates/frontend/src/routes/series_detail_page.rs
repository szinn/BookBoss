use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    Route,
    components::{BookGrid, BookGridContext, SelectionActionBar, SelectionToggle, filter_books_by_search},
    routes::books_page::BookSummary,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct SeriesPageData {
    pub token: String,
    pub name: String,
    pub description: Option<String>,
    pub books: Vec<BookSummary>,
}

#[cfg(feature = "server")]
use {
    crate::routes::{book_detail_page::to_reading_state_dto, books_page::hydrate_books, server_helpers::authenticated_user},
    crate::server::AuthSession,
    bb_core::CoreServices,
    bb_core::book::{BookQuery, SeriesToken},
    bb_core::reading::ReadStatus,
    std::str::FromStr,
    std::sync::Arc,
};

#[post("/api/v1/series", auth_session: axum::Extension<AuthSession>, core_services: axum::Extension<Arc<CoreServices>>)]
async fn get_series(token: String) -> Result<SeriesPageData, ServerFnError> {
    use std::collections::HashMap;
    let current_user = authenticated_user(&auth_session)?;

    let book_service = &core_services.book_service;

    let series_token = SeriesToken::from_str(&token).map_err(|_| ServerFnError::new("Invalid series token"))?;

    let series = book_service
        .find_series_by_token(series_token)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
        .ok_or_else(|| ServerFnError::new("Series not found"))?;

    let filter = BookQuery {
        series_id: Some(series.id),
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

    let book_ids: Vec<bb_core::book::BookId> = books.iter().map(|b| b.id).collect();
    let reading_metas = core_services
        .reading_service
        .list_for_user_and_books(current_user.id(), &book_ids)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let reading_map: HashMap<u64, _> = reading_metas
        .iter()
        .filter(|m| m.read_status != ReadStatus::Unread)
        .map(|m| (m.book_id, to_reading_state_dto(m)))
        .collect();

    let book_summaries = hydrate_books(&books, &core_services, Some(&reading_map)).await?;

    Ok(SeriesPageData {
        token: series.token.to_string(),
        name: series.name,
        description: series.description,
        books: book_summaries,
    })
}

#[component]
pub(crate) fn SeriesDetailPage(token: String) -> Element {
    use_context_provider(|| Signal::new(None::<String>)); // DraggedBookToken
    let mut series_resource = use_server_future(move || get_series(token.clone()))?;

    rsx! {
        div { class: "flex-1 overflow-auto p-6",
            match series_resource() {
                None => rsx! {
                    div { class: "flex items-center justify-center h-full text-gray-400 text-sm",
                        "Loading…"
                    }
                },
                Some(Err(e)) => rsx! {
                    div { class: "text-red-600 text-sm", "Failed to load series: {e}" }
                },
                Some(Ok(series)) => {
                    let query = crate::components::SEARCH_TEXT();
                    let filtered_books = filter_books_by_search(series.books, &query);
                    let has_search = !query.trim().is_empty();
                    let book_tokens: Vec<String> = filtered_books.iter().map(|b| b.token.clone()).collect();
                    rsx! {
                        Link {
                            to: Route::SeriesPage {},
                            class: "inline-flex items-center gap-1 text-sm text-indigo-600 hover:text-indigo-800 mb-6",
                            "← Series"
                        }

                        h1 { class: "text-2xl font-bold text-gray-900 mb-2", "{series.name}" }

                        if let Some(ref desc) = series.description {
                            p { class: "text-sm text-gray-600 leading-relaxed mb-6 max-w-prose", "{desc}" }
                        }

                        if filtered_books.is_empty() && has_search {
                            p { class: "text-gray-400 text-sm mt-4", "No books match your search." }
                        } else if !filtered_books.is_empty() {
                            div { class: "flex items-center gap-2 mb-4",
                                h2 { class: "text-xs font-semibold uppercase tracking-wider text-gray-500",
                                    "Books"
                                }
                                SelectionToggle {}
                            }
                            BookGrid {
                                books: filtered_books,
                                context: BookGridContext::ReadOnly {
                                    current_author_token: None,
                                    current_series_token: Some(series.token.clone()),
                                },
                                on_action: move |()| series_resource.restart(),
                            }
                        }
                        SelectionActionBar {
                            all_book_tokens: book_tokens,
                            on_action: move |()| series_resource.restart(),
                        }
                    }
                },
            }
        }
    }
}
