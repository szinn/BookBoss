mod devices_section;

use devices_section::DevicesSectionContent;
use dioxus::prelude::*;
#[cfg(feature = "server")]
use {
    crate::FrontendConfig,
    crate::routes::server_helpers::{authenticated_user, to_server_err},
    crate::server::AuthSession,
    bb_core::{
        CoreServices,
        device::{DeviceToken, OnRemovalAction},
        library::LibraryToken,
        types::{Capability, EmailAddress},
        user::User,
    },
    chrono::{DateTime, Utc},
    std::str::FromStr,
    std::sync::Arc,
};

use crate::Route;

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct ProfileInfo {
    full_name: String,
    email: String,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct DeviceRow {
    token: String,
    name: String,
    device_type: String,
    on_removal_action: String,
    sync_token_display: String,
    sync_url: String,
    last_synced_at: String,
    companion_shelf_name: Option<String>,
    companion_shelf_token: Option<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct OpdsInfo {
    has_access: bool,
    password: String,
    opds_url: String,
    koreader_url: String,
    username: String,
}

// ---------------------------------------------------------------------------
// Server functions
// ---------------------------------------------------------------------------

#[get(
    "/api/v1/profile/context",
    auth_session: axum::Extension<AuthSession>
)]
async fn get_profile_context() -> Result<(), ServerFnError> {
    authenticated_user(&auth_session)?;
    Ok(())
}

#[get(
    "/api/v1/profile/opds",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>,
    frontend_config: axum::Extension<Arc<FrontendConfig>>
)]
async fn get_opds_info() -> Result<OpdsInfo, ServerFnError> {
    let auth_user = authenticated_user(&auth_session)?;
    let user_id = auth_user.id();

    let user = core_services
        .user_service
        .find_by_id(user_id)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("User not found"))?;

    if !user.has_capability(Capability::OpdsAccess) {
        return Ok(OpdsInfo {
            has_access: false,
            password: String::new(),
            opds_url: String::new(),
            koreader_url: String::new(),
            username: String::new(),
        });
    }

    let password = core_services.opds_service.get_or_create_password(&user).await.map_err(to_server_err)?;

    let base = frontend_config.base_url.trim_end_matches('/');
    let opds_url = format!("{base}/opds");
    let koreader_url = format!("{base}/koreader");

    Ok(OpdsInfo {
        has_access: true,
        password,
        opds_url,
        koreader_url,
        username: user.username.clone(),
    })
}

#[post(
    "/api/v1/profile/opds/regenerate",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn regenerate_opds_password() -> Result<String, ServerFnError> {
    let auth_user = authenticated_user(&auth_session)?;
    let user_id = auth_user.id();

    let user = core_services
        .user_service
        .find_by_id(user_id)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("User not found"))?;

    if !user.has_capability(Capability::OpdsAccess) {
        return Err(ServerFnError::new("OPDS access not enabled"));
    }

    let pw = core_services.opds_service.regenerate_password(&user).await.map_err(to_server_err)?;

    Ok(pw)
}

#[get(
    "/api/v1/profile/info",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn get_profile_info() -> Result<ProfileInfo, ServerFnError> {
    let user_id = authenticated_user(&auth_session)?.id();

    let user = core_services
        .user_service
        .find_by_id(user_id)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("User not found"))?;

    Ok(ProfileInfo {
        full_name: user.full_name.clone(),
        email: user.email_address.to_string(),
    })
}

#[post(
    "/api/v1/profile/update",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn update_profile(full_name: String, email: String) -> Result<(), ServerFnError> {
    let full_name = full_name.trim().to_string();
    if full_name.is_empty() {
        return Err(ServerFnError::new("Full name must not be empty"));
    }
    let email_address = EmailAddress::new(&email).map_err(to_server_err)?;

    let user_id = authenticated_user(&auth_session)?.id();

    let existing = core_services
        .user_service
        .find_by_id(user_id)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("User not found"))?;

    core_services
        .user_service
        .update_user(User {
            full_name,
            email_address,
            ..existing
        })
        .await
        .map_err(to_server_err)?;

    Ok(())
}

