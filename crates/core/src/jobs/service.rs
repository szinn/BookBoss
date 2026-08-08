use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use serde::Serialize;

use crate::{
    Error,
    jobs::{Enqueueable, handler::ErasedJobHandler},
    repository::{RepositoryService, read_only_transaction, transaction},
};

/// Service port for enqueuing, counting, and dispatching background jobs.
///
/// Abstracts transaction management away from adapter crates — callers receive
/// an `Arc<dyn JobService>` and use [`JobServiceExt::enqueue`] without needing
/// to manage their own `Repository` or `Transaction` references.
///
/// Also serves as the handler registry — handlers are registered via
/// [`JobServiceExt::register`] and dispatched via [`JobService::dispatch`].
#[async_trait::async_trait]
#[cfg_attr(any(test, feature = "test-support"), mockall::automock)]
#[allow(unused_lifetimes, reason = "async_trait + mockall expansion emits a spurious 'life0 parameter")]
pub trait JobService: Send + Sync {
    /// Enqueue a raw job by type string and pre-serialised JSON payload.
    ///
    /// Prefer [`JobServiceExt::enqueue`] for typed payloads.
    async fn enqueue_raw(&self, job_type: &str, payload: serde_json::Value, priority: i16) -> Result<(), Error>;

    /// Enqueue a raw job that won't be picked up until `now + delay`.
    ///
    /// Prefer [`JobServiceExt::enqueue_after`] for typed payloads.
    async fn enqueue_raw_delayed(&self, job_type: &str, payload: serde_json::Value, priority: i16, delay: chrono::Duration) -> Result<(), Error>;

    /// Count jobs of the given type that are currently pending or running.
    async fn count_pending_by_type(&self, job_type: &str) -> Result<u64, Error>;

    /// Count all jobs that are currently pending or running, regardless of
    /// type.
    async fn count_all_pending(&self) -> Result<u64, Error>;

    /// Register a type-erased job handler for the given job type.
    ///
    /// Prefer [`JobServiceExt::register`] for typed registration.
    fn register_handler(&self, job_type: String, handler: Arc<dyn ErasedJobHandler>);

    /// Look up the handler for `job_type` and invoke it with `payload`.
    ///
    /// Returns an error if no handler is registered for the given type, or if
    /// the handler itself fails.
    async fn dispatch(&self, job_type: &str, payload: serde_json::Value) -> Result<(), Error>;
}

/// Extension methods on [`JobService`] for typed enqueueing and registration.
///
/// Blanket-implemented for all `JobService` impls — no manual work per job
/// type. Mirrors the [`JobRepositoryExt`] pattern but at the service layer.
pub trait JobServiceExt: JobService {
    fn enqueue<P: Enqueueable + Serialize + Send + Sync>(&self, payload: &P) -> impl std::future::Future<Output = Result<(), Error>> + Send {
        let value = serde_json::to_value(payload);
        async move {
            let value = value.map_err(|e| Error::Infrastructure(format!("failed to serialize job payload: {e}")))?;
            self.enqueue_raw(P::JOB_TYPE, value, P::DEFAULT_PRIORITY).await
        }
    }

