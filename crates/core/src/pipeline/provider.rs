use async_trait::async_trait;

use crate::{
    Error,
    pipeline::{ExtractedMetadata, ProviderBook},
};

/// Port trait for enriching extracted metadata via an external provider.
///
/// Implemented by `crates/metadata/` (M3.10). Returns `None` when there
/// is insufficient data to query (e.g. no ISBN available), allowing the
/// pipeline to proceed with embedded metadata only.
#[async_trait]
pub trait MetadataProvider: Send + Sync {
    async fn enrich(&self, extracted: &ExtractedMetadata) -> Result<Option<ProviderBook>, Error>;
}
