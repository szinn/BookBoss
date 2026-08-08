#[cfg(feature = "server")]
use bb_core::{CoreServices, library::LibraryToken};
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
pub(crate) struct LibraryRow {
    pub token: String,
    pub name: String,
    pub is_system: bool,
    pub user_count: u64,
    pub book_count: u64,
}

// ---------------------------------------------------------------------------
// Server functions
// ---------------------------------------------------------------------------

#[get(
    "/api/v1/admin/libraries",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn list_libraries_admin() -> Result<Vec<LibraryRow>, ServerFnError> {
    let user = authenticated_user(&auth_session)?;

    if !user.permissions.contains("SuperAdmin") && !user.permissions.contains("Admin") {
        return Err(ServerFnError::new("Insufficient permissions"));
    }

    let entries = core_services.library_service.list_libraries().await.map_err(to_server_err)?;

    let mut rows: Vec<LibraryRow> = entries
        .into_iter()
        .map(|e| LibraryRow {
            token: e.library.token.to_string(),
            name: e.library.name,
            is_system: e.library.is_system,
            user_count: e.user_count,
            book_count: e.book_count,
        })
        .collect();

    // System libraries first (alphabetically), then non-system (alphabetically).
    rows.sort_by(|a, b| b.is_system.cmp(&a.is_system).then_with(|| a.name.cmp(&b.name)));

    Ok(rows)
}

#[post(
    "/api/v1/admin/libraries/create",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn create_library_admin(name: String) -> Result<(), ServerFnError> {
    let user = authenticated_user(&auth_session)?;

    if !user.permissions.contains("SuperAdmin") && !user.permissions.contains("Admin") {
        return Err(ServerFnError::new("Insufficient permissions"));
    }

    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(ServerFnError::new("Name is required"));
    }

    core_services.library_service.create_library(name).await.map_err(|e| {
        let msg = e.to_string();
        if msg.contains("already exists") || msg.contains("Constraint") || msg.contains("unique") || msg.contains("duplicate") {
            ServerFnError::new("A library with this name already exists")
        } else {
            ServerFnError::new(msg)
        }
    })?;

    Ok(())
}

