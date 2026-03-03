use dioxus::prelude::*;

use crate::routes::books_page::BookSummary;

#[component]
pub(crate) fn BookTable(books: Vec<BookSummary>) -> Element {
    let mut selected: Signal<Option<BookSummary>> = use_context();

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
                            let is_selected = selected().as_ref().is_some_and(|s| s.token == book.token);
                            let book_clone = book.clone();
                            let author_str = book.author_names.join(", ");
                            let series_str = match (&book.series_name, &book.series_number) {
                                (Some(name), Some(num)) => format!("{name} #{num}"),
                                (Some(name), None) => name.clone(),
                                _ => String::new(),
                            };
                            rsx! {
                                tr {
                                    class: if is_selected { "bg-indigo-100 cursor-pointer" } else { "hover:bg-gray-50 cursor-pointer" },
                                    onclick: move |_| selected.set(Some(book_clone.clone())),
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
