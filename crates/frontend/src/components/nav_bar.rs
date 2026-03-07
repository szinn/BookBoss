use dioxus::prelude::*;
#[cfg(feature = "server")]
use {
    crate::server::{AuthSession, AuthUser, BackendSessionPool},
    axum::http::Method,
    axum_session_auth::{Auth, Rights},
    bb_core::{CoreServices, types::Capability, user::UserId},
    std::sync::Arc,
};

use crate::{Route, settings::BookDisplayView};

#[get("/api/v1/incoming/pending_count", auth_session: axum::Extension<AuthSession>, core_services: axum::Extension<Arc<CoreServices>>)]
async fn get_pending_count() -> Result<u32, ServerFnError> {
    let current_user = auth_session.current_user.clone().unwrap_or_default();
    let has_permission = Auth::<AuthUser, UserId, BackendSessionPool>::build([Method::GET], true)
        .requires(Rights::any([Rights::permission(Capability::ApproveImports.as_str())]))
        .validate(&current_user, &Method::GET, None)
        .await;
    if !has_permission {
        return Ok(0);
    }
    let jobs = core_services
        .import_job_service
        .list_needs_review(None, None)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(jobs.len() as u32)
}

#[put("/api/v1/logout", auth_session: axum::Extension<AuthSession>)]
async fn logout() -> Result<(), ServerFnError> {
    auth_session.logout_user();

    Ok(())
}

#[put("/api/v1/book_display_view", auth_session: axum::Extension<AuthSession>, core_services: axum::Extension<Arc<CoreServices>>)]
async fn save_book_display_view(view: BookDisplayView) -> Result<(), ServerFnError> {
    let user = auth_session.current_user.as_ref().ok_or_else(|| ServerFnError::new("Not authenticated"))?;
    user.set_book_display_view(view, &core_services)
        .await
        .map_err(|e: bb_core::Error| ServerFnError::new(e.to_string()))
}

#[component]
pub(crate) fn NavBar() -> Element {
    let navigator = use_navigator();
    let mut user_menu_open = use_signal(|| false);
    let mut view: Signal<BookDisplayView> = use_context();
    let route = use_route::<Route>();
    let pending_count = use_server_future(get_pending_count)?;

    let is_library = matches!(route, Route::BooksPage {});

    let on_logout = move |_| {
        user_menu_open.set(false);
        spawn(async move {
            let _ = logout().await;
            navigator.push(Route::LandingPage {});
        });
    };

    let on_toggle_view = move |_| {
        if !is_library {
            return;
        }
        let next = match *view.read() {
            BookDisplayView::GridView => BookDisplayView::TableView,
            BookDisplayView::TableView => BookDisplayView::GridView,
        };
        view.set(next.clone());
        spawn(async move {
            let _ = save_book_display_view(next).await;
        });
    };

    let toggle_title = match *view.read() {
        BookDisplayView::GridView => "Switch to Table View",
        BookDisplayView::TableView => "Switch to Grid View",
    };

    let toggle_class = if is_library {
        "flex items-center hover:text-indigo-200 cursor-pointer"
    } else {
        "flex items-center opacity-30 cursor-default"
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
                Link { to: Route::IncomingPage {}, class: "relative text-sm hover:text-indigo-200 flex items-center gap-1.5",
                    "Incoming"
                    {
                        let count = pending_count().and_then(|r| r.ok()).unwrap_or(0);
                        (count > 0).then(|| rsx! {
                            span {
                                class: "inline-flex items-center justify-center min-w-[1.1rem] h-[1.1rem] px-1 rounded-full bg-red-500 text-white text-[0.6rem] font-bold leading-none",
                                "{count}"
                            }
                        })
                    }
                }
            }
            div { class: "flex items-center gap-4",
                // View toggle: shows the icon for the OTHER view (what you'll switch to)
                button {
                    class: toggle_class,
                    title: toggle_title,
                    onclick: on_toggle_view,
                    match *view.read() {
                        // In GridView → show table icon to switch to TableView
                        BookDisplayView::GridView => rsx! {
                            svg {
                                class: "w-5 h-5",
                                fill: "none",
                                view_box: "0 0 24 24",
                                stroke_width: "1.5",
                                stroke: "currentColor",
                                path {
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    d: "M3.375 19.5h17.25m-17.25 0a1.125 1.125 0 0 1-1.125-1.125M3.375 19.5h7.5c.621 0 1.125-.504 1.125-1.125m-9.75 0V5.625m0 12.75v-1.5c0-.621.504-1.125 1.125-1.125m18.375 2.625V5.625m0 12.75c0 .621-.504 1.125-1.125 1.125m1.125-1.125v-1.5c0-.621-.504-1.125-1.125-1.125m0 3.75h-7.5A1.125 1.125 0 0 1 12 18.375m9.75-12.75c0-.621-.504-1.125-1.125-1.125H3.375c-.621 0-1.125.504-1.125 1.125m19.5 0v1.5c0 .621-.504 1.125-1.125 1.125M2.25 5.625v1.5c0 .621.504 1.125 1.125 1.125m0 0h17.25m-17.25 0c-.621 0-1.125.504-1.125 1.125v1.5c0 .621.504 1.125 1.125 1.125m17.25-3.75h1.5m-1.5 0c.621 0 1.125.504 1.125 1.125v1.5c0 .621-.504 1.125-1.125 1.125m-17.25 0h7.5m-7.5 0c-.621 0-1.125.504-1.125 1.125v1.5c0 .621.504 1.125 1.125 1.125m7.5-3.75h1.5m-1.5 0c.621 0 1.125.504 1.125 1.125v1.5c0 .621-.504 1.125-1.125 1.125m-7.5 0h7.5",
                                }
                            }
                        },
                        // In TableView → show grid icon to switch to GridView
                        BookDisplayView::TableView => rsx! {
                            svg {
                                class: "w-5 h-5",
                                fill: "none",
                                view_box: "0 0 24 24",
                                stroke_width: "1.5",
                                stroke: "currentColor",
                                path {
                                    stroke_linecap: "round",
                                    stroke_linejoin: "round",
                                    d: "M3.75 6A2.25 2.25 0 0 1 6 3.75h2.25A2.25 2.25 0 0 1 10.5 6v2.25a2.25 2.25 0 0 1-2.25 2.25H6a2.25 2.25 0 0 1-2.25-2.25V6ZM3.75 15.75A2.25 2.25 0 0 1 6 13.5h2.25a2.25 2.25 0 0 1 2.25 2.25V18a2.25 2.25 0 0 1-2.25 2.25H6A2.25 2.25 0 0 1 3.75 18v-2.25ZM13.5 6a2.25 2.25 0 0 1 2.25-2.25H18A2.25 2.25 0 0 1 20.25 6v2.25A2.25 2.25 0 0 1 18 10.5h-2.25a2.25 2.25 0 0 1-2.25-2.25V6ZM13.5 15.75a2.25 2.25 0 0 1 2.25-2.25H18a2.25 2.25 0 0 1 2.25 2.25V18A2.25 2.25 0 0 1 18 20.25h-2.25A2.25 2.25 0 0 1 13.5 18v-2.25Z",
                                }
                            }
                        },
                    }
                }
                button {
                    class: "flex items-center hover:text-indigo-200 ml-4 cursor-pointer",
                    title: "Settings",
                    onclick: move |_| { navigator.push(Route::SettingsPage {}); },
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
