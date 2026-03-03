use dioxus::prelude::*;

use crate::routes::books_page::BookSummary;

#[component]
pub(crate) fn DetailPanel() -> Element {
    let selected: Signal<Option<BookSummary>> = use_context();

    rsx! {
        aside { class: "w-72 shrink-0 bg-white border-l border-gray-200 overflow-y-auto p-4",
            match selected() {
                Some(book) => {
                    let author_str = book.author_names.join(", ");
                    let series_line = match (&book.series_name, &book.series_number) {
                        (Some(name), Some(num)) => Some(format!("{name} #{num}")),
                        (Some(name), None) => Some(name.clone()),
                        _ => None,
                    };
                    rsx! {
                        h2 { class: "text-lg font-semibold text-gray-900 mb-3", "{book.title}" }
                        dl { class: "space-y-2 text-sm",
                            div {
                                dt { class: "text-gray-500 font-medium", "Author(s)" }
                                dd { class: "text-gray-800", "{author_str}" }
                            }
                            if let Some(series) = series_line {
                                div {
                                    dt { class: "text-gray-500 font-medium", "Series" }
                                    dd { class: "text-gray-800", "{series}" }
                                }
                            }
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
