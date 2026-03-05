use std::collections::HashSet;

use axum_session_auth::Authentication;
use bb_core::{CoreServices, user::UserId};
use serde::{Deserialize, Serialize};

use crate::{
    server::BackendSessionPool,
    settings::{BookDisplayView, FrontendSettings},
};

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
            id: 0,
            anonymous: true,
            username: String::new(),
            permissions: HashSet::new(),
        }
    }
}

#[async_trait::async_trait]
impl Authentication<Self, UserId, BackendSessionPool> for AuthUser {
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
                permissions: user.capabilities.iter().map(|c| c.as_str().to_owned()).collect(),
            }),
            None => Ok(Self::default()),
        }
    }

    fn is_authenticated(&self) -> bool {
        !self.anonymous
    }

    fn is_active(&self) -> bool {
        !self.anonymous
    }

    fn is_anonymous(&self) -> bool {
        self.anonymous
    }
}

impl AuthUser {
    pub(crate) async fn get_book_display_view(&self, core: &CoreServices) -> BookDisplayView {
        if self.anonymous {
            return BookDisplayView::default();
        }
        core.user_setting_service
            .get(self.id, FrontendSettings::BookDisplayView.key())
            .await
            .inspect_err(|e| tracing::warn!("Failed to load book_display_view for user {}: {e}", self.id))
            .ok()
            .flatten()
            .and_then(|s| s.value.parse().ok())
            .unwrap_or_default()
    }

    pub(crate) async fn set_book_display_view(&self, view: BookDisplayView, core: &CoreServices) -> Result<(), bb_core::Error> {
        core.user_setting_service
            .set(self.id, FrontendSettings::BookDisplayView.key(), &view.to_string())
            .await
            .map(|_| ())
    }

    pub(crate) async fn get_api_key(&self, core: &CoreServices) -> String {
        if self.anonymous {
            return String::new();
        }
        core.user_setting_service
            .get(self.id, FrontendSettings::ApiKey.key())
            .await
            .inspect_err(|e| tracing::warn!("Failed to load api_key for user {}: {e}", self.id))
            .ok()
            .flatten()
            .map(|s| s.value)
            .unwrap_or_default()
    }

    pub(crate) async fn set_api_key(&self, key: &str, core: &CoreServices) -> Result<(), bb_core::Error> {
        core.user_setting_service.set(self.id, FrontendSettings::ApiKey.key(), key).await.map(|_| ())
    }
}
