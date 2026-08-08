#[cfg(feature = "server")]
use bb_core::{
    CoreServices,
    book::{GenreToken, TagToken},
};
use dioxus::prelude::*;
#[cfg(feature = "server")]
use {
    crate::routes::server_helpers::{authenticated_user, to_server_err},
    crate::server::AuthSession,
    std::sync::Arc,
};

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct GenreTagEntry {
    pub token: String,
    pub name: String,
    pub book_count: u64,
    pub has_incoming: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct GenreTagsData {
    pub genres: Vec<GenreTagEntry>,
    pub tags: Vec<GenreTagEntry>,
}

// ---------------------------------------------------------------------------
// Server functions
// ---------------------------------------------------------------------------

#[get(
    "/api/v1/admin/genre-tags",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn get_genre_tags() -> Result<GenreTagsData, ServerFnError> {
    authenticated_user(&auth_session)?;

    let book_service = &core_services.book_service;

    let genres = book_service
        .list_genres_with_counts()
        .await
        .map_err(to_server_err)?
        .into_iter()
        .map(|(g, count, has_incoming)| GenreTagEntry {
            token: g.token.to_string(),
            name: g.name,
            book_count: count,
            has_incoming,
        })
        .collect();

    let tags = book_service
        .list_tags_with_counts()
        .await
        .map_err(to_server_err)?
        .into_iter()
        .map(|(t, count, has_incoming)| GenreTagEntry {
            token: t.token.to_string(),
            name: t.name,
            book_count: count,
            has_incoming,
        })
        .collect();

    Ok(GenreTagsData { genres, tags })
}

#[post(
    "/api/v1/admin/genres/create",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn admin_create_genre(name: String) -> Result<(), ServerFnError> {
    let user = authenticated_user(&auth_session)?;

    if !user.permissions.contains("SuperAdmin") && !user.permissions.contains("Admin") {
        return Err(ServerFnError::new("Insufficient permissions"));
    }

    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(ServerFnError::new("Name is required"));
    }

    core_services.book_service.create_genre(name).await.map_err(|e| {
        let msg = e.to_string();
        if msg.contains("Constraint") || msg.contains("unique") || msg.contains("duplicate") {
            ServerFnError::new("Genre already exists")
        } else {
            ServerFnError::new(msg)
        }
    })?;

    Ok(())
}

#[post(
    "/api/v1/admin/genres/delete",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn admin_delete_genre(token: String) -> Result<(), ServerFnError> {
    let user = authenticated_user(&auth_session)?;

    if !user.permissions.contains("SuperAdmin") && !user.permissions.contains("Admin") {
        return Err(ServerFnError::new("Insufficient permissions"));
    }

    let genre_token: GenreToken = token.parse().map_err(|_| ServerFnError::new("Invalid genre token"))?;

    core_services.book_service.delete_genre(genre_token).await.map_err(to_server_err)?;

    Ok(())
}

#[post(
    "/api/v1/admin/tags/create",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn admin_create_tag(name: String) -> Result<(), ServerFnError> {
    let user = authenticated_user(&auth_session)?;

    if !user.permissions.contains("SuperAdmin") && !user.permissions.contains("Admin") {
        return Err(ServerFnError::new("Insufficient permissions"));
    }

    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(ServerFnError::new("Name is required"));
    }

    core_services.book_service.create_tag(name).await.map_err(|e| {
        let msg = e.to_string();
        if msg.contains("Constraint") || msg.contains("unique") || msg.contains("duplicate") {
            ServerFnError::new("Tag already exists")
        } else {
            ServerFnError::new(msg)
        }
    })?;

    Ok(())
}

#[post(
    "/api/v1/admin/tags/delete",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn admin_delete_tag(token: String) -> Result<(), ServerFnError> {
    let user = authenticated_user(&auth_session)?;

    if !user.permissions.contains("SuperAdmin") && !user.permissions.contains("Admin") {
        return Err(ServerFnError::new("Insufficient permissions"));
    }

    let tag_token: TagToken = token.parse().map_err(|_| ServerFnError::new("Invalid tag token"))?;

    core_services.book_service.delete_tag(tag_token).await.map_err(to_server_err)?;

    Ok(())
}

