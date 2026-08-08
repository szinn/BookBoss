#[cfg(feature = "server")]
use bb_core::{CoreServices, types::Capability, user::NewUser};
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
pub(crate) struct UserAdminRow {
    pub token: String,
    pub username: String,
    pub full_name: String,
    pub email: String,
    pub capabilities: Vec<String>,
}

impl UserAdminRow {
    pub fn role_label(&self) -> &'static str {
        if self.capabilities.iter().any(|c| c == "SuperAdmin") {
            "Super Admin"
        } else if self.capabilities.iter().any(|c| c == "Admin") {
            "Admin"
        } else {
            "User"
        }
    }

    pub fn role_sort_key(&self) -> u8 {
        if self.capabilities.iter().any(|c| c == "SuperAdmin") {
            0
        } else if self.capabilities.iter().any(|c| c == "Admin") {
            1
        } else {
            2
        }
    }
}

/// Simple library row for user assignment UI (no counts needed).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct LibraryAssignRow {
    pub token: String,
    pub name: String,
    pub is_system: bool,
}

/// The user's personal library (owned by them, not a system library).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PersonalLibraryInfo {
    pub token: String,
    pub name: String,
}

/// Returned by `get_user_assigned_libraries` — the set of library tokens
/// assigned to the user plus their current default.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct UserLibraryAssignment {
    pub assigned_tokens: Vec<String>,
    pub default_token: String,
    pub personal_library: Option<PersonalLibraryInfo>,
}

// ---------------------------------------------------------------------------
// Server functions
// ---------------------------------------------------------------------------

#[get(
    "/api/v1/admin/libraries/simple",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn list_all_libraries_simple() -> Result<Vec<LibraryAssignRow>, ServerFnError> {
    let user = authenticated_user(&auth_session)?;

    if !user.permissions.contains("SuperAdmin") && !user.permissions.contains("Admin") {
        return Err(ServerFnError::new("Insufficient permissions"));
    }

    let entries = core_services.library_service.list_libraries().await.map_err(to_server_err)?;

    let mut rows: Vec<LibraryAssignRow> = entries
        .into_iter()
        .map(|e| LibraryAssignRow {
            token: e.library.token.to_string(),
            name: e.library.name,
            is_system: e.library.is_system,
        })
        .collect();

    rows.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(rows)
}

#[post(
    "/api/v1/admin/users/libraries",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn get_user_assigned_libraries(user_token: String) -> Result<UserLibraryAssignment, ServerFnError> {
    use bb_core::user::UserToken;

    let actor = authenticated_user(&auth_session)?;

    if !actor.permissions.contains("SuperAdmin") && !actor.permissions.contains("Admin") {
        return Err(ServerFnError::new("Insufficient permissions"));
    }

    let user_token_parsed: UserToken = user_token.parse().map_err(|_| ServerFnError::new("Invalid user token"))?;

    let user = core_services
        .user_service
        .find_by_token(user_token_parsed)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("User not found"))?;

    let libraries = core_services.library_service.libraries_for_user(user.id).await.map_err(to_server_err)?;

    let assigned_tokens: Vec<String> = libraries.iter().map(|l| l.token.to_string()).collect();

    let default_token = core_services.library_service.get_default_library_token(user.id).await.map_err(to_server_err)?;

    let personal_library = core_services
        .library_service
        .find_personal_library_for_user(user.id)
        .await
        .map_err(to_server_err)?
        .map(|l| PersonalLibraryInfo {
            token: l.token.to_string(),
            name: l.name,
        });

    Ok(UserLibraryAssignment {
        assigned_tokens,
        default_token,
        personal_library,
    })
}

#[post(
    "/api/v1/admin/users/assign-libraries",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn assign_user_libraries(
    user_token: String,
    library_tokens: Vec<String>,
    default_library_token: String,
    create_personal_library: bool,
    personal_library_name: Option<String>,
) -> Result<(), ServerFnError> {
    use bb_core::{library::LibraryToken, user::UserToken};

    let actor = authenticated_user(&auth_session)?;

    if !actor.permissions.contains("SuperAdmin") && !actor.permissions.contains("Admin") {
        return Err(ServerFnError::new("Insufficient permissions"));
    }

    let user_token_parsed: UserToken = user_token.parse().map_err(|_| ServerFnError::new("Invalid user token"))?;

    let user = core_services
        .user_service
        .find_by_token(user_token_parsed)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("User not found"))?;

    // Optionally create a personal library first. Capture its token so we can
    // include it in the desired assignment set and make it the default.
    let new_personal_token: Option<String> = if create_personal_library {
        let name = personal_library_name
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| ServerFnError::new("Personal library name is required"))?;
        let lib = core_services
            .library_service
            .create_personal_library_for_user(user.id, name.trim().to_string())
            .await
            .map_err(to_server_err)?;
        Some(lib.token.to_string())
    } else {
        None
    };

    // Compute the desired set of library token strings. Always include the
    // newly created personal library so the assignment diff doesn't remove it.
    let mut desired_tokens: std::collections::HashSet<String> = library_tokens.iter().cloned().collect();
    if let Some(ref tok) = new_personal_token {
        desired_tokens.insert(tok.clone());
    }

    // Get the user's current library assignments.
    let current_libs = core_services.library_service.libraries_for_user(user.id).await.map_err(to_server_err)?;
    let current_tokens: std::collections::HashSet<String> = current_libs.iter().map(|l| l.token.to_string()).collect();

    // Remove libraries that are no longer desired.
    for lib in current_libs.iter().filter(|l| !desired_tokens.contains(&l.token.to_string())) {
        core_services
            .library_service
            .unassign_library_from_user(user.id, lib.token)
            .await
            .map_err(to_server_err)?;
    }

    // Add newly desired libraries that are not already assigned.
    for token_str in desired_tokens.iter().filter(|t| !current_tokens.contains(*t)) {
        let lib_token: LibraryToken = token_str.parse().map_err(|_| ServerFnError::new("Invalid library token"))?;
        core_services
            .library_service
            .assign_library_to_user(user.id, lib_token)
            .await
            .map_err(to_server_err)?;
    }

    // Set the default library. The newly created personal library takes
    // precedence; otherwise use whatever the frontend selected.
    let effective_default = new_personal_token.as_deref().unwrap_or(default_library_token.as_str());
    if !effective_default.is_empty() {
        let def_token: LibraryToken = effective_default.parse().map_err(|_| ServerFnError::new("Invalid default library token"))?;
        core_services
            .library_service
            .set_default_library(user.id, def_token)
            .await
            .map_err(to_server_err)?;
    }

    Ok(())
}

