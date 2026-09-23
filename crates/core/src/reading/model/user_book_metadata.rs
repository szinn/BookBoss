use std::{fmt, str::FromStr};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{book::BookId, user::UserId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadStatus {
    Unread,
    Reading,
    Paused,
    Rereading,
    Read,
    Abandoned,
}

impl fmt::Display for ReadStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unread => write!(f, "Unread"),
            Self::Reading => write!(f, "Reading"),
            Self::Paused => write!(f, "Paused"),
            Self::Rereading => write!(f, "Rereading"),
            Self::Read => write!(f, "Read"),
            Self::Abandoned => write!(f, "Abandoned"),
        }
    }
}

impl FromStr for ReadStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Unread" => Ok(Self::Unread),
            "Reading" => Ok(Self::Reading),
            "Paused" => Ok(Self::Paused),
            "Rereading" => Ok(Self::Rereading),
            "Read" => Ok(Self::Read),
            "Abandoned" => Ok(Self::Abandoned),
            _ => Err(format!("Invalid ReadStatus: {s}")),
        }
    }
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
    /// Location type for `position_token` (e.g. `"KoboSpanXPath"`, `"CFI"`).
    pub position_type: Option<String>,
    /// Raw device-specific resume position (EPUB CFI, Kobo XPath, etc.).
    pub position_token: Option<String>,
    /// Content file the position refers to (e.g. `"OEBPS/ch03.xhtml"`); Kobo
    /// `KoboSpan` values are only unique within this file.
    pub position_source: Option<String>,
    /// Progress within `position_source` in basis points, as reported by the
    /// device.
    pub content_source_progress_percentage: Option<u16>,
    pub last_progress_at: Option<DateTime<Utc>>,
    /// Minutes spent reading as reported by the device.
    pub spent_reading_minutes: Option<i32>,
    /// Estimated minutes remaining as reported by the device.
    pub remaining_time_minutes: Option<i32>,
    /// Personal star rating, 1–5.
    pub personal_rating: Option<u8>,
    /// Incremented each time a `reading → read` transition completes.
    pub times_read: u32,
    pub date_started: Option<DateTime<Utc>>,
    pub date_finished: Option<DateTime<Utc>>,
    pub last_opened_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

/// Reading state as reported by a sync device (Kobo, `KOReader`), applied via
/// [`ReadingService::sync_device_state`](crate::reading::ReadingService::sync_device_state).
#[derive(Debug, Clone, Default)]
pub struct DeviceReadingState {
    /// Overall progress in basis points.
    pub progress_bps: Option<u16>,
    /// Progress within `position_source` in basis points.
    pub content_source_progress_bps: Option<u16>,
    pub position_type: Option<String>,
    pub position_token: Option<String>,
    pub position_source: Option<String>,
    pub spent_reading_minutes: Option<i32>,
    pub remaining_time_minutes: Option<i32>,
    /// When the device recorded this state; `None` keeps the stored value.
    pub last_progress_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_status_serde_round_trip() {
        for status in [
            ReadStatus::Unread,
            ReadStatus::Reading,
            ReadStatus::Paused,
            ReadStatus::Rereading,
            ReadStatus::Read,
            ReadStatus::Abandoned,
        ] {
            let json = serde_json::to_string(&status).expect("serialise");
            let back: ReadStatus = serde_json::from_str(&json).expect("deserialise");
            assert_eq!(status, back);
        }
    }

    #[test]
    fn read_status_serialises_to_expected_strings() {
        assert_eq!(serde_json::to_string(&ReadStatus::Unread).unwrap(), r#""Unread""#);
        assert_eq!(serde_json::to_string(&ReadStatus::Reading).unwrap(), r#""Reading""#);
        assert_eq!(serde_json::to_string(&ReadStatus::Paused).unwrap(), r#""Paused""#);
        assert_eq!(serde_json::to_string(&ReadStatus::Rereading).unwrap(), r#""Rereading""#);
        assert_eq!(serde_json::to_string(&ReadStatus::Read).unwrap(), r#""Read""#);
        assert_eq!(serde_json::to_string(&ReadStatus::Abandoned).unwrap(), r#""Abandoned""#);
    }
}
