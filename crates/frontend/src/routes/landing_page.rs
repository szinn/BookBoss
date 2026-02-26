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

#[server]
async fn get_landing_state() -> Result<LandingState, ServerFnError> {
    use std::sync::Arc;

    use bb_core::CoreServices;

    use crate::server::AuthSession;

    let ctx = FullstackContext::current()
        .ok_or_else(|| ServerFnError::new("Not in a server context"))?;

    let core_services: Arc<CoreServices> = ctx
        .extension()
        .ok_or_else(|| ServerFnError::new("CoreServices not available"))?;

    let auth_session: AuthSession = ctx
        .extension()
        .ok_or_else(|| ServerFnError::new("AuthSession not available"))?;

    let is_authenticated = auth_session
        .current_user
        .as_ref()
        .map(|u| !u.username.is_empty())
        .unwrap_or(false);

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

#[server]
pub(crate) async fn perform_login(
    username: String,
    password: String,
) -> Result<(), ServerFnError> {
    use std::sync::Arc;

    use bb_core::CoreServices;

    use crate::server::AuthSession;

    let ctx = FullstackContext::current()
        .ok_or_else(|| ServerFnError::new("Not in a server context"))?;

    let core_services: Arc<CoreServices> = ctx
        .extension()
        .ok_or_else(|| ServerFnError::new("CoreServices not available"))?;

    let auth_session: AuthSession = ctx
        .extension()
        .ok_or_else(|| ServerFnError::new("AuthSession not available"))?;

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

#[server]
pub(crate) async fn register_admin(
    username: String,
    password: String,
    email: String,
) -> Result<(), ServerFnError> {
    use std::collections::HashSet;
    use std::sync::Arc;

    use bb_core::{CoreServices, types::Capability, user::NewUser};

    use crate::server::AuthSession;

    // Server-side password strength validation
    if password.len() < 12 {
        return Err(ServerFnError::new("Password must be at least 12 characters"));
    }
    if !password.chars().any(|c| c.is_uppercase()) {
        return Err(ServerFnError::new(
            "Password must contain at least one uppercase letter",
        ));
    }
    if !password.chars().any(|c| c.is_lowercase()) {
        return Err(ServerFnError::new(
            "Password must contain at least one lowercase letter",
        ));
    }
    if !password.chars().any(|c| c.is_ascii_digit()) {
        return Err(ServerFnError::new("Password must contain at least one digit"));
    }
    if !password
        .chars()
        .any(|c| "!@#$%^&*()_+-=[]{}|;:,.<>?".contains(c))
    {
        return Err(ServerFnError::new(
            "Password must contain at least one special character",
        ));
    }

    let ctx = FullstackContext::current()
        .ok_or_else(|| ServerFnError::new("Not in a server context"))?;

    let core_services: Arc<CoreServices> = ctx
        .extension()
        .ok_or_else(|| ServerFnError::new("CoreServices not available"))?;

    let auth_session: AuthSession = ctx
        .extension()
        .ok_or_else(|| ServerFnError::new("AuthSession not available"))?;

    // Safety check: ensure no users exist yet
    let existing = core_services
        .user_service
        .list_users(None, Some(1))
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if !existing.is_empty() {
        return Err(ServerFnError::new("An admin user already exists"));
    }

    let new_user =
        NewUser::new(username, password, email, HashSet::from([Capability::Admin]))
            .map_err(|e| ServerFnError::new(e.to_string()))?;

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