#[post(
    "/api/v1/profile/change-password",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn change_password(current: String, new_password: String) -> Result<(), ServerFnError> {
    if new_password.trim().is_empty() {
        return Err(ServerFnError::new("New password must not be empty"));
    }

    let user_id = authenticated_user(&auth_session)?.id();

    let existing = core_services
        .user_service
        .find_by_id(user_id)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("User not found"))?;

    if !existing.check_password(&current) {
        return Err(ServerFnError::new("Current password is incorrect"));
    }

    let new_hash = User::encrypt_password(&new_password).map_err(to_server_err)?;

    core_services
        .user_service
        .update_user(User {
            password_hash: new_hash,
            ..existing
        })
        .await
        .map_err(to_server_err)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Device server functions
// ---------------------------------------------------------------------------

#[cfg(feature = "server")]
fn removal_action_to_str(a: &OnRemovalAction) -> &'static str {
    match a {
        OnRemovalAction::Nothing => "nothing",
        OnRemovalAction::MarkRead => "mark_read",
        OnRemovalAction::MarkDnf => "mark_dnf",
    }
}

#[cfg(feature = "server")]
fn parse_removal_action(s: &str) -> Result<OnRemovalAction, ServerFnError> {
    match s {
        "nothing" => Ok(OnRemovalAction::Nothing),
        "mark_read" => Ok(OnRemovalAction::MarkRead),
        "mark_dnf" => Ok(OnRemovalAction::MarkDnf),
        _ => Err(ServerFnError::new("Invalid on_removal_action")),
    }
}

#[cfg(feature = "server")]
pub(crate) fn kobo_sync_url(base_url: &str, sync_token: &str) -> String {
    format!("{base_url}/kobo/{sync_token}")
}

#[get(
    "/api/v1/profile/devices",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>,
    frontend_config: axum::Extension<Arc<FrontendConfig>>
)]
async fn get_devices_for_profile() -> Result<Vec<DeviceRow>, ServerFnError> {
    let user_id = authenticated_user(&auth_session)?.id();

    let devices = core_services.device_service.list_devices_for_user(user_id).await.map_err(to_server_err)?;

    let mut rows = Vec::with_capacity(devices.len());
    for device in devices {
        let companion = core_services.device_service.get_companion_shelf(device.id).await.map_err(to_server_err)?;

        let sync_token_display = device.token.to_string().trim_start_matches("DV_").to_string();
        let sync_url = kobo_sync_url(frontend_config.base_url.trim_end_matches('/'), &sync_token_display);
        let last_synced_at = humanize_synced_at(device.last_synced_at);

        rows.push(DeviceRow {
            token: device.token.to_string(),
            name: device.name,
            device_type: device.device_type,
            on_removal_action: removal_action_to_str(&device.on_removal_action).to_string(),
            sync_token_display,
            sync_url,
            last_synced_at,
            companion_shelf_name: companion.as_ref().map(|s| s.name.clone()),
            companion_shelf_token: companion.as_ref().map(|s| s.token.to_string()),
        });
    }
    Ok(rows)
}

#[cfg(feature = "server")]
fn humanize_synced_at(ts: Option<DateTime<Utc>>) -> String {
    let Some(ts) = ts else { return "Never".to_string() };
    let secs = (Utc::now() - ts).num_seconds().max(0);
    match secs {
        s if s < 60 => "Just now".to_string(),
        s if s < 3600 => format!("{} min ago", s / 60),
        s if s < 86400 => format!("{} hr ago", s / 3600),
        s if s < 604_800 => format!("{} days ago", s / 86400),
        _ => ts.format("%Y-%m-%d").to_string(),
    }
}

#[get(
    "/api/v1/profile/devices/default-name",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn get_default_device_name() -> Result<String, ServerFnError> {
    let user_id = authenticated_user(&auth_session)?.id();

    core_services.device_service.default_device_name(user_id).await.map_err(to_server_err)
}

#[post(
    "/api/v1/profile/devices/create",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn create_device_for_profile(name: String, device_type: String, on_removal_action: String) -> Result<(), ServerFnError> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(ServerFnError::new("Device name must not be empty"));
    }

    let user_id = authenticated_user(&auth_session)?.id();

    let action = parse_removal_action(&on_removal_action)?;

    core_services
        .device_service
        .create_device(user_id, name, device_type, action)
        .await
        .map_err(to_server_err)?;

    Ok(())
}

