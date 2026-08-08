use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    Route,
    components::{LoginForm, RegisterAdminForm},
    password::{password_is_valid, password_requirements},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct LandingState {
    pub is_authenticated: bool,
    pub has_users: bool,
}

#[cfg(feature = "server")]
use {crate::password::validate_password_strength, crate::routes::server_helpers::to_server_err, crate::server::AuthSession, bb_core::CoreServices};

/// Returns the configured SSO button label when SSO is enabled, `None`
/// otherwise. Used by `LoginForm` to conditionally render the SSO button.
#[get("/api/v1/get_sso_config", oidc_config: Option<axum::Extension<std::sync::Arc<crate::OidcConfig>>>)]
pub(crate) async fn get_sso_config() -> Result<Option<String>, ServerFnError> {
    Ok(oidc_config.map(|ext| ext.0.button_label().to_string()))
}

#[get("/api/v1/get_landing_state", core_services: axum::Extension<std::sync::Arc<CoreServices>>, auth_session: axum::Extension<AuthSession>)]
async fn get_landing_state() -> Result<LandingState, ServerFnError> {
    let is_authenticated = auth_session.current_user.as_ref().is_some_and(|u| !u.username.is_empty());

    let users = core_services.user_service.list_users(None, Some(1)).await.map_err(to_server_err)?;

    Ok(LandingState {
        is_authenticated,
        has_users: !users.is_empty(),
    })
}

/// Returns `None` when the user is fully logged in.
/// Returns `Some(user_token)` when credentials are correct but
/// `change_password_on_login` is set — the session is **not** started yet.
#[put("/api/v1/login", core_services: axum::Extension<std::sync::Arc<CoreServices>>, auth_session: axum::Extension<AuthSession>)]
pub(crate) async fn perform_login(username: String, password: String) -> Result<Option<String>, ServerFnError> {
    match core_services.auth_service.is_valid_login(&username, &password).await.map_err(to_server_err)? {
        Some(user) if user.change_password_on_login => Ok(Some(user.token.to_string())),
        Some(user) => {
            auth_session.login_user(user.id);
            Ok(None)
        }
        None => Err(ServerFnError::new("Invalid username or password")),
    }
}

/// Changes the password for a user who has not yet been logged in
/// (identified by their token returned from `perform_login`).
/// Only succeeds when `change_password_on_login` is set, then starts
/// the session.
#[post(
    "/api/v1/change_password_and_login",
    core_services: axum::Extension<std::sync::Arc<CoreServices>>,
    auth_session: axum::Extension<AuthSession>
)]
pub(crate) async fn change_password_and_login(user_token: String, new_password: String) -> Result<(), ServerFnError> {
    use bb_core::user::UserToken;

    validate_password_strength(&new_password)?;

    let token: UserToken = user_token.parse().map_err(|_| ServerFnError::new("Invalid token"))?;

    let mut user = core_services
        .user_service
        .find_by_token(token)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("User not found"))?;

    // Guard: only usable when the flag is set
    if !user.change_password_on_login {
        return Err(ServerFnError::new("Password change not required"));
    }

    user.password_hash = bb_core::user::User::encrypt_password(new_password).map_err(to_server_err)?;
    user.change_password_on_login = false;

    core_services.user_service.update_user(user.clone()).await.map_err(to_server_err)?;

    auth_session.login_user(user.id);

    Ok(())
}

#[put("/api/v1/register_admin", core_services: axum::Extension<std::sync::Arc<CoreServices>>, auth_session: axum::Extension<AuthSession>)]
pub(crate) async fn register_admin(username: String, full_name: String, password: String, email: String) -> Result<(), ServerFnError> {
    use std::collections::HashSet;

    use bb_core::{types::Capability, user::NewUser};

    // Server-side full name validation
    if full_name.trim().is_empty() {
        return Err(ServerFnError::new("Full name is required"));
    }

    validate_password_strength(&password)?;

    // Safety check: ensure no users exist yet
    let existing = core_services.user_service.list_users(None, Some(1)).await.map_err(to_server_err)?;

    if !existing.is_empty() {
        return Err(ServerFnError::new("An admin user already exists"));
    }

    let new_user = NewUser::new(username, password, email, HashSet::from([Capability::SuperAdmin]), full_name, false).map_err(to_server_err)?;

    let user = core_services.user_service.add_user(new_user).await.map_err(to_server_err)?;

    auth_session.login_user(user.id);

    Ok(())
}

