use dioxus::prelude::*;

mod components;
pub(crate) mod routes;

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
pub mod server {
    use std::{collections::HashSet, fmt::Debug, sync::Arc, thread::JoinHandle};

    use axum::{
        Extension,
        http::{HeaderName, Request},
    };
    use axum_session::{DatabaseError, DatabasePool, SessionConfig, SessionLayer, SessionStore};
    use axum_session_auth::{AuthConfig, AuthSessionLayer, Authentication, HasPermission};
    use bb_core::{CoreServices, auth::NewSession, user::UserId};
    use chrono::DateTime;
    use serde::{Deserialize, Serialize};
    use tower::ServiceBuilder;
    use tower_http::{
        request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
        trace::TraceLayer,
    };

    use crate::{BookBossFrontend, FrontendConfig};

    #[derive(Clone)]
    pub(crate) struct BackendSessionPool {
        core_services: Arc<CoreServices>,
    }

    impl BackendSessionPool {
        fn new(core_services: Arc<CoreServices>) -> Self {
            Self { core_services }
        }
    }

    impl Debug for BackendSessionPool {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("BackendSessionPool").finish()
        }
    }

    pub(crate) type AuthSession = axum_session_auth::AuthSession<AuthUser, UserId, BackendSessionPool, BackendSessionPool>;

    #[async_trait::async_trait]
    impl DatabasePool for BackendSessionPool {
        #[tracing::instrument(level = "trace", skip(self))]
        async fn initiate(&self, _table_name: &str) -> Result<(), DatabaseError> {
            Ok(())
        }

        #[tracing::instrument(level = "trace", skip(self))]
        async fn count(&self, _table_name: &str) -> Result<i64, DatabaseError> {
            self.core_services
                .auth_service
                .count()
                .await
                .map_err(|e| DatabaseError::GenericSelectError(e.to_string()))
        }

        #[tracing::instrument(level = "trace", skip(self))]
        async fn store(&self, id: &str, session: &str, expires: i64, _table_name: &str) -> Result<(), DatabaseError> {
            let expires_at = DateTime::from_timestamp(expires, 0).ok_or_else(|| DatabaseError::GenericInsertError(format!("invalid timestamp: {expires}")))?;
            let new_session = NewSession::new(id, session, expires_at).map_err(|e| DatabaseError::GenericInsertError(e.to_string()))?;
            self.core_services
                .auth_service
                .store(new_session)
                .await
                .map(|_| ())
                .map_err(|e| DatabaseError::GenericInsertError(e.to_string()))
        }

        #[tracing::instrument(level = "trace", skip(self))]
        async fn load(&self, id: &str, _table_name: &str) -> Result<Option<String>, DatabaseError> {
            self.core_services
                .auth_service
                .load(id)
                .await
                .map(|opt| opt.map(|s| s.session))
                .map_err(|e| DatabaseError::GenericSelectError(e.to_string()))
        }

        #[tracing::instrument(level = "trace", skip(self))]
        async fn delete_one_by_id(&self, id: &str, _table_name: &str) -> Result<(), DatabaseError> {
            self.core_services
                .auth_service
                .delete_by_id(id)
                .await
                .map_err(|e| DatabaseError::GenericDeleteError(e.to_string()))
        }

        #[tracing::instrument(level = "trace", skip(self))]
        async fn exists(&self, id: &str, _table_name: &str) -> Result<bool, DatabaseError> {
            self.core_services
                .auth_service
                .exists(id)
                .await
                .map_err(|e| DatabaseError::GenericSelectError(e.to_string()))
        }

        #[tracing::instrument(level = "trace", skip(self))]
        async fn delete_by_expiry(&self, _table_name: &str) -> Result<Vec<String>, DatabaseError> {
            self.core_services
                .auth_service
                .delete_by_expiry()
                .await
                .map_err(|e| DatabaseError::GenericDeleteError(e.to_string()))
        }

        #[tracing::instrument(level = "trace", skip(self))]
        async fn delete_all(&self, _table_name: &str) -> Result<(), DatabaseError> {
            self.core_services
                .auth_service
                .delete_all()
                .await
                .map_err(|e| DatabaseError::GenericDeleteError(e.to_string()))
        }

        #[tracing::instrument(level = "trace", skip(self))]
        async fn get_ids(&self, _table_name: &str) -> Result<Vec<String>, DatabaseError> {
            self.core_services
                .auth_service
                .get_ids()
                .await
                .map_err(|e| DatabaseError::GenericSelectError(e.to_string()))
        }

        #[tracing::instrument(level = "trace", skip(self))]
        fn auto_handles_expiry(&self) -> bool {
            false
        }
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub(crate) struct AuthUser {
        id: UserId,
        anonymous: bool,
        pub username: String,
        pub permissions: HashSet<String>,
    }

    impl Default for AuthUser {
        fn default() -> Self {
            Self {
                id: 1,
                anonymous: true,
                username: String::new(),
                permissions: HashSet::new(),
            }
        }
    }

    #[async_trait::async_trait]
    impl Authentication<Self, UserId, BackendSessionPool> for AuthUser {
        #[tracing::instrument(level = "trace", skip(pool))]
        async fn load_user(userid: UserId, pool: Option<&BackendSessionPool>) -> Result<Self, anyhow::Error> {
            let Some(pool) = pool else {
                return Ok(Self::default());
            };
            let user = pool.core_services.user_service.find_by_id(userid).await?;
            match user {
                Some(user) => Ok(Self {
                    id: userid,
                    anonymous: false,
                    username: user.username,
                    permissions: user.capabilities.iter().map(|c| format!("{c:?}")).collect(),
                }),
                None => Ok(Self::default()),
            }
        }

        #[tracing::instrument(level = "trace")]
        fn is_authenticated(&self) -> bool {
            !self.anonymous
        }

        #[tracing::instrument(level = "trace")]
        fn is_active(&self) -> bool {
            !self.anonymous
        }

        #[tracing::instrument(level = "trace")]
        fn is_anonymous(&self) -> bool {
            self.anonymous
        }
    }

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
}

use components::AppLayout;
use routes::BooksPage;
use serde::Deserialize;

#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(AppLayout)]
        #[route("/books")]
        BooksPage {},
}

#[component]
fn BookBossFrontend() -> Element {
    rsx! { Router::<Route> {} }
}
