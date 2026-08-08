use base64::{Engine, engine::general_purpose::STANDARD as B64};
use dioxus::{html::HasFileData, prelude::*};

use super::{
    server::{
        accept_incoming_provider_cover, accept_library_provider_cover, approve_book, fetch_provider_for_edit, fetch_provider_metadata, get_book_libraries,
        get_picklist_data, list_non_system_libraries, reject_review_book, save_library_book, set_book_libraries, stage_incoming_cover, stage_library_cover,
    },
    types::{BookEditFields, BookReviewData, IdentifierMap, ProviderResult},
};
use crate::components::{AutocompleteInput, ChipInput, LANGUAGE_CODES};

const ALL_IDENTIFIER_TYPES: &[(&str, &str)] = &[
    ("Isbn13", "ISBN-13"),
    ("Isbn10", "ISBN-10"),
    ("Asin", "ASIN"),
    ("GoogleBooks", "Google Books"),
    ("OpenLibrary", "Open Library"),
    ("Hardcover", "Hardcover"),
];

// ── ReviewEditor sub-component
// ────────────────────────────────────────────────

#[component]
pub(crate) fn ReviewEditor(data: BookReviewData, edit_mode: bool, on_back: EventHandler<()>) -> Element {
    // ── Edit state ────────────────────────────────────────────────────────────
    let mut title = use_signal(|| data.title.clone());
    let mut description = use_signal(|| data.description.clone());
    let mut published_date = use_signal(|| data.published_date.clone());
    let mut language = use_signal(|| if data.language.is_empty() { vec![] } else { vec![data.language.clone()] });
    let mut series_name = use_signal(|| data.series_name.clone());
    let mut series_number = use_signal(|| data.series_number.clone());
    let mut publisher = use_signal(|| {
        if data.publisher_name.is_empty() {
            vec![]
        } else {
            vec![data.publisher_name.clone()]
        }
    });
    let mut page_count = use_signal(|| data.page_count.clone());
    let mut authors = use_signal(|| data.authors.clone());
    let genres = use_signal(|| data.genres.clone());
    let tags = use_signal(|| data.tags.clone());

    // ── Library membership state ──────────────────────────────────────────────
    // All non-system libraries: Vec<(token, name)>
    let non_system_libraries = use_resource(list_non_system_libraries);
    // For edit_mode: fetch book's current library memberships. In review mode
    // we skip the server round-trip by returning an empty vec immediately.
    let book_token_for_libs = data.book_token.clone();
    let book_existing_libraries = use_resource(move || {
        let token = book_token_for_libs.clone();
        async move { if edit_mode { get_book_libraries(token).await } else { Ok(vec![]) } }
    });
    // Checked library tokens (controlled by checkboxes). None = not yet
    // initialised.
    let mut checked_library_tokens: Signal<Option<Vec<String>>> = use_signal(|| None);

    // Initialise checked_library_tokens once resource data arrives (runs once per
    // render until initialised, then becomes a no-op because it won't be None
    // any more).
    {
        let nsl = non_system_libraries.read();
        let bel = book_existing_libraries.read();
        if checked_library_tokens.read().is_none()
            && let (Some(Ok(libs)), Some(bel_res)) = (nsl.as_ref(), bel.as_ref())
        {
            let init: Vec<String> = if edit_mode {
                // Pre-check the libraries the book already belongs to.
                match bel_res {
                    Ok(existing) => existing.clone(),
                    Err(_) => vec![],
                }
            } else {
                // Review mode: all non-system libraries checked by default.
                libs.iter().map(|(token, _)| token.clone()).collect()
            };
            checked_library_tokens.set(Some(init));
        }
    }

    // ── Pick-list data (loads client-side after hydration) ────────────────────
    let picklist = use_resource(move || get_picklist_data(()));
    let mut identifiers: Signal<IdentifierMap> = use_signal(|| data.identifiers.clone());
    let mut use_fetched_cover = use_signal(|| false);
    let cover_url = format!("/api/v1/covers/{}?v={}&full=true", data.book_token, data.updated_at);
    let mut current_cover = use_signal(|| cover_url.clone());
    let mut current_cover_dimensions: Signal<Option<(u32, u32)>> = use_signal(|| data.cover_dimensions);
    let mut cover_drag_over = use_signal(|| false);

    // ── Provider fetch state ──────────────────────────────────────────────────
    let mut provider_result: Signal<Option<ProviderResult>> = use_signal(|| None);
    let mut fetching: Signal<Option<String>> = use_signal(|| None); // provider name being fetched
    let mut action_busy = use_signal(|| false);
    let mut error_msg: Signal<Option<String>> = use_signal(|| None);

    let job_token = data.job_token.clone();
    let book_token_for_edit = data.book_token.clone();
    // cover_key identifies the temp cover file: job token for review, book token
    // for edit.
    let cover_key = if edit_mode { data.book_token.clone() } else { data.job_token.clone() };
    let original_missing = data.original_missing;

    rsx! {
        div { class: "flex-1 flex flex-col overflow-hidden",
            // ── Header ────────────────────────────────────────────────────────
            div { class: "px-6 py-4 border-b border-gray-200 dark:border-slate-700 flex items-center justify-between dark:bg-slate-800",
                div { class: "flex items-center gap-4",
                    button {
                        class: "text-sm text-indigo-600 dark:text-indigo-400 hover:text-indigo-800 dark:hover:text-indigo-300 cursor-pointer",
                        onclick: move |_| on_back.call(()),
                        if edit_mode { "← Book" } else { "← Incoming" }
                    }
                    h1 { class: "text-xl font-semibold text-gray-900 dark:text-slate-100",
                        if edit_mode { "Edit Metadata" } else { "Review Book" }
                    }
                }
                // ── Action buttons ────────────────────────────────────────────
                div { class: "flex items-center gap-3",
                    {
                        let is_busy = *action_busy.read();
                        let cancel_class = if is_busy {
                            "px-4 py-2 rounded border border-gray-300 dark:border-slate-600 text-sm font-medium text-gray-500 dark:text-slate-400 opacity-40 cursor-not-allowed"
                        } else {
                            "px-4 py-2 rounded border border-gray-300 dark:border-slate-600 text-sm font-medium text-gray-600 dark:text-slate-300 hover:bg-gray-50 dark:hover:bg-slate-700 cursor-pointer"
                        };
                        rsx! {
                            button {
                                class: cancel_class,
                                disabled: is_busy,
                                onclick: move |_| on_back.call(()),
                                "Cancel"
                            }
                        }
                    }
                    if !edit_mode {
                        {
                            let is_busy = *action_busy.read();
                            let reject_class = if is_busy {
                                "px-4 py-2 rounded border border-red-300 text-sm font-medium text-red-600 opacity-40 cursor-not-allowed"
                            } else {
                                "px-4 py-2 rounded border border-red-300 text-sm font-medium text-red-600 hover:bg-red-50 cursor-pointer"
                            };
                            let jt = job_token.clone();
                            rsx! {
                                button {
                                    class: reject_class,
                                    disabled: is_busy,
                                    onclick: move |_| {
                                        let jt = jt.clone();
                                        action_busy.set(true);
                                        error_msg.set(None);
                                        spawn(async move {
                                            match reject_review_book(jt).await {
                                                Ok(()) => on_back.call(()),
                                                Err(e) => {
                                                    error_msg.set(Some(e.to_string()));
                                                    action_busy.set(false);
                                                }
                                            }
                                        });
                                    },
                                    "Reject"
                                }
                            }
                        }
                    }
                    {
                        let is_busy = *action_busy.read();
                        let approve_disabled = is_busy || (original_missing && !edit_mode);
                        let primary_class = if approve_disabled {
                            "px-4 py-2 rounded bg-indigo-400 text-sm font-medium text-white cursor-not-allowed"
                        } else {
                            "px-4 py-2 rounded bg-indigo-600 text-sm font-medium text-white hover:bg-indigo-700 cursor-pointer"
                        };
                        let jt = job_token.clone();
                        let bk = book_token_for_edit.clone();
                        rsx! {
                            button {
                                class: primary_class,
                                disabled: approve_disabled,
                                onclick: move |_| {
                                    let fields = BookEditFields {
                                        job_token: jt.clone(),
                                        title: title.read().clone(),
                                        description: description.read().clone(),
                                        published_date: published_date.read().clone(),
                                        language: language.read().first().cloned().unwrap_or_default(),
                                        series_name: series_name.read().clone(),
                                        series_number: series_number.read().clone(),
                                        publisher_name: publisher.read().first().cloned().unwrap_or_default(),
                                        page_count: page_count.read().clone(),
                                        authors: authors.read().clone(),
                                        genres: genres.read().clone(),
                                        tags: tags.read().clone(),
                                        identifiers: identifiers.read().clone(),
                                        use_fetched_cover: *use_fetched_cover.read(),
                                    };
                                    action_busy.set(true);
                                    error_msg.set(None);
                                    let bk = bk.clone();
                                    let lib_tokens = checked_library_tokens.read().clone().unwrap_or_default();
                                    spawn(async move {
                                        let result = if edit_mode {
                                            save_library_book(bk.clone(), fields).await
                                        } else {
                                            approve_book(fields).await
                                        };
                                        match result {
                                            Ok(()) => {
                                                // Sync library memberships (best-effort — don't fail
                                                // the whole save if this errors).
                                                let _ = set_book_libraries(bk, lib_tokens).await;
                                                on_back.call(());
                                            }
                                            Err(e) => {
                                                error_msg.set(Some(e.to_string()));
                                                action_busy.set(false);
                                            }
                                        }
                                    });
                                },
                                if edit_mode {
                                    if *action_busy.read() { "Saving…" } else { "Save" }
                                } else {
                                    if *action_busy.read() { "Approving…" } else { "Approve" }
                                }
                            }
                        }
                    }
                }
            }

            // ── Missing original banner ───────────────────────────────────────
            if original_missing && !edit_mode {
                div { class: "mx-6 mt-3 px-4 py-3 bg-red-50 dark:bg-red-900/30 border border-red-300 dark:border-red-700 rounded text-sm text-red-800 dark:text-red-300 font-medium",
                    "The original file for this book is missing from disk. This book cannot be approved — only rejection is available."
                }
            }

            // ── Error banner ──────────────────────────────────────────────────
            if let Some(err) = error_msg.read().clone() {
                div { class: "mx-6 mt-3 px-4 py-2 bg-red-50 dark:bg-red-900/30 border border-red-200 dark:border-red-700 rounded text-sm text-red-700 dark:text-red-300",
                    {err}
                }
            }

            // ── 3-column metadata table ───────────────────────────────────────
            div { class: "flex-1 overflow-auto px-6 pb-6 dark:bg-slate-900",
                table { class: "w-full text-sm table-fixed",
                    thead {
                        tr { class: "border-b border-gray-200 dark:border-slate-700",
                            th { class: "py-2 pr-4 text-left text-xs font-medium text-gray-500 dark:text-slate-400 uppercase tracking-wide w-36", "Field" }
                            th { class: "py-2 pr-4 text-left text-xs font-medium text-gray-500 dark:text-slate-400 uppercase tracking-wide w-[46%]", "Current" }
                            th { class: "py-2 pr-4 w-8" }
                            th { class: "py-2 text-left text-xs font-medium text-gray-500 dark:text-slate-400 uppercase tracking-wide w-[46%]",
                                div { class: "flex items-center gap-2",
                                    span { "Search" }
                                    for pname in data.provider_names.clone() {
                                        {
                                            let pname = pname.clone();
                                            let ck = cover_key.clone();
                                            let is_fetching_this =
                                                fetching.read().as_deref() == Some(pname.as_str());
                                            let is_busy_any =
                                                fetching.read().is_some() || *action_busy.read();
                                            let btn_class = if is_busy_any {
                                                "px-2 py-0.5 rounded border border-indigo-200 dark:border-indigo-700 text-xs font-medium text-indigo-400 dark:text-indigo-500 normal-case opacity-50 cursor-not-allowed tracking-normal"
                                            } else {
                                                "px-2 py-0.5 rounded border border-indigo-300 dark:border-indigo-600 text-xs font-medium text-indigo-600 dark:text-indigo-400 normal-case hover:bg-indigo-50 dark:hover:bg-indigo-900/30 cursor-pointer tracking-normal"
                                            };
                                            rsx! {
                                                button {
                                                    key: "{pname}",
                                                    class: btn_class,
                                                    disabled: is_busy_any,
                                                    onclick: move |_| {
                                                        let pname = pname.clone();
                                                        let ck = ck.clone();
                                                        fetching.set(Some(pname.clone()));
                                                        error_msg.set(None);
                                                        let current_ids = identifiers.read().clone();
                                                        let current_title = title.read().clone();
                                                        let current_authors = authors.read().clone();
                                                        spawn(async move {
                                                            let result = if edit_mode {
                                                                fetch_provider_for_edit(
                                                                    ck,
                                                                    pname.clone(),
                                                                    current_title,
                                                                    current_authors,
                                                                    current_ids,
                                                                )
                                                                .await
                                                            } else {
                                                                fetch_provider_metadata(
                                                                    ck,
                                                                    pname.clone(),
                                                                    current_title,
                                                                    current_authors,
                                                                    current_ids,
                                                                )
                                                                .await
                                                            };
                                                            match result {
                                                                Ok(r) => provider_result.set(r),
                                                                Err(e) => {
                                                                    error_msg.set(Some(e.to_string()));
                                                                    provider_result.set(None);
                                                                }
                                                            }
                                                            fetching.set(None);
                                                        });
                                                    },
                                                    if is_fetching_this {
                                                        "{pname}…"
                                                    } else {
                                                        {pname.clone()}
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    if fetching.read().is_some() {
                                        span { class: "text-xs text-gray-400 dark:text-slate-500 normal-case font-normal tracking-normal", "Fetching…" }
                                    }
                                }
                            }
                        }
                    }
                    tbody { class: "divide-y divide-gray-100 dark:divide-slate-700",

                        // Title
                        tr {
                            td { class: "py-2 pr-4 text-gray-500 dark:text-slate-400 font-medium whitespace-nowrap", "Title" }
                            td { class: "py-2 pr-4",
                                input {
                                    class: "w-full border border-gray-300 dark:border-slate-600 rounded px-2 py-1 text-sm text-gray-900 dark:text-slate-100 bg-white dark:bg-slate-700 focus:outline-none focus:ring-1 focus:ring-indigo-400",
                                    value: "{title}",
                                    oninput: move |e| title.set(e.value()),
                                }
                            }
                            td { class: "py-2 pr-4 text-center",
                                if provider_result.read().is_some() {
                                    {
                                        let pv = provider_result.read().as_ref().map(|r| r.title.clone()).unwrap_or_default();
                                        rsx! {
                                            button {
                                                class: "text-indigo-500 hover:text-indigo-700 cursor-pointer text-xs font-bold",
                                                title: "Copy from provider",
                                                onclick: move |_| title.set(pv.clone()),
                                                "←"
                                            }
                                        }
                                    }
                                }
                            }
                            td { class: "py-2 text-gray-600 dark:text-slate-300",
                                if let Some(pr) = provider_result.read().as_ref() {
                                    "{pr.title}"
                                }
                            }
                        }

                        // Authors
                        tr {
                            td { class: "py-2 pr-4 text-gray-500 dark:text-slate-400 font-medium whitespace-nowrap align-top pt-2", "Authors" }
                            td { class: "py-2 pr-4",
                                {
                                    let picklist_ref = picklist.read();
                                    let author_options = picklist_ref
                                        .as_ref()
                                        .and_then(|r| r.as_ref().ok())
                                        .map(|p| p.authors.clone())
                                        .unwrap_or_default();
                                    rsx! {
                                        ChipInput {
                                            values: authors,
                                            options: author_options,
                                            placeholder: "Add author…".to_string(),
                                        }
                                    }
                                }
                            }
                            td { class: "py-2 pr-4 text-center align-top pt-2",
                                if provider_result.read().is_some() {
                                    {
                                        let pv = provider_result.read().as_ref().map(|r| r.authors.clone()).unwrap_or_default();
                                        rsx! {
                                            button {
                                                class: "text-indigo-500 hover:text-indigo-700 cursor-pointer text-xs font-bold",
                                                title: "Copy from provider",
                                                onclick: move |_| authors.set(pv.clone()),
                                                "←"
                                            }
                                        }
                                    }
                                }
                            }
                            td { class: "py-2 text-gray-600 dark:text-slate-300",
                                if let Some(pr) = provider_result.read().as_ref() {
                                    "{pr.authors.join(\", \")}"
                                }
                            }
                        }

                        // Description
                        tr {
                            td { class: "py-2 pr-4 text-gray-500 dark:text-slate-400 font-medium whitespace-nowrap align-top pt-3", "Description" }
                            td { class: "py-2 pr-4",
                                textarea {
                                    class: "w-full border border-gray-300 dark:border-slate-600 rounded px-2 py-1 text-sm text-gray-900 dark:text-slate-100 bg-white dark:bg-slate-700 focus:outline-none focus:ring-1 focus:ring-indigo-400 resize-y overflow-y-auto",
                                    rows: "20",
                                    value: "{description}",
                                    oninput: move |e| description.set(e.value()),
                                }
                            }
                            td { class: "py-2 pr-4 text-center align-top pt-3",
                                if provider_result.read().is_some() {
                                    {
                                        let pv = provider_result.read().as_ref().map(|r| r.description.clone()).unwrap_or_default();
                                        rsx! {
                                            button {
                                                class: "text-indigo-500 hover:text-indigo-700 cursor-pointer text-xs font-bold",
                                                title: "Copy from provider",
                                                onclick: move |_| description.set(pv.clone()),
                                                "←"
                                            }
                                        }
                                    }
                                }
                            }
                            td { class: "py-2 text-gray-600 dark:text-slate-300 text-xs max-w-xs overflow-hidden",
                                if let Some(pr) = provider_result.read().as_ref() {
                                    "{pr.description}"
                                }
                            }
                        }

                        // Publisher
                        tr {
                            td { class: "py-2 pr-4 text-gray-500 dark:text-slate-400 font-medium whitespace-nowrap", "Publisher" }
                            td { class: "py-2 pr-4",
                                {
                                    let picklist_ref = picklist.read();
                                    let publisher_options = picklist_ref
                                        .as_ref()
                                        .and_then(|r| r.as_ref().ok())
                                        .map(|p| p.publishers.clone())
                                        .unwrap_or_default();
                                    rsx! {
                                        ChipInput {
                                            values: publisher,
                                            options: publisher_options,
                                            placeholder: "Add publisher…".to_string(),
                                            max_chips: Some(1),
                                        }
                                    }
                                }
                            }
                            td { class: "py-2 pr-4 text-center",
                                if provider_result.read().is_some() {
                                    {
                                        let pv = provider_result.read().as_ref().map(|r| r.publisher_name.clone()).unwrap_or_default();
                                        rsx! {
                                            button {
                                                class: "text-indigo-500 hover:text-indigo-700 cursor-pointer text-xs font-bold",
                                                title: "Copy from provider",
                                                onclick: move |_| {
                                                    let name = pv.trim().to_string();
                                                    if !name.is_empty() {
                                                        publisher.write().clear();
                                                        publisher.write().push(name);
                                                    }
                                                },
                                                "←"
                                            }
                                        }
                                    }
                                }
                            }
                            td { class: "py-2 text-gray-600 dark:text-slate-300",
                                if let Some(pr) = provider_result.read().as_ref() {
                                    "{pr.publisher_name}"
                                }
                            }
                        }

                        // Published year
                        tr {
                            td { class: "py-2 pr-4 text-gray-500 dark:text-slate-400 font-medium whitespace-nowrap", "Published" }
                            td { class: "py-2 pr-4",
                                input {
                                    r#type: "number",
                                    class: "w-32 border border-gray-300 dark:border-slate-600 rounded px-2 py-1 text-sm text-gray-900 dark:text-slate-100 bg-white dark:bg-slate-700 focus:outline-none focus:ring-1 focus:ring-indigo-400",
                                    value: "{published_date}",
                                    placeholder: "YYYY",
                                    oninput: move |e| published_date.set(e.value()),
                                }
                            }
                            td { class: "py-2 pr-4 text-center",
                                if provider_result.read().is_some() {
                                    {
                                        let pv = provider_result.read().as_ref().map(|r| r.published_date.clone()).unwrap_or_default();
                                        rsx! {
                                            button {
                                                class: "text-indigo-500 hover:text-indigo-700 cursor-pointer text-xs font-bold",
                                                title: "Copy from provider",
                                                onclick: move |_| published_date.set(pv.clone()),
                                                "←"
                                            }
                                        }
                                    }
                                }
                            }
                            td { class: "py-2 text-gray-600 dark:text-slate-300",
                                if let Some(pr) = provider_result.read().as_ref() {
                                    "{pr.published_date}"
                                }
                            }
                        }

                        // Language
                        tr {
                            td { class: "py-2 pr-4 text-gray-500 dark:text-slate-400 font-medium whitespace-nowrap", "Language" }
                            td { class: "py-2 pr-4",
                                ChipInput {
                                    values: language,
                                    options: LANGUAGE_CODES.iter().map(std::string::ToString::to_string).collect(),
                                    placeholder: "Add language…".to_string(),
                                    max_chips: Some(1),
                                }
                            }
                            td { class: "py-2 pr-4 text-center",
                                if provider_result.read().is_some() {
                                    {
                                        let pv = provider_result.read().as_ref().map(|r| r.language.clone()).unwrap_or_default();
                                        rsx! {
                                            button {
                                                class: "text-indigo-500 hover:text-indigo-700 cursor-pointer text-xs font-bold",
                                                title: "Copy from provider",
                                                onclick: move |_| {
                                                    let code = pv.trim().to_string();
                                                    if !code.is_empty() {
                                                        language.write().clear();
                                                        language.write().push(code);
                                                    }
                                                },
                                                "←"
                                            }
                                        }
                                    }
                                }
                            }
                            td { class: "py-2 text-gray-600 dark:text-slate-300",
                                if let Some(pr) = provider_result.read().as_ref() {
                                    "{pr.language}"
                                }
                            }
                        }

                        // Series (name + number combined)
                        tr {
                            td { class: "py-2 pr-4 text-gray-500 dark:text-slate-400 font-medium whitespace-nowrap", "Series" }
                            td { class: "py-2 pr-4",
                                div { class: "flex items-center gap-2",
                                    {
                                        let picklist_ref = picklist.read();
                                        let series_options = picklist_ref
                                            .as_ref()
                                            .and_then(|r| r.as_ref().ok())
                                            .map(|p| p.series.iter().map(|s| (s.name.clone(), s.next_number)).collect::<Vec<_>>())
                                            .unwrap_or_default();
                                        rsx! {
                                            AutocompleteInput {
                                                value: series_name,
                                                options: series_options,
                                                on_series_selected: move |(_, next_num): (String, u32)| {
                                                    if series_number.read().trim().is_empty() {
                                                        series_number.set(next_num.to_string());
                                                    }
                                                },
                                                on_cleared: move |()| series_number.set(String::new()),
                                                on_blur: move |name: String| {
                                                    if !name.is_empty() && series_number.read().trim().is_empty() {
                                                        series_number.set("1".to_string());
                                                    }
                                                },
                                            }
                                        }
                                    }
                                    span { class: "text-gray-400 dark:text-slate-500 text-xs whitespace-nowrap", "Book" }
                                    input {
                                        class: "w-16 border border-gray-300 dark:border-slate-600 rounded px-2 py-1 text-sm text-gray-900 dark:text-slate-100 bg-white dark:bg-slate-700 focus:outline-none focus:ring-1 focus:ring-indigo-400",
                                        value: "{series_number}",
                                        placeholder: "#",
                                        oninput: move |e| series_number.set(e.value()),
                                    }
                                }
                            }
                            td { class: "py-2 pr-4 text-center",
                                if provider_result.read().is_some() {
                                    {
                                        let psn = provider_result.read().as_ref().map(|r| r.series_name.clone()).unwrap_or_default();
                                        let pnum = provider_result.read().as_ref().map(|r| r.series_number.clone()).unwrap_or_default();
                                        rsx! {
                                            button {
                                                class: "text-indigo-500 hover:text-indigo-700 cursor-pointer text-xs font-bold",
                                                title: "Copy from provider",
                                                onclick: move |_| {
                                                    series_name.set(psn.clone());
                                                    series_number.set(pnum.clone());
                                                },
                                                "←"
                                            }
                                        }
                                    }
                                }
                            }
                            td { class: "py-2 text-gray-600 dark:text-slate-300",
                                if let Some(pr) = provider_result.read().as_ref() {
                                    if !pr.series_name.is_empty() {
                                        span {
                                            "{pr.series_name}"
                                            if !pr.series_number.is_empty() {
                                                span { class: "text-gray-400 dark:text-slate-500 ml-1", "Book {pr.series_number}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Page count
                        tr {
                            td { class: "py-2 pr-4 text-gray-500 dark:text-slate-400 font-medium whitespace-nowrap", "Pages" }
                            td { class: "py-2 pr-4",
                                input {
                                    r#type: "number",
                                    class: "w-32 border border-gray-300 dark:border-slate-600 rounded px-2 py-1 text-sm text-gray-900 dark:text-slate-100 bg-white dark:bg-slate-700 focus:outline-none focus:ring-1 focus:ring-indigo-400",
                                    value: "{page_count}",
                                    placeholder: "",
                                    oninput: move |e| page_count.set(e.value()),
                                }
                            }
                            td { class: "py-2 pr-4 text-center",
                                if provider_result.read().is_some() {
                                    {
                                        let pv = provider_result.read().as_ref().map(|r| r.page_count.clone()).unwrap_or_default();
                                        rsx! {
                                            button {
                                                class: "text-indigo-500 hover:text-indigo-700 cursor-pointer text-xs font-bold",
                                                title: "Copy from provider",
                                                onclick: move |_| page_count.set(pv.clone()),
                                                "←"
                                            }
                                        }
                                    }
                                }
                            }
                            td { class: "py-2 text-gray-600 dark:text-slate-300",
                                if let Some(pr) = provider_result.read().as_ref() {
                                    "{pr.page_count}"
                                }
                            }
                        }

                        // Libraries
                        tr {
                            td { class: "py-2 pr-4 text-gray-500 dark:text-slate-400 font-medium whitespace-nowrap align-top pt-2", "Libraries" }
                            td { class: "py-2 pr-4 col-span-3",
                                colspan: "3",
                                {
                                    let nsl_ref = non_system_libraries.read();
                                    match nsl_ref.as_ref() {
                                        None => rsx! {
                                            span { class: "text-xs text-gray-400 dark:text-slate-500", "Loading…" }
                                        },
                                        Some(Err(e)) => rsx! {
                                            span { class: "text-xs text-red-500", "Could not load libraries: {e}" }
                                        },
                                        Some(Ok(libs)) if libs.is_empty() => rsx! {
                                            span { class: "text-xs text-gray-400 dark:text-slate-500 italic", "No custom libraries" }
                                        },
                                        Some(Ok(libs)) => {
                                            let checked = checked_library_tokens.read().clone().unwrap_or_default();
                                            rsx! {
                                                div { class: "flex flex-wrap gap-x-6 gap-y-1",
                                                    for (token , name) in libs.clone() {
                                                        {
                                                            let token = token.clone();
                                                            let name = name.clone();
                                                            let is_checked = checked.contains(&token);
                                                            rsx! {
                                                                label {
                                                                    key: "{token}",
                                                                    class: "flex items-center gap-1.5 text-sm text-gray-700 dark:text-slate-300 cursor-pointer select-none",
                                                                    input {
                                                                        r#type: "checkbox",
                                                                        class: "rounded border-gray-300 text-indigo-600",
                                                                        checked: is_checked,
                                                                        onchange: move |e| {
                                                                            let mut cur = checked_library_tokens.write();
                                                                            let tokens = cur.get_or_insert_with(Vec::new);
                                                                            if e.checked() {
                                                                                if !tokens.contains(&token) {
                                                                                    tokens.push(token.clone());
                                                                                }
                                                                            } else {
                                                                                tokens.retain(|t| t != &token);
                                                                            }
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
                        }

                        // Genres
                        tr {
                            td { class: "py-2 pr-4 text-gray-500 dark:text-slate-400 font-medium whitespace-nowrap align-top pt-2", "Genres" }
                            td { class: "py-2 pr-4",
                                {
                                    let picklist_ref = picklist.read();
                                    let genre_options = picklist_ref
                                        .as_ref()
                                        .and_then(|r| r.as_ref().ok())
                                        .map(|p| p.genres.clone())
                                        .unwrap_or_default();
                                    rsx! {
                                        ChipInput {
                                            values: genres,
                                            options: genre_options,
                                            placeholder: "Add genre…".to_string(),
                                        }
                                    }
                                }
                            }
                            td { class: "py-2 pr-4 text-center" }
                            td { class: "py-2 text-gray-600 dark:text-slate-300" }
                        }

                        // Tags
                        tr {
                            td { class: "py-2 pr-4 text-gray-500 dark:text-slate-400 font-medium whitespace-nowrap align-top pt-2", "Tags" }
                            td { class: "py-2 pr-4",
                                {
                                    let picklist_ref = picklist.read();
                                    let tag_options = picklist_ref
                                        .as_ref()
                                        .and_then(|r| r.as_ref().ok())
                                        .map(|p| p.tags.clone())
                                        .unwrap_or_default();
                                    rsx! {
                                        ChipInput {
                                            values: tags,
                                            options: tag_options,
                                            placeholder: "Add tag…".to_string(),
                                        }
                                    }
                                }
                            }
                            td { class: "py-2 pr-4 text-center" }
                            td { class: "py-2 text-gray-600 dark:text-slate-300" }
                        }

                        // One row per identifier type
                        for (type_key , label) in ALL_IDENTIFIER_TYPES {
                            {
                                let type_key = type_key.to_string();
                                let label = label.to_string();
                                let tk_copy = type_key.clone();
                                rsx! {
                                    tr { key: "{type_key}",
                                        td { class: "py-2 pr-4 text-gray-500 dark:text-slate-400 font-medium whitespace-nowrap", {label} }
                                        td { class: "py-2 pr-4",
                                            {
                                                let tk = type_key.clone();
                                                let cur_val = identifiers.read().get(&type_key).cloned().unwrap_or_default();
                                                rsx! {
                                                    input {
                                                        class: "w-full border border-gray-300 dark:border-slate-600 rounded px-2 py-1 text-sm font-mono text-gray-900 dark:text-slate-100 bg-white dark:bg-slate-700 focus:outline-none focus:ring-1 focus:ring-indigo-400",
                                                        value: cur_val,
                                                        oninput: move |e| {
                                                            identifiers.write().insert(tk.clone(), e.value());
                                                        },
                                                    }
                                                }
                                            }
                                        }
                                        td { class: "py-2 pr-4 text-center",
                                            if provider_result.read().is_some() {
                                                {
                                                    let provider_val = provider_result
                                                        .read()
                                                        .as_ref()
                                                        .and_then(|r| r.identifiers.get(&tk_copy).cloned())
                                                        .unwrap_or_default();
                                                    if provider_val.is_empty() {
                                                        rsx! {}
                                                    } else {
                                                        let tk2 = tk_copy.clone();
                                                        let pv = provider_val.clone();
                                                        rsx! {
                                                            button {
                                                                class: "text-indigo-500 hover:text-indigo-700 cursor-pointer text-xs font-bold",
                                                                title: "Copy from provider",
                                                                onclick: move |_| {
                                                                    identifiers.write().insert(tk2.clone(), pv.clone());
                                                                },
                                                                "←"
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        td { class: "py-2 text-gray-600 dark:text-slate-300 font-mono text-xs",
                                            if let Some(pr) = provider_result.read().as_ref() {
                                                if let Some(val) = pr.identifiers.get(&tk_copy) {
                                                    {val.clone()}
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Cover
                        tr {
                            td { class: "py-2 pr-4 text-gray-500 dark:text-slate-400 font-medium whitespace-nowrap align-top pt-3", "Cover" }
                            td { class: "py-2 pr-4",
                                div {
                                    class: if *cover_drag_over.read() {
                                        "flex flex-col items-center gap-0.5 outline outline-2 outline-indigo-400 rounded"
                                    } else {
                                        "flex flex-col items-center gap-0.5"
                                    },
                                    ondragover: move |evt: DragEvent| {
                                        evt.prevent_default();
                                        cover_drag_over.set(true);
                                    },
                                    ondragleave: move |_| cover_drag_over.set(false),
                                    ondrop: move |evt: DragEvent| {
                                        evt.prevent_default();
                                        let files = evt.files();
                                        let jt = job_token.clone();
                                        let bt = book_token_for_edit.clone();
                                        async move {
                                            cover_drag_over.set(false);
                                            let Some(file) = files.into_iter().next() else { return; };

                                            let lower = file.name().to_lowercase();
                                            if !lower.ends_with(".jpg")
                                                && !lower.ends_with(".jpeg")
                                                && !lower.ends_with(".png")
                                                && !lower.ends_with(".gif")
                                                && !lower.ends_with(".webp")
                                            {
                                                error_msg.set(Some(
                                                    "Please drop an image file (JPG, PNG, GIF, or WebP)".to_string(),
                                                ));
                                                return;
                                            }

                                            if file.size() > 10 * 1024 * 1024 {
                                                error_msg.set(Some("Image must be under 10 MB".to_string()));
                                                return;
                                            }

                                            let Ok(bytes_obj) = file.read_bytes().await else {
                                                error_msg.set(Some("Failed to read image file".to_string()));
                                                return;
                                            };
                                            let bytes = bytes_obj.as_ref();

                                            // Detect MIME type for preview data URL
                                            let mime = if bytes.starts_with(&[0xFF, 0xD8]) {
                                                "image/jpeg"
                                            } else if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
                                                "image/png"
                                            } else if bytes.starts_with(&[0x47, 0x49, 0x46]) {
                                                "image/gif"
                                            } else if bytes.len() >= 12
                                                && bytes.starts_with(b"RIFF")
                                                && bytes.get(8..12) == Some(b"WEBP")
                                            {
                                                "image/webp"
                                            } else {
                                                "image/jpeg"
                                            };

                                            let encoded = B64.encode(bytes);
                                            // Optimistic preview
                                            current_cover
                                                .set(format!("data:{mime};base64,{}", encoded.clone()));
                                            current_cover_dimensions.set(None);
                                            // Stage the dropped cover in the temp dir; it will be
                                            // committed to disk only when the user saves/approves.
                                            use_fetched_cover.set(true);
                                            error_msg.set(None);

                                            let bt_revert = bt.clone();
                                            let result = if edit_mode {
                                                stage_library_cover(bt, encoded).await
                                            } else {
                                                stage_incoming_cover(jt, encoded).await
                                            };

                                            if let Err(e) = result {
                                                error_msg.set(Some(format!("Failed to replace cover: {e}")));
                                                current_cover
                                                    .set(format!("/api/v1/covers/{bt_revert}?full=true"));
                                            }
                                        }
                                    },
                                    img {
                                        class: "max-h-32 max-w-24 object-contain rounded shadow-sm",
                                        src: "{current_cover}",
                                        alt: "Current cover",
                                    }
                                    if let Some((w, h)) = *current_cover_dimensions.read() {
                                        span { class: "text-gray-400 dark:text-slate-500 text-xs", "{w} × {h}" }
                                    }
                                    span { class: "text-gray-400 dark:text-slate-500 text-xs mt-1", "drop image to replace" }
                                }
                            }
                            td { class: "py-2 pr-4 text-center align-top pt-3",
                                if let Some(pr) = provider_result.read().as_ref() {
                                    if pr.cover_thumbnail.is_some() {
                                        button {
                                            class: "text-indigo-500 hover:text-indigo-700 cursor-pointer text-xs font-bold",
                                            title: "Use provider cover",
                                            onclick: move |_| {
                                                if let Some(pr) = provider_result.read().as_ref()
                                                    && let Some(thumb) = pr.cover_thumbnail.clone() {
                                                        current_cover.set(thumb);
                                                        current_cover_dimensions.set(pr.cover_dimensions);
                                                        use_fetched_cover.set(true);
                                                        let ck = cover_key.clone();
                                                        let is_edit = edit_mode;
                                                        spawn(async move {
                                                            if is_edit {
                                                                let _ = accept_library_provider_cover(ck).await;
                                                            } else {
                                                                let _ = accept_incoming_provider_cover(ck).await;
                                                            }
                                                        });
                                                    }
                                            },
                                            "←"
                                        }
                                    }
                                }
                            }
                            td { class: "py-2 align-top",
                                if let Some(pr) = provider_result.read().as_ref() {
                                    if let Some(thumb) = &pr.cover_thumbnail {
                                        div { class: "flex flex-col items-center gap-0.5",
                                            img {
                                                class: "max-h-32 max-w-24 object-contain rounded shadow-sm",
                                                src: thumb,
                                                alt: "Provider cover",
                                            }
                                            if let Some((w, h)) = pr.cover_dimensions {
                                                span { class: "text-gray-400 dark:text-slate-500 text-xs", "{w} × {h}" }
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
    }
}
