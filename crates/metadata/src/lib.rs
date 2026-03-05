mod open_library;

use std::sync::Arc;

use bb_core::pipeline::MetadataProvider;
pub use open_library::OpenLibraryAdapter;
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct MetadataConfig {
    pub hardcover_api_token: Option<String>,
}

/// Build the ordered list of configured metadata providers.
///
/// Providers are tried in order during the acquisition pipeline — the first
/// one that returns a result wins. Open Library is always included.
/// Hardcover is added only when an API token is configured.
pub fn create_metadata_providers(_config: &MetadataConfig) -> Vec<Arc<dyn MetadataProvider>> {
    // TODO: push HardcoverAdapter when config.hardcover_api_token is Some
    vec![Arc::new(OpenLibraryAdapter::new())]
}