#[post(
    "/api/v1/profile/devices/update",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn update_device_for_profile(token: String, name: String, on_removal_action: String) -> Result<(), ServerFnError> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(ServerFnError::new("Device name must not be empty"));
    }

    let user_id = authenticated_user(&auth_session)?.id();

    let device_token = DeviceToken::from_str(&token).map_err(to_server_err)?;
    let action = parse_removal_action(&on_removal_action)?;

    core_services
        .device_service
        .update_device(device_token, name, action, user_id)
        .await
        .map_err(to_server_err)?;

    Ok(())
}

#[post(
    "/api/v1/profile/devices/delete",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn delete_device_for_profile(token: String, delete_shelf: bool) -> Result<(), ServerFnError> {
    let user_id = authenticated_user(&auth_session)?.id();

    let device_token = DeviceToken::from_str(&token).map_err(to_server_err)?;

    core_services
        .device_service
        .delete_device(device_token, delete_shelf, user_id)
        .await
        .map_err(to_server_err)?;

    Ok(())
}

#[post(
    "/api/v1/profile/devices/reset-sync",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn reset_device_sync_for_profile(token: String) -> Result<(), ServerFnError> {
    let user_id = authenticated_user(&auth_session)?.id();

    let device_token = DeviceToken::from_str(&token).map_err(to_server_err)?;

    core_services
        .device_service
        .reset_device_sync(device_token, user_id)
        .await
        .map_err(to_server_err)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Default library server function
// ---------------------------------------------------------------------------

#[post(
    "/api/v1/profile/default-library",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn set_default_library_for_profile(library_token: String) -> Result<(), ServerFnError> {
    let user_id = authenticated_user(&auth_session)?.id();

    let lib_token = LibraryToken::from_str(&library_token).map_err(|_| ServerFnError::new("Invalid library token"))?;

    core_services
        .library_service
        .set_default_library(user_id, lib_token)
        .await
        .map_err(to_server_err)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// ProfilePage
// ---------------------------------------------------------------------------

#[component]
pub(crate) fn ProfilePage() -> Element {
    let navigator = use_navigator();
    let auth = use_server_future(get_profile_context)?;

    use_effect(move || {
        if let Some(Err(_)) = auth() {
            navigator.replace(Route::LandingPage { login_failed: None });
        }
    });

    rsx! {
        div { class: "flex-1 overflow-auto p-8",
            div { class: "max-w-lg mx-auto flex flex-col gap-10",

                // ── Profile ──────────────────────────────────────────────
                section {
                    h2 { class: "text-lg font-semibold text-gray-900 dark:text-slate-100 mb-4", "Profile" }
                    ProfileSectionContent {}
                }

                hr { class: "border-gray-200 dark:border-slate-700" }

                // ── Default Library (only if 2+ libraries) ────────────────
                DefaultLibrarySectionContent {}

                // ── My Devices ────────────────────────────────────────────
                section {
                    DevicesSectionContent {}
                }

                hr { class: "border-gray-200 dark:border-slate-700" }

                // ── OPDS ─────────────────────────────────────────────────
                OpdsSectionContent {}
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// Profile section
// ---------------------------------------------------------------------------

#[component]
fn ProfileSectionContent() -> Element {
    let info = use_server_future(get_profile_info)?;

    // Profile info signals
    let mut full_name = use_signal(String::new);
    let mut email = use_signal(String::new);
    let mut profile_saving = use_signal(|| false);
    let mut profile_saved = use_signal(|| false);
    let mut profile_error: Signal<Option<String>> = use_signal(|| None);

    // Change-password modal signals
    let mut pw_modal_open = use_signal(|| false);
    let mut current_pw = use_signal(String::new);
    let mut new_pw = use_signal(String::new);
    let mut confirm_pw = use_signal(String::new);
    let mut pw_saving = use_signal(|| false);
    let mut pw_error: Signal<Option<String>> = use_signal(|| None);

    use_effect(move || {
        if let Some(Ok(i)) = info() {
            full_name.set(i.full_name.clone());
            email.set(i.email.clone());
        }
    });

    let passwords_match = use_memo(move || confirm_pw().is_empty() || new_pw() == confirm_pw());

    let mut close_pw_modal = move || {
        pw_modal_open.set(false);
        current_pw.set(String::new());
        new_pw.set(String::new());
        confirm_pw.set(String::new());
        pw_error.set(None);
    };

    let input_class = "w-full rounded-md border border-gray-300 dark:border-slate-600 px-3 py-1.5 text-sm focus:outline-none focus:ring-1 \
                       focus:ring-indigo-500 bg-white dark:bg-slate-700 text-gray-900 dark:text-slate-100";
    let label_class = "block text-sm font-medium text-gray-700 dark:text-slate-300 mb-1";

    rsx! {
        // ── Profile info card ─────────────────────────────────────────────
        div { class: "rounded-lg border border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-800 px-4 py-4 flex flex-col gap-3",
            div {
                label { class: label_class, "Full Name" }
                input {
                    r#type: "text",
                    class: input_class,
                    value: full_name,
                    oninput: move |e| {
                        profile_saved.set(false);
                        full_name.set(e.value());
                    },
                }
            }
            div {
                label { class: label_class, "Email" }
                input {
                    r#type: "email",
                    class: input_class,
                    value: email,
                    oninput: move |e| {
                        profile_saved.set(false);
                        email.set(e.value());
                    },
                }
            }
            if let Some(err) = profile_error() {
                p { class: "text-xs text-red-600", {err} }
            }
            div { class: "flex items-center justify-between",
                // Left — Change Password trigger
                button {
                    class: "px-3 py-1.5 text-sm font-medium rounded border border-gray-300 dark:border-slate-600 text-gray-700 dark:text-slate-300 hover:bg-gray-50 dark:hover:bg-slate-700",
                    onclick: move |_| pw_modal_open.set(true),
                    "Change Password"
                }
                // Right — Save
                div { class: "flex items-center gap-3",
                    if profile_saved() {
                        span { class: "text-xs text-green-600", "Saved!" }
                    }
                    button {
                        class: "px-3 py-1.5 text-sm font-medium rounded bg-indigo-600 text-white hover:bg-indigo-700 disabled:opacity-50",
                        disabled: profile_saving(),
                        onclick: move |_| {
                            let name = full_name();
                            let em = email();
                            profile_saving.set(true);
                            profile_saved.set(false);
                            profile_error.set(None);
                            spawn(async move {
                                match update_profile(name, em).await {
                                    Ok(()) => profile_saved.set(true),
                                    Err(e) => profile_error.set(Some(e.to_string())),
                                }
                                profile_saving.set(false);
                            });
                        },
                        if profile_saving() { "Saving…" } else { "Save" }
                    }
                }
            }
        }

        // ── Change Password modal ─────────────────────────────────────────
        if pw_modal_open() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/40",
                tabindex: -1,
                onmounted: move |e| async move { let _ = e.set_focus(true).await; },
                onkeydown: move |e| { if e.key() == Key::Escape { close_pw_modal(); } },
                div { class: "bg-white dark:bg-slate-800 rounded-2xl shadow-xl w-full max-w-sm p-6",
                    h3 { class: "text-base font-semibold text-gray-900 dark:text-slate-100 mb-4", "Change Password" }

                    div { class: "flex flex-col gap-3",
                        div {
                            label { class: label_class, "Current password" }
                            input {
                                r#type: "password",
                                class: input_class,
                                value: current_pw,
                                oninput: move |e| current_pw.set(e.value()),
                            }
                        }
                        div {
                            label { class: label_class, "New password" }
                            input {
                                r#type: "password",
                                class: input_class,
                                value: new_pw,
                                oninput: move |e| new_pw.set(e.value()),
                            }
                        }
                        div {
                            label { class: label_class, "Confirm new password" }
                            input {
                                r#type: "password",
                                class: input_class,
                                value: confirm_pw,
                                oninput: move |e| confirm_pw.set(e.value()),
                            }
                            if !passwords_match() {
                                p { class: "text-xs text-red-600 mt-1", "Passwords do not match" }
                            }
                        }
                        if let Some(err) = pw_error() {
                            p { class: "text-xs text-red-600", {err} }
                        }
                    }

                    div { class: "flex justify-end gap-3 mt-5",
                        button {
                            class: "px-3 py-1.5 text-sm font-medium rounded border border-gray-300 dark:border-slate-600 text-gray-700 dark:text-slate-300 hover:bg-gray-50 dark:hover:bg-slate-700",
                            disabled: pw_saving(),
                            onclick: move |_| close_pw_modal(),
                            "Cancel"
                        }
                        button {
                            class: "px-3 py-1.5 text-sm font-medium rounded bg-indigo-600 text-white hover:bg-indigo-700 disabled:opacity-50",
                            disabled: pw_saving() || !passwords_match() || new_pw().is_empty(),
                            onclick: move |_| {
                                let cur = current_pw();
                                let new = new_pw();
                                pw_saving.set(true);
                                pw_error.set(None);
                                spawn(async move {
                                    match change_password(cur, new).await {
                                        Ok(()) => close_pw_modal(),
                                        Err(e) => pw_error.set(Some(e.to_string())),
                                    }
                                    pw_saving.set(false);
                                });
                            },
                            if pw_saving() { "Saving…" } else { "Change Password" }
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Default Library section
// ---------------------------------------------------------------------------

#[component]
fn DefaultLibrarySectionContent() -> Element {
    let user_libs = use_server_future(crate::components::get_user_libraries)?;
    let default_lib = use_server_future(crate::components::get_default_library_token_for_user)?;

    let libs: Vec<(String, String)> = user_libs()
        .and_then(std::result::Result::ok)
        .map(|libs| libs.into_iter().map(|l| (l.token, l.name)).collect())
        .unwrap_or_default();

    // Only render if user has 2+ libraries
    if libs.len() < 2 {
        return rsx! {};
    }

    let current_default = default_lib().and_then(std::result::Result::ok).unwrap_or_default();

    let mut saving = use_signal(|| false);
    let mut error: Signal<Option<String>> = use_signal(|| None);
    let mut saved = use_signal(|| false);

    let select_class = "rounded-md border border-gray-300 dark:border-slate-600 px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-indigo-500 \
                        bg-white dark:bg-slate-700 text-gray-900 dark:text-slate-100";

    rsx! {
        section {
            h2 { class: "text-lg font-semibold text-gray-900 dark:text-slate-100 mb-4", "Default Library" }
            div { class: "rounded-lg border border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-800 px-4 py-4 flex flex-col gap-3",
                p { class: "text-sm text-gray-600 dark:text-slate-400",
                    "Choose which library is shown by default when you open BookBoss."
                }
                div { class: "flex items-center gap-3",
                    select {
                        class: select_class,
                        value: current_default.clone(),
                        disabled: saving(),
                        onchange: move |e| {
                            let token = e.value();
                            saving.set(true);
                            saved.set(false);
                            error.set(None);
                            spawn(async move {
                                match set_default_library_for_profile(token.clone()).await {
                                    Ok(()) => {
                                        *crate::components::ACTIVE_LIBRARY.write() = Some(token);
                                        saved.set(true);
                                    }
                                    Err(e) => error.set(Some(e.to_string())),
                                }
                                saving.set(false);
                            });
                        },
                        for (token, name) in &libs {
                            option {
                                value: token,
                                selected: *token == current_default,
                                {name.clone()}
                            }
                        }
                    }
                    if saving() {
                        span { class: "text-sm text-gray-500 dark:text-slate-400", "Saving\u{2026}" }
                    } else if saved() {
                        span { class: "text-sm text-green-600", "Saved!" }
                    }
                }
                if let Some(err) = error() {
                    p { class: "text-xs text-red-600", {err} }
                }
            }
        }

        hr { class: "border-gray-200 dark:border-slate-700" }
    }
}

// ---------------------------------------------------------------------------
// OPDS section
// ---------------------------------------------------------------------------

#[component]
fn OpdsSectionContent() -> Element {
    let opds_info = use_server_future(get_opds_info)?;
    let mut password = use_signal(String::new);
    let mut regenerating = use_signal(|| false);
    let mut copied = use_signal(|| false);
    let mut show_password = use_signal(|| false);
    let mut error_msg: Signal<Option<String>> = use_signal(|| None);

    use_effect(move || {
        if let Some(Ok(info)) = opds_info() {
            password.set(info.password.clone());
        }
    });

    let info = match opds_info() {
        Some(Ok(info)) => info,
        Some(Err(_)) => return rsx! {},
        None => return rsx! {},
    };

    if !info.has_access {
        return rsx! {};
    }

    rsx! {
        section {
            h2 { class: "text-lg font-semibold text-gray-900 dark:text-slate-100 mb-4", "OPDS / KOReader" }
            div { class: "rounded-lg border border-gray-200 dark:border-slate-700 bg-white dark:bg-slate-800 px-4 py-4 flex flex-col gap-3",
                p { class: "text-sm text-gray-600 dark:text-slate-400",
                    "Use these credentials to connect your e-reader app (KOReader, Moon+ Reader, etc.) to your BookBoss library."
                }

                div {
                    span { class: "block text-sm font-medium text-gray-700 dark:text-slate-300 mb-1", "Catalog URL" }
                    div { class: "flex items-center gap-2",
                        code { class: "flex-1 text-sm bg-gray-50 dark:bg-slate-700 rounded px-3 py-1.5 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-slate-100 select-all",
                            "{info.opds_url}"
                        }
                    }
                }

                div {
                    span { class: "block text-sm font-medium text-gray-700 dark:text-slate-300 mb-1", "KOReader Sync URL" }
                    div { class: "flex items-center gap-2",
                        code { class: "flex-1 text-sm bg-gray-50 dark:bg-slate-700 rounded px-3 py-1.5 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-slate-100 select-all",
                            "{info.koreader_url}"
                        }
                    }
                }

                div { class: "flex items-start justify-between gap-4",
                    div {
                        span { class: "block text-sm font-medium text-gray-700 dark:text-slate-300 mb-1", "Username" }
                        code { class: "text-sm bg-gray-50 dark:bg-slate-700 rounded px-3 py-1.5 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-slate-100 select-all inline-block",
                            "{info.username}"
                        }
                    }

                    div {
                        span { class: "block text-sm font-medium text-gray-700 dark:text-slate-300 mb-1", "Password" }
                        div { class: "flex items-center gap-2",
                            button {
                                class: "text-sm bg-gray-50 dark:bg-slate-700 rounded px-3 py-1.5 border border-gray-200 dark:border-slate-600 text-gray-900 dark:text-slate-100 font-mono cursor-pointer hover:bg-gray-100 dark:hover:bg-slate-600 transition-colors w-[calc(12ch+1.5rem)] text-center",
                                title: "Click to copy password",
                                onclick: move |_| {
                                    let pw_val = password();
                                    spawn(async move {
                                        document::eval(&format!(
                                            "var t=document.createElement('textarea');\
                                             t.value='{pw_val}';\
                                             t.style.cssText='position:fixed;opacity:0';\
                                             document.body.appendChild(t);\
                                             t.select();\
                                             document.execCommand('copy');\
                                             document.body.removeChild(t);"
                                        ));
                                        copied.set(true);
                                        let mut timer = document::eval("setTimeout(() => dioxus.send(true), 1500)");
                                        let _ = timer.recv::<bool>().await;
                                        copied.set(false);
                                    });
                                },
                                if copied() { "Copied!" } else if show_password() { "{password}" } else { "••••••••••••" }
                            }
                        button {
                            class: "px-2 py-1.5 rounded border border-gray-300 dark:border-slate-600 text-gray-500 dark:text-slate-400 hover:bg-gray-50 dark:hover:bg-slate-700 hover:text-gray-700 dark:hover:text-slate-200",
                            title: if show_password() { "Hide password" } else { "Show password" },
                            onclick: move |_| show_password.set(!show_password()),
                            if show_password() {
                                // Eye-slash icon
                                svg {
                                    xmlns: "http://www.w3.org/2000/svg",
                                    class: "w-4 h-4",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "2",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    path { d: "M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24" }
                                    line { x1: "1", y1: "1", x2: "23", y2: "23" }
                                }
                            } else {
                                // Eye icon
                                svg {
                                    xmlns: "http://www.w3.org/2000/svg",
                                    class: "w-4 h-4",
                                    view_box: "0 0 24 24",
                                    fill: "none",
                                    stroke: "currentColor",
                                    stroke_width: "2",
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    path { d: "M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" }
                                    circle { cx: "12", cy: "12", r: "3" }
                                }
                            }
                        }
                        button {
                            class: "px-2 py-1.5 text-xs font-medium rounded border border-gray-300 dark:border-slate-600 text-gray-700 dark:text-slate-300 hover:bg-gray-50 dark:hover:bg-slate-700 disabled:opacity-50",
                            disabled: regenerating(),
                            onclick: move |_| {
                                regenerating.set(true);
                                error_msg.set(None);
                                copied.set(false);
                                spawn(async move {
                                    match regenerate_opds_password().await {
                                        Ok(pw) => password.set(pw),
                                        Err(e) => error_msg.set(Some(e.to_string())),
                                    }
                                    regenerating.set(false);
                                });
                            },
                            if regenerating() { "Regenerating…" } else { "Regenerate" }
                        }
                    }
                }
                }

                if let Some(err) = error_msg() {
                    p { class: "text-xs text-red-600", {err} }
                }
            }
        }
    }
}
