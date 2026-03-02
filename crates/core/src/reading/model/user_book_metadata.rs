use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{book::BookId, user::UserId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_status_serde_round_trip() {
        for status in [ReadStatus::Unread, ReadStatus::Reading, ReadStatus::Read, ReadStatus::Dnf] {
            let json = serde_json::to_string(&status).expect("serialise");
            let back: ReadStatus = serde_json::from_str(&json).expect("deserialise");
            assert_eq!(status, back);
        }
    }

    #[test]
    fn read_status_serialises_to_expected_strings() {
        assert_eq!(serde_json::to_string(&ReadStatus::Unread).unwrap(), r#""Unread""#);
        assert_eq!(serde_json::to_string(&ReadStatus::Reading).unwrap(), r#""Reading""#);
        assert_eq!(serde_json::to_string(&ReadStatus::Read).unwrap(), r#""Read""#);
        assert_eq!(serde_json::to_string(&ReadStatus::Dnf).unwrap(), r#""Dnf""#);
    }
}
