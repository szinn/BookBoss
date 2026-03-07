use dioxus::prelude::*;

mod components;
pub(crate) mod routes;
pub(crate) mod settings;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct FrontendConfig {
    /// IP address where the server should listen.
    /// e.g. 0.0.0.0
    /// Environment variable: BOOKBOSS__FRONTEND__LISTEN_IP
    pub listen_ip: String,

    /// Port the server should listen on.
    /// e.g. 8080
    /// Environment variable: BOOKBOSS__FRONTEND__LISTEN_PORT
    pub listen_port: u16,
}

impl Default for FrontendConfig {
    fn default() -> Self {
        Self {
            listen_ip: "0.0.0.0".to_string(),
            listen_port: 8080,
        }
    }
}

#[cfg(feature = "web")]
pub mod web {
    use crate::BookBossFrontend;

    pub fn launch_web_frontend() {
        dioxus::launch(BookBossFrontend)
    }
}

#[cfg(feature = "server")]
mod error;

#[cfg(feature = "server")]
pub use error::FrontendError;

#[cfg(feature = "server")]
pub mod server;

use components::AppLayout;
use routes::{AuthorDetailPage, BookDetailPage, BooksPage, EditMetadataPage, IncomingPage, LandingPage, ReviewPage, SeriesDetailPage, SettingsPage};
use serde::Deserialize;

#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[route("/")]
    LandingPage {},
    #[layout(AppLayout)]
        #[route("/library")]
        BooksPage {},
        #[route("/library/books/:token")]
        BookDetailPage { token: String },
        #[route("/library/books/:token/edit")]
        EditMetadataPage { token: String },
        #[route("/library/authors/:token")]
        AuthorDetailPage { token: String },
        #[route("/library/series/:token")]
        SeriesDetailPage { token: String },
        #[route("/library/incoming")]
        IncomingPage {},
        #[route("/library/incoming/:token")]
        ReviewPage { token: String },
        #[route("/settings")]
        SettingsPage {},
}

#[component]
fn BookBossFrontend() -> Element {
    rsx! { Router::<Route> {} }
}
