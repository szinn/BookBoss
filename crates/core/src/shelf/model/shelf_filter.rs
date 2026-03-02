use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    book::{AuthorId, GenreId, PublisherId, SeriesId, TagId},
    reading::ReadStatus,
};

/// Filter criteria for a smart shelf.
///
/// All set fields are ANDed together. Within each field, any match suffices.
/// Stored as JSONB in `shelves.filter_criteria`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShelfFilter {
    pub authors: Option<Vec<AuthorId>>,
    pub series: Option<Vec<SeriesId>>,
    pub genres: Option<Vec<GenreId>>,
    pub tags: Option<Vec<TagId>>,
    pub publishers: Option<Vec<PublisherId>>,
    pub languages: Option<Vec<String>>,
    /// Resolved relative to the requesting user's `user_book_metadata`.
    pub read_status: Option<Vec<ReadStatus>>,
    /// Minimum personal rating (1–5, inclusive).
    pub rating_min: Option<u8>,
    pub date_added_after: Option<DateTime<Utc>>,
}
