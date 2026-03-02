use chrono::{DateTime, Utc};

use crate::{book::BookId, user::UserId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadStatus {
    Unread,
    Reading,
    Read,
    Dnf,
}

/// Per-user reading state for a single book.
///
/// A missing row is semantically equivalent to `read_status: Unread`.
/// This struct is always written via upsert, never plain insert.
#[derive(Debug, Clone)]
pub struct UserBookMetadata {
    pub user_id: UserId,
    pub book_id: BookId,
    pub read_status: ReadStatus,
    /// Reading progress in basis points (0 = none, 10000 = complete).
    pub progress_percentage: Option<u16>,
    /// Raw device-specific resume position (EPUB CFI, Kindle location, etc.).
    pub position_token: Option<String>,
    pub last_progress_at: Option<DateTime<Utc>>,
    /// Personal star rating, 1–5.
    pub personal_rating: Option<u8>,
    /// Incremented each time a `reading → read` transition completes.
    pub times_read: u32,
    pub date_started: Option<DateTime<Utc>>,
    pub date_finished: Option<DateTime<Utc>>,
    pub last_opened_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}