#[post(
    "/api/v1/admin/users/rename-personal-library",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn admin_rename_personal_library(user_token: String, new_name: String) -> Result<(), ServerFnError> {
    use bb_core::user::UserToken;

    let actor = authenticated_user(&auth_session)?;
    if !actor.permissions.contains("SuperAdmin") && !actor.permissions.contains("Admin") {
        return Err(ServerFnError::new("Insufficient permissions"));
    }

    let user_token_parsed: UserToken = user_token.parse().map_err(|_| ServerFnError::new("Invalid user token"))?;
    let user = core_services
        .user_service
        .find_by_token(user_token_parsed)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("User not found"))?;

    let lib = core_services
        .library_service
        .find_personal_library_for_user(user.id)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("No personal library found"))?;

    core_services.library_service.rename_library(lib.token, new_name).await.map_err(to_server_err)
}

#[post(
    "/api/v1/admin/users/delete-personal-library",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
async fn admin_delete_personal_library(user_token: String) -> Result<(), ServerFnError> {
    use bb_core::user::UserToken;

    let actor = authenticated_user(&auth_session)?;
    if !actor.permissions.contains("SuperAdmin") && !actor.permissions.contains("Admin") {
        return Err(ServerFnError::new("Insufficient permissions"));
    }

    let user_token_parsed: UserToken = user_token.parse().map_err(|_| ServerFnError::new("Invalid user token"))?;
    let user = core_services
        .user_service
        .find_by_token(user_token_parsed)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("User not found"))?;

    let lib = core_services
        .library_service
        .find_personal_library_for_user(user.id)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("No personal library found"))?;

    // delete_library re-parents shelves to All Books and resets default settings.
    core_services.library_service.delete_library(lib.token).await.map_err(to_server_err)
}

#[post(
    "/api/v1/admin/users/list",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn list_users_admin() -> Result<Vec<UserAdminRow>, ServerFnError> {
    let user = authenticated_user(&auth_session)?;

    if !user.permissions.contains("SuperAdmin") && !user.permissions.contains("Admin") {
        return Err(ServerFnError::new("Insufficient permissions"));
    }

    let mut users = core_services.user_service.list_users(None, None).await.map_err(to_server_err)?;

    users.sort_by(|a, b| {
        let a_key = role_sort_key_caps(&a.capabilities);
        let b_key = role_sort_key_caps(&b.capabilities);
        a_key.cmp(&b_key).then(a.username.cmp(&b.username))
    });

    Ok(users
        .into_iter()
        .map(|u| {
            let caps: Vec<String> = u.capabilities.iter().map(|c| c.as_str().to_string()).collect();
            UserAdminRow {
                token: u.token.to_string(),
                username: u.username,
                full_name: u.full_name,
                email: u.email_address.as_str().to_string(),
                capabilities: caps,
            }
        })
        .collect())
}

#[cfg(feature = "server")]
fn role_sort_key_caps(caps: &bb_core::types::Capabilities) -> u8 {
    if caps.contains(&Capability::SuperAdmin) {
        0
    } else if caps.contains(&Capability::Admin) {
        1
    } else {
        2
    }
}

#[put(
    "/api/v1/admin/users/create",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn admin_create_user(
    username: String,
    full_name: String,
    email: String,
    password: String,
    capabilities: Vec<String>,
) -> Result<String, ServerFnError> {
    use std::collections::HashSet;

    let actor = authenticated_user(&auth_session)?;

    if !actor.permissions.contains("SuperAdmin") && !actor.permissions.contains("Admin") {
        return Err(ServerFnError::new("Insufficient permissions"));
    }

    let caps: HashSet<Capability> = parse_capabilities(&capabilities)?;

    if caps.contains(&Capability::SuperAdmin) {
        return Err(ServerFnError::new("Cannot assign Super Admin role"));
    }
    if caps.contains(&Capability::Admin) && !actor.permissions.contains("SuperAdmin") {
        return Err(ServerFnError::new("Only Super Admin can create Admin users"));
    }

    let new_user = NewUser::new(username, password, email, caps, full_name, true).map_err(|e| {
        let msg = e.to_string();
        if msg.contains("Constraint") || msg.contains("unique") || msg.contains("duplicate") {
            ServerFnError::new("Username or email address is already in use")
        } else {
            ServerFnError::new(msg)
        }
    })?;

    let user = core_services.user_service.add_user(new_user).await.map_err(|e| {
        let msg = e.to_string();
        if msg.contains("Constraint") || msg.contains("unique") || msg.contains("duplicate") {
            ServerFnError::new("Username or email address is already in use")
        } else {
            ServerFnError::new(msg)
        }
    })?;

    Ok(user.token.to_string())
}

#[post(
    "/api/v1/admin/users/update",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn admin_update_user(
    token: String,
    full_name: String,
    email: String,
    password: Option<String>,
    capabilities: Vec<String>,
) -> Result<(), ServerFnError> {
    use std::collections::HashSet;

    use bb_core::user::UserToken;

    let actor = authenticated_user(&auth_session)?;

    if !actor.permissions.contains("SuperAdmin") && !actor.permissions.contains("Admin") {
        return Err(ServerFnError::new("Insufficient permissions"));
    }

    let user_token: UserToken = token.parse().map_err(|_| ServerFnError::new("Invalid user token"))?;

    let mut user = core_services
        .user_service
        .find_by_token(user_token)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("User not found"))?;

    if user.capabilities.contains(&Capability::SuperAdmin) && !actor.permissions.contains("SuperAdmin") {
        return Err(ServerFnError::new("Only Super Admin can edit a Super Admin user"));
    }

    let new_caps: HashSet<Capability> = parse_capabilities(&capabilities)?;

    if new_caps.contains(&Capability::SuperAdmin) && !user.capabilities.contains(&Capability::SuperAdmin) {
        return Err(ServerFnError::new("Cannot assign Super Admin role"));
    }
    if user.capabilities.contains(&Capability::SuperAdmin) && !new_caps.contains(&Capability::SuperAdmin) {
        return Err(ServerFnError::new("Cannot remove Super Admin role"));
    }
    if new_caps.contains(&Capability::Admin) && !actor.permissions.contains("SuperAdmin") {
        return Err(ServerFnError::new("Only Super Admin can assign Admin role"));
    }

    let full_name = full_name.trim().to_string();
    if full_name.is_empty() {
        return Err(ServerFnError::new("Full name is required"));
    }

    user.full_name = full_name;
    user.email_address = bb_core::types::EmailAddress::new(email).map_err(to_server_err)?;
    user.capabilities = new_caps;

    if let Some(pw) = password.filter(|p| !p.is_empty()) {
        user.password_hash = bb_core::user::User::encrypt_password(pw).map_err(to_server_err)?;
        let is_self = bb_core::user::UserToken::new(actor.id()).to_string() == token;
        if !is_self {
            user.change_password_on_login = true;
        }
    }

    core_services.user_service.update_user(user).await.map_err(to_server_err)?;

    Ok(())
}

