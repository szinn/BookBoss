mod hardcover;
mod open_library;

use std::sync::Arc;

use bb_core::pipeline::MetadataProvider;
pub use hardcover::HardcoverAdapter;
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
/// one that returns a result wins. Hardcover is preferred when configured
/// (token present); Open Library is always the final fallback.
pub fn create_metadata_providers(config: &MetadataConfig) -> Vec<Arc<dyn MetadataProvider>> {
    let mut providers: Vec<Arc<dyn MetadataProvider>> = vec![];
    if let Some(token) = &config.hardcover_api_token {
        providers.push(Arc::new(HardcoverAdapter::new(token.clone())));
    }
    providers.push(Arc::new(OpenLibraryAdapter::new()));
    providers
}
