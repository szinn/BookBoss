pub mod app_setting;
pub mod auth;
pub mod book;
pub mod collection;
pub mod device;
pub mod error;
pub mod event;
pub mod filter;
pub mod format;
pub mod health;
pub mod import;
pub mod jobs;
pub mod koreader;
pub mod library;
pub mod message;
pub mod metadata;
pub mod opds;
pub mod pipeline;
pub mod reading;
pub mod repository;
pub mod resilience;
pub mod shelf;
pub mod storage;
pub mod token;
pub mod types;
pub mod user;

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use derive_builder::Builder;
pub use error::{Error, ErrorKind, RepositoryError};
pub use resilience::{CheckResult, CheckedSubsystem, ResilienceWrapper};
use tokio_graceful_shutdown::{
    ErrorAction, IntoSubsystem, SubsystemBuilder, SubsystemHandle,
    errors::{SubsystemError, SubsystemJoinError},
};

use crate::{
    app_setting::{AppSettingService, AppSettingServiceImpl},
    auth::{AuthService, AuthServiceImpl},
    book::{BookService, BookServiceImpl},
    collection::{CollectionService, CollectionServiceImpl},
    device::{DeviceService, service::DeviceServiceImpl},
    event::{EventService, create_event_service},
    format::FormatService,
    health::{HealthCheckSubsystem, HealthService, create_health_subsystem},
    import::{BookdropScanSubsystem, ImportJobService, create_bookdrop_scan_subsystem},
    jobs::{JobService, JobWorker, create_job_service},
    koreader::{KoReaderService, KoReaderServiceImpl},
    library::{LibraryService, LibraryServiceImpl},
    message::{SystemMessageService, SystemMessageServiceImpl},
    metadata::{MetadataService, create_metadata_service},
    opds::{OpdsService, OpdsServiceImpl},
    pipeline::{PipelineService, PipelineServiceImpl},
    reading::{ReadingService, ReadingServiceImpl},
    repository::RepositoryService,
    shelf::{ShelfService, service::ShelfServiceImpl},
    storage::FileStoreService,
    user::{UserService, UserServiceImpl, UserSettingService, UserSettingServiceImpl},
};

#[cfg(feature = "test-support")]
pub mod test_support;

/// All externally-provided adapter implementations required by `CoreServices`.
///
/// Use `ExternalServicesBuilder` to construct — all fields are required and
/// `.build()` returns an error if any are missing.
#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct ExternalServices {
    pub(crate) repository_service: Arc<RepositoryService>,
    pub(crate) file_store: Arc<dyn FileStoreService>,
    pub(crate) format_service: Arc<dyn FormatService>,
    /// Path to the bookdrop directory for automatic import scanning.
    pub(crate) bookdrop_path: PathBuf,
    /// Polling interval for the bookdrop scanner. Defaults to 1 minute.
    #[builder(default = "Duration::from_mins(1)")]
    pub(crate) scan_interval: Duration,
}

pub struct CoreServices {
    pub(crate) repository_service: Arc<RepositoryService>,
    pub auth_service: Arc<dyn AuthService>,
    pub user_service: Arc<dyn UserService>,
    pub user_setting_service: Arc<dyn UserSettingService>,
    pub book_service: Arc<dyn BookService>,
    pub import_job_service: Arc<dyn ImportJobService>,
    pub file_store: Arc<dyn FileStoreService>,
    pub format_service: Arc<dyn FormatService>,
    pub metadata_service: Arc<dyn MetadataService>,
    pub collection_service: Arc<dyn CollectionService>,
    pub library_service: Arc<dyn LibraryService>,
    pub pipeline_service: Arc<dyn PipelineService>,
    pub job_service: Arc<dyn JobService>,
    pub health_service: Arc<dyn HealthService>,
    pub shelf_service: Arc<dyn ShelfService>,
    pub reading_service: Arc<dyn ReadingService>,
    pub device_service: Arc<dyn DeviceService>,
    pub opds_service: Arc<dyn OpdsService>,
    pub event_service: Arc<dyn EventService>,
    pub system_message_service: Arc<dyn SystemMessageService>,
    pub koreader_service: Arc<dyn KoReaderService>,
    pub app_setting_service: Arc<dyn AppSettingService>,
    /// Internal: holds the bookdrop scan subsystem until `CoreSubsystem` takes
    /// it.
    bookdrop_scan_subsystem: Mutex<Option<BookdropScanSubsystem>>,
    /// Internal: holds the health subsystem until `CoreSubsystem` takes it.
    health_subsystem: Mutex<Option<HealthCheckSubsystem>>,
}

