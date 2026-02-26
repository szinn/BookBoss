use chrono::{DateTime, Utc};
use derive_builder::Builder;

use crate::Error;

#[derive(Debug, Clone, Builder)]
pub struct Session {
    pub id: String,
    pub session: String,
    pub expires_at: DateTime<Utc>,
    #[builder(default = "Utc::now()")]
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewSession {
    pub id: String,
    pub session: String,
    pub expires_at: DateTime<Utc>,
}

impl NewSession {
    pub fn new(id: impl Into<String>, session: impl Into<String>, expires_at: DateTime<Utc>) -> Result<Self, Error> {
        Ok(Self {
            id: id.into(),
            session: session.into(),
            expires_at,
        })
    }
}
