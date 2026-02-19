use dioxus::prelude::*;

use crate::routes::books_page::Book;

#[component]
pub(crate) fn DetailPanel() -> Element {
    let selected: Signal<Option<Book>> = use_context();

    rsx! {
        aside { class: "w-72 shrink-0 bg-white border-l border-gray-200 overflow-y-auto p-4",
            match selected() {
                Some(book) => rsx! {
                    h2 { class: "text-lg font-semibold text-gray-900 mb-3", "{book.title}" }
                    dl { class: "space-y-2 text-sm",
                        div {
                            dt { class: "text-gray-500 font-medium", "Author" }
                            dd { class: "text-gray-800", "{book.author}" }
                        }
                        div {
                            dt { class: "text-gray-500 font-medium", "Year" }
                            dd { class: "text-gray-800", "{book.year}" }
                        }
                        div {
                            dt { class: "text-gray-500 font-medium", "Genre" }
                            dd { class: "text-gray-800", "{book.genre}" }
                        }
                        div {
                            dt { class: "text-gray-500 font-medium", "Pages" }
                            dd { class: "text-gray-800", "{book.pages}" }
                        }
                    }
                },
                None => rsx! {
                    div { class: "flex items-center justify-center h-full text-gray-400 text-sm",
                        "Select a book to view details"
                    }
                },
            }
        }
    }
}
