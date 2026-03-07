use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::Route;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct IncomingBookSummary {
    pub job_token: String,
    pub file_path: String,
    pub file_format: String,
    pub detected_at: String,
    pub title: Option<String>,
    pub author_names: Vec<String>,
    pub cover_path: Option<String>,
}

#[cfg(feature = "server")]
use {
    crate::server::{AuthSession, AuthUser, BackendSessionPool},
    axum::http::Method,
    axum_session_auth::{Auth, Rights},
    bb_core::{
        CoreServices,
        book::{AuthorToken, BookToken},
        import::ImportJobToken,
        types::Capability,
        user::UserId,
    },
    std::sync::Arc,
};

#[get("/api/v1/incoming", auth_session: axum::Extension<AuthSession>, core_services: axum::Extension<Arc<CoreServices>>)]
async fn list_incoming_books() -> Result<Vec<IncomingBookSummary>, ServerFnError> {
    let current_user = auth_session.current_user.clone().unwrap_or_default();
    if !Auth::<AuthUser, UserId, BackendSessionPool>::build([Method::GET], true)
        .requires(Rights::any([Rights::permission(Capability::ApproveImports.as_str())]))
        .validate(&current_user, &Method::GET, None)
        .await
    {
        return Err(ServerFnError::new("Forbidden"));
    }

    let import_service = &core_services.import_job_service;
    let book_service = &core_services.book_service;

    let jobs = import_service
        .list_needs_review(None, None)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut summaries = Vec::with_capacity(jobs.len());
    for job in jobs {
        let (title, author_names, cover_path) = if let Some(book_id) = job.candidate_book_id {
            match book_service
                .find_book_by_token(&BookToken::new(book_id))
                .await
                .map_err(|e| ServerFnError::new(e.to_string()))?
            {
                Some(book) => {
                    let book_authors = book_service.authors_for_book(book.id).await.map_err(|e| ServerFnError::new(e.to_string()))?;
                    let mut names = Vec::new();
                    for ba in &book_authors {
                        if let Some(author) = book_service
                            .find_author_by_token(&AuthorToken::new(ba.author_id))
                            .await
                            .map_err(|e| ServerFnError::new(e.to_string()))?
                        {
                            names.push(author.name);
                        }
                    }
                    (Some(book.title), names, book.cover_path)
                }
                None => (None, vec![], None),
            }
        } else {
            (None, vec![], None)
        };

        let filename = std::path::Path::new(&job.file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&job.file_path)
            .to_owned();

        summaries.push(IncomingBookSummary {
            job_token: job.token.to_string(),
            file_path: filename,
            file_format: format!("{:?}", job.file_format),
            detected_at: job.detected_at.to_rfc3339(),
            title,
            author_names,
            cover_path,
        });
    }

    Ok(summaries)
}

