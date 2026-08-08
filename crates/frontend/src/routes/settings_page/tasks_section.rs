#[cfg(feature = "server")]
use bb_core::health::HealthService;
use dioxus::prelude::*;
#[cfg(feature = "server")]
use {crate::routes::server_helpers::authenticated_user, crate::server::AuthSession, std::sync::Arc};

// ---------------------------------------------------------------------------
// DTO
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct TaskRow {
    pub name: String,
    pub job_type: String,
    pub run_on_startup: bool,
    /// `None` for manual-only tasks.
    pub interval_minutes: Option<u64>,
    /// ISO 8601 timestamp, converted to relative + local time on the client.
    pub last_run_at: Option<String>,
    /// ISO 8601 timestamp, converted to relative + local time on the client.
    /// `None` for manual-only tasks that have not been triggered.
    pub next_run_at: Option<String>,
}

// ---------------------------------------------------------------------------
// Server functions
// ---------------------------------------------------------------------------

#[post(
    "/api/v1/admin/health/tasks",
    auth_session: axum::Extension<AuthSession>,
    health_service: axum::Extension<Arc<dyn HealthService>>
)]
pub(crate) async fn list_health_tasks() -> Result<Vec<TaskRow>, ServerFnError> {
    let user = authenticated_user(&auth_session)?;
    if !user.permissions.contains("SuperAdmin") && !user.permissions.contains("Admin") {
        return Err(ServerFnError::new("Insufficient permissions"));
    }

    let tasks = health_service.list_tasks().await;
    let mut rows: Vec<TaskRow> = tasks
        .into_iter()
        .map(|t| TaskRow {
            name: t.name,
            job_type: t.job_type,
            run_on_startup: t.run_on_startup,
            interval_minutes: t.interval_minutes,
            last_run_at: t.last_run_at.map(|dt| dt.to_rfc3339()),
            next_run_at: t.next_run_at.map(|dt| dt.to_rfc3339()),
        })
        .collect();
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(rows)
}

#[post(
    "/api/v1/admin/health/tasks/trigger",
    auth_session: axum::Extension<AuthSession>,
    health_service: axum::Extension<Arc<dyn HealthService>>
)]
pub(crate) async fn trigger_health_task(job_type: String) -> Result<(), ServerFnError> {
    let user = authenticated_user(&auth_session)?;
    if !user.permissions.contains("SuperAdmin") && !user.permissions.contains("Admin") {
        return Err(ServerFnError::new("Insufficient permissions"));
    }

    // Verify the job_type is a known health task.
    let tasks = health_service.list_tasks().await;
    if !tasks.iter().any(|t| t.job_type == job_type) {
        return Err(ServerFnError::new("Unknown health task"));
    }

    // Kick the health subsystem to enqueue and run the task immediately.
    // The subsystem handles: mark_due_now → enqueue → mark_run →
    // notify_jobs_changed.
    health_service.kick(job_type);

    Ok(())
}

// ---------------------------------------------------------------------------
// TasksSection component
// ---------------------------------------------------------------------------

