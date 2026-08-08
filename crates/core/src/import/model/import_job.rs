use std::{fmt, str::FromStr};

use chrono::{DateTime, Utc};

use crate::{
    book::{BookId, FileFormat},
    token::{Token, TokenAlphabet, define_token_prefix},
    user::UserId,
};

define_token_prefix!(ImportJobTokenPrefix, "IJ_");
pub type ImportJobId = u64;
pub type ImportJobToken = Token<ImportJobTokenPrefix, ImportJobId, TokenAlphabet, { i64::MAX as u128 }>;

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

impl ImportStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Extracting => "extracting",
            Self::Identifying => "identifying",
            Self::NeedsReview => "needs_review",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Error => "error",
        }
    }
}

impl fmt::Display for ImportStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ImportStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "extracting" => Ok(Self::Extracting),
            "identifying" => Ok(Self::Identifying),
            "needs_review" => Ok(Self::NeedsReview),
            "approved" => Ok(Self::Approved),
            "rejected" => Ok(Self::Rejected),
            "error" => Ok(Self::Error),
            _ => Err(format!("unknown import status: {s}")),
        }
    }
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
    GoogleBooks,
    OpenLibrary,
}

impl ImportSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Embedded => "embedded",
            Self::Hardcover => "hardcover",
            Self::GoogleBooks => "google_books",
            Self::OpenLibrary => "open_library",
        }
    }
}

impl fmt::Display for ImportSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ImportSource {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "embedded" => Ok(Self::Embedded),
            "hardcover" => Ok(Self::Hardcover),
            "google_books" => Ok(Self::GoogleBooks),
            "open_library" => Ok(Self::OpenLibrary),
            _ => Err(format!("unknown import source: {s}")),
        }
    }
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

/// Job queue payload for processing a newly discovered import file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProcessImportPayload {
    pub import_job_id: ImportJobId,
}

impl crate::jobs::Enqueueable for ProcessImportPayload {
    const JOB_TYPE: &'static str = "process_import";
    const DEFAULT_PRIORITY: i16 = crate::jobs::PRIORITY_USER;
}
