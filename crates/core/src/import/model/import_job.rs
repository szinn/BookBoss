use bb_utils::{define_token_prefix, token::Token};
use chrono::{DateTime, Utc};

use crate::{
    book::{BookId, FileFormat},
    user::UserId,
};

define_token_prefix!(ImportJobTokenPrefix, "IJ_");
pub type ImportJobId = u64;
pub type ImportJobToken = Token<ImportJobTokenPrefix, ImportJobId, { i64::MAX as u128 }>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportStatus {
    Pending,
    Extracting,
    Identifying,
    NeedsReview,
    Approved,
    Rejected,
    Error,
}

/// Which provider populated the metadata during the import pipeline.
///
/// Distinct from `book::MetadataSource`, which tracks the ongoing canonical
/// source for a book record and includes `Manual` for admin-edited entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportSource {
    /// Metadata extracted from the file itself (EPUB OPF, MOBI headers).
    Embedded,
    Hardcover,
    OpenLibrary,
}

#[derive(Debug, Clone)]
pub struct ImportJob {
    pub id: ImportJobId,
    pub version: u64,
    pub token: ImportJobToken,
    pub file_path: String,
    pub file_hash: String,
    pub file_format: FileFormat,
    pub detected_at: DateTime<Utc>,
    pub status: ImportStatus,
    pub candidate_book_id: Option<BookId>,
    pub metadata_source: Option<ImportSource>,
    pub error_message: Option<String>,
    pub reviewed_by: Option<UserId>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a new import job. Status always starts as `Pending`.
#[derive(Debug, Clone)]
pub struct NewImportJob {
    pub file_path: String,
    pub file_hash: String,
    pub file_format: FileFormat,
    pub detected_at: DateTime<Utc>,
}
