use bb_utils::{define_token_prefix, token::Token};
use chrono::{DateTime, Utc};
use derive_builder::Builder;
use rust_decimal::Decimal;

use crate::book::{AuthorId, GenreId, MetadataSource, SeriesId, TagId};

define_token_prefix!(BookTokenPrefix, "BK_");
pub type BookId = u64;
pub type BookToken = Token<BookTokenPrefix, BookId, { i64::MAX as u128 }>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BookStatus {
    Incoming,
    Available,
    Archived,
}

/// Filter criteria for listing books.
///
/// All set fields are ANDed together. An empty filter returns all books.
#[derive(Debug, Clone, Default)]
pub struct BookFilter {
    pub status: Option<BookStatus>,
    pub series_id: Option<SeriesId>,
    pub author_id: Option<AuthorId>,
    pub genre_id: Option<GenreId>,
    pub tag_id: Option<TagId>,
}

#[derive(Debug, Clone, Builder)]
pub struct Book {
    pub id: BookId,
    pub version: u64,
    pub token: BookToken,
    pub title: String,
    pub status: BookStatus,
    #[builder(default)]
    pub description: Option<String>,
    #[builder(default)]
    pub published_date: Option<i32>,
    #[builder(default)]
    pub language: Option<String>,
    #[builder(default)]
    pub series_id: Option<SeriesId>,
    #[builder(default)]
    pub series_number: Option<Decimal>,
    #[builder(default)]
    pub publisher_id: Option<crate::book::PublisherId>,
    #[builder(default)]
    pub page_count: Option<i32>,
    #[builder(default)]
    pub rating: Option<i16>,
    #[builder(default)]
    pub metadata_source: Option<MetadataSource>,
    #[builder(default)]
    pub cover_path: Option<String>,
    #[builder(default = "Utc::now()")]
    pub created_at: DateTime<Utc>,
    #[builder(default = "Utc::now()")]
    pub updated_at: DateTime<Utc>,
}

impl Book {
    #[cfg(any(test, feature = "test-support"))]
    pub fn fake(id: BookId, title: impl Into<String>, status: BookStatus) -> Self {
        BookBuilder::default()
            .id(id)
            .version(1)
            .token(BookToken::new(id))
            .title(title.into())
            .status(status)
            .build()
            .expect("fake book should build successfully")
    }
}

#[derive(Debug, Clone)]
pub struct NewBook {
    pub title: String,
    pub status: BookStatus,
    pub description: Option<String>,
    pub published_date: Option<i32>,
    pub language: Option<String>,
    pub series_id: Option<SeriesId>,
    pub series_number: Option<Decimal>,
    pub publisher_id: Option<crate::book::PublisherId>,
    pub page_count: Option<i32>,
    pub rating: Option<i16>,
    pub metadata_source: Option<MetadataSource>,
    pub cover_path: Option<String>,
}
