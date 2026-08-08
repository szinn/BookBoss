use std::collections::HashSet;

use dioxus::prelude::*;
#[cfg(feature = "server")]
use {
    crate::routes::server_helpers::{authenticated_user, require_capability, to_server_err},
    crate::server::{AuthSession, AuthUser, BackendSessionPool},
    axum::http::Method,
    axum_session_auth::{Auth, Rights},
    bb_core::{
        CoreServices,
        book::{AuthorToken, BookToken, IdentifierType, PublisherToken, SeriesToken},
        collection::BookEdit,
        library::LibraryToken,
        reading::ReadStatus,
        types::Capability,
        user::UserId,
    },
    std::str::FromStr,
    std::sync::Arc,
};

use crate::{
    components::{AutocompleteInput, ChipInput},
    routes::review_page::{BulkEditFields, get_picklist_data, list_non_system_libraries},
};

// ---------------------------------------------------------------------------
// Global selection state
// ---------------------------------------------------------------------------

/// Whether selection mode is currently active.
pub(crate) static SELECTION_MODE: GlobalSignal<bool> = Signal::global(|| false);

/// Set of book tokens currently selected.
pub(crate) static SELECTED_BOOKS: GlobalSignal<HashSet<String>> = Signal::global(HashSet::new);

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

pub(crate) fn toggle_selection(token: &str) {
    let mut selected = SELECTED_BOOKS.write();
    if selected.contains(token) {
        selected.remove(token);
    } else {
        selected.insert(token.to_string());
    }
}

pub(crate) fn select_all(tokens: impl IntoIterator<Item = String>) {
    let mut selected = SELECTED_BOOKS.write();
    for token in tokens {
        selected.insert(token);
    }
}

pub(crate) fn deselect_all() {
    SELECTED_BOOKS.write().clear();
}

pub(crate) fn is_selected(token: &str) -> bool {
    SELECTED_BOOKS.read().contains(token)
}

pub(crate) fn selection_count() -> usize {
    SELECTED_BOOKS.read().len()
}

pub(crate) fn exit_selection_mode() {
    *SELECTION_MODE.write() = false;
    SELECTED_BOOKS.write().clear();
}

// ---------------------------------------------------------------------------
// Server function: bulk set reading status
// ---------------------------------------------------------------------------

#[post(
    "/api/v1/books/bulk/reading-status",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn bulk_set_reading_status(tokens: Vec<String>, status: String) -> Result<u32, ServerFnError> {
    let user_id = authenticated_user(&auth_session)?.id();

    let new_status: ReadStatus = status.parse().map_err(|e: String| ServerFnError::new(e))?;

    let mut updated = 0u32;
    for token in &tokens {
        let book_token = BookToken::from_str(token).map_err(|_| ServerFnError::new(format!("Invalid book token: {token}")))?;
        let book = core_services
            .book_service
            .find_book_by_token(book_token)
            .await
            .map_err(to_server_err)?
            .ok_or_else(|| ServerFnError::new(format!("Book not found: {token}")))?;

        core_services
            .reading_service
            .set_status(user_id, book.id, new_status)
            .await
            .map_err(to_server_err)?;
        updated += 1;
    }

    Ok(updated)
}

// ---------------------------------------------------------------------------
// Server function: bulk edit metadata
// ---------------------------------------------------------------------------

#[put(
    "/api/v1/books/bulk/edit",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn bulk_edit_metadata(tokens: Vec<String>, fields: BulkEditFields) -> Result<u32, ServerFnError> {
    require_capability(&auth_session, Capability::EditBook, Method::PUT).await?;

    let temp_dir = std::env::temp_dir();

    let mut updated = 0u32;
    for token_str in &tokens {
        let result = bulk_edit_single_book(&core_services, token_str, &fields, &temp_dir).await;
        match result {
            Ok(()) => updated += 1,
            Err(e) => tracing::warn!(book_token = %token_str, error = %e, "bulk edit failed for book"),
        }
    }

    Ok(updated)
}

