pub mod auth;
pub mod book;
pub mod device;
pub mod error;
pub mod import;
pub mod jobs;
pub mod library;
pub mod pipeline;
pub mod reading;
pub mod repository;
pub mod shelf;
pub mod storage;
pub mod types;
pub mod user;

use std::{sync::Arc, time::Duration};

pub use error::{Error, ErrorKind, RepositoryError};
use tokio_graceful_shutdown::{IntoSubsystem, SubsystemBuilder, SubsystemHandle};

use crate::{
    auth::{AuthService, AuthServiceImpl},
    book::{BookService, BookServiceImpl},
    import::{ImportJobService, service::ImportJobServiceImpl},
    jobs::{JobRegistry, JobWorker},
    library::{LibraryService, LibraryServiceImpl},
    pipeline::PipelineService,
    repository::RepositoryService,
    storage::LibraryStore,
    user::{UserService, UserServiceImpl, UserSettingService, UserSettingServiceImpl},
};

#[cfg(feature = "test-support")]
pub mod test_support;

pub struct CoreServices {
    pub auth_service: Arc<dyn AuthService>,
    pub user_service: Arc<dyn UserService>,
    pub user_setting_service: Arc<dyn UserSettingService>,
    pub book_service: Arc<dyn BookService>,
    pub import_job_service: Arc<dyn ImportJobService>,
    pub library_store: Arc<dyn LibraryStore>,
    pub library_service: Arc<dyn LibraryService>,
    pub pipeline_service: Arc<dyn PipelineService>,
}

impl CoreServices {
    pub(crate) fn new(repository_service: Arc<RepositoryService>, library_store: Arc<dyn LibraryStore>, pipeline_service: Arc<dyn PipelineService>) -> Self {
        Self {
            auth_service: Arc::new(AuthServiceImpl::new(repository_service.clone())),
            user_service: Arc::new(UserServiceImpl::new(repository_service.clone())),
            user_setting_service: Arc::new(UserSettingServiceImpl::new(repository_service.clone())),
            book_service: Arc::new(BookServiceImpl::new(repository_service.clone())),
            import_job_service: Arc::new(ImportJobServiceImpl::new(repository_service.clone())),
            library_service: Arc::new(LibraryServiceImpl::new(repository_service.clone(), library_store.clone())),
            library_store,
            pipeline_service,
        }
    }
}

pub fn create_services(
    repository_service: Arc<RepositoryService>,
    library_store: Arc<dyn LibraryStore>,
    pipeline_service: Arc<dyn PipelineService>,
) -> Result<Arc<CoreServices>, Error> {
    Ok(Arc::new(CoreServices::new(repository_service, library_store, pipeline_service)))
}

pub struct CoreSubsystem {
    registry: JobRegistry,
    repository_service: Arc<RepositoryService>,
    poll_interval: Duration,
}

impl IntoSubsystem<Error> for CoreSubsystem {
    async fn run(self, subsys: &mut SubsystemHandle) -> Result<(), Error> {
        let worker = JobWorker::new(
            self.registry,
            self.repository_service.repository().clone(),
            self.repository_service.job_repository().clone(),
            self.poll_interval,
        );
        subsys.start(SubsystemBuilder::new("Worker", worker.into_subsystem()));

        tracing::info!("CoreSubsystem started");

        subsys.on_shutdown_requested().await;
        tracing::info!("CoreSubsystem shutting down");

        Ok(())
    }
}

pub fn create_core_subsystem(registry: JobRegistry, repository_service: Arc<RepositoryService>, poll_interval: Duration) -> CoreSubsystem {
    CoreSubsystem {
        registry,
        repository_service,
        poll_interval,
    }
}
