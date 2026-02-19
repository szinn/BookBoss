use dioxus::prelude::*;

use crate::Route;

#[component]
pub(crate) fn NavBar() -> Element {
    rsx! {
        nav { class: "bg-indigo-700 text-white px-6 py-3 flex items-center justify-between shadow",
            div { class: "flex items-center gap-6",
                Link { to: Route::BooksPage {}, class: "text-sm hover:text-indigo-200",
                    "Books"
                }
            }
            div { class: "flex items-center gap-4",
                button { class: "text-sm hover:text-indigo-200", "Settings" }
                button { class: "text-sm hover:text-indigo-200", "User" }
            }
        }
    }
}
