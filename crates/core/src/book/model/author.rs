use bb_utils::{define_token_prefix, token::Token};
use chrono::{DateTime, Utc};

use crate::book::BookId;

define_token_prefix!(AuthorTokenPrefix, "A_");
pub type AuthorId = u64;
pub type AuthorToken = Token<AuthorTokenPrefix, AuthorId, { i64::MAX as u128 }>;

#[derive(Debug, Clone)]
pub struct Author {
    pub id: AuthorId,
    pub version: u64,
    pub token: AuthorToken,
    pub name: String,
    pub bio: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewAuthor {
    pub name: String,
    pub bio: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorRole {
    Author,
    Editor,
    Translator,
    Illustrator,
}

#[derive(Debug, Clone)]
pub struct BookAuthor {
    pub book_id: BookId,
    pub author_id: AuthorId,
    pub role: AuthorRole,
    pub sort_order: i32,
}
