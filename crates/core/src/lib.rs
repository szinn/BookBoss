pub mod error;
pub mod repository;
pub mod types;
pub mod user;

use std::sync::Arc;

pub use error::{Error, ErrorKind, RepositoryError};

use crate::{
    repository::RepositoryService,
    user::{UserService, UserServiceImpl},
};

#[cfg(feature = "test-support")]
pub mod test_support;

pub struct CoreServices {
    pub user_service: Arc<dyn UserService>,
}

impl CoreServices {
    #[tracing::instrument(level = "trace", skip(repository_service))]
    pub(crate) fn new(repository_service: Arc<RepositoryService>) -> Self {
        Self {
            user_service: Arc::new(UserServiceImpl::new(repository_service.clone())),
        }
    }
}

pub fn create_services(repository_service: Arc<RepositoryService>) -> Result<Arc<CoreServices>, Error> {
    let core_services = CoreServices::new(repository_service);

    Ok(Arc::new(core_services))
}
