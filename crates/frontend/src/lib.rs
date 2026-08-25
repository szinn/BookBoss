#![allow(unreachable_code)] // false positive in Routable derive codegen on current nightly

use dioxus::prelude::*;

mod components;
pub(crate) mod password;
pub(crate) mod routes;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FrontendConfig {
    /// IP address where the server should listen.
    /// e.g. 0.0.0.0
    /// Environment variable: `BOOKBOSS__FRONTEND__LISTEN_IP`
    pub listen_ip: String,

    /// Port the server should listen on.
    /// e.g. 8080
    /// Environment variable: `BOOKBOSS__FRONTEND__LISTEN_PORT`
    pub listen_port: u16,

    /// Base URL where the application is running.
    /// e.g. https://bookboss.example.com
    /// Environment variable: `BOOKBOSS__FRONTEND__BASE_URL`
    pub base_url: String,
}

impl Default for FrontendConfig {
    fn default() -> Self {
        Self {
            listen_ip: "0.0.0.0".to_string(),
            listen_port: 8080,
            base_url: "http://0.0.0.0:8080".to_string(),
        }
    }
}

/// OIDC SSO configuration. All fields except `button_label` are required when
/// SSO is enabled. The struct uses `Option<String>` for each field so
/// [`OidcConfig::is_sso_available`] can distinguish "not configured at all"
/// (silent — SSO disabled) from "partially configured" (logs an error and
/// disables SSO). `button_label` is cosmetic and does not count toward
/// "configured".
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct OidcConfig {
    /// Full OIDC discovery URL ending in `/.well-known/openid-configuration`.
    /// For Kanidm this is the per-client URL
    /// (`https://<host>/oauth2/openid/<client_id>/.well-known/openid-configuration`).
    /// For Authentik/Authelia this is the issuer-level URL.
    /// Environment variable: `BOOKBOSS__OIDC__DISCOVERY_URL`
    pub discovery_url: Option<String>,

    /// OIDC client ID registered with the IdP.
    /// Environment variable: `BOOKBOSS__OIDC__CLIENT_ID`
    pub client_id: Option<String>,

    /// OIDC client secret registered with the IdP.
    /// Environment variable: `BOOKBOSS__OIDC__CLIENT_SECRET`
    pub client_secret: Option<String>,

    /// Login page button text. Defaults to "Sign in with SSO".
    /// Environment variable: `BOOKBOSS__OIDC__BUTTON_LABEL`
    pub button_label: Option<String>,
}

impl OidcConfig {
    pub const DEFAULT_BUTTON_LABEL: &'static str = "Sign in with SSO";

    /// Returns `true` if any required field is set — used to detect that the
    /// admin intended to enable SSO (even partially). `button_label` is
    /// cosmetic and does not count.
    #[must_use]
    pub fn is_set(&self) -> bool {
        self.discovery_url.is_some() || self.client_id.is_some() || self.client_secret.is_some()
    }

    /// Returns the configured button label or the default.
    #[must_use]
    pub fn button_label(&self) -> &str {
        self.button_label.as_deref().unwrap_or(Self::DEFAULT_BUTTON_LABEL)
    }

    /// Returns `true` only when SSO is fully configured and ready to use.
    ///
    /// Returns `false` when:
    /// - No fields are set at all → silent (SSO is simply disabled).
    /// - Some required fields are set but others are missing → each missing
    ///   field is logged via `tracing::error!` so the admin can see why the
    ///   "Sign in with SSO" button isn't appearing. Server startup continues
    ///   normally; password login remains available.
    ///
    /// This method is the sole logging site for missing-field problems —
    /// callers should treat the returned `bool` as authoritative and not log
    /// again.
    #[must_use]
    #[cfg(feature = "server")]
    pub fn is_sso_available(&self) -> bool {
        if !self.is_set() {
            return false;
        }

        let mut missing = Vec::new();
        if self.discovery_url.as_deref().is_none_or(str::is_empty) {
            missing.push("BOOKBOSS__OIDC__DISCOVERY_URL");
        }
        if self.client_id.as_deref().is_none_or(str::is_empty) {
            missing.push("BOOKBOSS__OIDC__CLIENT_ID");
        }
        if self.client_secret.as_deref().is_none_or(str::is_empty) {
            missing.push("BOOKBOSS__OIDC__CLIENT_SECRET");
        }

        if missing.is_empty() {
            return true;
        }

        for field in &missing {
            tracing::error!("OIDC SSO disabled — partial configuration: {} is missing or empty", field);
        }
        false
    }
}

