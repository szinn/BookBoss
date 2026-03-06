pub mod handler;
pub mod scanner;

use std::{path::PathBuf, sync::Arc, time::Duration};

use bb_core::{Error, repository::RepositoryService};
pub use handler::{ProcessImportHandler, ProcessImportPayload};
pub use scanner::LibraryScanner;
use tokio_graceful_shutdown::{IntoSubsystem, SubsystemBuilder, SubsystemHandle};

pub struct ImportSubsystem {
    watch_directory: PathBuf,
    poll_interval: Duration,
    repository_service: Arc<RepositoryService>,
}

impl IntoSubsystem<Error> for ImportSubsystem {
    async fn run(self, subsys: &mut SubsystemHandle) -> Result<(), Error> {
        let scanner = LibraryScanner::new(
            self.watch_directory,
            self.poll_interval,
            self.repository_service.repository().clone(),
            self.repository_service.import_job_repository().clone(),
            self.repository_service.job_repository().clone(),
        );
        subsys.start(SubsystemBuilder::new("Scanner", scanner.into_subsystem()));
        subsys.on_shutdown_requested().await;
        Ok(())
    }
}

pub fn create_import_subsystem(watch_directory: PathBuf, poll_interval: Duration, repository_service: Arc<RepositoryService>) -> ImportSubsystem {
    ImportSubsystem {
        watch_directory,
        poll_interval,
        repository_service,
    }
}