#[post(
    "/api/v1/admin/users/delete",
    auth_session: axum::Extension<AuthSession>,
    core_services: axum::Extension<Arc<CoreServices>>
)]
pub(crate) async fn admin_delete_user(token: String) -> Result<(), ServerFnError> {
    use bb_core::user::UserToken;

    let actor = authenticated_user(&auth_session)?;

    if !actor.permissions.contains("SuperAdmin") && !actor.permissions.contains("Admin") {
        return Err(ServerFnError::new("Insufficient permissions"));
    }

    if bb_core::user::UserToken::new(actor.id()).to_string() == token {
        return Err(ServerFnError::new("You cannot delete your own account"));
    }

    let user_token: UserToken = token.parse().map_err(|_| ServerFnError::new("Invalid user token"))?;

    let user = core_services
        .user_service
        .find_by_token(user_token)
        .await
        .map_err(to_server_err)?
        .ok_or_else(|| ServerFnError::new("User not found"))?;

    if user.capabilities.contains(&Capability::SuperAdmin) {
        return Err(ServerFnError::new("Cannot delete the Super Admin user"));
    }

    core_services.user_service.delete_user(user.id).await.map_err(to_server_err)?;

    Ok(())
}

#[post(
    "/api/v1/admin/generate-password",
    auth_session: axum::Extension<AuthSession>
)]
pub(crate) async fn generate_password() -> Result<String, ServerFnError> {
    authenticated_user(&auth_session)?;

    Ok(make_password())
}

#[cfg(feature = "server")]
fn make_password() -> String {
    use rand::RngExt;

    const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    const LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
    const DIGITS: &[u8] = b"0123456789";
    const SPECIAL: &[u8] = b"!@#$%^&*()_+-=[]{}|;:,.<>?";
    const ALL: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()_+-=[]{}|;:,.<>?";

    let mut rng = rand::rng();
    // Guarantee one of each required character class
    let mut pw: Vec<u8> = vec![
        UPPER[rng.random_range(0..UPPER.len())],
        LOWER[rng.random_range(0..LOWER.len())],
        DIGITS[rng.random_range(0..DIGITS.len())],
        SPECIAL[rng.random_range(0..SPECIAL.len())],
    ];
    for _ in 4..16 {
        pw.push(ALL[rng.random_range(0..ALL.len())]);
    }
    // Fisher-Yates shuffle
    for i in (1..pw.len()).rev() {
        let j = rng.random_range(0..=i);
        pw.swap(i, j);
    }
    String::from_utf8(pw).expect("all bytes are valid ASCII")
}

#[cfg(feature = "server")]
fn parse_capabilities(capabilities: &[String]) -> Result<bb_core::types::Capabilities, ServerFnError> {
    use std::collections::HashSet;
    let mut caps = HashSet::new();
    for s in capabilities {
        let cap = match s.as_str() {
            "Admin" => Capability::Admin,
            "ApproveImports" => Capability::ApproveImports,
            "ConvertBook" => Capability::ConvertBook,
            "DeleteBook" => Capability::DeleteBook,
            "EditBook" => Capability::EditBook,
            "OpdsAccess" => Capability::OpdsAccess,
            "SuperAdmin" => Capability::SuperAdmin,
            other => return Err(ServerFnError::new(format!("Unknown capability: {other}"))),
        };
        caps.insert(cap);
    }
    Ok(caps)
}

// ---------------------------------------------------------------------------
// UsersSection component
// ---------------------------------------------------------------------------

