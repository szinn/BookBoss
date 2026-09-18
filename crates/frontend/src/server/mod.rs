use std::sync::Arc;

use axum::{
    Extension,
    extract::DefaultBodyLimit,
    http::{HeaderName, Request},
};
use axum_session::{SessionConfig, SessionLayer, SessionStore};
use axum_session_auth::{AuthConfig, AuthSessionLayer};
use bb_core::{CoreServices, user::UserId};
use chrono::Duration;
use dioxus::server::DioxusRouterExt;
use tokio_graceful_shutdown::{IntoSubsystem, SubsystemHandle};
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer,
    limit::RequestBodyLimitLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};

use crate::{BookBossFrontend, FrontendConfig, OidcConfig};

pub(crate) mod covers;
pub(crate) mod downloads;
pub(crate) mod events;
pub(crate) mod kobo;
pub(crate) mod koreader;
pub(crate) mod oidc;
pub(crate) mod opds;
pub(crate) mod session_pool;

pub(crate) use oidc::OidcClientCell;
pub(crate) use session_pool::{AuthSession, BackendSessionPool};

pub(crate) mod auth_user;

pub(crate) use auth_user::AuthUser;

const REQUEST_ID_HEADER: &str = "x-request-id";
const DEFAULT_EXPIRATION_DURATION: Duration = Duration::days(7);
const MAX_REQUEST_BODY_SIZE: usize = 70 * 1024 * 1024; // 70 MiB

pub struct FrontendSubsystem {
    config: FrontendConfig,
    oidc_config: Option<OidcConfig>,
    core_services: Arc<CoreServices>,
}

impl IntoSubsystem<anyhow::Error> for FrontendSubsystem {
    async fn run(self, subsys: &mut SubsystemHandle) -> Result<(), anyhow::Error> {
        tracing::info!("FrontendSubsystem starting...");

        let core_services = self.core_services.clone();
        let backend_pool = BackendSessionPool::new(core_services.clone());
        let session_config = SessionConfig::default().with_lifetime(DEFAULT_EXPIRATION_DURATION);
        let auth_config = AuthConfig::<UserId>::default();

        let x_request_id = HeaderName::from_static(REQUEST_ID_HEADER);
        let session_store = SessionStore::<BackendSessionPool>::new(Some(backend_pool.clone()), session_config).await?;

        let middleware = ServiceBuilder::new()
            .layer(CompressionLayer::new())
            .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_SIZE))
            .layer(RequestBodyLimitLayer::new(MAX_REQUEST_BODY_SIZE))
            .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
            .layer(TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
                let request_id = request
                    .headers()
                    .get(REQUEST_ID_HEADER)
                    .map(|v| v.to_str().unwrap_or_default())
                    .unwrap_or_default();

                tracing::trace_span!(
                    "",
                    request_id = ?request_id,
                )
            }))
            .layer(PropagateRequestIdLayer::new(x_request_id))
            .layer(SessionLayer::new(session_store))
            .layer(AuthSessionLayer::<AuthUser, UserId, BackendSessionPool, BackendSessionPool>::new(Some(backend_pool)).with_config(auth_config));

        let frontend_config = Arc::new(self.config.clone());
        let kobo = kobo::kobo_router(&self.config.base_url, core_services.clone());
        let koreader = koreader::koreader_router();
        let opds = opds::opds_router();

        let mut app_router = axum::Router::new()
            .route("/api/v1/covers/{book_token}", axum::routing::get(covers::serve_cover))
            .route("/api/v1/books/{book_token}/download/{format}", axum::routing::get(downloads::serve_book_file))
            .route("/api/v1/events", axum::routing::get(events::event_stream))
            .serve_dioxus_application(dioxus_server::ServeConfig::new(), BookBossFrontend)
            .merge(kobo)
            .merge(koreader)
            .merge(opds);

        // When SSO is fully configured, merge the OIDC router and expose an
        // `OidcClientCell` to handlers / server fns. Discovery is NOT
        // performed here: the cell defers it to the first request that needs
        // it (login page render or the "sign in with SSO" button) and
        // retries on subsequent requests if it failed, so a transient IdP
        // outage doesn't permanently disable SSO for the process's lifetime.
        if let Some(cfg) = self.oidc_config.clone().filter(OidcConfig::is_sso_available) {
            let cell = Arc::new(OidcClientCell::new(cfg, self.config.base_url.clone()));
            app_router = app_router.merge(oidc::oidc_router()).layer(Extension(cell));
        }

        let app_router = app_router
            .layer(Extension(core_services.health_service.clone()))
            .layer(Extension(core_services))
            .layer(Extension(frontend_config))
            .layer(middleware);

        let health_handler = || async { axum::http::StatusCode::OK };
        let router = axum::Router::new()
            .route("/healthz", axum::routing::get(health_handler))
            .route("/readyz", axum::routing::get(health_handler))
            .merge(app_router);

        let ip = std::env::var("IP").ok().unwrap_or_else(|| self.config.listen_ip.clone());
        let port: u16 = std::env::var("PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(self.config.listen_port);
        let listener = tokio::net::TcpListener::bind(&format!("{ip}:{port}")).await?;

        tracing::info!("Frontend listening on {}", listener.local_addr()?);

        tokio::select! {
            () = subsys.on_shutdown_requested() => {
                tracing::info!("Frontend shutting down...");
            }
            result = axum::serve(listener, router) => {
                if let Err(e) = result {
                    tracing::error!("Frontend server error: {}", e);
                }
                subsys.request_shutdown();
            }
        }

        Ok(())
    }
}

#[must_use]
pub fn create_frontend_subsystem(config: &FrontendConfig, oidc_config: Option<OidcConfig>, core_services: Arc<CoreServices>) -> FrontendSubsystem {
    FrontendSubsystem {
        config: config.to_owned(),
        oidc_config,
        core_services,
    }
}
