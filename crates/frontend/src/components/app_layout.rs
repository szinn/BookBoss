use dioxus::prelude::*;
#[cfg(feature = "server")]
use {crate::server::AuthSession, bb_core::CoreServices, std::sync::Arc};

use crate::{Route, components::NavBar, settings::BookDisplayView};

#[get("/api/v1/check_auth", auth_session: axum::Extension<AuthSession>)]
async fn check_auth() -> Result<bool, ServerFnError> {
    Ok(auth_session.current_user.as_ref().map(|u| !u.username.is_empty()).unwrap_or(false))
}

#[get("/api/v1/book_display_view", auth_session: axum::Extension<AuthSession>, core_services: axum::Extension<Arc<CoreServices>>)]
async fn get_book_display_view() -> Result<BookDisplayView, ServerFnError> {
    let user = auth_session.current_user.as_ref().ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    Ok(user.get_book_display_view(&core_services).await)
}

#[component]
pub(crate) fn AppLayout() -> Element {
    let navigator = use_navigator();
    let auth = use_server_future(check_auth)?;
    let initial_view = use_server_future(get_book_display_view)?;

    use_effect(move || {
        if let Some(Ok(false)) = auth() {
            navigator.replace(Route::LandingPage {});
        }
    });

    let view = use_context_provider(|| {
        let v = initial_view().and_then(|r| r.ok()).unwrap_or_default();
        Signal::new(v)
    });
    let _ = view;

    // Shared counter bumped after approve/reject so NavBar re-fetches the pending
    // count.
    use_context_provider(|| Signal::new(0u32));

    rsx! {
        document::Stylesheet { href: asset!("/assets/tailwind.css") }
        document::Link { rel: "icon", href: asset!("/assets/favicon.ico") }
        document::Link { rel: "apple-touch-icon", sizes: "180x180", href: asset!("/assets/apple-touch-icon.png") }
        document::Link { rel: "apple-touch-icon", sizes: "32x32", href: asset!("/assets/favicon-32x32.png") }
        document::Link { rel: "apple-touch-icon", sizes: "16x16", href: asset!("/assets/favicon-16x16.png") }
        div { class: "min-h-screen flex flex-col bg-gray-50 text-gray-900",
            NavBar {}
            main { class: "flex-1 flex overflow-hidden",
                Outlet::<Route> {}
            }
        }
    }
}
