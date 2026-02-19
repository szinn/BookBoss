pub mod error;
pub mod repository;

use std::sync::Arc;

pub use error::{Error, ErrorKind, RepositoryError};

use crate::repository::RepositoryService;

#[cfg(feature = "test-support")]
pub mod test_support;

pub struct CoreServices {}

impl CoreServices {
    #[tracing::instrument(level = "trace", skip(_repository_service))]
    pub(crate) fn new(_repository_service: Arc<RepositoryService>) -> Self {
        Self {}
    }
}

pub fn create_services(repository_service: Arc<RepositoryService>) -> Result<Arc<CoreServices>, Error> {
    let core_services = CoreServices::new(repository_service);

    Ok(Arc::new(core_services))
}
