use dioxus::prelude::*;

use crate::{Route, routes::books_page::BookSummary};

#[component]
pub(crate) fn BookGrid(books: Vec<BookSummary>) -> Element {
    rsx! {
        div { class: "flex-1 overflow-auto p-4",
            div { class: "grid gap-x-8 gap-y-4",
                style: "grid-template-columns: repeat(auto-fill, minmax(120px, 1fr))",
                for book in &books {
                    BookCard { book: book.clone() }
                }
            }
        }
    }
}

#[component]
fn BookCard(book: BookSummary) -> Element {
    let navigator = use_navigator();
    let token = book.token.clone();
    let author_str = book.author_names.join(", ");
    let series_line = match (&book.series_name, &book.series_number) {
        (Some(name), Some(num)) => Some(format!("{name} #{num}")),
        (Some(name), None) => Some(name.clone()),
        _ => None,
    };

    rsx! {
        div {
            class: "flex flex-col cursor-pointer",
            onclick: move |_| { navigator.push(Route::BookDetailPage { token: token.clone() }); },
            img {
                src: "/api/v1/covers/{book.token}",
                alt: "{book.title}",
                class: "w-full object-cover rounded shadow-sm",
                style: "aspect-ratio: 2/3",
            }
            div { class: "mt-1 px-0.5",
                p { class: "text-xs font-semibold text-gray-900 leading-tight line-clamp-2",
                    "{book.title}"
                }
                p { class: "text-xs text-gray-500 leading-tight truncate mt-0.5",
                    "{author_str}"
                }
                if let Some(series) = series_line {
                    p { class: "text-xs text-gray-400 leading-tight truncate mt-0.5",
                        "{series}"
                    }
                }
            }
        }
    }
}
