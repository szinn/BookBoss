use dioxus::prelude::*;

#[cfg(feature = "server")]
use crate::server::AuthSession;
use crate::{Route, components::NavBar};

#[get("/api/v1/check_auth", auth_session: axum::Extension<AuthSession>)]
async fn check_auth() -> Result<bool, ServerFnError> {
    Ok(auth_session.current_user.as_ref().map(|u| !u.username.is_empty()).unwrap_or(false))
}

#[component]
pub(crate) fn AppLayout() -> Element {
    let navigator = use_navigator();
    let auth = use_server_future(check_auth)?;

    use_effect(move || {
        if let Some(Ok(false)) = auth() {
            navigator.replace(Route::LandingPage {});
        }
    });

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
