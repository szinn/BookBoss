use rust_decimal::Decimal;

use crate::{
    book::{AuthorRole, IdentifierType},
    import::ImportSource,
};

/// Edits submitted by the user during the import review step.
///
/// Carries all mutable book fields. The pipeline's `approve_job` method
/// commits these to the database and transitions the book to `Available`.
#[derive(Debug, Clone)]
pub struct BookEdit {
    pub title: String,
    pub description: Option<String>,
    pub published_date: Option<i32>,
    pub language: Option<String>,
    pub series_name: Option<String>,
    pub series_number: Option<Decimal>,
    pub publisher_name: Option<String>,
    pub page_count: Option<i32>,
    /// Primary authors in display order (comma-separated in UI, split before
    /// submission).
    pub authors: Vec<String>,
    /// Identifiers keyed by type; duplicates within the same type are ignored.
    pub identifiers: Vec<(IdentifierType, String)>,
    /// If `true`, the cover fetched by `fetch_from_provider` replaces the
    /// existing cover. The bytes are held in the server-side temp store keyed
    /// by the cover key passed to `fetch_from_provider`; no bytes are
    /// round-tripped through this struct.
    pub use_fetched_cover: bool,
}

#[derive(Debug, Clone)]
pub struct ExtractedAuthor {
    pub name: String,
    pub role: Option<AuthorRole>,
    pub sort_order: i32,
}

#[derive(Debug, Clone)]
pub struct ExtractedIdentifier {
    pub identifier_type: IdentifierType,
    pub value: String,
}

/// Metadata extracted directly from an e-book file's embedded headers or OPF.
///
/// All fields are `Option` — embedded metadata is frequently incomplete.
#[derive(Debug, Clone, Default)]
pub struct ExtractedMetadata {
    pub title: Option<String>,
    pub authors: Option<Vec<ExtractedAuthor>>,
    pub description: Option<String>,
    pub publisher: Option<String>,
    /// Publication year.
    pub published_date: Option<i32>,
    pub language: Option<String>,
    pub identifiers: Option<Vec<ExtractedIdentifier>>,
    pub series_name: Option<String>,
    pub series_number: Option<Decimal>,
    /// Cover image bytes extracted directly from the e-book file, if present.
    pub cover_bytes: Option<Vec<u8>>,
}

/// Enriched metadata returned by an external metadata provider.
///
/// Wraps [`ExtractedMetadata`] and adds cover art bytes fetched by the
/// provider. `source` identifies which provider produced the result so the
/// pipeline can record provenance without needing a separate construction-time
/// hint. The pipeline never makes HTTP calls directly.
#[derive(Debug, Clone)]
pub struct ProviderBook {
    pub metadata: ExtractedMetadata,
    pub cover_bytes: Option<Vec<u8>>,
    pub source: ImportSource,
}
