use dioxus::prelude::*;

#[cfg(feature = "server")]
use crate::server::AuthSession;
use crate::{
    Route,
    components::{
        NavBar, SEARCH_TEXT, THEME_MODE,
        selection::{SELECTION_MODE, exit_selection_mode},
        sort_control::{SORT_ORDER, get_sort_preference},
        theme::get_theme_preference,
    },
};

/// Newtype wrapper so `use_context` can distinguish the incoming-review refresh
/// signal from other `Signal<u32>` contexts.
#[derive(Clone, Copy)]
pub(crate) struct IncomingRefresh(pub Signal<u32>);

/// Newtype wrapper for the background-jobs refresh signal.
#[derive(Clone, Copy)]
pub(crate) struct JobsRefresh(pub Signal<u32>);

/// Newtype wrapper for the system-messages refresh signal.
#[derive(Clone, Copy)]
pub(crate) struct SystemMessagesRefresh(pub Signal<u32>);

#[get("/api/v1/check_auth", auth_session: axum::Extension<AuthSession>)]
async fn check_auth() -> Result<bool, ServerFnError> {
    Ok(auth_session.current_user.as_ref().is_some_and(|u| !u.username.is_empty()))
}

#[component]
pub(crate) fn AppLayout() -> Element {
    let mut incoming_refresh = use_context_provider(|| IncomingRefresh(Signal::new(0u32)));
    let mut jobs_refresh = use_context_provider(|| JobsRefresh(Signal::new(0u32)));
    let mut messages_refresh = use_context_provider(|| SystemMessagesRefresh(Signal::new(0u32)));

    // Connect to the SSE event stream so the UI updates in real time when the
    // backend processes imports or background jobs.
    //
    // The JS-side guard (`window.__bb_es`) ensures exactly one EventSource
    // exists even if the component remounts during fullstack hydration.  If a
    // prior connection is still open it is closed first, preventing zombie
    // connections from exhausting the browser's per-domain connection limit.
    use_hook(move || {
        spawn(async move {
            let mut eval = document::eval(
                r"
                if (window.__bb_es) {
                    window.__bb_es.close();
                }
                const es = new EventSource('/api/v1/events');
                window.__bb_es = es;
                es.addEventListener('incoming_changed', () => dioxus.send('incoming_changed'));
                es.addEventListener('jobs_changed', () => dioxus.send('jobs_changed'));
                es.addEventListener('system_messages_changed', () => dioxus.send('system_messages_changed'));
                // Keep the eval alive indefinitely — EventSource auto-reconnects.
                await new Promise(() => {});
                ",
            );

            while let Ok(msg) = eval.recv::<String>().await {
                match msg.as_str() {
                    "incoming_changed" => *incoming_refresh.0.write() += 1,
                    "jobs_changed" => *jobs_refresh.0.write() += 1,
                    "system_messages_changed" => *messages_refresh.0.write() += 1,
                    _ => {}
                }
            }
        });
    });

    // Load persisted sort preference once; write to global signal.
    let sort_pref = use_server_future(get_sort_preference);
    use_effect(move || {
        if let Ok(res) = sort_pref
            && let Some(Ok(Some(order))) = res()
        {
            *SORT_ORDER.write() = order;
        }
    });

    // Load persisted theme preference once; write to global signal.
    let theme_pref = use_server_future(get_theme_preference);
    use_effect(move || {
        if let Ok(res) = theme_pref
            && let Some(Ok(Some(mode))) = res()
        {
            *THEME_MODE.write() = mode;
        }
    });

    // Fallback: read localStorage so the icon and class are correct even when
    // the server future is unavailable (e.g. first hydration before SSR data
    // arrives). Runs once after mount, only on the WASM client.
    use_hook(move || {
        spawn(async move {
            if let Ok(val) = document::eval("return localStorage.getItem('bb_theme') || ''").await
                && let Some(s) = val.as_str()
                && !s.is_empty()
            {
                use crate::components::theme::ThemeMode;
                if *THEME_MODE.peek() == ThemeMode::System {
                    *THEME_MODE.write() = ThemeMode::from_str(s);
                }
            }
        });
    });

    // Apply dark class to <html> whenever THEME_MODE changes.
    // localStorage is written only in ThemeToggle (on explicit user action) to
    // avoid corrupting the value before the saved preference has been loaded.
    // spawn is required — document::eval doesn't execute from a synchronous
    // use_effect body. No .await so the eval queue never blocks.
    use_effect(move || {
        let _ = *THEME_MODE.read();
        spawn(async move {
            let mode = *THEME_MODE.peek();
            document::eval(&format!(
                "(function(){{var m={:?};var \
                 dark=m==='dark'||(m==='system'&&window.matchMedia('(prefers-color-scheme:dark)').matches);document.documentElement.classList.toggle('dark',\
                 dark);}})()",
                mode.as_str()
            ));
        });
    });

    // Exit selection mode and clear search whenever the route changes.
    let route = use_route::<Route>();
    use_effect(move || {
        // Subscribe to route changes so the effect re-runs on navigation.
        // Use peek() for SELECTION_MODE to avoid re-running when the user
        // toggles selection mode on the current page.
        let _ = &route;
        if *SELECTION_MODE.peek() {
            exit_selection_mode();
        }
        *SEARCH_TEXT.write() = String::new();
        // Discard any pending search when navigating away from BooksPage so a
        // stale PENDING_SEARCH can't be applied on a future unrelated visit.
        if !matches!(route, Route::BooksPage) {
            *crate::components::PENDING_SEARCH.write() = None;
        }
    });

    const ANTI_FLASH_SCRIPT: &str = "(function(){var t=localStorage.getItem('bb_theme');var \
                                     dark=t==='dark'||((!t||t==='system')&&window.matchMedia('(prefers-color-scheme:dark)').matches);document.documentElement.\
                                     classList.toggle('dark',dark);})();";

    rsx! {
        document::Script { {ANTI_FLASH_SCRIPT} }
        document::Stylesheet { href: asset!("/assets/tailwind.css") }
        document::Link { rel: "icon", href: asset!("/assets/favicon.ico") }
        document::Link { rel: "apple-touch-icon", sizes: "180x180", href: asset!("/assets/apple-touch-icon.png") }
        document::Link { rel: "apple-touch-icon", sizes: "32x32", href: asset!("/assets/favicon-32x32.png") }
        document::Link { rel: "apple-touch-icon", sizes: "16x16", href: asset!("/assets/favicon-16x16.png") }
        div { class: "h-screen flex flex-col bg-gray-50 dark:bg-slate-900 text-gray-900 dark:text-slate-100",
            NavBar {}
            main { class: "flex-1 flex overflow-hidden",
                SuspenseBoundary {
                    fallback: |_| rsx! {},
                    AuthGate {}
                }
            }
        }
    }
}

/// Wraps the Outlet so that only the page content area suspends during the auth
/// check, leaving the `NavBar` visible immediately.
#[component]
fn AuthGate() -> Element {
    let navigator = use_navigator();
    let auth = use_server_future(check_auth)?;

    use_effect(move || {
        if let Some(Ok(false)) = auth() {
            navigator.replace(Route::LandingPage { login_failed: None });
        }
    });

    rsx! { Outlet::<Route> {} }
}