    /// Enqueue a typed job that won't run until `now + delay`.
    fn enqueue_after<P: Enqueueable + Serialize + Send + Sync>(
        &self,
        payload: &P,
        delay: chrono::Duration,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send {
        let value = serde_json::to_value(payload);
        async move {
            let value = value.map_err(|e| Error::Infrastructure(format!("failed to serialize job payload: {e}")))?;
            self.enqueue_raw_delayed(P::JOB_TYPE, value, P::DEFAULT_PRIORITY, delay).await
        }
    }

    /// Register a typed [`JobHandler`](super::JobHandler).
    fn register<H: super::JobHandler>(&self, handler: H) {
        self.register_handler(H::JOB_TYPE.to_string(), Arc::new(handler));
    }
}

impl<S: JobService + ?Sized> JobServiceExt for S {}

pub(crate) struct JobServiceImpl {
    repository_service: Arc<RepositoryService>,
    handlers: RwLock<HashMap<String, Arc<dyn ErasedJobHandler>>>,
}

impl JobServiceImpl {
    pub(crate) fn new(repository_service: Arc<RepositoryService>) -> Self {
        Self {
            repository_service,
            handlers: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl JobService for JobServiceImpl {
    async fn enqueue_raw(&self, job_type: &str, payload: serde_json::Value, priority: i16) -> Result<(), Error> {
        let job_type = job_type.to_owned();
        let job_repo = self.repository_service.job_repository().clone();
        transaction(&**self.repository_service.repository(), |tx| {
            let job_repo = job_repo.clone();
            let job_type = job_type.clone();
            let payload = payload.clone();
            Box::pin(async move {
                job_repo.enqueue_raw(tx, &job_type, payload, priority).await?;
                Ok(())
            })
        })
        .await
    }

    async fn enqueue_raw_delayed(&self, job_type: &str, payload: serde_json::Value, priority: i16, delay: chrono::Duration) -> Result<(), Error> {
        let job_type = job_type.to_owned();
        let job_repo = self.repository_service.job_repository().clone();
        transaction(&**self.repository_service.repository(), |tx| {
            let job_repo = job_repo.clone();
            let job_type = job_type.clone();
            let payload = payload.clone();
            Box::pin(async move {
                job_repo.enqueue_delayed(tx, &job_type, payload, priority, delay).await?;
                Ok(())
            })
        })
        .await
    }

    async fn count_pending_by_type(&self, job_type: &str) -> Result<u64, Error> {
        let job_type = job_type.to_owned();
        let job_repo = self.repository_service.job_repository().clone();
        read_only_transaction(&**self.repository_service.repository(), |tx| {
            let job_repo = job_repo.clone();
            let job_type = job_type.clone();
            Box::pin(async move { job_repo.count_pending_by_type(tx, &job_type).await })
        })
        .await
    }

    async fn count_all_pending(&self) -> Result<u64, Error> {
        let job_repo = self.repository_service.job_repository().clone();
        read_only_transaction(&**self.repository_service.repository(), |tx| {
            let job_repo = job_repo.clone();
            Box::pin(async move { job_repo.count_all_pending(tx).await })
        })
        .await
    }

    fn register_handler(&self, job_type: String, handler: Arc<dyn ErasedJobHandler>) {
        self.handlers.write().expect("handler lock poisoned").insert(job_type, handler);
    }

    async fn dispatch(&self, job_type: &str, payload: serde_json::Value) -> Result<(), Error> {
        let handler = {
            let handlers = self.handlers.read().expect("handler lock poisoned");
            handlers
                .get(job_type)
                .ok_or_else(|| Error::Infrastructure(format!("no handler registered for job type '{job_type}'")))?
                .clone()
        };
        handler.handle(payload).await
    }
}

/// Creates a `JobService` backed by the given `RepositoryService`.
///
/// Called from the application wiring layer (e.g. `bookboss`) before
/// `CoreServices` is built, so that adapters like `ConversionServiceImpl` can
/// receive the service without circular dependency on `CoreServices`.
#[must_use]
pub fn create_job_service(repository_service: Arc<RepositoryService>) -> Arc<dyn JobService> {
    Arc::new(JobServiceImpl::new(repository_service))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::repository::MockJobRepository;

    fn create_service(mock: MockJobRepository) -> JobServiceImpl {
        let repository_service = Arc::new(
            crate::repository::testing::default_repository_service_builder()
                .job_repository(Arc::new(mock))
                .build()
                .expect("all fields provided"),
        );
        JobServiceImpl::new(repository_service)
    }

    #[tokio::test]
    async fn test_count_pending_by_type_delegates_to_repo() {
        let mut mock = MockJobRepository::new();
        mock.expect_count_pending_by_type().returning(|_, _| Box::pin(async { Ok(7) }));
        let svc = create_service(mock);

        let result = svc.count_pending_by_type("enrich_epub").await;

        assert_eq!(result.unwrap(), 7);
    }

    #[tokio::test]
    async fn test_count_pending_propagates_error() {
        let mut mock = MockJobRepository::new();
        mock.expect_count_pending_by_type()
            .returning(|_, _| Box::pin(async { Err(Error::RepositoryError(crate::RepositoryError::Database("db".into()))) }));
        let svc = create_service(mock);

        let result = svc.count_pending_by_type("enrich_epub").await;

        assert!(matches!(result, Err(Error::RepositoryError(_))));
    }

    #[tokio::test]
    async fn test_count_all_pending_delegates_to_repo() {
        let mut mock = MockJobRepository::new();
        mock.expect_count_all_pending().returning(|_| Box::pin(async { Ok(12) }));
        let svc = create_service(mock);

        let result = svc.count_all_pending().await;

        assert_eq!(result.unwrap(), 12);
    }

    #[tokio::test]
    async fn register_and_dispatch_handler() {
        use crate::jobs::JobHandler;

        struct TestHandler;
        impl JobHandler for TestHandler {
            const JOB_TYPE: &'static str = "test.job";
            const DISPLAY_NAME: &'static str = "Test Job";
            type Payload = serde_json::Value;
            fn handle(&self, _payload: serde_json::Value) -> impl std::future::Future<Output = Result<(), Error>> {
                std::future::ready(Ok(()))
            }
        }

        let mock = MockJobRepository::new();
        let svc = create_service(mock);
        svc.register_handler("test.job".to_string(), Arc::new(TestHandler));

        svc.dispatch("test.job", serde_json::json!({})).await.unwrap();
    }

    #[tokio::test]
    async fn dispatch_unknown_type_returns_error() {
        let mock = MockJobRepository::new();
        let svc = create_service(mock);

        let result = svc.dispatch("unknown.job", serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn enqueue_raw_delayed_delegates_to_repo() {
        use chrono::Duration;

        use crate::jobs::{PRIORITY_NORMAL, repository::MockJobRepository};

        let mut mock = MockJobRepository::new();
        mock.expect_enqueue_delayed().once().returning(|_, job_type, _, priority, delay| {
            assert_eq!(job_type, "test.job");
            assert_eq!(priority, PRIORITY_NORMAL);
            assert_eq!(delay, Duration::minutes(5));
            Box::pin(std::future::ready(Ok(crate::jobs::Job {
                id: 1,
                job_type: job_type.to_owned(),
                payload: serde_json::json!({}),
                status: crate::jobs::JobStatus::Pending,
                priority,
                attempt: 0,
                max_attempts: 3,
                version: 0,
                scheduled_at: chrono::Utc::now() + delay,
                started_at: None,
                completed_at: None,
                error_message: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })))
        });

        let svc = create_service(mock);
        svc.enqueue_raw_delayed("test.job", serde_json::json!({}), PRIORITY_NORMAL, Duration::minutes(5))
            .await
            .unwrap();
    }
}
