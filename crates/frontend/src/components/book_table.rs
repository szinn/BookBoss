use dioxus::prelude::*;

use crate::{Route, routes::books_page::BookSummary};

#[component]
pub(crate) fn BookTable(books: Vec<BookSummary>) -> Element {
    let navigator = use_navigator();

    rsx! {
        div { class: "flex-1 overflow-auto",
            table { class: "w-full text-sm text-left",
                thead { class: "sticky top-0 bg-gray-100 text-gray-600 uppercase text-xs",
                    tr {
                        th { class: "px-4 py-2", "Title" }
                        th { class: "px-4 py-2", "Author(s)" }
                        th { class: "px-4 py-2", "Series" }
                    }
                }
                tbody {
                    for book in &books {
                        {
                            let token = book.token.clone();
                            let author_str = book.author_names.join(", ");
                            let series_str = match (&book.series_name, &book.series_number) {
                                (Some(name), Some(num)) => format!("{name} #{num}"),
                                (Some(name), None) => name.clone(),
                                _ => String::new(),
                            };
                            rsx! {
                                tr {
                                    class: "hover:bg-gray-50 cursor-pointer",
                                    onclick: move |_| { navigator.push(Route::BookDetailPage { token: token.clone() }); },
                                    td { class: "px-4 py-2 font-medium text-gray-900", "{book.title}" }
                                    td { class: "px-4 py-2 text-gray-600", "{author_str}" }
                                    td { class: "px-4 py-2 text-gray-600", "{series_str}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
