use std::{fmt, str::FromStr};

use chrono::{DateTime, Utc};
use derive_builder::Builder;
use rust_decimal::Decimal;

use crate::{
    book::{AuthorId, FileFormat, GenreId, MetadataSource, SeriesId, TagId},
    token::{Token, TokenAlphabet, define_token_prefix},
};

// ── Slug / filename helpers
// ───────────────────────────────────────────────────

/// Converts a string to a filesystem-safe, lowercase, hyphenated slug.
fn slugify(s: &str) -> String {
    let raw: String = s.chars().map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    raw.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-")
}

/// Returns the slug portion of a book's filename: `{author}-{title}` if an
/// author name is provided, or just `{title}` otherwise.
pub fn book_slug(title: &str, first_author_name: Option<&str>) -> String {
    match first_author_name {
        Some(author) if !author.is_empty() => format!("{}-{}", slugify(author), slugify(title)),
        _ => slugify(title),
    }
}

/// Returns the full filename (`slug.ext`) for a book file, e.g.
/// `"tolkien-j-r-r-the-fellowship-of-the-ring.epub"`.
pub fn book_filename(title: &str, first_author_name: Option<&str>, format: &FileFormat) -> String {
    format!("{}.{}", book_slug(title, first_author_name), format.extension())
}

#[cfg(test)]
mod slug_tests {
    use super::*;

    #[test]
    fn slug_title_only() {
        assert_eq!(book_slug("The Hobbit", None), "the-hobbit");
    }

    #[test]
    fn slug_with_author() {
        assert_eq!(book_slug("The Hobbit", Some("Tolkien")), "tolkien-the-hobbit");
    }

    #[test]
    fn slug_empty_author_falls_back_to_title() {
        assert_eq!(book_slug("Dune", Some("")), "dune");
    }

    #[test]
    fn filename_epub() {
        assert_eq!(book_filename("Dune", Some("Herbert"), &FileFormat::Epub), "herbert-dune.epub");
    }

    #[test]
    fn filename_kepub() {
        assert_eq!(book_filename("Dune", None, &FileFormat::Kepub), "dune.kepub.epub");
    }
}

define_token_prefix!(BookTokenPrefix, "BK_");
pub type BookId = u64;
pub type BookToken = Token<BookTokenPrefix, BookId, TokenAlphabet, { i64::MAX as u128 }>;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BookStatus {
    Incoming,
    Available,
    Archived,
}

impl BookStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Incoming => "incoming",
            Self::Available => "available",
            Self::Archived => "archived",
        }
    }
}

impl fmt::Display for BookStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for BookStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "incoming" => Ok(Self::Incoming),
            "available" => Ok(Self::Available),
            "archived" => Ok(Self::Archived),
            _ => Err(format!("unknown book status: {s}")),
        }
    }
}

// ── Sort types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BookSortField {
    DateAdded,
    Title,
    AuthorTitle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BookSortOrder {
    pub field: BookSortField,
    pub direction: SortDirection,
}

impl Default for BookSortOrder {
    fn default() -> Self {
        Self {
            field: BookSortField::DateAdded,
            direction: SortDirection::Desc,
        }
    }
}

/// Filter criteria for listing books.
///
/// Only `Available` books are returned — status is not a caller-controlled
/// dimension. All set fields are `ANDed` together. An empty filter returns
/// all available books.
#[derive(Debug, Clone, Default)]
pub struct BookQuery {
    pub series_id: Option<SeriesId>,
    pub author_id: Option<AuthorId>,
    pub genre_id: Option<GenreId>,
    pub tag_id: Option<TagId>,
    pub sort: Option<BookSortOrder>,
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
    pub has_cover: bool,
    #[builder(default)]
    pub sidecar_fingerprint: Option<String>,
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
    pub has_cover: bool,
}
