use bb_utils::{define_token_prefix, token::Token};
use chrono::{DateTime, Utc};

define_token_prefix!(SeriesTokenPrefix, "SR_");
pub type SeriesId = u64;
pub type SeriesToken = Token<SeriesTokenPrefix, SeriesId, { i64::MAX as u128 }>;

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
        use chrono::Utc;
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
