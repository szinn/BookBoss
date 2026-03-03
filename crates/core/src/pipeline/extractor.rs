use std::path::Path;

use async_trait::async_trait;

use crate::{Error, book::FileFormat, pipeline::ExtractedMetadata};

/// Port trait for extracting metadata from an e-book file.
///
/// Implemented by `crates/formats/` (M3.7). For formats where extraction
/// is unsupported, implementations return an empty [`ExtractedMetadata`]
/// and let the provider enrichment step fill the gaps.
#[async_trait]
pub trait MetadataExtractor: Send + Sync {
    async fn extract(&self, path: &Path, format: FileFormat) -> Result<ExtractedMetadata, Error>;
}
