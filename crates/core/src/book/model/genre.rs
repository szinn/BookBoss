use bb_utils::{define_token_prefix, token::Token};
use chrono::{DateTime, Utc};

define_token_prefix!(GenreTokenPrefix, "G_");
pub type GenreId = u64;
pub type GenreToken = Token<GenreTokenPrefix, GenreId, { i64::MAX as u128 }>;

#[derive(Debug, Clone)]
pub struct Genre {
    pub id: GenreId,
    pub version: u64,
    pub token: GenreToken,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewGenre {
    pub name: String,
}
