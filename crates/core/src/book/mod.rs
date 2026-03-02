use bb_utils::{define_token_prefix, token::Token};

define_token_prefix!(AuthorTokenPrefix, "A_");
pub type AuthorId = u64;
pub type AuthorToken = Token<AuthorTokenPrefix, AuthorId, { i64::MAX as u128 }>;

define_token_prefix!(SeriesTokenPrefix, "SR_");
pub type SeriesId = u64;
pub type SeriesToken = Token<SeriesTokenPrefix, SeriesId, { i64::MAX as u128 }>;

define_token_prefix!(PublisherTokenPrefix, "P_");
pub type PublisherId = u64;
pub type PublisherToken = Token<PublisherTokenPrefix, PublisherId, { i64::MAX as u128 }>;

define_token_prefix!(GenreTokenPrefix, "G_");
pub type GenreId = u64;
pub type GenreToken = Token<GenreTokenPrefix, GenreId, { i64::MAX as u128 }>;

define_token_prefix!(TagTokenPrefix, "T_");
pub type TagId = u64;
pub type TagToken = Token<TagTokenPrefix, TagId, { i64::MAX as u128 }>;
