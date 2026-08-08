mod application_section;
mod genre_tags_section;
mod libraries_section;
mod messages_section;
mod tasks_section;
mod users_section;

use application_section::ApplicationSection;
use dioxus::prelude::*;
use genre_tags_section::GenreTagsSection;
use libraries_section::LibrariesSection;
use messages_section::MessagesSection;
use tasks_section::TasksSection;
use users_section::UsersSection;

use crate::Route;
#[cfg(feature = "server")]
use crate::routes::server_helpers::authenticated_user;
#[cfg(feature = "server")]
use crate::server::AuthSession;

// ---------------------------------------------------------------------------
// Settings context (admin status + current user identity)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct SettingsContext {
    pub is_admin: bool,
    pub is_super_admin: bool,
    pub current_user_token: String,
}

#[get(
    "/api/v1/settings/context",
    auth_session: axum::Extension<AuthSession>,
)]
async fn get_settings_context() -> Result<SettingsContext, ServerFnError> {
    let user = authenticated_user(&auth_session)?;

    let is_super_admin = user.permissions.contains("SuperAdmin");
    let is_admin = is_super_admin || user.permissions.contains("Admin");

    Ok(SettingsContext {
        is_admin,
        is_super_admin,
        current_user_token: bb_core::user::UserToken::new(user.id()).to_string(),
    })
}

// ---------------------------------------------------------------------------
// Section tabs
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Section {
    Users,
    Libraries,
    Tasks,
    Messages,
    GenreTags,
    Application,
}

impl Section {
    fn from_hash(hash: &str) -> Self {
        match hash.trim_start_matches('#') {
            "libraries" => Self::Libraries,
            "tasks" => Self::Tasks,
            "messages" => Self::Messages,
            "genre-tags" => Self::GenreTags,
            "application" => Self::Application,
            _ => Self::Users,
        }
    }

    fn as_hash(self) -> &'static str {
        match self {
            Self::Users => "#users",
            Self::Libraries => "#libraries",
            Self::Tasks => "#tasks",
            Self::Messages => "#messages",
            Self::GenreTags => "#genre-tags",
            Self::Application => "#application",
        }
    }
}

// ---------------------------------------------------------------------------
// SettingsPage
// ---------------------------------------------------------------------------

#[component]
pub(crate) fn SettingsPage() -> Element {
    let navigator = use_navigator();
    let ctx = use_server_future(get_settings_context)?;

    let context = ctx().and_then(std::result::Result::ok).unwrap_or(SettingsContext {
        is_admin: false,
        is_super_admin: false,
        current_user_token: String::new(),
    });

    use_effect(move || match ctx() {
        Some(Err(_)) => {
            navigator.replace(Route::LandingPage { login_failed: None });
        }
        Some(Ok(ref c)) if !c.is_admin => {
            navigator.replace(Route::BooksPage {});
        }
        _ => {}
    });

    let mut active_section = use_signal(|| Section::Users);

    // Restore section from URL hash on mount.
    use_effect(move || {
        spawn(async move {
            if let Ok(val) = document::eval("return window.location.hash").await
                && let Some(hash) = val.as_str()
            {
                let section = Section::from_hash(hash);
                if section != Section::Users {
                    active_section.set(section);
                }
            }
        });
    });

    // Update URL hash when section changes.
    use_effect(move || {
        let hash = active_section().as_hash();
        spawn(async move {
            let _ = document::eval(&format!("window.location.hash = '{hash}'")).await;
        });
    });

    let nav_button_class = |section: Section| {
        if active_section() == section {
            "w-full text-left px-4 py-2 text-sm font-medium bg-indigo-50 text-indigo-700 border-r-2 border-indigo-600 dark:bg-indigo-950 dark:text-indigo-300 \
             dark:border-indigo-400"
        } else {
            "w-full text-left px-4 py-2 text-sm font-medium text-gray-600 hover:bg-gray-50 hover:text-gray-900 dark:text-slate-400 dark:hover:bg-slate-700 \
             dark:hover:text-slate-200"
        }
    };

    rsx! {
        div { class: "flex h-full flex-1",
            // ----------------------------------------------------------------
            // Left panel — section list
            // ----------------------------------------------------------------
            nav { class: "w-48 shrink-0 border-r border-gray-200 bg-white dark:border-slate-700 dark:bg-slate-800",
                ul { class: "py-4",
                    li {
                        button {
                            class: nav_button_class(Section::Application),
                            onclick: move |_| active_section.set(Section::Application),
                            "Application"
                        }
                    }
                    li {
                        button {
                            class: nav_button_class(Section::Libraries),
                            onclick: move |_| active_section.set(Section::Libraries),
                            "Libraries"
                        }
                    }
                    li {
                        button {
                            class: nav_button_class(Section::GenreTags),
                            onclick: move |_| active_section.set(Section::GenreTags),
                            "Genre/Tags"
                        }
                    }
                    li {
                        button {
                            class: nav_button_class(Section::Users),
                            onclick: move |_| active_section.set(Section::Users),
                            "Users"
                        }
                    }
                    li {
                        button {
                            class: nav_button_class(Section::Tasks),
                            onclick: move |_| active_section.set(Section::Tasks),
                            "Tasks"
                        }
                    }
                    li {
                        button {
                            class: nav_button_class(Section::Messages),
                            onclick: move |_| active_section.set(Section::Messages),
                            "Messages"
                        }
                    }
                }
            }
            // ----------------------------------------------------------------
            // Right panel — section content
            // ----------------------------------------------------------------
            div { class: "flex-1 overflow-auto p-8 flex flex-col items-center",
                match active_section() {
                    Section::Users => rsx! {
                        UsersSection {
                            is_super_admin: context.is_super_admin,
                            current_user_token: context.current_user_token.clone(),
                        }
                    },
                    Section::Libraries => rsx! {
                        LibrariesSection {}
                    },
                    Section::Tasks => rsx! {
                        TasksSection {}
                    },
                    Section::Messages => rsx! {
                        MessagesSection {}
                    },
                    Section::GenreTags => rsx! {
                        GenreTagsSection {}
                    },
                    Section::Application => rsx! {
                        ApplicationSection {}
                    },
                }
            }
        }
    }
}