#[post(
    "/api/v1/admin/libraries/delete",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn delete_library_admin(token: String) -> Result<(), ServerFnError> {
    let user = authenticated_user(&auth_session)?;

    if !user.permissions.contains("SuperAdmin") && !user.permissions.contains("Admin") {
        return Err(ServerFnError::new("Insufficient permissions"));
    }

    let library_token: LibraryToken = token.parse().map_err(|_| ServerFnError::new("Invalid library token"))?;

    core_services.library_service.delete_library(library_token).await.map_err(to_server_err)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// LibrariesSection
// ---------------------------------------------------------------------------

#[component]
pub(crate) fn LibrariesSection() -> Element {
    let mut refresh = use_signal(|| 0u32);

    let data_resource = use_resource(move || async move {
        let _ = refresh();
        list_libraries_admin().await
    });

    let mut delete_target: Signal<Option<LibraryRow>> = use_signal(|| None);
    let mut delete_error: Signal<Option<String>> = use_signal(|| None);
    let mut deleting = use_signal(|| false);

    let mut add_error: Signal<Option<String>> = use_signal(|| None);
    let mut adding = use_signal(|| false);
    let mut new_name = use_signal(String::new);

    let mut submit_add = move || {
        let name = new_name();
        if name.trim().is_empty() {
            return;
        }
        add_error.set(None);
        spawn(async move {
            match create_library_admin(name).await {
                Ok(()) => {
                    adding.set(false);
                    new_name.set(String::new());
                    *refresh.write() += 1;
                }
                Err(ServerFnError::ServerError { message, .. }) => {
                    add_error.set(Some(message));
                }
                Err(e) => add_error.set(Some(e.to_string())),
            }
        });
    };

    rsx! {
        div { class: "w-full max-w-2xl",
            h2 { class: "text-lg font-semibold text-gray-900 mb-6 dark:text-slate-100", "Libraries" }

            // Global delete error banner
            if let Some(msg) = delete_error() {
                div { class: "mb-4 p-3 bg-red-50 border border-red-200 text-red-700 rounded-lg text-sm dark:bg-red-900/30 dark:border-red-800 dark:text-red-400",
                    {msg}
                }
            }

            // Panel header
            div { class: "flex items-center justify-between mb-3",
                h3 { class: "text-base font-semibold text-gray-900 dark:text-slate-100", "All Libraries" }
                button {
                    class: "px-3 py-1.5 text-sm font-medium rounded bg-indigo-600 text-white hover:bg-indigo-700",
                    onclick: move |_| {
                        new_name.set(String::new());
                        add_error.set(None);
                        adding.set(true);
                    },
                    "+ Add"
                }
            }

            // Inline add form
            if adding() {
                div { class: "mb-3 flex items-center gap-2",
                    input {
                        r#type: "text",
                        class: "flex-1 px-3 py-2 border border-gray-300 rounded-lg focus:outline-hidden focus:ring-2 focus:ring-indigo-500 text-sm dark:bg-slate-700 dark:border-slate-600 dark:text-slate-100 dark:placeholder-slate-400",
                        placeholder: "Library name…",
                        value: new_name,
                        autofocus: true,
                        oninput: move |e| new_name.set(e.value()),
                        onkeydown: move |e| {
                            match e.key() {
                                Key::Enter => submit_add(),
                                Key::Escape => {
                                    adding.set(false);
                                    new_name.set(String::new());
                                    add_error.set(None);
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
                            add_error.set(None);
                        },
                        "Cancel"
                    }
                }
                if let Some(msg) = add_error() {
                    div { class: "mb-2 p-2 bg-red-50 border border-red-200 text-red-700 rounded text-sm dark:bg-red-900/30 dark:border-red-800 dark:text-red-400",
                        {msg}
                    }
                }
            }

            // Libraries list
            match data_resource() {
                None => rsx! {
                    div { class: "text-gray-400 text-sm dark:text-slate-500", "Loading…" }
                },
                Some(Err(e)) => rsx! {
                    div { class: "p-3 bg-red-50 border border-red-200 text-red-700 rounded-lg text-sm dark:bg-red-900/30 dark:border-red-800 dark:text-red-400",
                        "{e}"
                    }
                },
                Some(Ok(rows)) => rsx! {
                    div { class: "rounded-lg border border-gray-200 bg-white overflow-hidden dark:border-slate-700 dark:bg-slate-800",
                        if rows.is_empty() {
                            div { class: "px-4 py-6 text-center text-gray-400 text-sm dark:text-slate-500",
                                "No libraries yet."
                            }
                        } else {
                            table { class: "w-full",
                                thead {
                                    tr { class: "bg-gray-50 border-b border-gray-200 dark:bg-slate-800 dark:border-slate-700",
                                        th { class: "px-4 py-2 text-left text-xs font-medium text-gray-500 uppercase tracking-wide dark:text-slate-400", "Name" }
                                        th { class: "px-4 py-2 text-right text-xs font-medium text-gray-500 uppercase tracking-wide whitespace-nowrap dark:text-slate-400", "Users" }
                                        th { class: "px-4 py-2 text-right text-xs font-medium text-gray-500 uppercase tracking-wide whitespace-nowrap dark:text-slate-400", "Books" }
                                        th { class: "px-4 py-2" }
                                    }
                                }
                                tbody { class: "divide-y divide-gray-100 dark:divide-slate-700",
                                    for row in rows {
                                        {
                                            let token = row.token.clone();
                                            let user_label = if row.user_count == 1 { "user" } else { "users" };
                                            let book_label = if row.book_count == 1 { "book" } else { "books" };
                                            rsx! {
                                                tr { class: "hover:bg-gray-50 dark:hover:bg-slate-700",
                                                    td { class: "px-4 py-2.5",
                                                        div { class: "flex items-center gap-2",
                                                            span { class: "text-sm text-gray-900 dark:text-slate-100", "{row.name}" }
                                                            if row.is_system {
                                                                span {
                                                                    class: "inline-flex items-center px-1.5 py-0.5 rounded text-xs font-medium bg-blue-50 text-blue-600 border border-blue-200 dark:bg-blue-900/30 dark:text-blue-400 dark:border-blue-800",
                                                                    "system"
                                                                }
                                                            }
                                                        }
                                                    }
                                                    td { class: "px-4 py-2.5 text-xs text-gray-400 text-right whitespace-nowrap dark:text-slate-500",
                                                        "{row.user_count} {user_label}"
                                                    }
                                                    td { class: "px-4 py-2.5 text-xs text-gray-400 text-right whitespace-nowrap dark:text-slate-500",
                                                        "{row.book_count} {book_label}"
                                                    }
                                                    td { class: "px-4 py-2.5 text-right",
                                                        if !row.is_system {
                                                            button {
                                                                class: "p-1 text-gray-400 hover:text-red-600 hover:bg-red-50 dark:text-slate-500 dark:hover:bg-red-900/30 rounded text-xs",
                                                                title: "Delete",
                                                                onclick: move |_| {
                                                                    delete_error.set(None);
                                                                    delete_target.set(Some(LibraryRow {
                                                                        token: token.clone(),
                                                                        name: row.name.clone(),
                                                                        is_system: row.is_system,
                                                                        user_count: row.user_count,
                                                                        book_count: row.book_count,
                                                                    }));
                                                                },
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
                },
            }
        }

        // ── Delete confirmation modal ───────────────────────────────────────
        if let Some(target) = delete_target() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/40",
                onkeydown: move |e| { if e.key() == Key::Escape { delete_target.set(None); } },
                div { class: "bg-white rounded-2xl shadow-xl w-full max-w-sm p-6 dark:bg-slate-800",
                    h3 { class: "text-base font-semibold text-gray-900 mb-2 dark:text-slate-100",
                        "Delete library"
                    }
                    p { class: "text-sm text-gray-600 mb-6 dark:text-slate-400",
                        "Are you sure you want to delete "
                        span { class: "font-medium text-gray-900 dark:text-slate-100", "{target.name}" }
                        "? This cannot be undone."
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
                                deleting.set(true);
                                spawn(async move {
                                    match delete_library_admin(tok).await {
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