#[component]
pub(crate) fn LandingPage(login_failed: Option<u8>) -> Element {
    let navigator = use_navigator();
    let landing_state = use_server_future(get_landing_state)?;
    let mut change_pw_token: Signal<Option<String>> = use_signal(|| None);

    // Match `perform_login`'s error text exactly so SSO failures are
    // indistinguishable from bad passwords (no leak that SSO was attempted).
    // `is_some()` rather than `== Some(1)` so any value of the param triggers
    // the message — the OIDC callback always writes `1`, but defending against
    // future producers writing `true`/empty/etc. is essentially free.
    let initial_error: Option<String> = login_failed.is_some().then(|| "Invalid username or password".to_string());

    use_effect(move || {
        if let Some(Ok(ref state)) = landing_state()
            && state.is_authenticated
        {
            navigator.push(Route::BooksPage {});
        }
    });

    rsx! {
        document::Stylesheet { href: asset!("/assets/tailwind.css") }
        document::Link { rel: "icon", href: asset!("/assets/favicon.ico") }
        document::Link {
            rel: "apple-touch-icon",
            sizes: "180x180",
            href: asset!("/assets/apple-touch-icon.png"),
        }

        div { class: "min-h-screen bg-gray-100 dark:bg-slate-900 flex items-center justify-center p-4",
            if let Some(token) = change_pw_token() {
                ForceChangePasswordForm {
                    user_token: token,
                    on_changed: move |()| { navigator.push(Route::BooksPage {}); },
                    on_cancel: move |()| change_pw_token.set(None),
                }
            } else {
                match landing_state() {
                    None => rsx! {
                        div { class: "text-gray-500 dark:text-slate-400 text-sm", "Loading…" }
                    },
                    Some(Err(e)) => rsx! {
                        div { class: "bg-white dark:bg-slate-800 rounded-2xl shadow-lg p-8 max-w-md w-full text-red-600 text-sm",
                            "Unable to load page: {e}"
                        }
                    },
                    Some(Ok(LandingState { is_authenticated: true, .. })) => rsx! {
                        div { class: "text-gray-500 dark:text-slate-400 text-sm", "Redirecting…" }
                    },
                    Some(Ok(LandingState { has_users: false, .. })) => rsx! {
                        RegisterAdminForm {}
                    },
                    _ => rsx! {
                        LoginForm {
                            on_must_change: move |token| change_pw_token.set(Some(token)),
                            initial_error: initial_error.clone(),
                        }
                    },
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ForceChangePasswordForm
// ---------------------------------------------------------------------------

#[component]
fn ForceChangePasswordForm(user_token: String, on_changed: EventHandler<()>, on_cancel: EventHandler<()>) -> Element {
    let mut password = use_signal(String::new);
    let mut confirm_password = use_signal(String::new);
    let mut error_msg: Signal<Option<String>> = use_signal(|| None);
    let mut saving = use_signal(|| false);

    let pw_touched = use_memo(move || !password().is_empty());
    let requirements = use_memo(move || password_requirements(&password()));
    let confirm_touched = use_memo(move || !confirm_password().is_empty());
    let passwords_match = use_memo(move || password() == confirm_password());

    // autofocus doesn't fire on dynamically inserted elements, so focus manually.
    use_effect(move || {
        spawn(async move {
            let _ = document::eval("document.getElementById('fcp-password')?.focus()").await;
        });
    });

    rsx! {
        div { class: "bg-white dark:bg-slate-800 rounded-2xl shadow-lg w-full max-w-md",
            div { class: "pt-8 pb-2",
                img {
                    src: asset!("/assets/BookBoss-Banner.png"),
                    alt: "BookBoss",
                    class: "w-full h-auto",
                }
            }
            form {
                class: "p-8",
                onsubmit: move |e| {
                    e.prevent_default();
                    let pw = password();
                    let cpw = confirm_password();

                    if !password_is_valid(&pw) {
                        error_msg.set(Some(
                            "Password does not meet all of the requirements listed above.".to_string(),
                        ));
                        return;
                    }
                    if pw != cpw {
                        error_msg.set(Some("Passwords do not match.".to_string()));
                        return;
                    }

                    error_msg.set(None);
                    saving.set(true);

                    let tok = user_token.clone();
                    spawn(async move {
                        match change_password_and_login(tok, pw).await {
                            Ok(()) => on_changed.call(()),
                            Err(ServerFnError::ServerError { message, .. }) => {
                                error_msg.set(Some(message));
                                saving.set(false);
                            }
                            Err(e) => {
                                error_msg.set(Some(e.to_string()));
                                saving.set(false);
                            }
                        }
                    });
                },

                h2 { class: "text-2xl font-bold text-gray-800 dark:text-slate-100 mb-1 text-center", "Change Password" }
                p { class: "text-sm text-gray-500 dark:text-slate-400 text-center mb-6",
                    "Your account requires a new password before you can continue."
                }

                if let Some(msg) = error_msg() {
                    div { class: "mb-4 p-3 bg-red-50 border border-red-200 text-red-700 rounded-lg text-sm",
                        {msg}
                    }
                }

                div { class: "mb-4",
                    label { class: "block text-sm font-medium text-gray-700 dark:text-slate-300 mb-1",
                        r#for: "fcp-password",
                        "New Password"
                    }
                    input {
                        id: "fcp-password",
                        r#type: "password",
                        class: "w-full px-3 py-2 border border-gray-300 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-gray-900 dark:text-slate-100 placeholder-gray-400 dark:placeholder-slate-400 focus:outline-hidden focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500",
                        placeholder: "Choose a strong password",
                        value: password,
                        oninput: move |e| password.set(e.value()),
                        disabled: saving,
                        autofocus: true,
                    }
                    if pw_touched() {
                        div { class: "mt-2 space-y-1",
                            for (rule, satisfied) in requirements() {
                                div {
                                    class: if satisfied {
                                        "flex items-center gap-1.5 text-xs text-green-600"
                                    } else {
                                        "flex items-center gap-1.5 text-xs text-gray-400"
                                    },
                                    span { if satisfied { "✓" } else { "○" } }
                                    span { {rule} }
                                }
                            }
                        }
                    }
                }

                div { class: "mb-6",
                    label { class: "block text-sm font-medium text-gray-700 dark:text-slate-300 mb-1",
                        r#for: "fcp-confirm",
                        "Confirm Password"
                    }
                    input {
                        id: "fcp-confirm",
                        r#type: "password",
                        class: "w-full px-3 py-2 border border-gray-300 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-gray-900 dark:text-slate-100 placeholder-gray-400 dark:placeholder-slate-400 focus:outline-hidden focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500",
                        placeholder: "Re-enter your password",
                        value: confirm_password,
                        oninput: move |e| confirm_password.set(e.value()),
                        disabled: saving,
                    }
                    if confirm_touched() {
                        div {
                            class: if passwords_match() {
                                "mt-1 flex items-center gap-1.5 text-xs text-green-600"
                            } else {
                                "mt-1 flex items-center gap-1.5 text-xs text-red-500"
                            },
                            span { if passwords_match() { "✓" } else { "✗" } }
                            span {
                                if passwords_match() { "Passwords match" } else { "Passwords do not match" }
                            }
                        }
                    }
                }

                button {
                    class: "w-full py-2 px-4 bg-indigo-600 hover:bg-indigo-700 disabled:bg-indigo-400 text-white font-semibold rounded-lg transition-colors mb-3",
                    r#type: "submit",
                    disabled: saving(),
                    if saving() { "Saving…" } else { "Set New Password" }
                }

                button {
                    class: "w-full py-2 px-4 border border-gray-300 dark:border-slate-600 text-gray-700 dark:text-slate-300 font-medium rounded-lg hover:bg-gray-50 dark:hover:bg-slate-700 transition-colors",
                    r#type: "button",
                    disabled: saving(),
                    onclick: move |_| on_cancel.call(()),
                    "Cancel"
                }
            }
        }
    }
}