impl CoreServices {
    pub(crate) fn new(external: ExternalServices, encryption_secret: &str) -> Self {
        let ExternalServices {
            repository_service,
            file_store,
            format_service,
            bookdrop_path,
            scan_interval,
        } = external;

        let event_service = create_event_service(64);
        let job_service = create_job_service(repository_service.clone());
        let metadata_service = create_metadata_service();
        let pipeline_service: Arc<dyn PipelineService> = Arc::new(PipelineServiceImpl::new(
            repository_service.clone(),
            file_store.clone(),
            format_service.clone(),
            metadata_service.clone(),
            event_service.clone(),
        ));
        let (health_service, health_subsystem) = create_health_subsystem(job_service.clone(), event_service.clone());
        let system_message_service: Arc<dyn SystemMessageService> = Arc::new(SystemMessageServiceImpl::new(repository_service.clone(), event_service.clone()));

        let (import_job_service, bookdrop_scan_subsystem) = {
            let (svc, subsystem) = create_bookdrop_scan_subsystem(
                repository_service.clone(),
                file_store.clone(),
                format_service.clone(),
                system_message_service.clone(),
                bookdrop_path,
                scan_interval,
            );
            (svc, Some(subsystem))
        };

        let user_setting_service: Arc<dyn UserSettingService> = Arc::new(UserSettingServiceImpl::new(repository_service.clone()));

        Self {
            repository_service: repository_service.clone(),
            auth_service: Arc::new(AuthServiceImpl::new(repository_service.clone())),
            user_service: Arc::new(UserServiceImpl::new(repository_service.clone())),
            user_setting_service: user_setting_service.clone(),
            book_service: Arc::new(BookServiceImpl::new(repository_service.clone(), job_service.clone())),
            import_job_service,
            library_service: Arc::new(LibraryServiceImpl::new(repository_service.clone(), user_setting_service)),
            collection_service: Arc::new(CollectionServiceImpl::new(
                repository_service.clone(),
                file_store.clone(),
                format_service.clone(),
                job_service.clone(),
                event_service.clone(),
            )),
            file_store,
            format_service,
            metadata_service,
            pipeline_service,
            job_service,
            health_service,
            shelf_service: Arc::new(ShelfServiceImpl::new(repository_service.clone())),
            reading_service: Arc::new(ReadingServiceImpl::new(repository_service.clone())),
            device_service: Arc::new(DeviceServiceImpl::new(repository_service.clone())),
            opds_service: Arc::new(OpdsServiceImpl::new(repository_service.clone(), encryption_secret)),
            system_message_service,
            event_service,
            koreader_service: Arc::new(KoReaderServiceImpl::new(repository_service.clone())),
            app_setting_service: Arc::new(AppSettingServiceImpl::new(repository_service.clone())),
            bookdrop_scan_subsystem: Mutex::new(bookdrop_scan_subsystem),
            health_subsystem: Mutex::new(Some(health_subsystem)),
        }
    }

    /// Takes the bookdrop scan subsystem, if configured. Can only be called
    /// once (returns `None` on subsequent calls). Used by `CoreSubsystem`.
    fn take_bookdrop_scan_subsystem(&self) -> Option<BookdropScanSubsystem> {
        self.bookdrop_scan_subsystem.lock().expect("bookdrop_scan_subsystem mutex poisoned").take()
    }

    /// Takes the health subsystem. Can only be called once. Used by
    /// `CoreSubsystem` to start the `HealthCheckSubsystem`.
    fn take_health_subsystem(&self) -> Option<HealthCheckSubsystem> {
        self.health_subsystem.lock().expect("health_subsystem mutex poisoned").take()
    }
}

pub fn create_services(external: ExternalServices, encryption_secret: &str) -> Result<Arc<CoreServices>, Error> {
    Ok(Arc::new(CoreServices::new(external, encryption_secret)))
}

