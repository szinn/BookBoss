use std::collections::HashSet;

use axum_session_auth::Authentication;
use bb_core::user::UserId;
use serde::{Deserialize, Serialize};

use crate::server::BackendSessionPool;

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
