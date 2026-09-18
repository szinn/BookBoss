use dioxus::prelude::*;
#[cfg(feature = "server")]
use {
    crate::routes::server_helpers::{authenticated_user, to_server_err},
    crate::server::AuthSession,
    bb_core::{
        CoreServices,
        book::{AuthorToken, BookQuery},
    },
    std::sync::Arc,
};

use crate::{
    Route,
    components::{SeriesTile, SeriesTileData},
};

#[get("/api/v1/series/list", auth_session: axum::Extension<AuthSession>, core_services: axum::Extension<Arc<CoreServices>>)]
async fn get_all_series() -> Result<Vec<SeriesTileData>, ServerFnError> {
    authenticated_user(&auth_session)?;

    let book_service = &core_services.book_service;

    let all_series = book_service.list_all_series().await.map_err(to_server_err)?;

    let mut tiles = Vec::with_capacity(all_series.len());
    for series in &all_series {
        // Load only Available books for this series (list_books filters by
        // status).
        let filter = BookQuery {
            series_id: Some(series.id),
            ..Default::default()
        };
        let mut books = book_service.list_books(&filter, None, None, None).await.map_err(to_server_err)?;

        // Skip series with no available books (e.g. all still incoming).
        if books.is_empty() {
            continue;
        }

        let book_count = books.len() as u64;

        books.sort_by(|a, b| match (&a.series_number, &b.series_number) {
            (Some(a), Some(b)) => a.cmp(b),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });
        books.truncate(3);

        let cover_book_tokens: Vec<String> = books.iter().map(|b| b.token.to_string()).collect();

        // Get first author from first book
        let (first_author_token, first_author_name) = if let Some(first_book) = books.first() {
            let book_authors = book_service.authors_for_book(first_book.id).await.map_err(to_server_err)?;
            if let Some(ba) = book_authors.iter().min_by_key(|a| a.sort_order) {
                let author = book_service.find_author_by_token(AuthorToken::new(ba.author_id)).await.map_err(to_server_err)?;
                match author {
                    Some(a) => (Some(a.token.to_string()), Some(a.name)),
                    None => (None, None),
                }
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        tiles.push(SeriesTileData {
            token: series.token.to_string(),
            name: series.name.clone(),
            book_count,
            first_author_token,
            first_author_name,
            cover_book_tokens,
        });
    }

    Ok(tiles)
}

#[component]
pub(crate) fn SeriesPage() -> Element {
    let series = use_server_future(get_all_series)?;

    rsx! {
        div { class: "flex-1 overflow-auto p-6",
            match series() {
                None => rsx! {
                    div { class: "flex items-center justify-center h-full text-gray-400 dark:text-slate-500 text-sm",
                        "Loading…"
                    }
                },
                Some(Err(e)) => rsx! {
                    div { class: "text-red-600 text-sm", "Failed to load series: {e}" }
                },
                Some(Ok(all_series)) => {
                    let query = crate::components::SEARCH_TEXT();
                    let query_lower = query.trim().to_lowercase();
                    let filtered: Vec<&SeriesTileData> = if query_lower.is_empty() {
                        all_series.iter().collect()
                    } else {
                        all_series.iter().filter(|s| {
                            s.name.to_lowercase().contains(&query_lower)
                                || s.first_author_name.as_ref().is_some_and(|n| n.to_lowercase().contains(&query_lower))
                        }).collect()
                    };
                    let has_search = !query_lower.is_empty();

                    rsx! {
                        Link {
                            to: Route::BooksPage {},
                            class: "inline-flex items-center gap-1 text-sm text-indigo-600 dark:text-indigo-400 hover:text-indigo-800 dark:hover:text-indigo-300 mb-6",
                            "← Library"
                        }

                        h1 { class: "text-2xl font-bold text-gray-900 dark:text-slate-100 mb-6", "Series" }

                        if filtered.is_empty() && has_search {
                            p { class: "text-gray-400 dark:text-slate-500 text-sm mt-4", "No series match your search." }
                        } else if filtered.is_empty() {
                            p { class: "text-gray-400 dark:text-slate-500 text-sm mt-4", "No series in your library." }
                        } else {
                            div {
                                class: "grid gap-8",
                                style: "grid-template-columns: repeat(auto-fill, minmax(160px, 1fr))",
                                for tile_data in &filtered {
                                    SeriesTile { key: "{tile_data.token}", data: (*tile_data).clone() }
                                }
                            }
                        }
                    }
                },
            }
        }
    }
}
