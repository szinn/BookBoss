pub mod extractor;
pub mod model;
pub mod provider;

pub use extractor::MetadataExtractor;
pub use model::{ExtractedAuthor, ExtractedIdentifier, ExtractedMetadata, ProviderBook};
pub use provider::MetadataProvider;
