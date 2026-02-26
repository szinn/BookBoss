use std::{sync::Arc, thread::JoinHandle};

use axum::{
    Extension,
    http::{HeaderName, Request},
};
use axum_session::{SessionConfig, SessionLayer, SessionStore};
use axum_session_auth::{AuthConfig, AuthSessionLayer, HasPermission};
use bb_core::{CoreServices, user::UserId};
use tower::ServiceBuilder;
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};

use crate::{BookBossFrontend, FrontendConfig};

pub(crate) mod session_pool;

pub(crate) use session_pool::{AuthSession, BackendSessionPool};

pub(crate) mod auth_user;

pub(crate) use auth_user::AuthUser;

#[async_trait::async_trait]
impl HasPermission<BackendSessionPool> for AuthUser {
    #[tracing::instrument(level = "trace", skip(self, _pool))]
    async fn has(&self, perm: &str, _pool: &Option<&BackendSessionPool>) -> bool {
        self.permissions.contains(perm)
    }
}

const REQUEST_ID_HEADER: &str = "x-request-id";

pub fn launch_server_frontend(config: &FrontendConfig, core_services: Arc<CoreServices>) -> JoinHandle<usize> {
    let listen_ip = config.listen_ip.clone();
    let listen_port = config.listen_port;

    std::thread::spawn(move || {
        // SAFETY: Called at the start of a dedicated thread before any other work,
        // so no other threads are reading these env vars concurrently.
        // Env vars set by `dx serve` take priority; only apply config as fallback.
        unsafe {
            if std::env::var_os("IP").is_none() {
                std::env::set_var("IP", &listen_ip);
            }
            if std::env::var_os("PORT").is_none() {
                std::env::set_var("PORT", listen_port.to_string());
            }
        }
        let effective_ip = std::env::var("IP").unwrap_or(listen_ip);
        let effective_port = std::env::var("PORT").unwrap_or_else(|_| listen_port.to_string());
        tracing::info!("Frontend started on {effective_ip}:{effective_port}");

        dioxus::serve(|| {
            let core_services = core_services.clone();
            let backend_pool = BackendSessionPool::new(core_services.clone());
            let session_config = SessionConfig::default();
            let auth_config = AuthConfig::<UserId>::default().with_anonymous_user_id(Some(1));
            async move {
                let x_request_id = HeaderName::from_static(REQUEST_ID_HEADER);
                let session_store = SessionStore::<BackendSessionPool>::new(Some(backend_pool.clone()), session_config).await?;

                let middleware = ServiceBuilder::new()
                    .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
                    .layer(TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
                        let request_id = request
                            .headers()
                            .get(REQUEST_ID_HEADER)
                            .map(|v| v.to_str().unwrap_or_default())
                            .unwrap_or_default();

                        tracing::info_span!(
                            "http",
                            request_id = ?request_id,
                        )
                    }))
                    .layer(PropagateRequestIdLayer::new(x_request_id))
                    .layer(SessionLayer::new(session_store))
                    .layer(AuthSessionLayer::<AuthUser, UserId, BackendSessionPool, BackendSessionPool>::new(Some(backend_pool)).with_config(auth_config));

                let router = dioxus::server::router(BookBossFrontend).layer(Extension(core_services)).layer(middleware);

                Ok(router)
            }
        })
    })
}
