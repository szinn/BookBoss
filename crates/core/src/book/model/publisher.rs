use bb_utils::{define_token_prefix, token::Token};
use chrono::{DateTime, Utc};

define_token_prefix!(PublisherTokenPrefix, "P_");
pub type PublisherId = u64;
pub type PublisherToken = Token<PublisherTokenPrefix, PublisherId, { i64::MAX as u128 }>;

#[derive(Debug, Clone)]
pub struct Publisher {
    pub id: PublisherId,
    pub version: u64,
    pub token: PublisherToken,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewPublisher {
    pub name: String,
}
