use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::Route;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct AuthorRow {
    pub token: String,
    pub name: String,
    pub book_count: u64,
}

#[cfg(feature = "server")]
use {
    crate::routes::server_helpers::{authenticated_user, to_server_err},
    crate::server::AuthSession,
    bb_core::{CoreServices, book::BookQuery},
    std::sync::Arc,
};

#[get("/api/v1/authors/list", auth_session: axum::Extension<AuthSession>, core_services: axum::Extension<Arc<CoreServices>>)]
async fn get_authors() -> Result<Vec<AuthorRow>, ServerFnError> {
    authenticated_user(&auth_session)?;

    let book_service = &core_services.book_service;

    let authors = book_service.list_all_authors().await.map_err(to_server_err)?;

    let mut rows = Vec::with_capacity(authors.len());
    for author in &authors {
        // list_books filters by Available status, so this excludes incoming
        // books.
        let books = book_service
            .list_books(
                &BookQuery {
                    author_id: Some(author.id),
                    ..Default::default()
                },
                None,
                None,
                None,
            )
            .await
            .map_err(to_server_err)?;

        // Skip authors with no available books (e.g. all still incoming).
        if books.is_empty() {
            continue;
        }

        rows.push(AuthorRow {
            token: author.token.to_string(),
            name: author.name.clone(),
            book_count: books.len() as u64,
        });
    }

    Ok(rows)
}

#[component]
pub(crate) fn AuthorsPage() -> Element {
    let authors = use_server_future(get_authors)?;

    rsx! {
        div { class: "flex-1 overflow-auto p-6",
            match authors() {
                None => rsx! {
                    div { class: "flex items-center justify-center h-full text-gray-400 dark:text-slate-500 text-sm",
                        "Loading…"
                    }
                },
                Some(Err(e)) => rsx! {
                    div { class: "text-red-600 text-sm", "Failed to load authors: {e}" }
                },
                Some(Ok(authors)) => {
                    let query = crate::components::SEARCH_TEXT();
                    let query_lower = query.trim().to_lowercase();
                    let filtered: Vec<&AuthorRow> = if query_lower.is_empty() {
                        authors.iter().collect()
                    } else {
                        authors.iter().filter(|a| a.name.to_lowercase().contains(&query_lower)).collect()
                    };
                    let has_search = !query_lower.is_empty();

                    rsx! {
                        Link {
                            to: Route::BooksPage {},
                            class: "inline-flex items-center gap-1 text-sm text-indigo-600 dark:text-indigo-400 hover:text-indigo-800 dark:hover:text-indigo-300 mb-6",
                            "← Library"
                        }

                        h1 { class: "text-2xl font-bold text-gray-900 dark:text-slate-100 mb-6", "Authors" }

                        if filtered.is_empty() && has_search {
                            p { class: "text-gray-400 dark:text-slate-500 text-sm mt-4", "No authors match your search." }
                        } else if filtered.is_empty() {
                            p { class: "text-gray-400 dark:text-slate-500 text-sm mt-4", "No authors in your library." }
                        } else {
                            table { class: "w-full max-w-2xl",
                                thead {
                                    tr { class: "border-b border-gray-200 dark:border-slate-700",
                                        th { class: "text-left text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-slate-400 pb-2 pr-4",
                                            "Name"
                                        }
                                        th { class: "text-center text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-slate-400 pb-2",
                                            "Books"
                                        }
                                    }
                                }
                                tbody {
                                    for author in &filtered {
                                        tr { class: "border-b border-gray-100 dark:border-slate-700 hover:bg-gray-50 dark:hover:bg-slate-700",
                                            key: "{author.token}",
                                            td { class: "py-2 pr-4",
                                                Link {
                                                    to: Route::AuthorDetailPage { token: author.token.clone() },
                                                    class: "text-sm text-indigo-600 dark:text-indigo-400 hover:text-indigo-800 dark:hover:text-indigo-300",
                                                    "{author.name}"
                                                }
                                            }
                                            td { class: "py-2 text-center text-sm text-gray-600 dark:text-slate-400",
                                                "{author.book_count}"
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
