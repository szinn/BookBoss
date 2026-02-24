use dioxus::prelude::*;

use crate::{Route, components::NavBar};

#[component]
pub(crate) fn AppLayout() -> Element {
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
