use dioxus::prelude::*;

use super::chip_input::word_match;

/// Single-value text input with a live pick-list dropdown for series names.
///
/// - When a value is committed, it renders as a removable pill. Chips whose
///   name is not found in `options` display a **new** badge in green.
/// - Clicking the pill re-opens the input pre-populated with the current value.
/// - When empty or editing, shows a text input. Focusing immediately shows all
///   options from the pick-list.
/// - Typing 1+ characters filters `options` by word-order-independent matching.
/// - Press `↓`/`↑` to navigate the dropdown; `Enter` to select the highlighted
///   item or commit the typed text; `Escape` to cancel.
/// - Clicking a suggestion commits the value, closes the dropdown, and fires
///   `on_series_selected` with `(series_name, suggested_next_number)`.
/// - The × on the pill clears the value and fires `on_cleared`.
/// - `on_blur` (fired on focus-out with the current value) lets the parent set
///   a default book number when the user types a brand-new series name.
#[component]
pub(crate) fn AutocompleteInput(
    mut value: Signal<String>,
    options: Vec<(String, u32)>,
    on_series_selected: EventHandler<(String, u32)>,
    on_cleared: EventHandler<()>,
    on_blur: EventHandler<String>,
) -> Element {
    let mut editing = use_signal(|| false);
    let mut input_text = use_signal(String::new);
    let mut show_dropdown = use_signal(|| false);
    let mut focused_index = use_signal(|| None::<usize>);

    let is_editing = *editing.read();
    let committed = value.read().clone();
    let is_set = !committed.is_empty() && !is_editing;

    let query = if is_editing { input_text.read().clone() } else { String::new() };

    let show = *show_dropdown.read();
    let filtered: Vec<(String, u32)> = if is_editing {
        if query.is_empty() {
            if show { options.clone() } else { vec![] }
        } else {
            options.iter().filter(|(name, _)| word_match(name, &query)).cloned().collect()
        }
    } else {
        vec![]
    };

    let filtered_for_keys = filtered.clone();

    let is_new = is_set && !options.iter().any(|(name, _)| name.eq_ignore_ascii_case(&committed));

    rsx! {
        div { class: "relative flex-1",
            if is_set {
                // ── Pill display ──────────────────────────────────────────────
                div {
                    class: "flex flex-wrap gap-1 items-center border border-gray-300 dark:border-slate-600 rounded px-2 py-1 min-h-[34px] cursor-text bg-white dark:bg-slate-700",
                    onclick: move |_| {
                        input_text.set(committed.clone());
                        editing.set(true);
                        show_dropdown.set(true);
                    },
                    {
                        let label = committed.clone();
                        let chip_class = if is_new {
                            "inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs bg-green-100 dark:bg-green-900 text-green-800 dark:text-green-100 border border-green-300 dark:border-green-700"
                        } else {
                            "inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs bg-gray-100 dark:bg-slate-600 text-gray-700 dark:text-slate-200 border border-gray-300 dark:border-slate-500"
                        };
                        rsx! {
                            span { class: chip_class,
                                {label}
                                if is_new {
                                    span { class: "font-semibold ml-0.5 text-green-700 dark:text-green-300", "new" }
                                }
                                button {
                                    r#type: "button",
                                    class: "ml-0.5 text-gray-400 dark:text-slate-400 hover:text-gray-700 dark:hover:text-slate-200 cursor-pointer leading-none",
                                    onclick: move |e| {
                                        e.stop_propagation();
                                        value.write().clear();
                                        input_text.set(String::new());
                                        editing.set(false);
                                        on_cleared.call(());
                                    },
                                    "×"
                                }
                            }
                        }
                    }
                }
            } else {
                // ── Text input ────────────────────────────────────────────────
                input {
                    class: "w-full border border-gray-300 dark:border-slate-600 rounded px-2 py-1 text-sm text-gray-900 dark:text-slate-100 bg-white dark:bg-slate-700 placeholder-gray-400 dark:placeholder-slate-400 focus:outline-none focus:ring-1 focus:ring-indigo-400",
                    value: "{input_text}",
                    placeholder: "Series name",
                    autofocus: is_editing,
                    oninput: move |e| {
                        input_text.set(e.value());
                        show_dropdown.set(true);
                        focused_index.set(None);
                    },
                    onfocus: move |_| {
                        editing.set(true);
                        show_dropdown.set(true);
                    },
                    onfocusout: move |_| {
                        let text = input_text.read().clone();
                        show_dropdown.set(false);
                        editing.set(false);
                        focused_index.set(None);
                        if !text.is_empty() {
                            value.set(text.clone());
                            on_blur.call(text);
                        } else if committed.is_empty() {
                            // Was empty, still empty — nothing to do.
                        } else {
                            // User blanked out the input — clear the value.
                            value.write().clear();
                            on_cleared.call(());
                        }
                    },
                    onkeydown: move |e| {
                        match e.key() {
                            Key::ArrowDown => {
                                e.prevent_default();
                                if *show_dropdown.read() && !filtered_for_keys.is_empty() {
                                    let current = *focused_index.read();
                                    let next = match current {
                                        None => 0,
                                        Some(n) => (n + 1).min(filtered_for_keys.len() - 1),
                                    };
                                    focused_index.set(Some(next));
                                }
                            }
                            Key::ArrowUp => {
                                e.prevent_default();
                                let current = *focused_index.read();
                                if let Some(n) = current {
                                    focused_index.set(if n == 0 { None } else { Some(n - 1) });
                                }
                            }
                            Key::Enter => {
                                e.prevent_default();
                                let current = *focused_index.read();
                                if let Some(idx) = current {
                                    let (name, next_num) = filtered_for_keys[idx].clone();
                                    value.set(name.clone());
                                    input_text.set(name.clone());
                                    show_dropdown.set(false);
                                    editing.set(false);
                                    focused_index.set(None);
                                    on_series_selected.call((name, next_num));
                                } else {
                                    let text = input_text.read().clone();
                                    if !text.is_empty() {
                                        show_dropdown.set(false);
                                        editing.set(false);
                                        value.set(text.clone());
                                        on_blur.call(text);
                                    }
                                }
                            }
                            Key::Escape => {
                                show_dropdown.set(false);
                                editing.set(false);
                                focused_index.set(None);
                                // Restore previous committed value.
                                input_text.set(value.read().clone());
                            }
                            _ => {
                                focused_index.set(None);
                            }
                        }
                    },
                }
            }
            // ── Dropdown ──────────────────────────────────────────────────────
            if show && !filtered.is_empty() {
                div { class: "absolute left-0 right-0 top-full mt-1 bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded shadow-lg z-50 max-h-48 overflow-y-auto",
                    for (i, (name, next_num)) in filtered.iter().enumerate() {
                        {
                            let label = name.clone();
                            let click_name = name.clone();
                            let is_focused = focused_index() == Some(i);
                            let row_class = if is_focused {
                                "px-3 py-1.5 text-sm text-gray-700 dark:text-slate-200 cursor-pointer bg-indigo-50 dark:bg-slate-700 border-l-2 border-indigo-400"
                            } else {
                                "px-3 py-1.5 text-sm text-gray-700 dark:text-slate-200 hover:bg-indigo-50 dark:hover:bg-slate-700 cursor-pointer"
                            };
                            let next_num = *next_num;
                            rsx! {
                                div {
                                    key: "{label}",
                                    class: row_class,
                                    onmousedown: move |e| e.prevent_default(),
                                    onclick: move |_| {
                                        value.set(click_name.clone());
                                        input_text.set(click_name.clone());
                                        show_dropdown.set(false);
                                        editing.set(false);
                                        focused_index.set(None);
                                        on_series_selected.call((click_name.clone(), next_num));
                                    },
                                    {label}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
