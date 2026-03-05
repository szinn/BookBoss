use async_trait::async_trait;

use crate::{
    Error,
    pipeline::{ExtractedMetadata, ProviderBook},
};

/// Port trait for enriching extracted metadata via an external provider.
///
/// Implemented by `crates/metadata/`. Returns `None` when there is
/// insufficient data to query (e.g. no ISBN available), allowing the
/// pipeline to proceed with embedded metadata only.
///
/// `name()` returns a human-readable label (e.g. `"Open Library"`) used
/// by the UI to identify the provider in the metadata editor.
#[async_trait]
pub trait MetadataProvider: Send + Sync {
    fn name(&self) -> &'static str;
    async fn enrich(&self, extracted: &ExtractedMetadata) -> Result<Option<ProviderBook>, Error>;
}