#[cfg(feature = "web")]
pub mod web {
    use crate::BookBossFrontend;

    pub fn launch_web_frontend() {
        dioxus::launch(BookBossFrontend);
    }
}

#[cfg(feature = "server")]
mod error;

#[cfg(feature = "server")]
pub use error::FrontendError;

#[cfg(feature = "server")]
pub mod server;

use components::AppLayout;
use routes::{
    AuthorDetailPage, AuthorsPage, BookDetailPage, BooksPage, EditMetadataPage, IncomingPage, LandingPage, ProfilePage, ReviewPage, SeriesDetailPage,
    SeriesPage, SettingsPage, ShelfPage,
};
use serde::Deserialize;

#[derive(Routable, Clone, Debug, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[route("/?:login_failed")]
    LandingPage { login_failed: Option<u8> },
    #[layout(AppLayout)]
        #[route("/library")]
        BooksPage,
        #[route("/library/authors")]
        AuthorsPage,
        #[route("/library/series")]
        SeriesPage,
        #[route("/library/books/:token")]
        BookDetailPage { token: String },
        #[route("/library/books/:token/edit")]
        EditMetadataPage { token: String },
        #[route("/library/authors/:token")]
        AuthorDetailPage { token: String },
        #[route("/library/series/:token")]
        SeriesDetailPage { token: String },
        #[route("/library/incoming")]
        IncomingPage,
        #[route("/library/incoming/:token")]
        ReviewPage { token: String },
        #[route("/settings")]
        SettingsPage,
        #[route("/profile")]
        ProfilePage,
        #[route("/shelves/:token")]
        ShelfPage { token: String },
}

#[component]
fn BookBossFrontend() -> Element {
    rsx! { Router::<Route> {} }
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    #[test]
    fn oidc_config_unavailable_when_empty() {
        let config = OidcConfig::default();
        assert!(!config.is_set());
        assert!(!config.is_sso_available());
    }

    #[test]
    fn oidc_config_available_when_all_required_present() {
        let config = OidcConfig {
            discovery_url: Some("https://idp.example.com/.well-known/openid-configuration".into()),
            client_id: Some("bookboss".into()),
            client_secret: Some("secret".into()),
            button_label: None,
        };
        assert!(config.is_set());
        assert!(config.is_sso_available());
    }

    #[test]
    fn oidc_config_unavailable_when_partial() {
        let config = OidcConfig {
            discovery_url: Some("https://idp.example.com/.well-known/openid-configuration".into()),
            client_id: None,
            client_secret: None,
            button_label: None,
        };
        assert!(config.is_set());
        // Partial config — should log via tracing::error! and return false.
        // (Log capture is not asserted; manual smoke testing covers that.)
        assert!(!config.is_sso_available());
    }

    #[test]
    fn oidc_config_button_label_default() {
        let config = OidcConfig::default();
        assert_eq!(config.button_label(), "Sign in with SSO");
    }

    #[test]
    fn oidc_config_button_label_custom() {
        let config = OidcConfig {
            button_label: Some("Login with Authentik".into()),
            ..Default::default()
        };
        assert_eq!(config.button_label(), "Login with Authentik");
    }

    #[test]
    fn oidc_config_button_label_alone_does_not_enable_sso() {
        let config = OidcConfig {
            button_label: Some("Custom".into()),
            ..Default::default()
        };
        assert!(!config.is_set());
        assert!(!config.is_sso_available());
    }
}
