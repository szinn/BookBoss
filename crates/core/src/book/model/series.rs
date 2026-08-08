use chrono::{DateTime, Utc};

use crate::token::{Token, TokenAlphabet, define_token_prefix};

define_token_prefix!(SeriesTokenPrefix, "SR_");
pub type SeriesId = u64;
pub type SeriesToken = Token<SeriesTokenPrefix, SeriesId, TokenAlphabet, { i64::MAX as u128 }>;

#[derive(Debug, Clone)]
pub struct Series {
    pub id: SeriesId,
    pub version: u64,
    pub token: SeriesToken,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Series {
    #[cfg(any(test, feature = "test-support"))]
    pub fn fake(id: SeriesId, name: impl Into<String>) -> Self {
        Self {
            id,
            version: 1,
            token: SeriesToken::new(id),
            name: name.into(),
            description: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NewSeries {
    pub name: String,
    pub description: Option<String>,
}