/// Register core job handlers and health tasks.
///
/// Called once after `CoreServices` is built — before the subsystem event loop
/// starts. Each crate that owns handlers exposes a similar function.
pub fn before_start(core: &Arc<CoreServices>) {
    use format::{handler::EnrichBookFilesHandler, mobi_handler::ConvertMobiHandler};
    use health::{
        HealthTaskConfig,
        handlers::{
            backfill_thumbnails, cleanup_expired_sessions, cleanup_old_import_jobs, cleanup_old_jobs, cleanup_old_system_messages, cleanup_orphan_authors,
            cleanup_orphan_publishers, cleanup_orphan_series, ensure_enrichments, reconcile_fingerprints, recover_enrichments, reset_stale_import_jobs,
            verify_file_integrity,
        },
    };
    use import::handler::ProcessImportHandler;
    use jobs::{ErasedJobHandler, JobServiceExt, PRIORITY_HEALTH, PRIORITY_SWEEP};

    let js = &core.job_service;
    let hs = &core.health_service;

    // Import pipeline handler
    js.register(ProcessImportHandler::new(core.clone()));

    // Format enrichment handler
    js.register(EnrichBookFilesHandler::new(core.clone()));

    // MOBI conversion handler
    js.register(ConvertMobiHandler::new(core.clone()));

    // Health check handlers + their scheduled tasks
    let handler = recover_enrichments::RecoverEnrichmentsHandler::new(core.clone());
    hs.register_task(HealthTaskConfig {
        name: handler.display_name().into(),
        job_type: handler.job_type().into(),
        run_on_startup: true,
        interval_minutes: Some(60),
        priority: PRIORITY_HEALTH,
    });
    js.register(handler);

    let handler = ensure_enrichments::EnsureEnrichmentsHandler::new(core.clone());
    hs.register_task(HealthTaskConfig {
        name: handler.display_name().into(),
        job_type: handler.job_type().into(),
        run_on_startup: true,
        interval_minutes: Some(120),
        priority: PRIORITY_SWEEP,
    });
    js.register(handler);

    let handler = reconcile_fingerprints::ReconcileFingerprintsHandler::new(core.clone());
    hs.register_task(HealthTaskConfig {
        name: handler.display_name().into(),
        job_type: handler.job_type().into(),
        run_on_startup: false,
        interval_minutes: None,
        priority: PRIORITY_SWEEP,
    });
    js.register(handler);

    let handler = cleanup_orphan_authors::CleanupOrphanAuthorsHandler::new(core.clone());
    hs.register_task(HealthTaskConfig {
        name: handler.display_name().into(),
        job_type: handler.job_type().into(),
        run_on_startup: false,
        interval_minutes: Some(360),
        priority: PRIORITY_HEALTH,
    });
    js.register(handler);

    let handler = cleanup_orphan_series::CleanupOrphanSeriesHandler::new(core.clone());
    hs.register_task(HealthTaskConfig {
        name: handler.display_name().into(),
        job_type: handler.job_type().into(),
        run_on_startup: false,
        interval_minutes: Some(360),
        priority: PRIORITY_HEALTH,
    });
    js.register(handler);

    let handler = cleanup_orphan_publishers::CleanupOrphanPublishersHandler::new(core.clone());
    hs.register_task(HealthTaskConfig {
        name: handler.display_name().into(),
        job_type: handler.job_type().into(),
        run_on_startup: false,
        interval_minutes: Some(360),
        priority: PRIORITY_HEALTH,
    });
    js.register(handler);

    let handler = cleanup_old_jobs::CleanupOldJobsHandler::new(core.clone());
    hs.register_task(HealthTaskConfig {
        name: handler.display_name().into(),
        job_type: handler.job_type().into(),
        run_on_startup: false,
        interval_minutes: Some(1440),
        priority: PRIORITY_HEALTH,
    });
    js.register(handler);

    let handler = cleanup_old_import_jobs::CleanupOldImportJobsHandler::new(core.clone());
    hs.register_task(HealthTaskConfig {
        name: handler.display_name().into(),
        job_type: handler.job_type().into(),
        run_on_startup: false,
        interval_minutes: Some(1440),
        priority: PRIORITY_HEALTH,
    });
    js.register(handler);

    let handler = cleanup_old_system_messages::CleanupOldSystemMessagesHandler::new(core.clone());
    hs.register_task(HealthTaskConfig {
        name: handler.display_name().into(),
        job_type: handler.job_type().into(),
        run_on_startup: false,
        interval_minutes: Some(1440),
        priority: PRIORITY_HEALTH,
    });
    js.register(handler);

    let handler = cleanup_expired_sessions::CleanupExpiredSessionsHandler::new(core.clone());
    hs.register_task(HealthTaskConfig {
        name: handler.display_name().into(),
        job_type: handler.job_type().into(),
        run_on_startup: false,
        interval_minutes: Some(1440),
        priority: PRIORITY_HEALTH,
    });
    js.register(handler);

    let handler = verify_file_integrity::VerifyFileIntegrityHandler::new(core.clone());
    hs.register_task(HealthTaskConfig {
        name: handler.display_name().into(),
        job_type: handler.job_type().into(),
        run_on_startup: false,
        interval_minutes: Some(720),
        priority: PRIORITY_HEALTH,
    });
    js.register(handler);

    let handler = reset_stale_import_jobs::ResetStaleImportJobsHandler::new(core.clone());
    hs.register_task(HealthTaskConfig {
        name: handler.display_name().into(),
        job_type: handler.job_type().into(),
        run_on_startup: true,
        interval_minutes: Some(360),
        priority: PRIORITY_HEALTH,
    });
    js.register(handler);

    let handler = backfill_thumbnails::BackfillThumbnailsHandler::new(core.clone());
    hs.register_task(HealthTaskConfig {
        name: handler.display_name().into(),
        job_type: handler.job_type().into(),
        run_on_startup: true,
        interval_minutes: None,
        priority: PRIORITY_SWEEP,
    });
    js.register(handler);
}

