use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    Route,
    components::{BookGrid, BookGridContext, SelectionActionBar, SelectionToggle, filter_books_by_search},
    routes::books_page::BookSummary,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct AuthorPageData {
    pub token: String,
    pub name: String,
    pub bio: Option<String>,
    pub books: Vec<BookSummary>,
}

#[cfg(feature = "server")]
use {
    crate::routes::{
        book_detail_page::to_reading_state_dto,
        books_page::hydrate_books,
        server_helpers::{authenticated_user, to_server_err},
    },
    crate::server::AuthSession,
    bb_core::CoreServices,
    bb_core::book::{AuthorToken, BookQuery, BookSortField, BookSortOrder, SortDirection},
    bb_core::reading::ReadStatus,
    std::str::FromStr,
    std::sync::Arc,
};

#[post("/api/v1/author", auth_session: axum::Extension<AuthSession>, core_services: axum::Extension<Arc<CoreServices>>)]
async fn get_author(token: String) -> Result<AuthorPageData, ServerFnError> {
    use std::collections::HashMap;
    let current_user = authenticated_user(&auth_session)?;

    let book_service = &core_services.book_service;

    let author_token = AuthorToken::from_str(&token).map_err(|_| ServerFnError::new("Invalid author token"))?;

    let author = book_service
        .find_author_by_token(author_token)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("Author not found"))?;

    let filter = BookQuery {
        author_id: Some(author.id),
        sort: Some(BookSortOrder {
            field: BookSortField::Title,
            direction: SortDirection::Asc,
        }),
        ..Default::default()
    };
    let books = book_service.list_books(&filter, None, None, None).await.map_err(to_server_err)?;

    let book_ids: Vec<bb_core::book::BookId> = books.iter().map(|b| b.id).collect();
    let reading_metas = core_services
        .reading_service
        .list_for_user_and_books(current_user.id(), &book_ids)
        .await
        .map_err(to_server_err)?;
    let reading_map: HashMap<u64, _> = reading_metas
        .iter()
        .filter(|m| m.read_status != ReadStatus::Unread)
        .map(|m| (m.book_id, to_reading_state_dto(m)))
        .collect();

    let book_summaries = hydrate_books(&books, &core_services, Some(&reading_map)).await?;

    Ok(AuthorPageData {
        token: author.token.to_string(),
        name: author.name,
        bio: author.bio,
        books: book_summaries,
    })
}

#[component]
pub(crate) fn AuthorDetailPage(token: String) -> Element {
    use_context_provider(|| Signal::new(None::<String>)); // DraggedBookToken
    let mut author_resource = use_server_future(move || get_author(token.clone()))?;

    rsx! {
        div { class: "flex-1 overflow-auto p-6",
            match author_resource() {
                None => rsx! {
                    div { class: "flex items-center justify-center h-full text-gray-400 dark:text-slate-500 text-sm",
                        "Loading…"
                    }
                },
                Some(Err(e)) => rsx! {
                    div { class: "text-red-600 text-sm", "Failed to load author: {e}" }
                },
                Some(Ok(author)) => {
                    let query = crate::components::SEARCH_TEXT();
                    let filtered_books = filter_books_by_search(author.books, &query);
                    let has_search = !query.trim().is_empty();
                    let book_tokens: Vec<String> = filtered_books.iter().map(|b| b.token.clone()).collect();
                    rsx! {
                        Link {
                            to: Route::AuthorsPage {},
                            class: "inline-flex items-center gap-1 text-sm text-indigo-600 dark:text-indigo-400 hover:text-indigo-800 dark:hover:text-indigo-300 mb-6",
                            "← Authors"
                        }

                        h1 { class: "text-2xl font-bold text-gray-900 dark:text-slate-100 mb-2", "{author.name}" }

                        if let Some(ref bio) = author.bio {
                            p { class: "text-sm text-gray-600 dark:text-slate-400 leading-relaxed mb-6 max-w-prose", {bio.clone()} }
                        }

                        if filtered_books.is_empty() && has_search {
                            p { class: "text-gray-400 dark:text-slate-500 text-sm mt-4", "No books match your search." }
                        } else if !filtered_books.is_empty() {
                            div { class: "flex items-center gap-2 mb-4",
                                h2 { class: "text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-slate-400",
                                    "Books"
                                }
                                SelectionToggle {}
                            }
                            BookGrid {
                                books: filtered_books,
                                context: BookGridContext::ReadOnly {
                                    current_author_token: Some(author.token.clone()),
                                    current_series_token: None,
                                },
                                on_action: move |()| author_resource.restart(),
                            }
                        }
                        SelectionActionBar {
                            all_book_tokens: book_tokens,
                            on_action: move |()| author_resource.restart(),
                        }
                    }
                },
            }
        }
    }
}
