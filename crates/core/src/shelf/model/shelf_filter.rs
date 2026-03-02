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

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::reading::ReadStatus;

    #[test]
    fn shelf_filter_empty_round_trip() {
        let filter = ShelfFilter::default();
        let json = serde_json::to_string(&filter).expect("serialise");
        let back: ShelfFilter = serde_json::from_str(&json).expect("deserialise");
        assert!(back.authors.is_none());
        assert!(back.series.is_none());
        assert!(back.genres.is_none());
        assert!(back.tags.is_none());
        assert!(back.publishers.is_none());
        assert!(back.languages.is_none());
        assert!(back.read_status.is_none());
        assert!(back.rating_min.is_none());
        assert!(back.date_added_after.is_none());
    }

    #[test]
    fn shelf_filter_all_fields_round_trip() {
        let filter = ShelfFilter {
            authors: Some(vec![1, 2, 3]),
            series: Some(vec![10]),
            genres: Some(vec![5, 6]),
            tags: Some(vec![20, 21, 22]),
            publishers: Some(vec![7]),
            languages: Some(vec!["en".to_string(), "fr".to_string()]),
            read_status: Some(vec![ReadStatus::Unread, ReadStatus::Reading]),
            rating_min: Some(3),
            date_added_after: Some(Utc.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap()),
        };

        let json = serde_json::to_string(&filter).expect("serialise");
        let back: ShelfFilter = serde_json::from_str(&json).expect("deserialise");

        assert_eq!(back.authors, Some(vec![1, 2, 3]));
        assert_eq!(back.series, Some(vec![10]));
        assert_eq!(back.genres, Some(vec![5, 6]));
        assert_eq!(back.tags, Some(vec![20, 21, 22]));
        assert_eq!(back.publishers, Some(vec![7]));
        assert_eq!(back.languages, Some(vec!["en".to_string(), "fr".to_string()]));
        assert_eq!(back.read_status, Some(vec![ReadStatus::Unread, ReadStatus::Reading]));
        assert_eq!(back.rating_min, Some(3));
        assert_eq!(back.date_added_after, Some(Utc.with_ymd_and_hms(2025, 1, 15, 0, 0, 0).unwrap()));
    }

    #[test]
    fn shelf_filter_partial_fields_round_trip() {
        let filter = ShelfFilter {
            authors: Some(vec![42]),
            read_status: Some(vec![ReadStatus::Read]),
            rating_min: Some(5),
            ..Default::default()
        };

        let json = serde_json::to_string(&filter).expect("serialise");
        let back: ShelfFilter = serde_json::from_str(&json).expect("deserialise");

        assert_eq!(back.authors, Some(vec![42]));
        assert_eq!(back.read_status, Some(vec![ReadStatus::Read]));
        assert_eq!(back.rating_min, Some(5));
        assert!(back.series.is_none());
        assert!(back.languages.is_none());
    }
}
