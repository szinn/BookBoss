use dioxus::prelude::*;

/// Returns true if every whitespace-separated word in `query` appears
/// (case-insensitive) anywhere in `candidate`.
pub(crate) fn word_match(candidate: &str, query: &str) -> bool {
    let lower = candidate.to_lowercase();
    query.split_whitespace().all(|w| lower.contains(&w.to_lowercase()))
}

/// Multi-value chip input with a live pick-list dropdown.
///
/// - Each selected value renders as a removable chip with an ✕ button.
/// - Typing 1+ characters filters `options` and shows a dropdown with all
///   matching items. The filter is word-order-independent.
/// - Press **Enter** or click a dropdown item to add a chip.
/// - Press **Backspace** on an empty input to remove the last chip.
/// - Chips whose value is not found in `options` (case-insensitive) display a
///   **new** badge in green, giving the user a visual cue that the entry will
///   be created in the database on save.
/// - Focusing an empty input immediately shows all options, signalling that a
///   pick-list is available even before any typing.
/// - `max_chips`: when set, the text input is hidden once the limit is reached,
///   preventing additional entries (useful for single-value fields like
///   Publisher).
/// - Arrow keys navigate the dropdown without moving DOM focus away from the
///   input; Enter selects the highlighted item.
#[component]
pub(crate) fn ChipInput(mut values: Signal<Vec<String>>, options: Vec<String>, placeholder: String, #[props(default)] max_chips: Option<usize>) -> Element {
    let mut input_text = use_signal(String::new);
    let mut show_dropdown = use_signal(|| false);
    let mut focus_on_mount = use_signal(|| false);
    let mut focused_index = use_signal(|| None::<usize>);

    let query = input_text.read().clone();
    let current = values.read().clone();
    let at_limit = max_chips.is_some_and(|max| current.len() >= max);

    let show = *show_dropdown.read();
    let filtered: Vec<String> = if query.is_empty() {
        if show {
            options
                .iter()
                .filter(|opt| !current.iter().any(|v| v.eq_ignore_ascii_case(opt)))
                .cloned()
                .collect()
        } else {
            vec![]
        }
    } else {
        options
            .iter()
            .filter(|opt| word_match(opt, &query) && !current.iter().any(|v| v.eq_ignore_ascii_case(opt)))
            .cloned()
            .collect()
    };

    let filtered_for_keys = filtered.clone();

    let ph = if current.is_empty() { placeholder.as_str() } else { "" };

    rsx! {
        div { class: "relative",
            // ── Chip container ─────────────────────────────────────────────────
            div {
                class: "flex flex-wrap gap-1 items-center border border-gray-300 dark:border-slate-600 rounded px-2 py-1 min-h-[34px] bg-white dark:bg-slate-700 focus-within:ring-1 focus-within:ring-indigo-400",
                // When max_chips is 1 and the field is full, clicking anywhere on the
                // container re-opens the input pre-populated with the current value.
                onclick: move |_| {
                    if at_limit && max_chips == Some(1) {
                        let current_val = values.write().pop().unwrap_or_default();
                        input_text.set(current_val);
                        show_dropdown.set(true);
                        focused_index.set(None);
                        focus_on_mount.set(true);
                    }
                },
                for (i, chip) in current.iter().enumerate() {
                    {
                        let is_new = !options.iter().any(|o| o.eq_ignore_ascii_case(chip));
                        let label = chip.clone();
                        let chip_class = if is_new {
                            "inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs bg-green-100 dark:bg-green-900 text-green-800 dark:text-green-100 border border-green-300 dark:border-green-700"
                        } else {
                            "inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs bg-gray-100 dark:bg-slate-600 text-gray-700 dark:text-slate-200 border border-gray-300 dark:border-slate-500"
                        };
                        rsx! {
                            span { key: "{label}", class: chip_class,
                                {label}
                                if is_new {
                                    span { class: "font-semibold ml-0.5 text-green-700 dark:text-green-300", "new" }
                                }
                                button {
                                    r#type: "button",
                                    class: "ml-0.5 text-gray-400 dark:text-slate-400 hover:text-gray-700 dark:hover:text-slate-200 cursor-pointer leading-none",
                                    onclick: move |e| {
                                        e.stop_propagation();
                                        values.write().remove(i);
                                        if max_chips.is_some() {
                                            focus_on_mount.set(true);
                                        }
                                    },
                                    "×"
                                }
                            }
                        }
                    }
                }
                if !at_limit {
                input {
                    class: "flex-1 min-w-[120px] text-sm text-gray-900 dark:text-slate-100 placeholder-gray-400 dark:placeholder-slate-400 outline-none bg-transparent py-0.5",
                    value: "{input_text}",
                    placeholder: ph,
                    onmounted: move |e| {
                        if focus_on_mount() {
                            focus_on_mount.set(false);
                            spawn(async move { let _ = e.set_focus(true).await; });
                        }
                    },
                    oninput: move |e| {
                        input_text.set(e.value());
                        show_dropdown.set(true);
                        focused_index.set(None);
                    },
                    onkeydown: move |e| {
                        match e.key() {
                            Key::ArrowDown => {
                                e.prevent_default();
                                if *show_dropdown.read() && !filtered_for_keys.is_empty() {
                                    let next = match *focused_index.read() {
                                        None => 0,
                                        Some(n) => (n + 1).min(filtered_for_keys.len() - 1),
                                    };
                                    focused_index.set(Some(next));
                                }
                            }
                            Key::ArrowUp => {
                                e.prevent_default();
                                let current_idx = *focused_index.read();
                                if let Some(n) = current_idx {
                                    focused_index.set(if n == 0 { None } else { Some(n - 1) });
                                }
                            }
                            Key::Enter => {
                                e.prevent_default();
                                let current_idx = *focused_index.read();
                                if let Some(idx) = current_idx {
                                    let name = filtered_for_keys[idx].trim().to_string();
                                    if !name.is_empty() {
                                        let mut v = values.write();
                                        if !v.iter().any(|x| x.eq_ignore_ascii_case(&name)) {
                                            v.push(name);
                                        }
                                    }
                                    input_text.set(String::new());
                                    show_dropdown.set(false);
                                    focused_index.set(None);
                                } else {
                                    let text = input_text.read().trim().to_string();
                                    if !text.is_empty() {
                                        let mut v = values.write();
                                        if !v.iter().any(|x| x.eq_ignore_ascii_case(&text)) {
                                            v.push(text);
                                        }
                                        input_text.set(String::new());
                                        show_dropdown.set(false);
                                    }
                                }
                            }
                            Key::Backspace if input_text.read().is_empty() => {
                                let mut v = values.write();
                                if !v.is_empty() {
                                    v.pop();
                                }
                            }
                            Key::Escape => {
                                input_text.set(String::new());
                                show_dropdown.set(false);
                                focused_index.set(None);
                            }
                            _ => {
                                // Any other key clears the highlight so it doesn't stray
                                focused_index.set(None);
                            }
                        }
                    },
                    onfocus: move |_| {
                        show_dropdown.set(true);
                    },
                    onfocusout: move |_| {
                        show_dropdown.set(false);
                        focused_index.set(None);
                    },
                }
                } // end if !at_limit
            }
            // ── Dropdown ───────────────────────────────────────────────────────
            if *show_dropdown.read() && !filtered.is_empty() {
                div { class: "absolute left-0 right-0 top-full mt-1 bg-white dark:bg-slate-800 border border-gray-200 dark:border-slate-700 rounded shadow-lg z-50 max-h-48 overflow-y-auto",
                    for (i, option) in filtered.iter().enumerate() {
                        {
                            let label = option.clone();
                            let click_val = option.clone();
                            let is_focused = focused_index() == Some(i);
                            let row_class = if is_focused {
                                "px-3 py-1.5 text-sm text-gray-700 dark:text-slate-200 cursor-pointer bg-indigo-50 dark:bg-slate-700 border-l-2 border-indigo-400"
                            } else {
                                "px-3 py-1.5 text-sm text-gray-700 dark:text-slate-200 hover:bg-indigo-50 dark:hover:bg-slate-700 cursor-pointer"
                            };
                            rsx! {
                                div {
                                    key: "{label}",
                                    class: row_class,
                                    onmousedown: move |e| e.prevent_default(),
                                    onclick: move |_| {
                                        let name = click_val.trim().to_string();
                                        if !name.is_empty() {
                                            let mut v = values.write();
                                            if !v.iter().any(|x| x.eq_ignore_ascii_case(&name)) {
                                                v.push(name);
                                            }
                                        }
                                        input_text.set(String::new());
                                        show_dropdown.set(false);
                                        focused_index.set(None);
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
