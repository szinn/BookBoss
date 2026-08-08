use dioxus::prelude::*;

use crate::{
    Route,
    components::{BookFilter, DraggedBookToken, FilterBuilder, FilterEntityOptions, SelectionToggle, SortControl, default_book_filter, filter_to_summary},
    routes::shelf_page::{ShelfSummary, add_book_to_shelf_with_library, create_shelf, create_smart_shelf, get_filter_entity_options},
};

/// Horizontal pill-button row listing the user's shelves and a "+" new-shelf
/// button. Shown above the book grid on both `BooksPage` and `ShelfPage`.
///
/// When `current_shelf_token` matches an own shelf, edit (✎) and delete
/// buttons appear at the right edge, calling the optional handlers.
#[component]
pub(crate) fn ShelfBar(
    shelves: Vec<ShelfSummary>,
    current_shelf_token: Option<String>,
    /// Called when the user clicks ✎ on the current own shelf.
    on_edit_shelf: EventHandler<()>,
    /// Called when the user clicks "Delete" on the current own shelf.
    on_delete_shelf: EventHandler<()>,
    /// When `true` the delete button is disabled (shelf is managed by a
    /// device).
    #[props(default = false)]
    is_device_shelf: bool,
) -> Element {
    let navigator = use_navigator();

    // Static shelf modal state
    let mut show_modal = use_signal(|| false);
    let mut shelf_name = use_signal(String::new);
    let mut creating = use_signal(|| false);
    let mut error_msg: Signal<Option<String>> = use_signal(|| None);

    // Smart shelf modal state
    let mut show_smart_modal = use_signal(|| false);
    let mut smart_name = use_signal(String::new);
    let mut smart_filter = use_signal(default_book_filter);
    let mut smart_creating = use_signal(|| false);
    let mut smart_error: Signal<Option<String>> = use_signal(|| None);

    // "+" dropdown state
    let mut show_add_dropdown = use_signal(|| false);

    // Entity options — always loaded so the hook order is stable
    let entity_options_resource = use_resource(get_filter_entity_options);

    // Admin check for Library filter rule visibility
    let is_admin_resource = use_resource(crate::components::get_is_admin);

    let dragged_token = use_context::<DraggedBookToken>();
    let drag_active = dragged_token().is_some();
    let mut success_shelf: Signal<Option<String>> = use_signal(|| None);

    let own_names: Vec<String> = shelves.iter().filter(|s| s.is_own).map(|s| s.name.to_lowercase()).collect();

    let current_is_own = current_shelf_token
        .as_ref()
        .and_then(|tok| shelves.iter().find(|s| &s.token == tok))
        .is_some_and(|s| s.is_own);

    let mut do_create = move || {
        let name = shelf_name().trim().to_string();
        if name.is_empty() {
            error_msg.set(Some("Shelf name is required.".into()));
            return;
        }
        if own_names.contains(&name.to_lowercase()) {
            error_msg.set(Some("You already have a shelf with that name.".into()));
            return;
        }
        creating.set(true);
        error_msg.set(None);
        spawn(async move {
            match create_shelf(name).await {
                Ok(token) => {
                    show_modal.set(false);
                    creating.set(false);
                    navigator.push(Route::ShelfPage { token });
                }
                Err(e) => {
                    creating.set(false);
                    error_msg.set(Some(e.to_string()));
                }
            }
        });
    };

    let entity_options: FilterEntityOptions = entity_options_resource().and_then(std::result::Result::ok).unwrap_or_default();
    let is_admin = is_admin_resource().and_then(std::result::Result::ok).unwrap_or(false);

    rsx! {
        // Outer bar: flex but no overflow — "+" and Edit/Delete are never clipped.
        // Own shelves and others get their own overflow-x-auto sections so "+" sits
        // exactly between them.
        div { class: "flex items-center gap-2 px-4 py-2 bg-gray-50 dark:bg-slate-900 border-b border-gray-200 dark:border-slate-700 shrink-0",

            // Own-shelves scrollable section (All Books + personal shelves)
            div { class: "flex items-center gap-2 overflow-x-auto min-w-0",

                // "All Books" pill
                Link {
                    to: Route::BooksPage {},
                    class: if current_shelf_token.is_none() {
                        "px-3 py-1 rounded-full text-sm bg-indigo-600 text-white font-medium whitespace-nowrap shrink-0 dark:bg-indigo-900 dark:text-indigo-300"
                    } else {
                        "px-3 py-1 rounded-full text-sm bg-gray-100 text-gray-700 hover:bg-indigo-50 hover:text-indigo-600 whitespace-nowrap shrink-0 dark:bg-slate-800 dark:text-slate-300 dark:hover:bg-slate-700 dark:hover:text-indigo-300"
                    },
                    "All Books"
                }

                // Own shelves
                for shelf in shelves.iter().filter(|s| s.is_own) {
                    {
                        let is_active = current_shelf_token.as_deref() == Some(shelf.token.as_str());
                        let is_drop_target = !is_active && drag_active && !shelf.is_smart;
                        let is_success = success_shelf().as_deref() == Some(shelf.token.as_str());
                        let pill_class = if is_active {
                            "px-3 py-1 rounded-full text-sm bg-indigo-600 text-white font-medium whitespace-nowrap shrink-0 cursor-pointer dark:bg-indigo-900 dark:text-indigo-300"
                        } else if is_drop_target {
                            "px-3 py-1 rounded-full text-sm bg-gray-100 text-gray-700 whitespace-nowrap shrink-0 cursor-pointer ring-2 ring-inset ring-indigo-300 hover:ring-indigo-500 dark:bg-slate-800 dark:text-slate-300"
                        } else {
                            "px-3 py-1 rounded-full text-sm bg-gray-100 text-gray-700 hover:bg-indigo-50 hover:text-indigo-600 whitespace-nowrap shrink-0 cursor-pointer dark:bg-slate-800 dark:text-slate-300 dark:hover:bg-slate-700 dark:hover:text-indigo-300"
                        };
                        let stok = shelf.token.clone();
                        let is_smart = shelf.is_smart;
                        let smart_title = if is_smart {
                            shelf.filter_json.as_deref().and_then(|j| serde_json::from_str::<BookFilter>(j).ok()).map(|f| filter_to_summary(&f))
                        } else {
                            None
                        };
                        rsx! {
                            div {
                                class: if is_success {
                                    format!("{pill_class} shelf-drop-success")
                                } else {
                                    pill_class.to_string()
                                },
                                title: smart_title.unwrap_or_default(),
                                onclick: {
                                    let stok = stok.clone();
                                    move |_| { navigator.push(Route::ShelfPage { token: stok.clone() }); }
                                },
                                ondragover: move |e| {
                                    if !is_active && !is_smart {
                                        e.prevent_default();
                                    }
                                },
                                ondrop: move |e| {
                                    if is_smart { return; }
                                    e.prevent_default();
                                    if let Some(book_tok) = dragged_token() {
                                        let s = stok.clone();
                                        spawn(async move {
                                            if add_book_to_shelf_with_library(s.clone(), book_tok).await.is_ok() {
                                                success_shelf.set(Some(s));
                                            }
                                        });
                                    }
                                },
                                onanimationend: move |_| success_shelf.set(None),
                                if is_smart { "✦ " }
                                "{shelf.name}"
                                if let Some(n) = shelf.count {
                                    span { class: "ml-1 opacity-60", "· {n}" }
                                }
                            }
                        }
                    }
                }
            }

            // "+" new shelf — right after own shelves, outside overflow so dropdown isn't clipped
            div { class: "relative shrink-0",
                button {
                    class: "px-2 py-1 rounded-full text-sm border border-dashed border-gray-300 text-gray-500 hover:border-indigo-400 hover:text-indigo-600 whitespace-nowrap dark:border-slate-600 dark:text-slate-400 dark:hover:border-indigo-400 dark:hover:text-indigo-300",
                    onclick: move |_| show_add_dropdown.set(!show_add_dropdown()),
                    "+"
                }
                if show_add_dropdown() {
                    // Clickaway backdrop
                    div {
                        class: "fixed inset-0 z-40",
                        onclick: move |_| show_add_dropdown.set(false),
                    }
                    div { class: "absolute right-0 top-full mt-1 bg-white dark:bg-slate-800 rounded-lg shadow-lg border border-gray-200 dark:border-slate-700 z-50 py-1 min-w-[150px]",
                        button {
                            class: "w-full text-left px-3 py-2 text-sm text-gray-700 dark:text-slate-300 hover:bg-gray-50 dark:hover:bg-slate-700",
                            onclick: move |_| {
                                show_add_dropdown.set(false);
                                shelf_name.set(String::new());
                                error_msg.set(None);
                                show_modal.set(true);
                            },
                            "Static Shelf"
                        }
                        button {
                            class: "w-full text-left px-3 py-2 text-sm text-gray-700 dark:text-slate-300 hover:bg-gray-50 dark:hover:bg-slate-700",
                            onclick: move |_| {
                                show_add_dropdown.set(false);
                                smart_name.set(String::new());
                                smart_filter.set(default_book_filter());
                                smart_error.set(None);
                                show_smart_modal.set(true);
                            },
                            "Smart Shelf"
                        }
                    }
                }
            }

            // Sort control + Edit/Delete — pushed to far right
            div { class: "flex items-center gap-2 shrink-0 ml-auto",
                SelectionToggle {}
                SortControl {}
                if current_is_own {
                    button {
                        class: "text-sm text-gray-500 dark:text-slate-400 hover:text-indigo-600 dark:hover:text-indigo-300",
                        onclick: move |_| on_edit_shelf.call(()),
                        "Edit"
                    }
                    button {
                        class: if is_device_shelf {
                            "text-sm text-gray-300 dark:text-slate-600 cursor-not-allowed"
                        } else {
                            "text-sm text-red-400 hover:text-red-600 dark:text-red-500 dark:hover:text-red-400"
                        },
                        title: if is_device_shelf {
                            "Managed by device — delete the device from your Profile to remove this shelf"
                        } else {
                            "Delete shelf"
                        },
                        disabled: is_device_shelf,
                        onclick: move |_| {
                            if !is_device_shelf {
                                on_delete_shelf.call(());
                            }
                        },
                        "Delete"
                    }
                }
            }
        }

        // Static shelf modal
        if show_modal() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/40",
                tabindex: -1,
                onkeydown: move |e| { if e.key() == Key::Escape { show_modal.set(false); } },
                div { class: "bg-white dark:bg-slate-800 rounded-lg shadow-xl p-6 w-full max-w-sm mx-4",
                    h2 { class: "text-lg font-semibold text-gray-900 dark:text-slate-100 mb-4", "New Shelf" }

                    form {
                        onsubmit: move |e| {
                            e.prevent_default();
                            do_create();
                        },

                        div { class: "mb-4",
                            label { class: "block text-sm font-medium text-gray-700 dark:text-slate-300 mb-1",
                                r#for: "shelf-name-input",
                                "Shelf name"
                            }
                            input {
                                id: "shelf-name-input",
                                class: "w-full px-3 py-2 border rounded text-sm outline-none focus:ring-1 bg-white dark:bg-slate-700 text-gray-900 dark:text-slate-100",
                                class: if error_msg().is_some() {
                                    "border-red-400 dark:border-red-500 focus:border-red-500 focus:ring-red-500"
                                } else {
                                    "border-gray-300 dark:border-slate-600 focus:border-indigo-500 focus:ring-indigo-500"
                                },
                                r#type: "text",
                                placeholder: "e.g. To Read",
                                autofocus: true,
                                value: shelf_name(),
                                oninput: move |e| {
                                    shelf_name.set(e.value());
                                    error_msg.set(None);
                                },
                                onkeydown: move |e: KeyboardEvent| {
                                    if e.key() == Key::Escape {
                                        show_modal.set(false);
                                    }
                                },
                            }
                            if let Some(msg) = error_msg() {
                                p { class: "mt-1 text-xs text-red-600 dark:text-red-400", {msg} }
                            }
                        }

                        div { class: "flex gap-3 justify-end",
                            button {
                                r#type: "button",
                                class: "px-4 py-2 text-sm font-medium rounded border border-gray-300 dark:border-slate-600 text-gray-700 dark:text-slate-300 hover:bg-gray-50 dark:hover:bg-slate-700",
                                onclick: move |_| show_modal.set(false),
                                "Cancel"
                            }
                            button {
                                r#type: "submit",
                                class: "px-4 py-2 text-sm font-medium rounded bg-indigo-600 text-white hover:bg-indigo-700 disabled:opacity-50",
                                disabled: creating(),
                                if creating() { "Creating…" } else { "Create" }
                            }
                        }
                    }
                }
            }
        }

        // Smart shelf modal
        if show_smart_modal() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/40",
                tabindex: -1,
                onkeydown: move |e| { if e.key() == Key::Escape { show_smart_modal.set(false); } },
                div { class: "bg-white dark:bg-slate-800 rounded-lg shadow-xl p-6 w-full max-w-2xl mx-4 max-h-[85vh] overflow-y-auto",
                    h2 { class: "text-lg font-semibold text-gray-900 dark:text-slate-100 mb-4", "New Smart Shelf" }

                    div { class: "mb-4",
                        label { class: "block text-sm font-medium text-gray-700 dark:text-slate-300 mb-1",
                            r#for: "smart-shelf-name",
                            "Shelf name"
                        }
                        input {
                            id: "smart-shelf-name",
                            class: "w-full px-3 py-2 border border-gray-300 dark:border-slate-600 rounded text-sm outline-none focus:ring-1 focus:border-indigo-500 focus:ring-indigo-500 bg-white dark:bg-slate-700 text-gray-900 dark:text-slate-100",
                            r#type: "text",
                            placeholder: "e.g. Science Fiction Must-Reads",
                            autofocus: true,
                            value: smart_name(),
                            oninput: move |e| {
                                smart_name.set(e.value());
                                smart_error.set(None);
                            },
                            onkeydown: move |e: KeyboardEvent| {
                                if e.key() == Key::Escape {
                                    show_smart_modal.set(false);
                                }
                            },
                        }
                    }

                    div { class: "mb-6",
                        p { class: "text-sm font-medium text-gray-700 dark:text-slate-300 mb-2", "Filter rules" }
                        FilterBuilder { filter: smart_filter, entity_options: entity_options.clone(), current_shelf_id: None, is_admin }
                    }

                    if let Some(msg) = smart_error() {
                        p { class: "mb-3 text-sm text-red-600 dark:text-red-400", {msg} }
                    }

                    div { class: "flex gap-3 justify-end",
                        button {
                            r#type: "button",
                            class: "px-4 py-2 text-sm font-medium rounded border border-gray-300 dark:border-slate-600 text-gray-700 dark:text-slate-300 hover:bg-gray-50 dark:hover:bg-slate-700",
                            onclick: move |_| show_smart_modal.set(false),
                            "Cancel"
                        }
                        button {
                            r#type: "button",
                            class: "px-4 py-2 text-sm font-medium rounded bg-indigo-600 text-white hover:bg-indigo-700 disabled:opacity-50",
                            disabled: smart_creating(),
                            onclick: move |_| {
                                let name = smart_name().trim().to_string();
                                if name.is_empty() {
                                    smart_error.set(Some("Shelf name is required.".into()));
                                    return;
                                }
                                let filter_json = match serde_json::to_string(&smart_filter()) {
                                    Ok(j) => j,
                                    Err(e) => {
                                        smart_error.set(Some(format!("Filter error: {e}")));
                                        return;
                                    }
                                };
                                smart_creating.set(true);
                                smart_error.set(None);
                                spawn(async move {
                                    match create_smart_shelf(name, filter_json).await {
                                        Ok(token) => {
                                            show_smart_modal.set(false);
                                            smart_creating.set(false);
                                            navigator.push(Route::ShelfPage { token });
                                        }
                                        Err(e) => {
                                            smart_creating.set(false);
                                            smart_error.set(Some(e.to_string()));
                                        }
                                    }
                                });
                            },
                            if smart_creating() { "Creating…" } else { "Create Smart Shelf" }
                        }
                    }
                }
            }
        }
    }
}
