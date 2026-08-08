use chrono::{DateTime, Utc};

use crate::token::{Token, TokenAlphabet, define_token_prefix};

define_token_prefix!(TagTokenPrefix, "T_");
pub type TagId = u64;
pub type TagToken = Token<TagTokenPrefix, TagId, TokenAlphabet, { i64::MAX as u128 }>;

#[derive(Debug, Clone)]
pub struct Tag {
    pub id: TagId,
    pub version: u64,
    pub token: TagToken,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewTag {
    pub name: String,
}
