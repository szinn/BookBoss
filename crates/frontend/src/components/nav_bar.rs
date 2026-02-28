use dioxus::prelude::*;

use crate::Route;
#[cfg(feature = "server")]
use crate::server::AuthSession;

#[put("/api/v1/logout", auth_session: axum::Extension<AuthSession>)]
#[tracing::instrument(level = "trace", skip(auth_session))]
async fn logout() -> Result<(), ServerFnError> {
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
                img {
                    src: asset!("/assets/BookBoss-Title.png"),
                    alt: "BookBoss",
                    class: "h-8 w-auto",
                }
                Link { to: Route::BooksPage {}, class: "text-sm hover:text-indigo-200",
                    "Library"
                }
            }
            div { class: "flex items-center gap-4",
                button { class: "flex items-center hover:text-indigo-200", title: "Settings",
                    svg {
                        class: "w-5 h-5",
                        fill: "none",
                        view_box: "0 0 24 24",
                        stroke_width: "1.5",
                        stroke: "currentColor",
                        path {
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            d: "M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.325.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 0 1 1.37.49l1.296 2.247a1.125 1.125 0 0 1-.26 1.431l-1.003.827c-.293.241-.438.613-.43.992a7.723 7.723 0 0 1 0 .255c-.008.378.137.75.43.991l1.004.827c.424.35.534.955.26 1.43l-1.298 2.247a1.125 1.125 0 0 1-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.47 6.47 0 0 1-.22.128c-.331.183-.581.495-.644.869l-.213 1.281c-.09.543-.56.94-1.11.94h-2.594c-.55 0-1.019-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 0 1-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 0 1-1.369-.49l-1.297-2.247a1.125 1.125 0 0 1 .26-1.431l1.004-.827c.292-.24.437-.613.43-.991a6.932 6.932 0 0 1 0-.255c.007-.38-.138-.751-.43-.992l-1.004-.827a1.125 1.125 0 0 1-.26-1.43l1.297-2.247a1.125 1.125 0 0 1 1.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.086.22-.128.332-.183.582-.495.644-.869l.214-1.28Z",
                        }
                        path {
                            stroke_linecap: "round",
                            stroke_linejoin: "round",
                            d: "M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z",
                        }
                    }
                }
                div { class: "relative",
                    button {
                        class: "flex items-center hover:text-indigo-200",
                        title: "User",
                        onclick: move |_| user_menu_open.toggle(),
                        svg {
                            class: "w-5 h-5",
                            fill: "none",
                            view_box: "0 0 24 24",
                            stroke_width: "1.5",
                            stroke: "currentColor",
                            path {
                                stroke_linecap: "round",
                                stroke_linejoin: "round",
                                d: "M15.75 6a3.75 3.75 0 1 1-7.5 0 3.75 3.75 0 0 1 7.5 0ZM4.501 20.118a7.5 7.5 0 0 1 14.998 0A17.933 17.933 0 0 1 12 21.75c-2.676 0-5.216-.584-7.499-1.632Z",
                            }
                        }
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