#[component]
pub(crate) fn TasksSection() -> Element {
    let mut refresh = use_signal(|| 0u32);
    let jobs_refresh = use_context::<crate::components::JobsRefresh>();

    let tasks_resource = use_resource(move || async move {
        let _ = refresh(); // subscribe — local (Run Now button)
        let _ = (jobs_refresh.0)(); // subscribe — SSE (job completed)
        list_health_tasks().await
    });

    rsx! {
        div { class: "w-full max-w-4xl",
            div { class: "flex items-center justify-between mb-6",
                h2 { class: "text-lg font-semibold text-gray-900 dark:text-slate-100", "Health Tasks" }
            }

            match tasks_resource() {
                None => rsx! {
                    div { class: "text-gray-400 text-sm dark:text-slate-500", "Loading..." }
                },
                Some(Err(e)) => rsx! {
                    div { class: "p-3 bg-red-50 border border-red-200 text-red-700 rounded-lg text-sm dark:bg-red-900/30 dark:border-red-800 dark:text-red-400",
                        "{e}"
                    }
                },
                Some(Ok(rows)) => rsx! {
                    div { class: "rounded-lg border border-gray-200 bg-white overflow-hidden dark:border-slate-700 dark:bg-slate-800",
                        table { class: "w-full text-sm table-fixed",
                            thead {
                                tr { class: "bg-gray-50 border-b border-gray-200 dark:bg-slate-800 dark:border-slate-700",
                                    th { class: "w-[30%] px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wide dark:text-slate-400", "Task" }
                                    th { class: "w-[12%] px-4 py-3 text-center text-xs font-medium text-gray-500 uppercase tracking-wide dark:text-slate-400", "Interval" }
                                    th { class: "w-[22%] px-4 py-3 text-center text-xs font-medium text-gray-500 uppercase tracking-wide dark:text-slate-400", "Last Run" }
                                    th { class: "w-[22%] px-4 py-3 text-center text-xs font-medium text-gray-500 uppercase tracking-wide dark:text-slate-400", "Next Run" }
                                    th { class: "w-[14%] px-4 py-3 text-center text-xs font-medium text-gray-500 uppercase tracking-wide dark:text-slate-400", "Actions" }
                                }
                            }
                            tbody { class: "divide-y divide-gray-100 dark:divide-slate-700",
                                for row in rows {
                                    {
                                        let jt = row.job_type.clone();
                                        rsx! {
                                            tr { class: "hover:bg-gray-50 dark:hover:bg-slate-700",
                                                td { class: "px-4 py-3",
                                                    div { class: "font-medium text-gray-900 dark:text-slate-100", "{row.name}" }
                                                    if row.run_on_startup {
                                                        span { class: "inline-flex items-center px-1.5 py-0.5 rounded text-xs font-medium bg-blue-100 text-blue-700 mt-0.5 dark:bg-blue-900 dark:text-blue-300",
                                                            "startup"
                                                        }
                                                    }
                                                }
                                                td { class: "px-4 py-3 text-center text-gray-600 dark:text-slate-400",
                                                    match row.interval_minutes {
                                                        Some(m) => rsx! { "{format_interval(m)}" },
                                                        None => rsx! { span { class: "text-gray-400 dark:text-slate-500", "Manual only" } },
                                                    }
                                                }
                                                td { class: "px-4 py-3 text-center text-gray-600 dark:text-slate-400",
                                                    if let Some(ref last) = row.last_run_at {
                                                        RelativeTime { iso: last.clone(), direction: "past" }
                                                    } else {
                                                        span { class: "text-gray-400 dark:text-slate-500", "Never" }
                                                    }
                                                }
                                                td { class: "px-4 py-3 text-center text-gray-600 dark:text-slate-400",
                                                    match row.next_run_at.as_ref() {
                                                        Some(next) => rsx! { RelativeTime { iso: next.clone(), direction: "future" } },
                                                        None => rsx! { span { class: "text-gray-400 dark:text-slate-500", "Manual only" } },
                                                    }
                                                }
                                                td { class: "px-4 py-3",
                                                    div { class: "flex items-center justify-center",
                                                        RunNowButton { job_type: jt, on_triggered: move || *refresh.write() += 1 }
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

// ---------------------------------------------------------------------------
// RelativeTime — shows "in 6 hours, 22 min" or "3 min ago" with a tooltip
// of the absolute local time.
// ---------------------------------------------------------------------------

#[component]
fn RelativeTime(iso: String, direction: String) -> Element {
    let mut rel_text = use_signal(|| iso.clone());
    let mut abs_text = use_signal(String::new);

    use_effect(move || {
        let iso = iso.clone();
        let direction = direction.clone();
        spawn(async move {
            let js = format!(
                r#"
                const dt = new Date("{iso}");
                const now = Date.now();
                const diffMs = {diff_expr};
                const abs = dt.toLocaleString(undefined, {{dateStyle: "medium", timeStyle: "short"}});

                const totalMin = Math.round(diffMs / 60000);
                if (totalMin < 1) return JSON.stringify({{ rel: "now", abs }});
                const d = Math.floor(totalMin / 1440);
                const h = Math.floor((totalMin % 1440) / 60);
                const m = totalMin % 60;
                let parts = [];
                if (d > 0) parts.push(d + (d === 1 ? " day" : " days"));
                if (h > 0) parts.push(h + (h === 1 ? " hr" : " hrs"));
                if (d === 0 && m > 0) parts.push(m + " min");
                const rel = "{prefix}" + parts.join(", ") + "{suffix}";
                return JSON.stringify({{ rel, abs }});
                "#,
                iso = iso,
                diff_expr = if direction == "past" { "now - dt.getTime()" } else { "dt.getTime() - now" },
                prefix = if direction == "past" { "" } else { "in " },
                suffix = if direction == "past" { " ago" } else { "" },
            );
            if let Ok(val) = document::eval(&js).await
                && let Some(s) = val.as_str()
                && let Ok(parsed) = serde_json::from_str::<RelTimeResult>(s)
            {
                rel_text.set(parsed.rel);
                abs_text.set(parsed.abs);
            }
        });
    });

    rsx! {
        span { title: "{abs_text}", "{rel_text}" }
    }
}

#[derive(serde::Deserialize)]
struct RelTimeResult {
    rel: String,
    abs: String,
}

#[component]
fn RunNowButton(job_type: String, on_triggered: EventHandler<()>) -> Element {
    let mut running = use_signal(|| false);

    rsx! {
        button {
            class: "px-2.5 py-1 text-xs font-medium rounded bg-indigo-50 text-indigo-700 hover:bg-indigo-100 dark:bg-indigo-900/30 dark:text-indigo-300 dark:hover:bg-indigo-900/50 disabled:opacity-50",
            disabled: running(),
            onclick: move |_| {
                let jt = job_type.clone();
                running.set(true);
                spawn(async move {
                    let _ = trigger_health_task(jt).await;
                    on_triggered.call(());
                    running.set(false);
                });
            },
            if running() { "Running..." } else { "Run Now" }
        }
    }
}

fn format_interval(minutes: u64) -> String {
    if minutes >= 1440 && minutes.is_multiple_of(1440) {
        let days = minutes / 1440;
        if days == 1 { "1 day".to_string() } else { format!("{days} days") }
    } else if minutes >= 60 && minutes.is_multiple_of(60) {
        let hours = minutes / 60;
        if hours == 1 { "1 hour".to_string() } else { format!("{hours} hours") }
    } else if minutes == 1 {
        "1 minute".to_string()
    } else {
        format!("{minutes} minutes")
    }
}
