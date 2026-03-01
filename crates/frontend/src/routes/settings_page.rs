#[cfg(feature = "server")]
use bb_core::CoreServices;
use dioxus::prelude::*;
#[cfg(feature = "server")]
use {crate::server::AuthSession, std::sync::Arc};

use crate::Route;

// ---------------------------------------------------------------------------
// Library statistics
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct LibraryStats {
    pub books: u64,
    pub authors: u64,
}

/// Returns library statistics for the About section.
///
/// TODO: Replace the hardcoded values with real counts once `LibraryService`
/// is added to `CoreServices`.
#[get(
    "/api/v1/library/stats",
    auth_session: axum::Extension<AuthSession>,
    _core_services: axum::Extension<Arc<CoreServices>>
)]
#[tracing::instrument(level = "trace", skip(auth_session, _core_services))]
async fn get_library_stats() -> Result<LibraryStats, ServerFnError> {
    auth_session
        .current_user
        .as_ref()
        .filter(|u| !u.username.is_empty())
        .ok_or_else(|| ServerFnError::new("Not authenticated"))?;

    Ok(LibraryStats { books: 0, authors: 0 })
}

// ---------------------------------------------------------------------------
// Settings sections
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq)]
enum SettingSection {
    About,
}

impl SettingSection {
    fn all() -> &'static [SettingSection] {
        &[SettingSection::About]
    }

    fn label(&self) -> &'static str {
        match self {
            SettingSection::About => "About",
        }
    }

    /// Capability required to view this section, or `None` for all
    /// authenticated users.
    ///
    /// TODO: Return the required `bb_core::Capability` when sections that need
    /// one are added (e.g. `Some(Capability::Admin)`). The section list
    /// already filters on this value.
    fn required_capability(&self) -> Option<&'static str> {
        match self {
            SettingSection::About => None,
        }
    }
}

// ---------------------------------------------------------------------------
// SettingsPage
// ---------------------------------------------------------------------------

#[component]
pub(crate) fn SettingsPage() -> Element {
    let navigator = use_navigator();
    let mut active_section = use_signal(|| SettingSection::About);
    let stats = use_server_future(get_library_stats)?;

    // Auth guard: AppLayout already handles this, but we redirect defensively
    // in case this component is ever rendered outside that layout.
    use_effect(move || {
        if let Some(Err(_)) = stats() {
            navigator.replace(Route::LandingPage {});
        }
    });

    // TODO: Fetch user capabilities from a server function when sections that
    // require specific capabilities are added. For now, all sections are
    // visible (About requires none).
    let visible_sections: Vec<&SettingSection> = SettingSection::all().iter().filter(|s| s.required_capability().is_none()).collect();

    rsx! {
        div { class: "flex h-full flex-1",
            // ----------------------------------------------------------------
            // Left panel — section list
            // ----------------------------------------------------------------
            nav { class: "w-48 shrink-0 border-r border-gray-200 bg-white",
                ul { class: "py-4",
                    for section in visible_sections {
                        {
                            let is_active = *active_section.read() == *section;
                            let item_class = if is_active {
                                "w-full text-left px-4 py-2 text-sm font-medium bg-indigo-50 text-indigo-700 border-r-2 border-indigo-600"
                            } else {
                                "w-full text-left px-4 py-2 text-sm text-gray-700 hover:bg-gray-50 cursor-pointer"
                            };
                            let section_clone = section.clone();
                            rsx! {
                                li {
                                    button {
                                        class: item_class,
                                        onclick: move |_| active_section.set(section_clone.clone()),
                                        { section.label() }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ----------------------------------------------------------------
            // Right panel — section content
            // ----------------------------------------------------------------
            div { class: "flex-1 overflow-auto p-8 flex flex-col items-center",
                match *active_section.read() {
                    SettingSection::About => rsx! {
                        AboutSection { stats: stats().and_then(|r| r.ok()) }
                    },
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// About section
// ---------------------------------------------------------------------------

#[component]
fn AboutSection(stats: Option<LibraryStats>) -> Element {
    rsx! {
        div { class: "w-full max-w-lg",
            img {
                src: asset!("/assets/BookBoss-Banner.png"),
                alt: "BookBoss",
                class: "w-full mb-8",
            }
            h2 { class: "text-lg font-semibold text-gray-900 mb-4", "Library Statistics" }
            dl { class: "divide-y divide-gray-100 rounded-lg border border-gray-200 bg-white",
                StatRow {
                    label: "Books",
                    value: stats.as_ref().map(|s| s.books.to_string()),
                }
                StatRow {
                    label: "Authors",
                    value: stats.as_ref().map(|s| s.authors.to_string()),
                }
            }
        }
    }
}

#[component]
fn StatRow(label: &'static str, value: Option<String>) -> Element {
    rsx! {
        div { class: "flex justify-between px-4 py-3",
            dt { class: "text-sm text-gray-500", { label } }
            dd { class: "text-sm font-medium text-gray-900",
                { value.as_deref().unwrap_or("—") }
            }
        }
    }
}