/// Load a single book's existing data, merge bulk edit fields, and save.
#[cfg(feature = "server")]
async fn bulk_edit_single_book(
    core_services: &CoreServices,
    token_str: &str,
    fields: &BulkEditFields,
    temp_dir: &std::path::Path,
) -> Result<(), ServerFnError> {
    let book_service = &core_services.book_service;
    let token = BookToken::from_str(token_str).map_err(|_| ServerFnError::new("Invalid book token"))?;
    let book = book_service
        .find_book_by_token(token)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("Book not found"))?;

    // Load existing authors
    let existing_authors = {
        let mut links = book_service.authors_for_book(book.id).await.map_err(to_server_err)?;
        links.sort_by_key(|a| a.sort_order);
        let mut names = Vec::with_capacity(links.len());
        for ba in &links {
            if let Some(author) = book_service.find_author_by_token(AuthorToken::new(ba.author_id)).await.map_err(to_server_err)? {
                names.push(author.name);
            }
        }
        names
    };

    // Load existing series name
    let existing_series = if let Some(sid) = book.series_id {
        book_service
            .find_series_by_token(SeriesToken::new(sid))
            .await
            .map_err(to_server_err)?
            .map(|s| s.name)
    } else {
        None
    };

    // Load existing publisher name
    let existing_publisher = if let Some(pid) = book.publisher_id {
        book_service
            .find_publisher_by_token(PublisherToken::new(pid))
            .await
            .map_err(to_server_err)?
            .map(|p| p.name)
    } else {
        None
    };

    // Load existing identifiers
    let existing_identifiers: Vec<(IdentifierType, String)> = book_service
        .identifiers_for_book(book.id)
        .await
        .map_err(to_server_err)?
        .into_iter()
        .map(|i| (i.identifier_type, i.value))
        .collect();

    // Load existing genres and tags
    let existing_genres: Vec<String> = book_service
        .genres_for_book(book.id)
        .await
        .map_err(to_server_err)?
        .into_iter()
        .map(|g| g.name)
        .collect();
    let existing_tags: Vec<String> = book_service
        .tags_for_book(book.id)
        .await
        .map_err(to_server_err)?
        .into_iter()
        .map(|t| t.name)
        .collect();

    // Merge: bulk fields override existing; None means keep existing
    let edit = BookEdit {
        title: book.title.clone(),
        description: book.description.clone(),
        published_date: book.published_date,
        language: fields.language.clone().or(book.language.clone()),
        series_name: match &fields.series_name {
            Some(s) if s.is_empty() => None,
            Some(s) => Some(s.clone()),
            None => existing_series,
        },
        series_number: book.series_number,
        publisher_name: match &fields.publisher {
            Some(p) if p.is_empty() => None,
            Some(p) => Some(p.clone()),
            None => existing_publisher,
        },
        page_count: book.page_count,
        authors: fields.authors.clone().unwrap_or(existing_authors),
        identifiers: existing_identifiers,
        use_fetched_cover: false,
        genres: fields.genres.clone().unwrap_or(existing_genres),
        tags: fields.tags.clone().unwrap_or(existing_tags),
    };

    core_services
        .collection_service
        .edit_book(token, edit, token_str, temp_dir)
        .await
        .map_err(to_server_err)?;

    // Library membership update (best-effort — skip on error to keep processing
    // other books)
    if let Some(desired_tokens) = &fields.library_tokens {
        let all_libraries = core_services.library_service.list_libraries().await.map_err(to_server_err)?;
        let non_system: Vec<_> = all_libraries.into_iter().filter(|e| !e.library.is_system).collect();
        let current_ids = core_services.library_service.library_ids_for_book(book.id).await.map_err(to_server_err)?;

        for entry in &non_system {
            let lib = &entry.library;
            let desired = desired_tokens.contains(&lib.token.to_string());
            let has = current_ids.contains(&lib.id);
            if desired && !has {
                let _ = core_services.library_service.add_book_to_library(lib.token, token).await;
            } else if !desired && has {
                let _ = core_services.library_service.remove_book_from_library(lib.token, token).await;
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Server function: bulk delete books
// ---------------------------------------------------------------------------

#[post(
    "/api/v1/books/bulk/delete",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn bulk_delete_books(tokens: Vec<String>) -> Result<u32, ServerFnError> {
    require_capability(&auth_session, Capability::DeleteBook, Method::POST).await?;

    let mut deleted = 0u32;
    for token_str in &tokens {
        let Ok(book_token) = BookToken::from_str(token_str) else {
            tracing::warn!(book_token = %token_str, "bulk delete: invalid token, skipping");
            continue;
        };
        match core_services.collection_service.delete_book(book_token).await {
            Ok(()) => deleted += 1,
            Err(e) => tracing::warn!(book_token = %token_str, error = %e, "bulk delete failed for book"),
        }
    }

    Ok(deleted)
}

// ---------------------------------------------------------------------------
// Server function: check DeleteBook capability
// ---------------------------------------------------------------------------

#[get(
    "/api/v1/books/bulk/can-delete",
    auth_session: axum::Extension<AuthSession>
)]
async fn check_delete_book_capability() -> Result<bool, ServerFnError> {
    let current_user = auth_session.current_user.clone().unwrap_or_default();
    Ok(Auth::<AuthUser, UserId, BackendSessionPool>::build([Method::GET], true)
        .requires(Rights::any([Rights::permission(Capability::DeleteBook.as_str())]))
        .validate(&current_user, &Method::GET, None)
        .await)
}

// ---------------------------------------------------------------------------
// SelectionToggle — small icon button to enter/exit selection mode
// ---------------------------------------------------------------------------

#[component]
pub(crate) fn SelectionToggle() -> Element {
    let mode = SELECTION_MODE();

    rsx! {
        button {
            r#type: "button",
            class: if mode {
                "flex items-center justify-center px-2 py-1.5 rounded bg-indigo-600 text-white cursor-pointer"
            } else {
                "flex items-center justify-center px-2 py-1.5 rounded text-gray-500 dark:text-slate-400 hover:text-indigo-600 dark:hover:text-indigo-400 hover:bg-gray-100 dark:hover:bg-slate-700 cursor-pointer"
            },
            title: if mode { "Exit selection mode" } else { "Select books" },
            onclick: move |_| {
                if mode {
                    exit_selection_mode();
                } else {
                    *SELECTION_MODE.write() = true;
                }
            },
            // Heroicons mini: check in a rounded square
            svg {
                class: "w-5 h-5",
                xmlns: "http://www.w3.org/2000/svg",
                view_box: "0 0 20 20",
                fill: "currentColor",
                // Rounded rectangle
                path {
                    d: "M3 5a2 2 0 012-2h10a2 2 0 012 2v10a2 2 0 01-2 2H5a2 2 0 01-2-2V5zm1.5.5v9a.5.5 0 00.5.5h10a.5.5 0 00.5-.5v-9a.5.5 0 00-.5-.5H5a.5.5 0 00-.5.5z",
                    fill_rule: "evenodd",
                    clip_rule: "evenodd",
                }
                // Checkmark inside
                path {
                    d: "M13.854 7.146a.5.5 0 010 .708l-4.5 4.5a.5.5 0 01-.708 0l-2-2a.5.5 0 11.708-.708L9 11.293l4.146-4.147a.5.5 0 01.708 0z",
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// BulkEditModal — modal for editing metadata on multiple books at once
// ---------------------------------------------------------------------------

/// ISO 639-1 language codes — North American / European subset.
pub(crate) const LANGUAGE_CODES: &[&str] = &[
    "af", "be", "bg", "ca", "cs", "cy", "da", "de", "el", "en", "es", "et", "fi", "fr", "ga", "hr", "hu", "is", "it", "lv", "lt", "mk", "nl", "no", "pl", "pt",
    "ro", "ru", "sk", "sl", "sq", "sr", "sv", "tr", "uk",
];

#[component]
fn BulkEditModal(on_close: EventHandler<()>, on_saved: EventHandler<()>) -> Element {
    let count = selection_count();

    // Field value signals
    let mut authors = use_signal(Vec::<String>::new);
    let mut publisher = use_signal(Vec::<String>::new);
    let mut language = use_signal(Vec::<String>::new);
    let mut series_name = use_signal(String::new);
    let mut genres = use_signal(Vec::<String>::new);
    let mut tags = use_signal(Vec::<String>::new);
    let mut library_tokens: Signal<Vec<String>> = use_signal(Vec::new);

    // Apply flags — independent checkboxes
    let mut apply_authors = use_signal(|| false);
    let mut apply_publisher = use_signal(|| false);
    let mut apply_language = use_signal(|| false);
    let mut apply_series = use_signal(|| false);
    let mut apply_genres = use_signal(|| false);
    let mut apply_tags = use_signal(|| false);
    let mut apply_libraries = use_signal(|| false);

    // Auto-sync: content changes update checkboxes
    use_effect(move || {
        let v = authors();
        apply_authors.set(!v.is_empty());
    });
    use_effect(move || {
        let v = publisher();
        apply_publisher.set(!v.is_empty());
    });
    use_effect(move || {
        let v = language();
        apply_language.set(!v.is_empty());
    });
    use_effect(move || {
        let v = series_name();
        apply_series.set(!v.is_empty());
    });
    use_effect(move || {
        let v = genres();
        apply_genres.set(!v.is_empty());
    });
    use_effect(move || {
        let v = tags();
        apply_tags.set(!v.is_empty());
    });

    // Load non-system libraries for the Libraries row
    let non_system_libraries = use_resource(list_non_system_libraries);

    let mut busy = use_signal(|| false);
    let mut error_msg: Signal<Option<String>> = use_signal(|| None);

    // Picklist data for autocomplete
    let picklist = use_resource(move || get_picklist_data(()));

    let any_checked = apply_authors() || apply_publisher() || apply_language() || apply_series() || apply_genres() || apply_tags() || apply_libraries();
    let authors_invalid = apply_authors() && authors.read().is_empty();

    // Non-system libraries for the Libraries row; empty = hide the row
    let available_libraries: Vec<(String, String)> = non_system_libraries.read().as_ref().and_then(|r| r.as_ref().ok()).cloned().unwrap_or_default();

    let label = if count == 1 {
        "Editing 1 book — checked fields will be applied".to_string()
    } else {
        format!("Editing {count} books — checked fields will be applied")
    };

    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/40",
            tabindex: -1,
            onmounted: move |e| async move { let _ = e.set_focus(true).await; },
            onclick: move |_| if !busy() { on_close.call(()); },
            onkeydown: move |e| { if e.key() == Key::Escape && !busy() { on_close.call(()); } },

            div {
                class: "bg-white dark:bg-slate-800 rounded-xl shadow-xl w-full max-w-lg mx-4",
                onclick: |e| e.stop_propagation(),

                // Header
                div { class: "flex items-center justify-between px-6 pt-5 pb-2",
                    h2 { class: "text-lg font-semibold text-gray-900 dark:text-slate-100", "Bulk Edit Metadata" }
                    button {
                        class: "text-gray-400 dark:text-slate-500 hover:text-gray-600 dark:hover:text-slate-300 cursor-pointer",
                        disabled: busy(),
                        onclick: move |_| on_close.call(()),
                        svg {
                            class: "w-5 h-5",
                            xmlns: "http://www.w3.org/2000/svg",
                            fill: "none",
                            view_box: "0 0 24 24",
                            stroke_width: "2",
                            stroke: "currentColor",
                            path {
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                                d: "M6 18L18 6M6 6l12 12",
                            }
                        }
                    }
                }

                // Subtitle
                div { class: "px-6 pb-4",
                    p { class: "text-sm text-gray-500 dark:text-slate-400", {label} }
                }

                // Fields
                div { class: "px-6 space-y-3",
                    // Authors
                    BulkEditRow {
                        label: "Authors",
                        checked: apply_authors(),
                        on_toggle: move |()| {
                            if apply_authors() {
                                authors.write().clear();
                                apply_authors.set(false);
                            } else {
                                apply_authors.set(true);
                            }
                        },
                        {
                            let opts = picklist.read().as_ref().and_then(|r| r.as_ref().ok()).map(|p| p.authors.clone()).unwrap_or_default();
                            rsx! {
                                ChipInput {
                                    values: authors,
                                    options: opts,
                                    placeholder: "Add authors…".to_string(),
                                }
                            }
                        }
                    }

                    // Publisher
                    BulkEditRow {
                        label: "Publisher",
                        checked: apply_publisher(),
                        on_toggle: move |()| {
                            if apply_publisher() {
                                publisher.write().clear();
                                apply_publisher.set(false);
                            } else {
                                apply_publisher.set(true);
                            }
                        },
                        {
                            let opts = picklist.read().as_ref().and_then(|r| r.as_ref().ok()).map(|p| p.publishers.clone()).unwrap_or_default();
                            rsx! {
                                ChipInput {
                                    values: publisher,
                                    options: opts,
                                    placeholder: "Set publisher…".to_string(),
                                    max_chips: Some(1),
                                }
                            }
                        }
                    }

                    // Language
                    BulkEditRow {
                        label: "Language",
                        checked: apply_language(),
                        on_toggle: move |()| {
                            if apply_language() {
                                language.write().clear();
                                apply_language.set(false);
                            } else {
                                apply_language.set(true);
                            }
                        },
                        ChipInput {
                            values: language,
                            options: LANGUAGE_CODES.iter().map(std::string::ToString::to_string).collect(),
                            placeholder: "Set language…".to_string(),
                            max_chips: Some(1),
                        }
                    }

                    // Series Name
                    BulkEditRow {
                        label: "Series",
                        checked: apply_series(),
                        on_toggle: move |()| {
                            if apply_series() {
                                series_name.set(String::new());
                                apply_series.set(false);
                            } else {
                                apply_series.set(true);
                            }
                        },
                        {
                            let series_options = picklist.read().as_ref().and_then(|r| r.as_ref().ok())
                                .map(|p| p.series.iter().map(|s| (s.name.clone(), s.next_number)).collect::<Vec<_>>())
                                .unwrap_or_default();
                            rsx! {
                                AutocompleteInput {
                                    value: series_name,
                                    options: series_options,
                                    on_series_selected: move |_: (String, u32)| {},
                                    on_cleared: move |()| {},
                                    on_blur: move |_: String| {},
                                }
                            }
                        }
                    }

                    // Genres
                    BulkEditRow {
                        label: "Genres",
                        checked: apply_genres(),
                        on_toggle: move |()| {
                            if apply_genres() {
                                genres.write().clear();
                                apply_genres.set(false);
                            } else {
                                apply_genres.set(true);
                            }
                        },
                        {
                            let opts = picklist.read().as_ref().and_then(|r| r.as_ref().ok()).map(|p| p.genres.clone()).unwrap_or_default();
                            rsx! {
                                ChipInput {
                                    values: genres,
                                    options: opts,
                                    placeholder: "Add genres…".to_string(),
                                }
                            }
                        }
                    }

                    // Tags
                    BulkEditRow {
                        label: "Tags",
                        checked: apply_tags(),
                        on_toggle: move |()| {
                            if apply_tags() {
                                tags.write().clear();
                                apply_tags.set(false);
                            } else {
                                apply_tags.set(true);
                            }
                        },
                        {
                            let opts = picklist.read().as_ref().and_then(|r| r.as_ref().ok()).map(|p| p.tags.clone()).unwrap_or_default();
                            rsx! {
                                ChipInput {
                                    values: tags,
                                    options: opts,
                                    placeholder: "Add tags…".to_string(),
                                }
                            }
                        }
                    }

                    // Libraries — only rendered when there is at least one non-system library
                    if !available_libraries.is_empty() {
                        BulkEditRow {
                            label: "Libraries",
                            checked: apply_libraries(),
                            on_toggle: move |()| {
                                if apply_libraries() {
                                    library_tokens.write().clear();
                                    apply_libraries.set(false);
                                } else {
                                    apply_libraries.set(true);
                                }
                            },
                            {
                                rsx! {
                                    div { class: "flex flex-wrap gap-x-6 gap-y-1 pt-1",
                                        for (tok , name) in available_libraries.clone() {
                                            {
                                                let tok = tok.clone();
                                                let is_checked = library_tokens.read().contains(&tok);
                                                rsx! {
                                                    label {
                                                        key: "{tok}",
                                                        class: "flex items-center gap-1.5 text-sm text-gray-700 dark:text-slate-300 cursor-pointer select-none",
                                                        input {
                                                            r#type: "checkbox",
                                                            class: "rounded border-gray-300 dark:border-slate-600 text-indigo-600 dark:text-indigo-400",
                                                            checked: is_checked,
                                                            onchange: move |e| {
                                                                let mut cur = library_tokens.write();
                                                                if e.checked() {
                                                                    if !cur.contains(&tok) { cur.push(tok.clone()); }
                                                                } else {
                                                                    cur.retain(|t| t != &tok);
                                                                }
                                                                apply_libraries.set(true);
                                                            },
                                                        }
                                                        {name}
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Validation / error messages
                if authors_invalid {
                    div { class: "px-6 pt-2",
                        p { class: "text-sm text-amber-600 dark:text-amber-400", "Authors cannot be cleared — add at least one author or uncheck the field." }
                    }
                }
                if let Some(err) = error_msg() {
                    div { class: "px-6 pt-2",
                        p { class: "text-sm text-red-600 dark:text-red-400", {err} }
                    }
                }

                // Footer
                div { class: "flex justify-end gap-3 px-6 pt-4 pb-5",
                    button {
                        class: "px-4 py-2 text-sm font-medium rounded border border-gray-300 dark:border-slate-600 text-gray-700 dark:text-slate-300 hover:bg-gray-50 dark:hover:bg-slate-700 cursor-pointer disabled:opacity-40",
                        disabled: busy(),
                        onclick: move |_| on_close.call(()),
                        "Cancel"
                    }
                    button {
                        class: "px-4 py-2 text-sm font-medium rounded bg-indigo-600 text-white hover:bg-indigo-700 cursor-pointer disabled:opacity-40",
                        disabled: busy() || !any_checked || authors_invalid,
                        onclick: move |_| {
                            let fields = BulkEditFields {
                                authors: if apply_authors() { Some(authors.read().clone()) } else { None },
                                publisher: if apply_publisher() { Some(publisher.read().first().cloned().unwrap_or_default()) } else { None },
                                language: if apply_language() { Some(language.read().first().cloned().unwrap_or_default()) } else { None },
                                series_name: if apply_series() { Some(series_name.read().clone()) } else { None },
                                genres: if apply_genres() { Some(genres.read().clone()) } else { None },
                                tags: if apply_tags() { Some(tags.read().clone()) } else { None },
                                library_tokens: if apply_libraries() { Some(library_tokens.read().clone()) } else { None },
                            };
                            let tokens: Vec<String> = SELECTED_BOOKS.read().iter().cloned().collect();
                            busy.set(true);
                            error_msg.set(None);
                            spawn(async move {
                                match bulk_edit_metadata(tokens, fields).await {
                                    Ok(_) => {
                                        busy.set(false);
                                        on_saved.call(());
                                    }
                                    Err(e) => {
                                        error_msg.set(Some(format!("{e}")));
                                        busy.set(false);
                                    }
                                }
                            });
                        },
                        if busy() {
                            "Updating {count} books…"
                        } else {
                            "Apply Changes"
                        }
                    }
                }
            }
        }
    }
}

/// A single row in the bulk edit modal: checkbox + label + field content.
#[component]
fn BulkEditRow(label: &'static str, checked: bool, on_toggle: EventHandler<()>, children: Element) -> Element {
    rsx! {
        div { class: "flex items-start gap-3",
            // Checkbox
            button {
                r#type: "button",
                class: "mt-1.5 flex-none w-5 h-5 rounded border cursor-pointer flex items-center justify-center",
                class: if checked { "bg-indigo-600 border-indigo-600" } else { "border-gray-300 dark:border-slate-600 hover:border-indigo-400 dark:hover:border-indigo-500" },
                onclick: move |_| on_toggle.call(()),
                if checked {
                    svg {
                        class: "w-3.5 h-3.5 text-white",
                        xmlns: "http://www.w3.org/2000/svg",
                        fill: "none",
                        view_box: "0 0 24 24",
                        stroke_width: "3",
                        stroke: "currentColor",
                        path {
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            d: "M4.5 12.75l6 6 9-13.5",
                        }
                    }
                }
            }
            // Label
            span { class: "mt-1.5 flex-none w-20 text-sm font-medium text-gray-600 dark:text-slate-400", {label} }
            // Field — when unchecked, render a placeholder instead of children
            // so any open ChipInput/AutocompleteInput dropdowns are unmounted.
            div { class: "flex-1 min-w-0",
                if checked {
                    {children}
                } else {
                    div { class: "border border-gray-300 dark:border-slate-600 bg-white dark:bg-slate-700 rounded px-2 py-1 min-h-[34px] opacity-50" }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// BulkDeleteModal — confirmation dialog for deleting multiple books
// ---------------------------------------------------------------------------

#[component]
fn BulkDeleteModal(on_close: EventHandler<()>, on_deleted: EventHandler<()>) -> Element {
    let count = selection_count();
    let mut busy = use_signal(|| false);
    let mut error_msg: Signal<Option<String>> = use_signal(|| None);

    let label = if count == 1 {
        "This will permanently delete 1 book and all its files. This cannot be undone.".to_string()
    } else {
        format!("This will permanently delete {count} books and all their files. This cannot be undone.")
    };

    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/40",
            tabindex: -1,
            onmounted: move |e| async move { let _ = e.set_focus(true).await; },
            onclick: move |_| if !busy() { on_close.call(()); },
            onkeydown: move |e| { if e.key() == Key::Escape && !busy() { on_close.call(()); } },

            div {
                class: "bg-white dark:bg-slate-800 rounded-lg shadow-xl p-6 w-full max-w-sm mx-4",
                onclick: |e| e.stop_propagation(),

                h2 { class: "text-lg font-semibold text-gray-900 dark:text-slate-100 mb-2",
                    if count == 1 { "Delete Book?" } else { "Delete Books?" }
                }
                p { class: "text-sm text-gray-600 dark:text-slate-400 mb-6", {label} }

                if let Some(err) = error_msg() {
                    p { class: "text-sm text-red-600 dark:text-red-400 mb-4", {err} }
                }

                div { class: "flex gap-3 justify-end",
                    button {
                        class: "px-4 py-2 text-sm font-medium rounded border border-gray-300 dark:border-slate-600 text-gray-700 dark:text-slate-300 hover:bg-gray-50 dark:hover:bg-slate-700 cursor-pointer",
                        autofocus: true,
                        disabled: busy(),
                        onclick: move |_| on_close.call(()),
                        "No, Keep Them"
                    }
                    button {
                        class: "px-4 py-2 text-sm font-medium rounded bg-red-600 text-white hover:bg-red-700 disabled:opacity-50 cursor-pointer",
                        disabled: busy(),
                        onclick: move |_| {
                            let tokens: Vec<String> = SELECTED_BOOKS.read().iter().cloned().collect();
                            busy.set(true);
                            error_msg.set(None);
                            spawn(async move {
                                match bulk_delete_books(tokens).await {
                                    Ok(_) => {
                                        busy.set(false);
                                        on_deleted.call(());
                                    }
                                    Err(e) => {
                                        error_msg.set(Some(format!("{e}")));
                                        busy.set(false);
                                    }
                                }
                            });
                        },
                        if busy() {
                            {format!("Deleting {count} books…")}
                        } else {
                            "Yes, Delete"
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Server function: check EditBook capability
// ---------------------------------------------------------------------------

#[get(
    "/api/v1/books/bulk/can-edit",
    auth_session: axum::Extension<AuthSession>
)]
async fn check_edit_book_capability() -> Result<bool, ServerFnError> {
    let current_user = auth_session.current_user.clone().unwrap_or_default();
    Ok(Auth::<AuthUser, UserId, BackendSessionPool>::build([Method::GET], true)
        .requires(Rights::any([Rights::permission(Capability::EditBook.as_str())]))
        .validate(&current_user, &Method::GET, None)
        .await)
}

// ---------------------------------------------------------------------------
// Server function: list non-system libraries for bulk add target
// ---------------------------------------------------------------------------

#[get(
    "/api/v1/books/bulk/libraries",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn list_bulk_target_libraries() -> Result<Vec<(String, String)>, ServerFnError> {
    require_capability(&auth_session, Capability::EditBook, Method::GET).await?;
    let all = core_services.library_service.list_libraries().await.map_err(to_server_err)?;
    Ok(all
        .into_iter()
        .filter(|e| !e.library.is_system)
        .map(|e| (e.library.token.to_string(), e.library.name))
        .collect())
}

// ---------------------------------------------------------------------------
// Server function: bulk add books to library
// ---------------------------------------------------------------------------

#[post(
    "/api/v1/books/bulk/add-to-library",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn bulk_add_books_to_library(book_tokens: Vec<String>, library_token: String) -> Result<u32, ServerFnError> {
    require_capability(&auth_session, Capability::EditBook, Method::POST).await?;

    let lib_token = LibraryToken::from_str(&library_token).map_err(|_| ServerFnError::new("Invalid library token"))?;

    let mut added = 0u32;
    for token_str in &book_tokens {
        let book_token = BookToken::from_str(token_str).map_err(|_| ServerFnError::new(format!("Invalid book token: {token_str}")))?;
        match core_services.library_service.add_book_to_library(lib_token, book_token).await {
            Ok(()) => added += 1,
            Err(e) => tracing::warn!(book_token = %token_str, error = %e, "bulk add to library failed for book"),
        }
    }
    Ok(added)
}

// ---------------------------------------------------------------------------
// SelectionActionBar — fixed bottom bar with actions for selected books
// ---------------------------------------------------------------------------

#[component]
pub(crate) fn SelectionActionBar(
    /// Tokens of all books currently rendered in the grid (for Select All).
    all_book_tokens: Vec<String>,
    /// Called after a bulk operation completes so the parent can refresh data.
    #[props(default)]
    on_action: EventHandler<()>,
) -> Element {
    let mode = SELECTION_MODE();
    let count = selection_count();
    let mut show_status_dropdown = use_signal(|| false);
    let mut show_bulk_edit = use_signal(|| false);
    let mut show_bulk_delete = use_signal(|| false);
    let mut show_add_to_library = use_signal(|| false);
    let mut busy = use_signal(|| false);
    let mut status_message = use_signal(|| None::<String>);
    let available_libraries = use_resource(list_bulk_target_libraries);

    // Check if user has EditBook capability (for showing Edit Metadata button).
    let can_edit_resource = use_server_future(check_edit_book_capability);
    let can_edit = can_edit_resource.ok().and_then(|r| r()).and_then(std::result::Result::ok).unwrap_or(false);

    // Check if user has DeleteBook capability (for showing Delete button).
    let can_delete_resource = use_server_future(check_delete_book_capability);
    let can_delete = can_delete_resource.ok().and_then(|r| r()).and_then(std::result::Result::ok).unwrap_or(false);

    let has_user_libraries = available_libraries
        .read()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .is_some_and(|libs| !libs.is_empty());

    if !mode {
        return rsx! {};
    }

    let label = match count {
        0 => "No books selected".to_string(),
        1 => "1 book selected".to_string(),
        _ => format!("{count} books selected"),
    };

    let has_selection = count > 0;

    let all_selected = {
        let selected = SELECTED_BOOKS.read();
        !all_book_tokens.is_empty() && all_book_tokens.iter().all(|t| selected.contains(t))
    };

    rsx! {
        div {
            class: "fixed bottom-0 left-0 right-0 z-40 bg-white dark:bg-slate-800 border-t border-gray-200 dark:border-slate-700 shadow-lg px-6 py-3 flex items-center gap-4",
            tabindex: -1,
            onmounted: move |e| async move { let _ = e.set_focus(true).await; },
            onkeydown: {
                let tokens_for_shortcut = all_book_tokens.clone();
                move |e: KeyboardEvent| {
                    // Don't handle shortcuts when modal is open or busy
                    if show_bulk_edit() || show_bulk_delete() || show_add_to_library() || busy() { return; }
                    match e.key() {
                        Key::Escape => {
                            // Close status dropdown if open, otherwise exit selection mode
                            if show_status_dropdown() {
                                show_status_dropdown.set(false);
                            } else {
                                exit_selection_mode();
                            }
                        }
                        Key::Character(ref c) if c == "a" && (e.modifiers().meta() || e.modifiers().ctrl()) => {
                            e.prevent_default();
                            if all_selected {
                                deselect_all();
                            } else {
                                select_all(tokens_for_shortcut.clone());
                            }
                        }
                        _ => {}
                    }
                }
            },

            // Selection count or status message
            if let Some(msg) = status_message() {
                span { class: "text-sm font-medium text-indigo-600 dark:text-indigo-400", {msg} }
            } else {
                span { class: "text-sm font-medium text-gray-700 dark:text-slate-300", {label} }
            }

            // Select All / Deselect All toggle
            button {
                class: "text-sm text-indigo-600 dark:text-indigo-400 hover:text-indigo-800 dark:hover:text-indigo-300 font-medium cursor-pointer disabled:opacity-40",
                disabled: busy(),
                onclick: {
                    let tokens = all_book_tokens.clone();
                    move |_| {
                        if all_selected {
                            deselect_all();
                        } else {
                            select_all(tokens.clone());
                        }
                    }
                },
                if all_selected { "Deselect All" } else { "Select All" }
            }

            // Select None — clear selections but stay in selection mode
            // Hidden when all are selected since "Deselect All" already covers that.
            if has_selection && !all_selected {
                button {
                    class: "text-sm text-indigo-600 dark:text-indigo-400 hover:text-indigo-800 dark:hover:text-indigo-300 font-medium cursor-pointer disabled:opacity-40",
                    disabled: busy(),
                    onclick: move |_| deselect_all(),
                    "Select None"
                }
            }

            // Spacer
            div { class: "flex-1" }

            // Edit Metadata (only shown if user has EditBook capability)
            if can_edit {
                button {
                    class: "px-4 py-2 text-sm font-medium rounded bg-indigo-600 text-white hover:bg-indigo-700 cursor-pointer disabled:opacity-40",
                    disabled: busy() || !has_selection,
                    onclick: move |_| show_bulk_edit.set(true),
                    "Edit Metadata"
                }
            }

            // Add to Library (only shown if user has EditBook capability and user libraries exist)
            if can_edit && has_user_libraries {
                div { class: "relative",
                    button {
                        class: "px-4 py-2 text-sm font-medium rounded bg-indigo-600 text-white hover:bg-indigo-700 cursor-pointer disabled:opacity-40",
                        disabled: busy() || !has_selection,
                        onclick: move |_| show_add_to_library.set(!show_add_to_library()),
                        "Add to Library"
                    }
                    if show_add_to_library() {
                        div { class: "absolute bottom-full right-0 mb-1 z-50 bg-white dark:bg-slate-800 rounded-lg shadow-lg border border-gray-200 dark:border-slate-700 py-1 min-w-max",
                            match available_libraries.read().as_ref() {
                                None => rsx! { div { class: "px-4 py-2 text-sm text-gray-500 dark:text-slate-400", "Loading…" } },
                                Some(Err(_)) => rsx! { div { class: "px-4 py-2 text-sm text-red-500 dark:text-red-400", "Failed to load libraries" } },
                                Some(Ok(libs)) if libs.is_empty() => rsx! { div { class: "px-4 py-2 text-sm text-gray-500 dark:text-slate-400", "No libraries available" } },
                                Some(Ok(libs)) => rsx! {
                                    for (token, name) in libs.iter() {
                                        {
                                            let t = token.clone();
                                            let n = name.clone();
                                            rsx! {
                                                button {
                                                    class: "block w-full text-left px-4 py-2 text-sm text-gray-700 dark:text-slate-300 hover:bg-gray-50 dark:hover:bg-slate-700 cursor-pointer",
                                                    onclick: move |_| {
                                                        let lib_token = t.clone();
                                                        let tokens: Vec<String> = SELECTED_BOOKS.read().iter().cloned().collect();
                                                        let count = tokens.len();
                                                        show_add_to_library.set(false);
                                                        busy.set(true);
                                                        status_message.set(Some(format!("Adding {count} books to library…")));
                                                        spawn(async move {
                                                            match bulk_add_books_to_library(tokens.clone(), lib_token).await {
                                                                Ok(n) => {
                                                                    busy.set(false);
                                                                    let selected_count = tokens.len() as u32;
                                                                    if n < selected_count {
                                                                        status_message.set(Some(format!("Added {n} of {selected_count} books (some may already be in this library or were not found)")));
                                                                        on_action.call(());
                                                                    } else {
                                                                        status_message.set(None);
                                                                        on_action.call(());
                                                                        exit_selection_mode();
                                                                    }
                                                                }
                                                                Err(e) => {
                                                                    status_message.set(Some(format!("Error: {e}")));
                                                                    busy.set(false);
                                                                }
                                                            }
                                                        });
                                                    },
                                                    {n}
                                                }
                                            }
                                        }
                                    }
                                },
                            }
                        }
                    }
                }
            }

            // Set Status with dropdown
            div { class: "relative",
                button {
                    class: "px-4 py-2 text-sm font-medium rounded bg-indigo-600 text-white hover:bg-indigo-700 cursor-pointer disabled:opacity-40",
                    disabled: busy() || !has_selection,
                    onclick: move |_| show_status_dropdown.set(!show_status_dropdown()),
                    "Set Status"
                }
                if show_status_dropdown() {
                    div { class: "absolute bottom-full left-0 mb-1 z-50 bg-white dark:bg-slate-800 rounded-lg shadow-lg border border-gray-200 dark:border-slate-700 py-1 min-w-max",
                        for s in ["Unread", "Reading", "Paused", "Rereading", "Read", "Abandoned"] {
                            {
                                let s_owned = s.to_string();
                                rsx! {
                                    button {
                                        class: "block w-full text-left px-4 py-2 text-sm text-gray-700 dark:text-slate-300 hover:bg-gray-50 dark:hover:bg-slate-700 cursor-pointer",
                                        onclick: move |_| {
                                            let status_str = s_owned.clone();
                                            let tokens: Vec<String> = SELECTED_BOOKS.read().iter().cloned().collect();
                                            let count = tokens.len();
                                            show_status_dropdown.set(false);
                                            busy.set(true);
                                            status_message.set(Some(format!("Updating {count} books…")));
                                            spawn(async move {
                                                match bulk_set_reading_status(tokens, status_str).await {
                                                    Ok(_) => {
                                                        busy.set(false);
                                                        status_message.set(None);
                                                        on_action.call(());
                                                        exit_selection_mode();
                                                    }
                                                    Err(e) => {
                                                        status_message.set(Some(format!("Error: {e}")));
                                                        busy.set(false);
                                                    }
                                                }
                                            });
                                        },
                                        {s}
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Delete (only shown if user has DeleteBook capability)
            if can_delete {
                button {
                    class: "px-4 py-2 text-sm font-medium rounded bg-red-600 text-white hover:bg-red-700 cursor-pointer disabled:opacity-40",
                    disabled: busy() || !has_selection,
                    onclick: move |_| show_bulk_delete.set(true),
                    "Delete"
                }
            }

            // Cancel
            button {
                class: "px-4 py-2 text-sm font-medium rounded border border-gray-300 dark:border-slate-600 text-gray-700 dark:text-slate-300 hover:bg-gray-50 dark:hover:bg-slate-700 cursor-pointer disabled:opacity-40",
                disabled: busy(),
                onclick: move |_| exit_selection_mode(),
                "Cancel"
            }
        }

        // Bulk edit modal
        if show_bulk_edit() {
            BulkEditModal {
                on_close: move |()| show_bulk_edit.set(false),
                on_saved: move |()| {
                    show_bulk_edit.set(false);
                    exit_selection_mode();
                    on_action.call(());
                },
            }
        }

        // Bulk delete confirmation modal
        if show_bulk_delete() {
            BulkDeleteModal {
                on_close: move |()| show_bulk_delete.set(false),
                on_deleted: move |()| {
                    show_bulk_delete.set(false);
                    exit_selection_mode();
                    on_action.call(());
                },
            }
        }
    }
}
