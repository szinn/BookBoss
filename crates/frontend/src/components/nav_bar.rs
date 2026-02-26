use dioxus::prelude::*;

use crate::Route;

#[server]
async fn logout() -> Result<(), ServerFnError> {
    use crate::server::AuthSession;

    let ctx = FullstackContext::current().ok_or_else(|| ServerFnError::new("Not in a server context"))?;

    let auth_session: AuthSession = ctx.extension().ok_or_else(|| ServerFnError::new("AuthSession not available"))?;

    auth_session.logout_user();

    Ok(())
}

#[component]
pub(crate) fn NavBar() -> Element {
    let navigator = use_navigator();
    let mut user_menu_open = use_signal(|| false);

    let on_logout = move |_| {
        user_menu_open.set(false);
        spawn(async move {
            let _ = logout().await;
            navigator.push(Route::LandingPage {});
        });
    };

    rsx! {
        nav { class: "bg-indigo-700 text-white px-6 py-3 flex items-center justify-between shadow-sm",
            div { class: "flex items-center gap-6",
                Link { to: Route::BooksPage {}, class: "text-sm hover:text-indigo-200",
                    "Books"
                }
            }
            div { class: "flex items-center gap-4",
                button { class: "text-sm hover:text-indigo-200", "Settings" }
                div { class: "relative",
                    button {
                        class: "text-sm hover:text-indigo-200",
                        onclick: move |_| user_menu_open.toggle(),
                        "User"
                    }
                    if user_menu_open() {
                        div {
                            class: "fixed inset-0 z-40",
                            onclick: move |_| user_menu_open.set(false),
                        }
                        div { class: "absolute right-0 top-full mt-1 w-36 bg-white rounded-lg shadow-lg py-1 z-50",
                            button {
                                class: "w-full text-left px-4 py-2 text-sm text-gray-700 hover:bg-gray-100",
                                onclick: on_logout,
                                "Logout"
                            }
                        }
                    }
                }
            }
        }
    }
}
