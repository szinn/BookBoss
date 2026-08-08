#[cfg(feature = "server")]
use bb_core::{CoreServices, health::HealthService};
use dioxus::prelude::*;
#[cfg(feature = "server")]
use {crate::routes::server_helpers::authenticated_user, crate::server::AuthSession, std::sync::Arc};

// ---------------------------------------------------------------------------
// Server functions
// ---------------------------------------------------------------------------

#[post(
    "/api/v1/settings/application",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn get_application_settings() -> Result<bool, ServerFnError> {
    let user = authenticated_user(&auth_session)?;
    if !user.permissions.contains("SuperAdmin") && !user.permissions.contains("Admin") {
        return Err(ServerFnError::new("Insufficient permissions"));
    }
    let enabled = core_services
        .app_setting_service
        .mobi_enabled()
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(enabled)
}

#[post(
    "/api/v1/settings/application/mobi",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn set_mobi_enabled(enabled: bool) -> Result<(), ServerFnError> {
    let user = authenticated_user(&auth_session)?;
    if !user.permissions.contains("SuperAdmin") && !user.permissions.contains("Admin") {
        return Err(ServerFnError::new("Insufficient permissions"));
    }
    let value = if enabled { "true" } else { "false" };
    core_services
        .app_setting_service
        .set("enrichment.mobi_enabled", value)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[post(
    "/api/v1/settings/application/kick-enrichments",
    auth_session: axum::Extension<AuthSession>,
    health_service: axum::Extension<Arc<dyn HealthService>>
)]
pub(crate) async fn kick_ensure_enrichments() -> Result<(), ServerFnError> {
    let user = authenticated_user(&auth_session)?;
    if !user.permissions.contains("SuperAdmin") && !user.permissions.contains("Admin") {
        return Err(ServerFnError::new("Insufficient permissions"));
    }
    health_service.kick("health.ensure_enrichments".to_string());
    Ok(())
}

// ---------------------------------------------------------------------------
// ApplicationSection component
// ---------------------------------------------------------------------------

#[component]
pub(crate) fn ApplicationSection() -> Element {
    let mut mobi_enabled = use_signal(|| false);
    let mut show_modal = use_signal(|| false);
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);

    let settings_resource = use_resource(get_application_settings);

    use_effect(move || {
        if let Some(Ok(enabled)) = settings_resource() {
            mobi_enabled.set(enabled);
        }
    });

    rsx! {
        div { class: "w-full max-w-2xl",
            div { class: "flex items-center justify-between mb-6",
                h2 { class: "text-lg font-semibold text-gray-900 dark:text-slate-100", "Application Settings" }
            }

            div { class: "rounded-lg border border-gray-200 bg-white px-6 dark:border-slate-700 dark:bg-slate-800",
                // MOBI toggle row
                div { class: "flex items-center gap-6 py-4 border-b border-gray-100 dark:border-slate-700",
                    div { class: "flex-1 min-w-0",
                        p { class: "text-sm font-medium text-gray-900 dark:text-slate-100", "Generate MOBI files for Kindle" }
                        p { class: "text-sm text-gray-500 mt-0.5 dark:text-slate-400",
                            "Generates a MOBI file for each book so Kindle devices can read them directly. MOBI files are created after each book's EPUB enrichment completes."
                        }
                    }
                    button {
                        class: if mobi_enabled() {
                            "relative inline-flex shrink-0 h-6 w-11 items-center rounded-full bg-indigo-600 transition-colors"
                        } else {
                            "relative inline-flex shrink-0 h-6 w-11 items-center rounded-full bg-gray-200 dark:bg-slate-600 transition-colors"
                        },
                        disabled: loading(),
                        onclick: move |_| {
                            let new_value = !mobi_enabled();
                            loading.set(true);
                            error.set(None);
                            spawn(async move {
                                match set_mobi_enabled(new_value).await {
                                    Ok(()) => {
                                        mobi_enabled.set(new_value);
                                        loading.set(false);
                                        if new_value {
                                            show_modal.set(true);
                                        }
                                    }
                                    Err(e) => {
                                        error.set(Some(e.to_string()));
                                        loading.set(false);
                                    }
                                }
                            });
                        },
                        span {
                            class: if mobi_enabled() {
                                "inline-block h-4 w-4 transform rounded-full bg-white transition-transform translate-x-6"
                            } else {
                                "inline-block h-4 w-4 transform rounded-full bg-white transition-transform translate-x-1"
                            }
                        }
                    }
                }
            }

            // Inline error
            if let Some(ref msg) = error() {
                div { class: "mt-3 p-3 bg-red-50 border border-red-200 text-red-700 rounded-lg text-sm dark:bg-red-900/30 dark:border-red-800 dark:text-red-400",
                    {msg.clone()}
                }
            }
        }

        // Confirmation modal
        if show_modal() {
            div { class: "fixed inset-0 z-50 flex items-center justify-center bg-black/50",
                div { class: "bg-white rounded-xl shadow-xl max-w-md w-full mx-4 p-6 dark:bg-slate-800",
                    h3 { class: "text-base font-semibold text-gray-900 mb-2 dark:text-slate-100", "Generate MOBI Files?" }
                    p { class: "text-sm text-gray-600 mb-6 dark:text-slate-400",
                        "Would you like to generate MOBI files for all existing books in your library? This will queue conversion for all books. You can also do this later."
                    }
                    div { class: "flex justify-end gap-3",
                        button {
                            class: "px-4 py-2 text-sm font-medium text-gray-700 hover:bg-gray-100 rounded-lg dark:text-slate-300 dark:hover:bg-slate-700",
                            onclick: move |_| show_modal.set(false),
                            "Not now"
                        }
                        button {
                            class: "px-4 py-2 text-sm font-medium text-white bg-indigo-600 hover:bg-indigo-700 rounded-lg",
                            onclick: move |_| {
                                show_modal.set(false);
                                spawn(async move {
                                    let _ = kick_ensure_enrichments().await;
                                });
                            },
                            "Yes, generate now"
                        }
                    }
                }
            }
        }
    }
}