#[post(
    "/api/v1/admin/genres/remove-unused",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn admin_remove_unused_genres() -> Result<u64, ServerFnError> {
    let user = authenticated_user(&auth_session)?;

    if !user.permissions.contains("SuperAdmin") && !user.permissions.contains("Admin") {
        return Err(ServerFnError::new("Insufficient permissions"));
    }

    core_services.book_service.remove_unused_genres().await.map_err(to_server_err)
}

#[post(
    "/api/v1/admin/tags/remove-unused",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn admin_remove_unused_tags() -> Result<u64, ServerFnError> {
    let user = authenticated_user(&auth_session)?;

    if !user.permissions.contains("SuperAdmin") && !user.permissions.contains("Admin") {
        return Err(ServerFnError::new("Insufficient permissions"));
    }

    core_services.book_service.remove_unused_tags().await.map_err(to_server_err)
}

// ---------------------------------------------------------------------------
// EntityPanel — shared panel for genres and tags
// ---------------------------------------------------------------------------

#[component]
fn EntityPanel(
    title: &'static str,
    entries: Vec<GenreTagEntry>,
    on_add: EventHandler<String>,
    on_delete: EventHandler<String>,
    on_click_name: EventHandler<String>,
    on_remove_unused: EventHandler<()>,
    add_error: Option<String>,
) -> Element {
    let mut adding = use_signal(|| false);
    let mut new_name = use_signal(String::new);
    let unused_count = entries.iter().filter(|e| e.book_count == 0).count();

    let submit_add = move || {
        let name = new_name();
        if name.trim().is_empty() {
            return;
        }
        on_add.call(name);
    };

    rsx! {
        div { class: "mb-8",
            // Panel header
            div { class: "flex items-center justify-between mb-3",
                h3 { class: "text-base font-semibold text-gray-900 dark:text-slate-100", {title} }
                div { class: "flex items-center gap-2",
                    if unused_count > 0 {
                        button {
                            class: "px-3 py-1.5 text-sm font-medium rounded border border-gray-300 text-gray-600 hover:bg-gray-50 dark:border-slate-600 dark:text-slate-400 dark:hover:bg-slate-700",
                            onclick: move |_| on_remove_unused.call(()),
                            "Remove unused"
                        }
                    }
                    button {
                        class: "px-3 py-1.5 text-sm font-medium rounded bg-indigo-600 text-white hover:bg-indigo-700",
                        onclick: move |_| {
                            new_name.set(String::new());
                            adding.set(true);
                        },
                        "+ Add"
                    }
                }
            }

            // Inline add form
            if adding() {
                div { class: "mb-3 flex items-center gap-2",
                    input {
                        r#type: "text",
                        class: "flex-1 px-3 py-2 border border-gray-300 rounded-lg focus:outline-hidden focus:ring-2 focus:ring-indigo-500 text-sm dark:bg-slate-700 dark:border-slate-600 dark:text-slate-100 dark:placeholder-slate-400",
                        placeholder: "Name…",
                        value: new_name,
                        autofocus: true,
                        oninput: move |e| new_name.set(e.value()),
                        onkeydown: move |e| {
                            match e.key() {
                                Key::Enter => submit_add(),
                                Key::Escape => {
                                    adding.set(false);
                                    new_name.set(String::new());
                                }
                                _ => {}
                            }
                        },
                    }
                    button {
                        class: "px-3 py-2 text-sm font-medium rounded-lg bg-indigo-600 text-white hover:bg-indigo-700",
                        onclick: move |_| submit_add(),
                        "Add"
                    }
                    button {
                        class: "px-3 py-2 text-sm font-medium rounded-lg border border-gray-300 text-gray-700 hover:bg-gray-50 dark:border-slate-600 dark:text-slate-300 dark:hover:bg-slate-700",
                        onclick: move |_| {
                            adding.set(false);
                            new_name.set(String::new());
                        },
                        "Cancel"
                    }
                }
                if let Some(msg) = add_error {
                    div { class: "mb-2 p-2 bg-red-50 border border-red-200 text-red-700 rounded text-sm dark:bg-red-900/30 dark:border-red-800 dark:text-red-400",
                        {msg}
                    }
                }
            }

            // Entries list
            div { class: "rounded-lg border border-gray-200 bg-white overflow-hidden dark:border-slate-700 dark:bg-slate-800",
                if entries.is_empty() {
                    div { class: "px-4 py-6 text-center text-gray-400 text-sm dark:text-slate-500",
                        "No {title.to_lowercase()} yet."
                    }
                } else {
                    ul { class: "divide-y divide-gray-100 dark:divide-slate-700",
                        for entry in entries {
                            {
                                let token = entry.token.clone();
                                let name = entry.name.clone();
                                let book_label = if entry.book_count == 1 { "book" } else { "books" };
                                let is_active = entry.book_count > 0;
                                rsx! {
                                    li { class: "flex items-center justify-between px-4 py-2.5 hover:bg-gray-50 dark:hover:bg-slate-700",
                                        // Name — clickable button when active, plain text otherwise
                                        if is_active {
                                            button {
                                                class: "text-sm text-indigo-600 hover:underline text-left",
                                                onclick: move |_| on_click_name.call(name.clone()),
                                                "{entry.name}"
                                            }
                                        } else {
                                            span { class: "text-sm text-gray-900 dark:text-slate-100", "{entry.name}" }
                                        }
                                        div { class: "flex items-center gap-3",
                                            span { class: "text-xs text-gray-400 dark:text-slate-500",
                                                "{entry.book_count} {book_label}"
                                            }
                                            // Incoming badge — only when no available books but pipeline references exist
                                            if entry.has_incoming {
                                                span {
                                                    class: "inline-flex items-center px-1.5 py-0.5 rounded text-xs font-medium bg-amber-50 text-amber-600 border border-amber-200 dark:bg-amber-900/30 dark:text-amber-400 dark:border-amber-800",
                                                    "incoming"
                                                }
                                            }
                                            button {
                                                class: "p-1 text-gray-400 hover:text-red-600 hover:bg-red-50 dark:text-slate-500 dark:hover:bg-red-900/30 rounded text-xs",
                                                title: "Delete",
                                                onclick: move |_| on_delete.call(token.clone()),
                                                "✕"
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

// ---------------------------------------------------------------------------
// GenreTagsSection
// ---------------------------------------------------------------------------

#[component]
pub(crate) fn GenreTagsSection() -> Element {
    let navigator = use_navigator();
    let mut refresh = use_signal(|| 0u32);

    let data_resource = use_resource(move || async move {
        let _ = refresh();
        get_genre_tags().await
    });

    let mut delete_target: Signal<Option<GenreTagEntry>> = use_signal(|| None);
    let mut delete_kind: Signal<Option<&'static str>> = use_signal(|| None);
    let mut delete_error: Signal<Option<String>> = use_signal(|| None);
    let mut deleting = use_signal(|| false);

    let mut add_genre_error: Signal<Option<String>> = use_signal(|| None);
    let mut add_tag_error: Signal<Option<String>> = use_signal(|| None);

    // (kind, count) — Some(...) means the modal is open
    let mut remove_unused_target: Signal<Option<(&'static str, usize)>> = use_signal(|| None);
    let mut removing_unused = use_signal(|| false);
    let mut remove_unused_error: Signal<Option<String>> = use_signal(|| None);

    rsx! {
        div { class: "w-full max-w-2xl",
            h2 { class: "text-lg font-semibold text-gray-900 mb-6 dark:text-slate-100", "Genre/Tags" }

            // Global delete error banner
            if let Some(msg) = delete_error() {
                div { class: "mb-4 p-3 bg-red-50 border border-red-200 text-red-700 rounded-lg text-sm dark:bg-red-900/30 dark:border-red-800 dark:text-red-400",
                    {msg}
                }
            }

            if let Some(msg) = remove_unused_error() {
                div { class: "mb-4 p-3 bg-red-50 border border-red-200 text-red-700 rounded-lg text-sm dark:bg-red-900/30 dark:border-red-800 dark:text-red-400",
                    {msg}
                }
            }

            match data_resource() {
                None => rsx! {
                    div { class: "text-gray-400 text-sm dark:text-slate-500", "Loading…" }
                },
                Some(Err(e)) => rsx! {
                    div { class: "p-3 bg-red-50 border border-red-200 text-red-700 rounded-lg text-sm dark:bg-red-900/30 dark:border-red-800 dark:text-red-400",
                        "{e}"
                    }
                },
                Some(Ok(data)) => {
                    let unused_genres_count = data.genres.iter().filter(|e| e.book_count == 0).count();
                    let unused_tags_count = data.tags.iter().filter(|e| e.book_count == 0).count();
                    rsx! {
                        EntityPanel {
                            title: "Genres",
                            entries: data.genres.clone(),
                            add_error: add_genre_error(),
                            on_add: move |name: String| {
                                add_genre_error.set(None);
                                spawn(async move {
                                    match admin_create_genre(name).await {
                                        Ok(()) => *refresh.write() += 1,
                                        Err(ServerFnError::ServerError { message, .. }) => {
                                            add_genre_error.set(Some(message));
                                        }
                                        Err(e) => add_genre_error.set(Some(e.to_string())),
                                    }
                                });
                            },
                            on_delete: move |token: String| {
                                delete_error.set(None);
                                let entry = data.genres.iter().find(|g| g.token == token).cloned();
                                if let Some(entry) = entry {
                                    delete_kind.set(Some("genre"));
                                    delete_target.set(Some(entry));
                                }
                            },
                            on_click_name: move |name: String| {
                                *crate::components::PENDING_SEARCH.write() = Some(format!("genre:\"{name}\""));
                                navigator.push(crate::Route::BooksPage {});
                            },
                            on_remove_unused: move |()| {
                                remove_unused_error.set(None);
                                remove_unused_target.set(Some(("genre", unused_genres_count)));
                            },
                        }
                        EntityPanel {
                            title: "Tags",
                            entries: data.tags.clone(),
                            add_error: add_tag_error(),
                            on_add: move |name: String| {
                                add_tag_error.set(None);
                                spawn(async move {
                                    match admin_create_tag(name).await {
                                        Ok(()) => *refresh.write() += 1,
                                        Err(ServerFnError::ServerError { message, .. }) => {
                                            add_tag_error.set(Some(message));
                                        }
                                        Err(e) => add_tag_error.set(Some(e.to_string())),
                                    }
                                });
                            },
                            on_delete: move |token: String| {
                                delete_error.set(None);
                                let entry = data.tags.iter().find(|t| t.token == token).cloned();
                                if let Some(entry) = entry {
                                    delete_kind.set(Some("tag"));
                                    delete_target.set(Some(entry));
                                }
                            },
                            on_click_name: move |name: String| {
                                *crate::components::PENDING_SEARCH.write() = Some(format!("tag:\"{name}\""));
                                navigator.push(crate::Route::BooksPage {});
                            },
                            on_remove_unused: move |()| {
                                remove_unused_error.set(None);
                                remove_unused_target.set(Some(("tag", unused_tags_count)));
                            },
                        }
                    }
                },
            }
        }

        // ── Remove-unused confirmation modal ────────────────────────────────────
        if let Some((kind, count)) = remove_unused_target() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/40",
                onkeydown: move |e| { if e.key() == Key::Escape { remove_unused_target.set(None); } },
                div { class: "bg-white rounded-2xl shadow-xl w-full max-w-sm p-6 dark:bg-slate-800",
                    h3 { class: "text-base font-semibold text-gray-900 mb-2 dark:text-slate-100",
                        "Remove unused {kind}s"
                    }
                    p { class: "text-sm text-gray-600 mb-6 dark:text-slate-400",
                        {
                            let plural = if count == 1 { "" } else { "s" };
                            let have = if count == 1 { "has" } else { "have" };
                            rsx! {
                                "This will permanently delete "
                                span { class: "font-medium text-gray-900 dark:text-slate-100", "{count} {kind}{plural}" }
                                " that {have} no books assigned. This cannot be undone."
                            }
                        }
                    }
                    div { class: "flex justify-end gap-3",
                        button {
                            class: "px-4 py-2 text-sm font-medium rounded-lg border border-gray-300 text-gray-700 hover:bg-gray-50 dark:border-slate-600 dark:text-slate-300 dark:hover:bg-slate-700",
                            disabled: removing_unused(),
                            onmounted: move |e| { spawn(async move { let _ = e.set_focus(true).await; }); },
                            onclick: move |_| remove_unused_target.set(None),
                            "Cancel"
                        }
                        button {
                            class: "px-4 py-2 text-sm font-medium rounded-lg bg-red-600 text-white hover:bg-red-700 disabled:opacity-50",
                            disabled: removing_unused(),
                            onclick: move |_| {
                                removing_unused.set(true);
                                spawn(async move {
                                    let result = match kind {
                                        "genre" => admin_remove_unused_genres().await.map(|_| ()),
                                        "tag"   => admin_remove_unused_tags().await.map(|_| ()),
                                        _       => Ok(()),
                                    };
                                    match result {
                                        Ok(()) => {
                                            remove_unused_target.set(None);
                                            *refresh.write() += 1;
                                        }
                                        Err(e) => {
                                            remove_unused_error.set(Some(e.to_string()));
                                            remove_unused_target.set(None);
                                        }
                                    }
                                    removing_unused.set(false);
                                });
                            },
                            if removing_unused() { "Removing…" } else { "Remove {count}" }
                        }
                    }
                }
            }
        }

        // ── Delete confirmation modal ───────────────────────────────────────
        if let Some(target) = delete_target() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/40",
                onkeydown: move |e| { if e.key() == Key::Escape { delete_target.set(None); } },
                div { class: "bg-white rounded-2xl shadow-xl w-full max-w-sm p-6 dark:bg-slate-800",
                    h3 { class: "text-base font-semibold text-gray-900 mb-2 dark:text-slate-100",
                        "Delete {delete_kind().unwrap_or(\"item\")}"
                    }
                    p { class: "text-sm text-gray-600 mb-6 dark:text-slate-400",
                        "Are you sure you want to delete "
                        span { class: "font-medium text-gray-900 dark:text-slate-100", "{target.name}" }
                        "? This will remove it from all books and cannot be undone."
                    }
                    div { class: "flex justify-end gap-3",
                        button {
                            class: "px-4 py-2 text-sm font-medium rounded-lg border border-gray-300 text-gray-700 hover:bg-gray-50 dark:border-slate-600 dark:text-slate-300 dark:hover:bg-slate-700",
                            disabled: deleting(),
                            onclick: move |_| delete_target.set(None),
                            "Cancel"
                        }
                        button {
                            class: "px-4 py-2 text-sm font-medium rounded-lg bg-red-600 text-white hover:bg-red-700 disabled:opacity-50",
                            onmounted: move |e| { spawn(async move { let _ = e.set_focus(true).await; }); },
                            disabled: deleting(),
                            onclick: move |_| {
                                let tok = target.token.clone();
                                let kind = delete_kind();
                                deleting.set(true);
                                spawn(async move {
                                    let result = match kind {
                                        Some("genre") => admin_delete_genre(tok).await,
                                        Some("tag") => admin_delete_tag(tok).await,
                                        _ => Ok(()),
                                    };
                                    match result {
                                        Ok(()) => {
                                            delete_target.set(None);
                                            *refresh.write() += 1;
                                        }
                                        Err(e) => {
                                            delete_error.set(Some(e.to_string()));
                                            delete_target.set(None);
                                        }
                                    }
                                    deleting.set(false);
                                });
                            },
                            if deleting() { "Deleting…" } else { "Delete" }
                        }
                    }
                }
            }
        }
    }
}
