use std::sync::Arc;

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    Route,
    components::{LoginForm, RegisterAdminForm},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct LandingState {
    pub is_authenticated: bool,
    pub has_users: bool,
}

pub(crate) const MIN_PASSWORD_LEN: usize = 12;

#[cfg(feature = "server")]
use {crate::server::AuthSession, bb_core::CoreServices};

#[get("/api/v1/get_landing_state", core_services: axum::Extension<Arc<CoreServices>>, auth_session: axum::Extension<AuthSession>)]
#[tracing::instrument(level = "trace", skip(core_services, auth_session))]
async fn get_landing_state() -> Result<LandingState, ServerFnError> {
    let is_authenticated = auth_session.current_user.as_ref().map(|u| !u.username.is_empty()).unwrap_or(false);

    let users = core_services
        .user_service
        .list_users(None, Some(1))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(LandingState {
        is_authenticated,
        has_users: !users.is_empty(),
    })
}

#[put("/api/v1/login", core_services: axum::Extension<Arc<CoreServices>>, auth_session: axum::Extension<AuthSession>)]
#[tracing::instrument(level = "trace", skip(core_services, auth_session))]
pub(crate) async fn perform_login(username: String, password: String) -> Result<(), ServerFnError> {
    match core_services
        .auth_service
        .is_valid_login(&username, &password)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?
    {
        Some(user) => {
            auth_session.login_user(user.id);
            Ok(())
        }
        None => Err(ServerFnError::new("Invalid username or password")),
    }
}

#[put("/api/v1/register_admin", core_services: axum::Extension<Arc<CoreServices>>, auth_session: axum::Extension<AuthSession>)]
#[tracing::instrument(level = "trace", skip(core_services, auth_session))]
pub(crate) async fn register_admin(username: String, password: String, email: String) -> Result<(), ServerFnError> {
    use std::collections::HashSet;

    use bb_core::{types::Capability, user::NewUser};

    // Server-side password strength validation
    if password.len() < MIN_PASSWORD_LEN {
        return Err(ServerFnError::new("Password must be at least 12 characters"));
    }
    if !password.chars().any(|c| c.is_uppercase()) {
        return Err(ServerFnError::new("Password must contain at least one uppercase letter"));
    }
    if !password.chars().any(|c| c.is_lowercase()) {
        return Err(ServerFnError::new("Password must contain at least one lowercase letter"));
    }
    if !password.chars().any(|c| c.is_ascii_digit()) {
        return Err(ServerFnError::new("Password must contain at least one digit"));
    }
    if !password.chars().any(|c| "!@#$%^&*()_+-=[]{}|;:,.<>?".contains(c)) {
        return Err(ServerFnError::new("Password must contain at least one special character"));
    }

    // Safety check: ensure no users exist yet
    let existing = core_services
        .user_service
        .list_users(None, Some(1))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if !existing.is_empty() {
        return Err(ServerFnError::new("An admin user already exists"));
    }

    let new_user = NewUser::new(username, password, email, HashSet::from([Capability::SuperAdmin])).map_err(|e| ServerFnError::new(e.to_string()))?;

    let user = core_services
        .user_service
        .add_user(new_user)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    auth_session.login_user(user.id);

    Ok(())
}

#[component]
pub(crate) fn LandingPage() -> Element {
    let navigator = use_navigator();
    let landing_state = use_server_future(get_landing_state)?;

    use_effect(move || {
        if let Some(Ok(ref state)) = landing_state() {
            if state.is_authenticated {
                navigator.push(Route::BooksPage {});
            }
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

        div { class: "min-h-screen bg-gray-100 flex items-center justify-center p-4",
            match landing_state() {
                None => rsx! {
                    div { class: "text-gray-500 text-sm", "Loading…" }
                },
                Some(Err(e)) => rsx! {
                    div { class: "bg-white rounded-2xl shadow-lg p-8 max-w-md w-full text-red-600 text-sm",
                        "Unable to load page: {e}"
                    }
                },
                Some(Ok(LandingState { is_authenticated: true, .. })) => rsx! {
                    div { class: "text-gray-500 text-sm", "Redirecting…" }
                },
                Some(Ok(LandingState { has_users: false, .. })) => rsx! {
                    RegisterAdminForm {}
                },
                _ => rsx! {
                    LoginForm {}
                },
            }
        }
    }
}
