use dioxus::prelude::*;

use crate::{
    Route,
    password::{password_is_valid, password_requirements},
    routes::landing_page::register_admin,
};

#[component]
pub(crate) fn RegisterAdminForm() -> Element {
    let navigator = use_navigator();
    let mut username = use_signal(String::new);
    let mut full_name = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut confirm_password = use_signal(String::new);
    let mut email = use_signal(String::new);
    let mut error_msg: Signal<Option<String>> = use_signal(|| None);
    let mut loading = use_signal(|| false);

    let pw_touched = use_memo(move || !password().is_empty());
    let requirements = use_memo(move || password_requirements(&password()));
    let confirm_touched = use_memo(move || !confirm_password().is_empty());
    let passwords_match = use_memo(move || password() == confirm_password());

    rsx! {
        div { class: "bg-white dark:bg-slate-800 rounded-2xl shadow-lg w-full max-w-md",
            div { class: "pt-8 pb-2",
                img {
                    src: asset!("/assets/BookBoss-Banner.png"),
                    alt: "BookBoss",
                    class: "w-full h-auto",
                }
            }
            form {
                class: "p-8",
                onsubmit: move |e| {
                    e.prevent_default();
                    let un = username();
                    let fn_ = full_name();
                    let pw = password();
                    let cpw = confirm_password();
                    let em = email();

                    if un.trim().is_empty() {
                        error_msg.set(Some("Username is required.".to_string()));
                        return;
                    }
                    if fn_.trim().is_empty() {
                        error_msg.set(Some("Full name is required.".to_string()));
                        return;
                    }
                    if !password_is_valid(&pw) {
                        error_msg.set(Some(
                            "Password does not meet all of the requirements listed below."
                                .to_string(),
                        ));
                        return;
                    }
                    if pw != cpw {
                        error_msg.set(Some("Passwords do not match.".to_string()));
                        return;
                    }
                    if em.trim().is_empty() {
                        error_msg.set(Some("Email address is required.".to_string()));
                        return;
                    }

                    error_msg.set(None);
                    loading.set(true);

                    spawn(async move {
                        match register_admin(un, fn_, pw, em).await {
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
                h2 { class: "text-2xl font-bold text-gray-800 dark:text-slate-100 mb-1 text-center",
                    "Create Administrator"
                }
                p { class: "text-sm text-gray-500 dark:text-slate-400 text-center mb-6",
                    "No users found — set up your admin account to get started."
                }

                if let Some(msg) = error_msg() {
                    div {
                        class: "mb-4 p-3 bg-red-50 dark:bg-red-950 border border-red-200 dark:border-red-800 text-red-700 dark:text-red-300 rounded-lg text-sm",
                        {msg}
                    }
                }

                div { class: "mb-4",
                    label {
                        class: "block text-sm font-medium text-gray-700 dark:text-slate-300 mb-1",
                        r#for: "reg-username",
                        "Username"
                    }
                    input {
                        id: "reg-username",
                        r#type: "text",
                        class: "w-full px-3 py-2 border border-gray-300 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-gray-900 dark:text-slate-100 placeholder-gray-400 dark:placeholder-slate-400 focus:outline-hidden focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500",
                        placeholder: "Choose a username",
                        value: username,
                        oninput: move |e| username.set(e.value()),
                        disabled: loading,
                        autofocus: true,
                    }
                }

                div { class: "mb-4",
                    label {
                        class: "block text-sm font-medium text-gray-700 dark:text-slate-300 mb-1",
                        r#for: "reg-full-name",
                        "Full Name"
                    }
                    input {
                        id: "reg-full-name",
                        r#type: "text",
                        class: "w-full px-3 py-2 border border-gray-300 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-gray-900 dark:text-slate-100 placeholder-gray-400 dark:placeholder-slate-400 focus:outline-hidden focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500",
                        placeholder: "Your full name",
                        value: full_name,
                        oninput: move |e| full_name.set(e.value()),
                        disabled: loading,
                    }
                }

                div { class: "mb-4",
                    label {
                        class: "block text-sm font-medium text-gray-700 dark:text-slate-300 mb-1",
                        r#for: "reg-password",
                        "Password"
                    }
                    input {
                        id: "reg-password",
                        r#type: "password",
                        class: "w-full px-3 py-2 border border-gray-300 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-gray-900 dark:text-slate-100 placeholder-gray-400 dark:placeholder-slate-400 focus:outline-hidden focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500",
                        placeholder: "Choose a strong password",
                        value: password,
                        oninput: move |e| password.set(e.value()),
                        disabled: loading,
                    }
                    if pw_touched() {
                        div { class: "mt-2 space-y-1",
                            for (rule, satisfied) in requirements() {
                                div {
                                    class: if satisfied { "flex items-center gap-1.5 text-xs text-green-600 dark:text-green-400" } else { "flex items-center gap-1.5 text-xs text-gray-400 dark:text-slate-500" },
                                    span { if satisfied { "✓" } else { "○" } }
                                    span { {rule} }
                                }
                            }
                        }
                    }
                }

                div { class: "mb-4",
                    label {
                        class: "block text-sm font-medium text-gray-700 dark:text-slate-300 mb-1",
                        r#for: "reg-confirm-password",
                        "Confirm Password"
                    }
                    input {
                        id: "reg-confirm-password",
                        r#type: "password",
                        class: "w-full px-3 py-2 border border-gray-300 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-gray-900 dark:text-slate-100 placeholder-gray-400 dark:placeholder-slate-400 focus:outline-hidden focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500",
                        placeholder: "Re-enter your password",
                        value: confirm_password,
                        oninput: move |e| confirm_password.set(e.value()),
                        disabled: loading,
                    }
                    if confirm_touched() {
                        div {
                            class: if passwords_match() { "mt-1 flex items-center gap-1.5 text-xs text-green-600 dark:text-green-400" } else { "mt-1 flex items-center gap-1.5 text-xs text-red-500 dark:text-red-400" },
                            span { if passwords_match() { "✓" } else { "✗" } }
                            span { if passwords_match() { "Passwords match" } else { "Passwords do not match" } }
                        }
                    }
                }

                div { class: "mb-6",
                    label {
                        class: "block text-sm font-medium text-gray-700 dark:text-slate-300 mb-1",
                        r#for: "reg-email",
                        "Email Address"
                    }
                    input {
                        id: "reg-email",
                        r#type: "email",
                        class: "w-full px-3 py-2 border border-gray-300 dark:border-slate-600 rounded-lg bg-white dark:bg-slate-700 text-gray-900 dark:text-slate-100 placeholder-gray-400 dark:placeholder-slate-400 focus:outline-hidden focus:ring-2 focus:ring-indigo-500 focus:border-indigo-500",
                        placeholder: "admin@example.com",
                        value: email,
                        oninput: move |e| email.set(e.value()),
                        disabled: loading,
                    }
                }

                button {
                    class: "w-full py-2 px-4 bg-indigo-600 hover:bg-indigo-700 disabled:bg-indigo-400 text-white font-semibold rounded-lg transition-colors",
                    r#type: "submit",
                    disabled: loading,
                    if loading() { "Creating account…" } else { "Register Administrator" }
                }
            }
        }
    }
}
