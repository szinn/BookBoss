use std::{fmt::Debug, sync::Arc};

use axum_session::{DatabaseError, DatabasePool};
use bb_core::{CoreServices, auth::NewSession, user::UserId};
use chrono::DateTime;

use crate::server::AuthUser;

#[derive(Clone)]
pub(crate) struct BackendSessionPool {
    pub(crate) core_services: Arc<CoreServices>,
}

impl BackendSessionPool {
    pub(crate) fn new(core_services: Arc<CoreServices>) -> Self {
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
