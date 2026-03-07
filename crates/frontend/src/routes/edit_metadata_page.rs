use dioxus::prelude::*;

use super::review_page::{ReviewEditor, get_book_for_edit};
use crate::Route;

#[component]
pub(crate) fn EditMetadataPage(token: String) -> Element {
    let nav = use_navigator();
    let book_token = token.clone();
    let review_data = use_server_future(move || get_book_for_edit(token.clone()))?;

    match review_data() {
        None => rsx! {
            div { class: "flex-1 flex items-center justify-center text-gray-400 text-sm",
                "Loading…"
            }
        },
        Some(Err(e)) => rsx! {
            div { class: "flex-1 flex items-center justify-center text-red-600 text-sm",
                "Failed to load: {e}"
            }
        },
        Some(Ok(data)) => {
            rsx! {
                ReviewEditor {
                    data,
                    edit_mode: true,
                    on_back: move |_| {
                        nav.push(Route::BookDetailPage { token: book_token.clone() });
                    },
                }
            }
        }
    }
}