#[derive(Clone)]
pub struct CoreSubsystem {
    core_services: Arc<CoreServices>,
    poll_interval: Duration,
}

#[async_trait::async_trait]
impl CheckedSubsystem for CoreSubsystem {
    async fn check(&self) -> CheckResult {
        match self.core_services.repository_service.repository().ping().await {
            Ok(()) => CheckResult::Ok,
            Err(e) => CheckResult::Transient(e.to_string()),
        }
    }

    async fn run(self, subsys: &mut SubsystemHandle) -> Result<(), Error> {
        // ── Health and bookdrop: start once with CatchAndLocalShutdown ──────
        // These subsystems handle transient errors internally and rarely exit.
        // CatchAndLocalShutdown prevents their failure from killing the server.
        if let Some(health_subsystem) = self.core_services.take_health_subsystem() {
            subsys.start(
                SubsystemBuilder::new("Health", health_subsystem.into_subsystem())
                    .on_failure(ErrorAction::CatchAndLocalShutdown)
                    .on_panic(ErrorAction::CatchAndLocalShutdown),
            );
        }
        if let Some(scan_subsystem) = self.core_services.take_bookdrop_scan_subsystem() {
            subsys.start(
                SubsystemBuilder::new("BookdropScan", scan_subsystem.into_subsystem())
                    .on_failure(ErrorAction::CatchAndLocalShutdown)
                    .on_panic(ErrorAction::CatchAndLocalShutdown),
            );
        }

        // ── Worker keepalive loop ────────────────────────────────────────────
        // JobWorker is recreated on each iteration so it can restart cleanly.
        let core = &self.core_services;
        loop {
            let worker = JobWorker::new(
                core.job_service.clone(),
                core.repository_service.repository().clone(),
                core.repository_service.job_repository().clone(),
                self.poll_interval,
                core.event_service.clone(),
            );
            let handle = subsys.start(
                SubsystemBuilder::new("Worker", worker.into_subsystem())
                    .on_failure(ErrorAction::CatchAndLocalShutdown)
                    .on_panic(ErrorAction::CatchAndLocalShutdown),
            );

            tokio::select! {
                () = subsys.on_shutdown_requested() => {
                    tracing::info!("CoreSubsystem shutting down...");
                    break;
                }
                result = handle.join() => match result {
                    Ok(()) => {
                        tracing::info!("Worker exited cleanly");
                        break;
                    }
                    Err(join_err) => {
                        let inner = first_subsystem_error(join_err);
                        if inner.is_transient() {
                            tracing::warn!("Worker exited with transient error, restarting: {inner}");
                            // Loop — recreate and restart the worker.
                        } else {
                            tracing::error!("Worker exited with permanent error: {inner}");
                            return Err(inner);
                        }
                    }
                }
            }
        }

        tracing::info!("CoreSubsystem stopped");
        Ok(())
    }
}

/// Extracts the first inner error from a `SubsystemJoinError`.
///
/// Subsystem errors are boxed as `Box<dyn Error + Send + Sync + 'static>` by
/// the tokio-graceful-shutdown infrastructure. We attempt to downcast back to
/// the concrete `Error` type; if that fails (e.g. a panic message), we wrap it
/// as `Error::Infrastructure`.
fn first_subsystem_error(join_err: SubsystemJoinError<Box<dyn std::error::Error + Send + Sync + 'static>>) -> Error {
    let SubsystemJoinError::SubsystemsFailed(errors) = join_err;
    errors
        .iter()
        .find_map(|e| match e {
            SubsystemError::Failed(_, failure) => {
                // Deref chain: SubsystemFailure → Box<dyn Error> → dyn Error + Send + Sync +
                // 'static
                (***failure)
                    .downcast_ref::<Error>()
                    .cloned()
                    .or_else(|| Some(Error::Infrastructure(failure.to_string())))
            }
            SubsystemError::Panicked(name) => Some(Error::Infrastructure(format!("Worker panicked: {name}"))),
        })
        .unwrap_or_else(|| Error::Infrastructure("Worker failed with unknown error".into()))
}

#[must_use]
pub fn create_core_subsystem(core_services: Arc<CoreServices>, poll_interval: Duration) -> CoreSubsystem {
    CoreSubsystem { core_services, poll_interval }
}