#[put("/api/v1/incoming/reject", auth_session: axum::Extension<AuthSession>, core_services: axum::Extension<Arc<CoreServices>>)]
async fn reject_incoming_book(job_token: String) -> Result<(), ServerFnError> {
    let current_user = auth_session.current_user.clone().unwrap_or_default();
    if !Auth::<AuthUser, UserId, BackendSessionPool>::build([Method::PUT], true)
        .requires(Rights::any([Rights::permission(Capability::ApproveImports.as_str())]))
        .validate(&current_user, &Method::PUT, None)
        .await
    {
        return Err(ServerFnError::new("Forbidden"));
    }

    let token: ImportJobToken = job_token.parse().map_err(|_| ServerFnError::new("Invalid token"))?;

    core_services
        .pipeline_service
        .reject_job(token)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

/// Renders an ISO 8601 timestamp, reformatting it to the browser's local
/// timezone after hydration via a `use_effect`.
#[component]
fn LocalTime(iso: String) -> Element {
    let mut display = use_signal(|| iso.clone());

    use_effect(move || {
        let iso = iso.clone();
        spawn(async move {
            let js = format!(
                r#"return new Date("{}").toLocaleString(undefined, {{dateStyle: "medium", timeStyle: "short"}})"#,
                iso
            );
            if let Ok(val) = document::eval(&js).await {
                if let Some(s) = val.as_str() {
                    display.set(s.to_owned());
                }
            }
        });
    });

    rsx! { "{display}" }
}

#[component]
pub(crate) fn IncomingPage() -> Element {
    let mut jobs = use_server_future(list_incoming_books)?;
    let mut rejecting: Signal<Option<String>> = use_signal(|| None);
    let mut incoming_refresh: Signal<u32> = use_context();

    rsx! {
        div { class: "flex-1 flex flex-col overflow-hidden",
            div { class: "px-6 py-4 border-b border-gray-200",
                h1 { class: "text-xl font-semibold text-gray-900", "Incoming" }
            }
            match jobs() {
                None => rsx! {
                    div { class: "flex-1 flex items-center justify-center text-gray-400 text-sm",
                        "Loading…"
                    }
                },
                Some(Err(e)) => rsx! {
                    div { class: "flex-1 flex items-center justify-center text-red-600 text-sm",
                        "{e}"
                    }
                },
                Some(Ok(items)) => rsx! {
                    if items.is_empty() {
                        div { class: "flex-1 flex items-center justify-center text-gray-400 text-sm",
                            "No books awaiting review."
                        }
                    } else {
                        div { class: "flex-1 overflow-auto",
                            table { class: "min-w-full divide-y divide-gray-200 text-sm",
                                thead { class: "bg-gray-50",
                                    tr {
                                        th { class: "px-6 py-3 text-left font-medium text-gray-500 uppercase tracking-wider", "Title" }
                                        th { class: "px-6 py-3 text-left font-medium text-gray-500 uppercase tracking-wider", "Authors" }
                                        th { class: "px-6 py-3 text-left font-medium text-gray-500 uppercase tracking-wider", "Format" }
                                        th { class: "px-6 py-3 text-left font-medium text-gray-500 uppercase tracking-wider", "File" }
                                        th { class: "px-6 py-3 text-left font-medium text-gray-500 uppercase tracking-wider", "Detected" }
                                        th { class: "px-6 py-3" }
                                    }
                                }
                                tbody { class: "bg-white divide-y divide-gray-100",
                                    for item in items {
                                        tr { key: "{item.job_token}",
                                            td { class: "px-6 py-4 text-gray-900",
                                                match &item.title {
                                                    Some(t) => rsx! { "{t}" },
                                                    None => rsx! {
                                                        span { class: "text-gray-400 italic", "Unknown" }
                                                    },
                                                }
                                            }
                                            td { class: "px-6 py-4 text-gray-600",
                                                if item.author_names.is_empty() {
                                                    span { class: "text-gray-400 italic", "Unknown" }
                                                } else {
                                                    "{item.author_names.join(\", \")}"
                                                }
                                            }
                                            td { class: "px-6 py-4 text-gray-600", "{item.file_format}" }
                                            td { class: "px-6 py-4 text-gray-500 font-mono text-xs", "{item.file_path}" }
                                            td { class: "px-6 py-4 text-gray-500 whitespace-nowrap",
                                                LocalTime { iso: item.detected_at.clone() }
                                            }
                                            td { class: "px-6 py-4 text-right flex items-center justify-end gap-3",
                                                Link {
                                                    to: Route::ReviewPage { token: item.job_token.clone() },
                                                    class: "px-3 py-1 rounded border border-indigo-300 text-sm font-medium text-indigo-600 hover:bg-indigo-50",
                                                    "Review"
                                                }
                                                {
                                                    let token = item.job_token.clone();
                                                    let is_rejecting = rejecting.read().as_deref() == Some(&token);
                                                    let any_rejecting = rejecting.read().is_some();
                                                    let btn_class = if any_rejecting {
                                                        "px-3 py-1 rounded border border-red-300 text-sm font-medium text-red-600 opacity-40 cursor-not-allowed"
                                                    } else {
                                                        "px-3 py-1 rounded border border-red-300 text-sm font-medium text-red-600 hover:bg-red-50 cursor-pointer"
                                                    };
                                                    rsx! {
                                                        button {
                                                            class: "{btn_class}",
                                                            disabled: any_rejecting,
                                                            onclick: move |_| {
                                                                let token = token.clone();
                                                                rejecting.set(Some(token.clone()));
                                                                spawn(async move {
                                                                    let result = reject_incoming_book(token).await;
                                                                    rejecting.set(None);
                                                                    if result.is_ok() {
                                                                        *incoming_refresh.write() += 1;
                                                                        jobs.restart();
                                                                    }
                                                                });
                                                            },
                                                            if is_rejecting { "Rejecting…" } else { "Reject" }
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
    }
}
