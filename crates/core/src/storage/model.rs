use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::book::{AuthorRole, BookStatus, FileFormat, IdentifierType, MetadataSource};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarAuthor {
    pub name: String,
    pub role: AuthorRole,
    pub sort_order: i32,
    pub file_as: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarSeries {
    pub name: String,
    pub number: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarIdentifier {
    pub identifier_type: IdentifierType,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarFile {
    pub format: FileFormat,
    pub hash: String,
}

/// Typed representation of a `metadata.opf` sidecar file.
///
/// Standard Dublin Core fields are stored directly on this struct.
/// BookBoss-specific extensions (series, genres, tags, etc.) are serialised
/// into the single `bookboss:metadata` JSON meta element in the OPF output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookSidecar {
    pub title: String,
    pub authors: Vec<SidecarAuthor>,
    pub description: Option<String>,
    pub publisher: Option<String>,
    /// Publication year.
    pub published_date: Option<i32>,
    pub language: Option<String>,
    pub identifiers: Vec<SidecarIdentifier>,
    pub series: Option<SidecarSeries>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    pub rating: Option<i16>,
    pub status: BookStatus,
    pub metadata_source: Option<MetadataSource>,
    pub files: Vec<SidecarFile>,
}
