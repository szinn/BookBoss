pub mod auth;
pub mod book;
pub mod device;
pub mod error;
pub mod import;
pub mod reading;
pub mod repository;
pub mod shelf;
pub mod types;
pub mod user;

use std::sync::Arc;

pub use error::{Error, ErrorKind, RepositoryError};

use crate::{
    auth::{AuthService, AuthServiceImpl},
    repository::RepositoryService,
    user::{UserService, UserServiceImpl, UserSettingService, UserSettingServiceImpl},
};

#[cfg(feature = "test-support")]
pub mod test_support;

pub struct CoreServices {
    pub auth_service: Arc<dyn AuthService>,
    pub user_service: Arc<dyn UserService>,
    pub user_setting_service: Arc<dyn UserSettingService>,
}

impl CoreServices {
    #[tracing::instrument(level = "trace", skip(repository_service))]
    pub(crate) fn new(repository_service: Arc<RepositoryService>) -> Self {
        Self {
            auth_service: Arc::new(AuthServiceImpl::new(repository_service.clone())),
            user_service: Arc::new(UserServiceImpl::new(repository_service.clone())),
            user_setting_service: Arc::new(UserSettingServiceImpl::new(repository_service.clone())),
        }
    }
}

pub fn create_services(repository_service: Arc<RepositoryService>) -> Result<Arc<CoreServices>, Error> {
    let core_services = CoreServices::new(repository_service);

    Ok(Arc::new(core_services))
}
