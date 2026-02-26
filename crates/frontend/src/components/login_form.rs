use dioxus::prelude::*;

use crate::{Route, routes::landing_page::perform_login};

#[component]
pub(crate) fn LoginForm() -> Element {
    let navigator = use_navigator();
    let mut username = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut error_msg: Signal<Option<String>> = use_signal(|| None);
    let mut loading = use_signal(|| false);

    rsx! {
        div { class: "bg-white rounded-2xl shadow-lg w-full max-w-md",
            div { class: "flex justify-center pt-8 pb-2",
                img {
                    src: asset!("/assets/BookBoss Banner.png"),
                    alt: "BookBoss",
                    class: "w-[33vw] max-w-full h-auto",
                }
            }
            form {
                class: "p-8",
                onsubmit: move |e| {
                    e.prevent_default();
                    let un = username();
                    let pw = password();
                    if un.is_empty() || pw.is_empty() {
                        error_msg
                            .set(
                                Some("Please enter your username and password.".to_string()),
                            );
                        return;
                    }
                    error_msg.set(None);
                    loading.set(true);
                    spawn(async move {
                        match perform_login(un, pw).await {
                            Ok(()) => {
                                navigator.push(Route::BooksPage {});
                            }
                            Err(ServerFnError::ServerError { message, .. }) => {
                                error_msg.set(Some(message));
                                loading.set(false);
                            }
                            Err(e) => {
                                error_msg.set(Some(e.to_string()));
                                loading.set(false);
                            }
                        }
                    });
                },
                if let Some(msg) = error_msg() {
                    div {
                        class: "mb-4 p-3 bg-red-50 border border-red-200 text-red-700 rounded-lg text-sm",
                        "{msg}"
                    }
                }

                div { class: "mb-4",
                    label {
                        class: "block text-sm font-medium text-gray-700 mb-1",
                        r#for: "login-username",
                        "Username"
                    }
                    input {
                        id: "login-username",
                        r#type: "text",
                        class: "w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-hidden focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500",
                        placeholder: "Enter your username",
                        value: username,
                        oninput: move |e| username.set(e.value()),
                        disabled: loading,
                        autofocus: true,
                    }
                }

                div { class: "mb-6",
                    label {
                        class: "block text-sm font-medium text-gray-700 mb-1",
                        r#for: "login-password",
                        "Password"
                    }
                    input {
                        id: "login-password",
                        r#type: "password",
                        class: "w-full px-3 py-2 border border-gray-300 rounded-lg focus:outline-hidden focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500",
                        placeholder: "Enter your password",
                        value: password,
                        oninput: move |e| password.set(e.value()),
                        disabled: loading,
                    }
                }

                button {
                    class: "w-full py-2 px-4 bg-indigo-600 hover:bg-indigo-700 disabled:bg-indigo-400 text-white font-semibold rounded-lg transition-colors",
                    r#type: "submit",
                    disabled: loading,
                    if loading() { "Signing in…" } else { "Login" }
                }
            }
        }
    }
}
