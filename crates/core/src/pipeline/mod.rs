pub mod extractor;
pub mod model;
pub mod provider;
pub mod service;

pub use extractor::MetadataExtractor;
pub use model::{ExtractedAuthor, ExtractedIdentifier, ExtractedMetadata, ProviderBook};
pub use provider::MetadataProvider;
pub use service::{PipelineService, PipelineServiceImpl};
