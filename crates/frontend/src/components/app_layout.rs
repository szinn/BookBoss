use dioxus::prelude::*;

use crate::{Route, components::NavBar};

#[component]
pub(crate) fn AppLayout() -> Element {
    rsx! {
        document::Stylesheet { href: asset!("/assets/tailwind.css") }
        div { class: "min-h-screen flex flex-col bg-gray-50 text-gray-900",
            NavBar {}
            main { class: "flex-1 flex overflow-hidden",
                Outlet::<Route> {}
            }
        }
    }
}