#[component]
pub(crate) fn UsersSection(is_super_admin: bool, current_user_token: String) -> Element {
    // Increment to trigger a reload of the user list.
    let mut refresh = use_signal(|| 0u32);

    let users_resource = use_resource(move || async move {
        let _ = refresh(); // subscribe
        list_users_admin().await
    });

    let mut modal_target: Signal<Option<Option<UserAdminRow>>> = use_signal(|| None);
    let mut delete_target: Signal<Option<UserAdminRow>> = use_signal(|| None);
    let mut delete_error: Signal<Option<String>> = use_signal(|| None);
    let mut deleting = use_signal(|| false);

    rsx! {
        div { class: "w-full max-w-3xl",
            // Header
            div { class: "flex items-center justify-between mb-6",
                h2 { class: "text-lg font-semibold text-gray-900 dark:text-slate-100", "Users" }
                button {
                    class: "px-3 py-1.5 text-sm font-medium rounded bg-indigo-600 text-white hover:bg-indigo-700",
                    onclick: move |_| modal_target.set(Some(None)),
                    "+ Add User"
                }
            }

            // Error from delete
            if let Some(msg) = delete_error() {
                div { class: "mb-4 p-3 bg-red-50 border border-red-200 text-red-700 rounded-lg text-sm dark:bg-red-900/30 dark:border-red-800 dark:text-red-400",
                    {msg}
                }
            }

            // User table
            match users_resource() {
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
                        table { class: "w-full text-sm",
                            thead {
                                tr { class: "bg-gray-50 border-b border-gray-200 dark:bg-slate-800 dark:border-slate-700",
                                    th { class: "px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wide dark:text-slate-400", "Username" }
                                    th { class: "px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wide dark:text-slate-400", "Full Name" }
                                    th { class: "px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wide dark:text-slate-400", "Role" }
                                    th { class: "px-4 py-3 text-right text-xs font-medium text-gray-500 uppercase tracking-wide dark:text-slate-400", "Actions" }
                                }
                            }
                            tbody { class: "divide-y divide-gray-100 dark:divide-slate-700",
                                for row in rows {
                                    {
                                        let is_self = row.token == current_user_token;
                                        let is_super = row.role_sort_key() == 0;
                                        let can_edit = is_super_admin || !is_super;
                                        let row_edit = row.clone();
                                        let row_del = row.clone();
                                        rsx! {
                                            tr { class: "hover:bg-gray-50 dark:hover:bg-slate-700",
                                                td { class: "px-4 py-3 font-medium text-gray-900 dark:text-slate-100", "{row.username}" }
                                                td { class: "px-4 py-3 text-gray-600 dark:text-slate-400", "{row.full_name}" }
                                                td { class: "px-4 py-3",
                                                    span {
                                                        class: match row.role_sort_key() {
                                                            0 => "inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-purple-100 text-purple-800 dark:bg-purple-900 dark:text-purple-300",
                                                            1 => "inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-indigo-100 text-indigo-800 dark:bg-indigo-900 dark:text-indigo-300",
                                                            _ => "inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-gray-100 text-gray-700 dark:bg-slate-700 dark:text-slate-300",
                                                        },
                                                        { row.role_label() }
                                                    }
                                                }
                                                td { class: "px-4 py-3",
                                                    div { class: "flex items-center justify-end gap-2",
                                                        button {
                                                            class: if can_edit {
                                                                "p-1.5 text-gray-500 hover:text-indigo-600 hover:bg-indigo-50 dark:hover:bg-indigo-900/30 rounded"
                                                            } else {
                                                                "p-1.5 text-gray-300 cursor-not-allowed rounded dark:text-slate-600"
                                                            },
                                                            disabled: !can_edit,
                                                            title: "Edit user",
                                                            onclick: move |_| {
                                                                if can_edit {
                                                                    modal_target.set(Some(Some(row_edit.clone())));
                                                                }
                                                            },
                                                            "✎"
                                                        }
                                                        if !is_super && !is_self {
                                                            button {
                                                                class: "p-1.5 text-gray-500 hover:text-red-600 hover:bg-red-50 dark:hover:bg-red-900/30 rounded",
                                                                title: "Delete user",
                                                                onclick: move |_| {
                                                                    delete_error.set(None);
                                                                    delete_target.set(Some(row_del.clone()));
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
                        if let Some(Ok(ref rows)) = users_resource() {
                            if rows.is_empty() {
                                div { class: "px-4 py-8 text-center text-gray-400 text-sm dark:text-slate-500", "No users found." }
                            }
                        }
                    }
                },
            }
        }

        // ── User create/edit modal ──────────────────────────────────────────
        if let Some(target) = modal_target() {
            UserModal {
                is_self: target.as_ref().is_some_and(|r| r.token == current_user_token),
                editing: target,
                is_super_admin,
                on_close: move || modal_target.set(None),
                on_saved: move || {
                    modal_target.set(None);
                    *refresh.write() += 1;
                },
            }
        }

        // ── Delete confirmation dialog ──────────────────────────────────────
        if let Some(target) = delete_target() {
            div {
                class: "fixed inset-0 z-50 flex items-center justify-center bg-black/40",
                tabindex: -1,
                onmounted: move |e| async move { let _ = e.set_focus(true).await; },
                onkeydown: move |e| { if e.key() == Key::Escape { delete_target.set(None); } },
                div { class: "bg-white rounded-2xl shadow-xl w-full max-w-sm p-6 dark:bg-slate-800",
                    h3 { class: "text-base font-semibold text-gray-900 mb-2 dark:text-slate-100", "Delete User" }
                    p { class: "text-sm text-gray-600 mb-6 dark:text-slate-400",
                        "Are you sure you want to delete "
                        span { class: "font-medium text-gray-900 dark:text-slate-100", "{target.username}" }
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
                            disabled: deleting(),
                            onclick: move |_| {
                                let tok = target.token.clone();
                                deleting.set(true);
                                spawn(async move {
                                    match admin_delete_user(tok).await {
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

// ---------------------------------------------------------------------------
// UserModal component
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq)]
enum RoleChoice {
    SuperAdmin,
    Admin,
    User,
}

impl RoleChoice {
    fn from_caps(caps: &[String]) -> Self {
        if caps.iter().any(|c| c == "SuperAdmin") {
            Self::SuperAdmin
        } else if caps.iter().any(|c| c == "Admin") {
            Self::Admin
        } else {
            Self::User
        }
    }
}

#[component]
fn UserModal(editing: Option<UserAdminRow>, is_self: bool, is_super_admin: bool, on_close: EventHandler<()>, on_saved: EventHandler<()>) -> Element {
    let is_edit = editing.is_some();

    let initial_role = editing.as_ref().map_or(RoleChoice::User, |r| RoleChoice::from_caps(&r.capabilities));
    let editing_is_super = initial_role == RoleChoice::SuperAdmin;
    let initial_user_caps: Vec<String> = editing
        .as_ref()
        .map(|r| {
            r.capabilities
                .iter()
                .filter(|c| c.as_str() != "Admin" && c.as_str() != "SuperAdmin")
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    let mut username = use_signal(|| editing.as_ref().map(|r| r.username.clone()).unwrap_or_default());
    let mut full_name = use_signal(|| editing.as_ref().map(|r| r.full_name.clone()).unwrap_or_default());
    let mut email = use_signal(|| editing.as_ref().map(|r| r.email.clone()).unwrap_or_default());
    let mut password = use_signal(String::new);
    let mut role = use_signal(|| initial_role);
    let mut user_caps: Signal<Vec<String>> = use_signal(|| if is_edit { initial_user_caps } else { vec!["OpdsAccess".to_string()] });
    let mut error_msg: Signal<Option<String>> = use_signal(|| None);
    let mut saving = use_signal(|| false);
    let mut generating = use_signal(|| false);

    // Library assignment state
    // checked_library_tokens: set of library tokens the admin has checked
    let mut checked_library_tokens: Signal<Vec<String>> = use_signal(Vec::new);
    // default_library_token: the selected default library (from checked ones)
    let mut default_library_token = use_signal(String::new);
    // create_personal_library: whether to create a personal library on save (create
    // mode only)
    let mut create_personal_library = use_signal(|| false);
    // personal_library_name: name for new (create mode) or existing (edit mode)
    // personal library
    let mut personal_library_name = use_signal(String::new);
    // personal_name_dirty: true once the user has manually edited the personal
    // library name field
    let mut personal_name_dirty = use_signal(|| false);
    // edit mode: token of the user's existing personal library (None if they don't
    // have one)
    let mut personal_library_token: Signal<Option<String>> = use_signal(|| None);
    // edit mode: original name of the personal library (to detect changes)
    let mut personal_name_original = use_signal(String::new);
    // edit mode: whether to show the inline delete confirmation
    let mut confirm_delete_shown = use_signal(|| false);
    // edit mode: whether to delete the personal library on save
    let mut delete_personal_on_save = use_signal(|| false);

    // Load all libraries on mount
    let libraries_resource = use_resource(move || async move { list_all_libraries_simple().await });

    // For edit mode: load the user's current library assignments
    let edit_token_for_load = editing.as_ref().map(|r| r.token.clone());
    let user_assignment_resource = use_resource(move || {
        let tok = edit_token_for_load.clone();
        async move {
            if let Some(t) = tok {
                Some(get_user_assigned_libraries(t).await)
            } else {
                None
            }
        }
    });

    // When user assignment data loads, populate signals
    use_effect(move || {
        if let Some(Some(Ok(assignment))) = user_assignment_resource() {
            checked_library_tokens.set(assignment.assigned_tokens.clone());
            default_library_token.set(assignment.default_token.clone());
            if let Some(ref p) = assignment.personal_library {
                personal_library_token.set(Some(p.token.clone()));
                personal_library_name.set(p.name.clone());
                personal_name_original.set(p.name.clone());
            }
        }
    });

    // When full_name changes and this is a create form, update personal library
    // name (only if user hasn't manually changed it)
    use_effect(move || {
        if !is_edit && !personal_name_dirty() {
            let name = full_name();
            if !name.trim().is_empty() {
                personal_library_name.set(format!("{}'s Library", name.trim()));
            }
        }
    });

    const GRANTABLE: [(&str, &str); 5] = [
        ("ApproveImports", "Approve Imports"),
        ("ConvertBook", "Convert Books"),
        ("DeleteBook", "Delete Books"),
        ("EditBook", "Edit Books"),
        ("OpdsAccess", "OPDS Access"),
    ];

    rsx! {
        div {
            class: "fixed inset-0 z-50 flex items-center justify-center bg-black/40",
            tabindex: -1,
            onmounted: move |e| async move { let _ = e.set_focus(true).await; },
            onkeydown: move |e| {
                if e.key() == Key::Escape {
                    on_close.call(());
                }
            },
            div { class: "bg-white rounded-2xl shadow-xl w-full max-w-lg max-h-[90vh] overflow-y-auto dark:bg-slate-800",
                div { class: "p-6",
                    h3 { class: "text-base font-semibold text-gray-900 mb-5 dark:text-slate-100",
                        if is_edit { "Edit User" } else { "Add User" }
                    }

                    if let Some(msg) = error_msg() {
                        div { class: "mb-4 p-3 bg-red-50 border border-red-200 text-red-700 rounded-lg text-sm dark:bg-red-900/30 dark:border-red-800 dark:text-red-400",
                            {msg}
                        }
                    }

                    // Username
                    div { class: "mb-4",
                        label { class: "block text-sm font-medium text-gray-700 mb-1 dark:text-slate-300", "Username" }
                        input {
                            r#type: "text",
                            class: if is_edit {
                                "w-full px-3 py-2 border border-gray-200 rounded-lg bg-gray-50 text-gray-500 cursor-not-allowed text-sm dark:border-slate-700 dark:bg-slate-700/50 dark:text-slate-400"
                            } else {
                                "w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-hidden focus:ring-2 focus:ring-indigo-500 text-sm dark:bg-slate-700 dark:border-slate-600 dark:text-slate-100"
                            },
                            placeholder: "username",
                            value: username,
                            readonly: is_edit,
                            oninput: move |e| {
                                if !is_edit { username.set(e.value()); }
                            },
                            disabled: saving,
                        }
                    }

                    // Full Name
                    div { class: "mb-4",
                        label { class: "block text-sm font-medium text-gray-700 mb-1 dark:text-slate-300", "Full Name" }
                        input {
                            r#type: "text",
                            class: "w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-hidden focus:ring-2 focus:ring-indigo-500 text-sm dark:bg-slate-700 dark:border-slate-600 dark:text-slate-100",
                            placeholder: "Full name",
                            value: full_name,
                            oninput: move |e| full_name.set(e.value()),
                            disabled: saving,
                        }
                    }

                    // Email
                    div { class: "mb-4",
                        label { class: "block text-sm font-medium text-gray-700 mb-1 dark:text-slate-300", "Email Address" }
                        input {
                            r#type: "email",
                            class: "w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-hidden focus:ring-2 focus:ring-indigo-500 text-sm dark:bg-slate-700 dark:border-slate-600 dark:text-slate-100",
                            placeholder: "user@example.com",
                            value: email,
                            oninput: move |e| email.set(e.value()),
                            disabled: saving,
                        }
                    }

                    // Password
                    div { class: "mb-4",
                        label { class: "block text-sm font-medium text-gray-700 mb-1 dark:text-slate-300", "Password" }
                        if is_edit {
                            p { class: "text-xs text-gray-400 mb-1 dark:text-slate-500",
                                if is_self {
                                    "Leave blank to keep your current password."
                                } else {
                                    "Leave blank to keep current password. Setting a new password will require the user to change it on next login."
                                }
                            }
                        }
                        div { class: "flex gap-2",
                            input {
                                r#type: "text",
                                class: "flex-1 min-w-0 px-3 py-2 border border-gray-300 rounded-lg focus:outline-hidden focus:ring-2 focus:ring-indigo-500 text-sm font-mono dark:bg-slate-700 dark:border-slate-600 dark:text-slate-100",
                                placeholder: if is_edit { "New password (optional)" } else { "Password" },
                                value: password,
                                oninput: move |e| password.set(e.value()),
                                disabled: saving,
                            }
                            button {
                                class: "px-3 py-2 text-xs font-medium rounded-lg border border-gray-300 text-gray-700 hover:bg-gray-50 disabled:opacity-50 whitespace-nowrap dark:border-slate-600 dark:text-slate-300 dark:hover:bg-slate-700",
                                disabled: saving() || generating(),
                                title: "Generate password",
                                onclick: move |_| {
                                    generating.set(true);
                                    spawn(async move {
                                        match generate_password().await {
                                            Ok(pw) => password.set(pw),
                                            Err(e) => error_msg.set(Some(e.to_string())),
                                        }
                                        generating.set(false);
                                    });
                                },
                                if generating() { "…" } else { "Generate" }
                            }
                            button {
                                class: "px-3 py-2 text-xs font-medium rounded-lg border border-gray-300 text-gray-700 hover:bg-gray-50 disabled:opacity-50 dark:border-slate-600 dark:text-slate-300 dark:hover:bg-slate-700",
                                disabled: password().is_empty(),
                                title: "Copy to clipboard",
                                onclick: move |_| {
                                    let pw = password();
                                    if !pw.is_empty() {
                                        // Escape backticks for JS template literal safety.
                                        let escaped = pw.replace('`', "\\`").replace('$', "\\$");
                                        spawn(async move {
                                            let _ = document::eval(&format!("navigator.clipboard.writeText(`{escaped}`)")).await;
                                        });
                                    }
                                },
                                "Copy"
                            }
                        }
                    }

                    // Role
                    div { class: "mb-4",
                        label { class: "block text-sm font-medium text-gray-700 mb-1 dark:text-slate-300", "Role" }
                        select {
                            class: if editing_is_super {
                                "w-full px-3 py-2 border border-gray-200 rounded-lg text-sm bg-gray-50 text-gray-500 cursor-not-allowed dark:border-slate-700 dark:bg-slate-700/50 dark:text-slate-400"
                            } else {
                                "w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-hidden focus:ring-2 focus:ring-indigo-500 text-sm bg-white dark:bg-slate-700 dark:border-slate-600 dark:text-slate-100"
                            },
                            disabled: saving() || editing_is_super,
                            onchange: move |e| {
                                let new_role = match e.value().as_str() {
                                    "SuperAdmin" => RoleChoice::SuperAdmin,
                                    "Admin" => RoleChoice::Admin,
                                    _ => RoleChoice::User,
                                };
                                if new_role == RoleChoice::User {
                                    user_caps.set(Vec::new());
                                }
                                role.set(new_role);
                            },
                            option { value: "User", selected: role() == RoleChoice::User, "User" }
                            option {
                                value: "Admin",
                                selected: role() == RoleChoice::Admin,
                                disabled: !is_super_admin,
                                "Admin"
                            }
                            option {
                                value: "SuperAdmin",
                                selected: role() == RoleChoice::SuperAdmin,
                                disabled: true,
                                "Super Admin"
                            }
                        }
                    }

                    // Capability toggles (User role only)
                    if role() == RoleChoice::User {
                        div { class: "mb-4",
                            label { class: "block text-sm font-medium text-gray-700 mb-2 dark:text-slate-300", "Capabilities" }
                            div { class: "space-y-2 rounded-lg border border-gray-200 p-3 dark:border-slate-700",
                                for (cap_key, cap_label) in GRANTABLE {
                                    {
                                        let key = cap_key.to_string();
                                        let key_remove = key.clone();
                                        let is_checked = user_caps().contains(&key);
                                        rsx! {
                                            label { class: "flex items-center gap-2 cursor-pointer select-none",
                                                input {
                                                    r#type: "checkbox",
                                                    class: "rounded border-gray-300 text-indigo-600 focus:ring-indigo-500",
                                                    checked: is_checked,
                                                    disabled: saving,
                                                    onchange: move |e| {
                                                        if e.checked() {
                                                            user_caps.write().push(key.clone());
                                                        } else {
                                                            user_caps.write().retain(|c| c != &key_remove);
                                                        }
                                                    },
                                                }
                                                span { class: "text-sm text-gray-700 dark:text-slate-300", {cap_label} }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // ── Library assignment ──────────────────────────────────
                    div { class: "mb-4",
                        label { class: "block text-sm font-medium text-gray-700 mb-2 dark:text-slate-300", "Libraries" }

                        match libraries_resource() {
                            None => rsx! {
                                div { class: "text-gray-400 text-xs dark:text-slate-500", "Loading libraries…" }
                            },
                            Some(Err(e)) => rsx! {
                                div { class: "p-2 bg-red-50 border border-red-200 text-red-700 rounded text-xs dark:bg-red-900/30 dark:border-red-800 dark:text-red-400",
                                    "{e}"
                                }
                            },
                            Some(Ok(libs)) => {
                                // Separate system and non-system libraries
                                let non_system: Vec<_> = libs.iter().filter(|l| !l.is_system).collect();
                                let system_libs: Vec<_> = libs.iter().filter(|l| l.is_system).collect();
                                let checked = checked_library_tokens();
                                // Compute the set of checked non-system library tokens for the default picker
                                let checked_non_system: Vec<LibraryAssignRow> = non_system
                                    .iter()
                                    .filter(|l| checked.contains(&l.token))
                                    .map(|l| (*l).clone())
                                    .collect();
                                let checked_system: Vec<LibraryAssignRow> = system_libs
                                    .iter()
                                    .filter(|l| checked.contains(&l.token))
                                    .map(|l| (*l).clone())
                                    .collect();
                                // All checked libraries (for default picker)
                                let all_checked: Vec<LibraryAssignRow> = checked_non_system
                                    .iter()
                                    .chain(checked_system.iter())
                                    .cloned()
                                    .collect();

                                rsx! {
                                    div { class: "rounded-lg border border-gray-200 p-3 space-y-1.5 max-h-40 overflow-y-auto dark:border-slate-600",
                                        if libs.is_empty() {
                                            div { class: "text-xs text-gray-400 dark:text-slate-500", "No libraries configured." }
                                        }
                                        for lib in &libs {
                                            {
                                                let tok = lib.token.clone();
                                                let tok_remove = tok.clone();
                                                let tok_def = tok.clone();
                                                let is_checked = checked_library_tokens().contains(&tok);
                                                rsx! {
                                                    label { class: "flex items-center gap-2 cursor-pointer select-none",
                                                        input {
                                                            r#type: "checkbox",
                                                            class: "rounded border-gray-300 text-indigo-600 focus:ring-indigo-500",
                                                            checked: is_checked,
                                                            disabled: saving,
                                                            onchange: move |e| {
                                                                if e.checked() {
                                                                    checked_library_tokens.write().push(tok.clone());
                                                                    // Auto-select as default if none set yet
                                                                    if default_library_token().is_empty() {
                                                                        default_library_token.set(tok.clone());
                                                                    }
                                                                } else {
                                                                    checked_library_tokens.write().retain(|t| t != &tok_remove);
                                                                    // If this was the default, clear it
                                                                    if default_library_token() == tok_def {
                                                                        default_library_token.set(String::new());
                                                                    }
                                                                }
                                                            },
                                                        }
                                                        span { class: "text-sm text-gray-700 dark:text-slate-300", "{lib.name}" }
                                                        if lib.is_system {
                                                            span { class: "text-xs text-blue-500 font-medium dark:text-blue-400", "(system)" }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // Default library picker (shown when at least one library is checked)
                                    if !all_checked.is_empty() {
                                        div { class: "mt-3",
                                            label { class: "block text-xs font-medium text-gray-600 mb-1 dark:text-slate-400", "Default Library" }
                                            select {
                                                class: "w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-hidden focus:ring-2 focus:ring-indigo-500 text-sm bg-white dark:bg-slate-700 dark:border-slate-600 dark:text-slate-100",
                                                disabled: saving,
                                                value: default_library_token,
                                                onchange: move |e| default_library_token.set(e.value()),
                                                option { value: "", disabled: true, selected: default_library_token().is_empty(), "Select default…" }
                                                for lib in &all_checked {
                                                    option {
                                                        value: lib.token.clone(),
                                                        selected: default_library_token() == lib.token,
                                                        "{lib.name}"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                        }
                    }

                    // ── Personal library ───────────────────────────────────
                    if is_edit {
                        if let Some(_tok) = personal_library_token() {
                            // User has a personal library — show rename / delete controls.
                            div { class: "mb-4",
                                label { class: "block text-sm font-medium text-gray-700 mb-1 dark:text-slate-300",
                                    "Personal Library"
                                }
                                if delete_personal_on_save() {
                                    // Marked for deletion — show undo option.
                                    div { class: "flex items-center gap-2 p-2 bg-red-50 border border-red-200 rounded-lg dark:bg-red-900/30 dark:border-red-800",
                                        span { class: "flex-1 text-sm text-red-600 line-through dark:text-red-400", "{personal_library_name()}" }
                                        span { class: "text-xs text-red-500 dark:text-red-400", "Will be deleted on save" }
                                        button {
                                            r#type: "button",
                                            class: "ml-2 text-xs text-gray-500 hover:text-gray-700 underline dark:text-slate-400 dark:hover:text-slate-200",
                                            disabled: saving,
                                            onclick: move |_| {
                                                delete_personal_on_save.set(false);
                                                confirm_delete_shown.set(false);
                                            },
                                            "Undo"
                                        }
                                    }
                                } else if confirm_delete_shown() {
                                    // Confirmation prompt.
                                    div { class: "p-3 bg-amber-50 border border-amber-200 rounded-lg dark:bg-amber-900/30 dark:border-amber-800",
                                        p { class: "text-sm text-amber-800 mb-2 dark:text-amber-300",
                                            "Delete personal library on save? Shelves will be moved to All Books."
                                        }
                                        div { class: "flex gap-2",
                                            button {
                                                r#type: "button",
                                                class: "px-3 py-1 text-xs font-medium bg-red-600 text-white rounded hover:bg-red-700",
                                                disabled: saving,
                                                onclick: move |_| {
                                                    delete_personal_on_save.set(true);
                                                    confirm_delete_shown.set(false);
                                                },
                                                "Yes, delete"
                                            }
                                            button {
                                                r#type: "button",
                                                class: "px-3 py-1 text-xs font-medium border border-gray-300 text-gray-700 rounded hover:bg-gray-50 dark:border-slate-600 dark:text-slate-300 dark:hover:bg-slate-700",
                                                disabled: saving,
                                                onclick: move |_| confirm_delete_shown.set(false),
                                                "Cancel"
                                            }
                                        }
                                    }
                                } else {
                                    // Normal state — editable name + delete button.
                                    div { class: "flex items-center gap-2",
                                        input {
                                            r#type: "text",
                                            class: "flex-1 px-3 py-2 border border-gray-300 rounded-lg focus:outline-hidden focus:ring-2 focus:ring-indigo-500 text-sm dark:bg-slate-700 dark:border-slate-600 dark:text-slate-100",
                                            value: personal_library_name,
                                            disabled: saving,
                                            oninput: move |e| personal_library_name.set(e.value()),
                                        }
                                        button {
                                            r#type: "button",
                                            class: "p-2 text-red-400 hover:text-red-600 hover:bg-red-50 rounded",
                                            title: "Delete personal library",
                                            disabled: saving,
                                            onclick: move |_| confirm_delete_shown.set(true),
                                            svg {
                                                class: "w-4 h-4",
                                                xmlns: "http://www.w3.org/2000/svg",
                                                fill: "none",
                                                view_box: "0 0 24 24",
                                                stroke_width: "2",
                                                stroke: "currentColor",
                                                path {
                                                    stroke_linecap: "round",
                                                    stroke_linejoin: "round",
                                                    d: "M6 18 18 6M6 6l12 12",
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            // No personal library — offer to create one.
                            div { class: "mb-4",
                                label { class: "flex items-center gap-2 cursor-pointer select-none",
                                    input {
                                        r#type: "checkbox",
                                        class: "rounded border-gray-300 text-indigo-600 focus:ring-indigo-500",
                                        checked: create_personal_library,
                                        disabled: saving,
                                        onchange: move |e| {
                                            let checked = e.checked();
                                            create_personal_library.set(checked);
                                            if checked && !personal_name_dirty() {
                                                let name = full_name().trim().to_string();
                                                if !name.is_empty() {
                                                    personal_library_name.set(format!("{name}'s Library"));
                                                }
                                            }
                                        },
                                    }
                                    span { class: "text-sm font-medium text-gray-700 dark:text-slate-300", "Create personal library" }
                                }
                                if create_personal_library() {
                                    input {
                                        r#type: "text",
                                        class: "mt-2 w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-hidden focus:ring-2 focus:ring-indigo-500 text-sm dark:bg-slate-700 dark:border-slate-600 dark:text-slate-100",
                                        placeholder: "Personal library name",
                                        value: personal_library_name,
                                        oninput: move |e| {
                                            personal_name_dirty.set(true);
                                            personal_library_name.set(e.value());
                                        },
                                        disabled: saving,
                                    }
                                }
                            }
                        }
                    } else {
                        // Create mode — always show create checkbox.
                        div { class: "mb-4",
                            label { class: "flex items-center gap-2 cursor-pointer select-none",
                                input {
                                    r#type: "checkbox",
                                    class: "rounded border-gray-300 text-indigo-600 focus:ring-indigo-500",
                                    checked: create_personal_library,
                                    disabled: saving,
                                    onchange: move |e| {
                                        let checked = e.checked();
                                        create_personal_library.set(checked);
                                        if checked && !personal_name_dirty() {
                                            let name = full_name().trim().to_string();
                                            if !name.is_empty() {
                                                personal_library_name.set(format!("{name}'s Library"));
                                            }
                                        }
                                    },
                                }
                                span { class: "text-sm font-medium text-gray-700 dark:text-slate-300", "Create personal library" }
                            }
                            if create_personal_library() {
                                input {
                                    r#type: "text",
                                    class: "mt-2 w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-hidden focus:ring-2 focus:ring-indigo-500 text-sm dark:bg-slate-700 dark:border-slate-600 dark:text-slate-100",
                                    placeholder: "Personal library name",
                                    value: personal_library_name,
                                    oninput: move |e| {
                                        personal_name_dirty.set(true);
                                        personal_library_name.set(e.value());
                                    },
                                    disabled: saving,
                                }
                            }
                        }
                    }

                    // Actions
                    div { class: "flex justify-end gap-3 pt-2",
                        button {
                            class: "px-4 py-2 text-sm font-medium rounded-lg border border-gray-300 text-gray-700 hover:bg-gray-50 dark:border-slate-600 dark:text-slate-300 dark:hover:bg-slate-700",
                            disabled: saving(),
                            onclick: move |_| on_close.call(()),
                            "Cancel"
                        }
                        button {
                            class: "px-4 py-2 text-sm font-medium rounded-lg bg-indigo-600 text-white hover:bg-indigo-700 disabled:opacity-50",
                            disabled: saving(),
                            onclick: move |_| {
                                let un = username().trim().to_string();
                                let fn_ = full_name().trim().to_string();
                                let em = email().trim().to_string();
                                let pw = password();
                                let chosen_role = role();
                                let caps = user_caps();
                                let lib_tokens = checked_library_tokens();
                                let def_lib = default_library_token();
                                let do_create_personal = create_personal_library();
                                let personal_name = personal_library_name().trim().to_string();
                                let pers_lib_tok = personal_library_token();
                                let do_delete_personal = delete_personal_on_save();
                                let pers_name_original = personal_name_original();

                                if !is_edit && un.is_empty() {
                                    error_msg.set(Some("Username is required.".to_string()));
                                    return;
                                }
                                if fn_.is_empty() {
                                    error_msg.set(Some("Full name is required.".to_string()));
                                    return;
                                }
                                if em.is_empty() {
                                    error_msg.set(Some("Email address is required.".to_string()));
                                    return;
                                }
                                if !is_edit && pw.is_empty() {
                                    error_msg.set(Some("Password is required for new users.".to_string()));
                                    return;
                                }
                                if do_create_personal && personal_name.is_empty() {
                                    error_msg.set(Some("Personal library name is required.".to_string()));
                                    return;
                                }

                                let capabilities: Vec<String> = match chosen_role {
                                    RoleChoice::SuperAdmin => vec!["SuperAdmin".to_string()],
                                    RoleChoice::Admin => vec!["Admin".to_string()],
                                    RoleChoice::User => caps,
                                };

                                let edit_token = editing.as_ref().map(|r| r.token.clone());
                                error_msg.set(None);
                                saving.set(true);

                                spawn(async move {
                                    // Step 1: create or update the user
                                    // For create, admin_create_user returns the new user's token
                                    // directly so we avoid a list_users round-trip.
                                    let target_token: Option<String> = if let Some(tok) = edit_token.clone() {
                                        let pw_opt = if pw.is_empty() { None } else { Some(pw) };
                                        match admin_update_user(tok.clone(), fn_, em, pw_opt, capabilities).await {
                                            Err(ServerFnError::ServerError { message, .. }) => {
                                                error_msg.set(Some(message));
                                                saving.set(false);
                                                return;
                                            }
                                            Err(e) => {
                                                error_msg.set(Some(e.to_string()));
                                                saving.set(false);
                                                return;
                                            }
                                            Ok(()) => Some(tok),
                                        }
                                    } else {
                                        match admin_create_user(un.clone(), fn_, em, pw, capabilities).await {
                                            Err(ServerFnError::ServerError { message, .. }) => {
                                                error_msg.set(Some(message));
                                                saving.set(false);
                                                return;
                                            }
                                            Err(e) => {
                                                error_msg.set(Some(e.to_string()));
                                                saving.set(false);
                                                return;
                                            }
                                            Ok(new_token) => Some(new_token),
                                        }
                                    };

                                    // Step 2: assign libraries (if any checked or personal library requested)
                                    let Some(tok) = target_token else {
                                        error_msg.set(Some("User created, but could not retrieve user token for library assignment.".to_string()));
                                        saving.set(false);
                                        return;
                                    };

                                    if !lib_tokens.is_empty() || do_create_personal {
                                        let personal_opt = if do_create_personal { Some(personal_name.clone()) } else { None };
                                        match assign_user_libraries(tok.clone(), lib_tokens, def_lib, do_create_personal, personal_opt).await {
                                            Err(ServerFnError::ServerError { message, .. }) => {
                                                error_msg.set(Some(format!("User saved, but library assignment failed: {message}")));
                                                saving.set(false);
                                                return;
                                            }
                                            Err(e) => {
                                                error_msg.set(Some(format!("User saved, but library assignment failed: {e}")));
                                                saving.set(false);
                                                return;
                                            }
                                            Ok(()) => {}
                                        }
                                    }

                                    // Step 3: personal library rename or delete (edit mode only)
                                    if pers_lib_tok.is_some() {
                                        if do_delete_personal {
                                            if let Err(e) = admin_delete_personal_library(tok.clone()).await {
                                                let msg = match e { ServerFnError::ServerError { message, .. } => message, other => other.to_string() };
                                                error_msg.set(Some(format!("User saved, but library deletion failed: {msg}")));
                                                saving.set(false);
                                                return;
                                            }
                                        } else if personal_name != pers_name_original
                                            && let Err(e) = admin_rename_personal_library(tok.clone(), personal_name).await {
                                                let msg = match e { ServerFnError::ServerError { message, .. } => message, other => other.to_string() };
                                                error_msg.set(Some(format!("User saved, but library rename failed: {msg}")));
                                                saving.set(false);
                                                return;
                                            }
                                    }

                                    on_saved.call(());
                                });
                            },
                            if saving() { "Saving…" } else { "Save" }
                        }
                    }
                }
            }
        }
    }
}
