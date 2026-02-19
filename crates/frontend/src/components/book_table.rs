use dioxus::prelude::*;

use crate::routes::books_page::Book;

#[component]
pub(crate) fn BookTable(books: Vec<Book>) -> Element {
    let mut selected: Signal<Option<Book>> = use_context();

    rsx! {
        div { class: "flex-1 overflow-auto",
            table { class: "w-full text-sm text-left",
                thead { class: "sticky top-0 bg-gray-100 text-gray-600 uppercase text-xs",
                    tr {
                        th { class: "px-4 py-2", "Title" }
                        th { class: "px-4 py-2", "Author" }
                        th { class: "px-4 py-2", "Year" }
                        th { class: "px-4 py-2", "Genre" }
                        th { class: "px-4 py-2", "Pages" }
                    }
                }
                tbody {
                    for book in &books {
                        {
                            let is_selected = selected().as_ref().is_some_and(|s| s.title == book.title);
                            let book_clone = book.clone();
                            rsx! {
                                tr {
                                    class: if is_selected { "bg-indigo-100 cursor-pointer" } else { "hover:bg-gray-50 cursor-pointer" },
                                    onclick: move |_| selected.set(Some(book_clone.clone())),
                                    td { class: "px-4 py-2 font-medium text-gray-900", "{book.title}" }
                                    td { class: "px-4 py-2 text-gray-600", "{book.author}" }
                                    td { class: "px-4 py-2 text-gray-600", "{book.year}" }
                                    td { class: "px-4 py-2 text-gray-600", "{book.genre}" }
                                    td { class: "px-4 py-2 text-gray-600", "{book.pages}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
