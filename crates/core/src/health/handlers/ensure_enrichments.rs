use std::sync::Arc;

use crate::{
    CoreServices, Error,
    book::BookId,
    format::{handler::EnrichBookFilesPayload, mobi_handler::ConvertMobiPayload},
    jobs::{BookIdSweep, BookSweepPayload, JobHandler, JobRepositoryExt, run_book_id_sweep},
    repository::{read_only_transaction, transaction},
};

pub struct EnsureEnrichmentsHandler {
    core: Arc<CoreServices>,
}

impl EnsureEnrichmentsHandler {
    #[must_use]
    pub fn new(core: Arc<CoreServices>) -> Self {
        Self { core }
    }
}

#[async_trait::async_trait]
impl BookIdSweep for EnsureEnrichmentsHandler {
    fn job_type(&self) -> &'static str {
        Self::JOB_TYPE
    }

    fn continuation_delay(&self) -> chrono::Duration {
        chrono::Duration::zero()
    }

    async fn fetch_batch(&self, core: &CoreServices, after_id: Option<BookId>, batch_size: u64) -> Result<Vec<BookId>, Error> {
        let book_repo = core.repository_service.book_repository().clone();
        read_only_transaction(&**core.repository_service.repository(), |tx| {
            Box::pin(async move { book_repo.find_book_ids_needing_any_enrichment(tx, after_id, batch_size).await })
        })
        .await
    }

    async fn process_batch(&self, core: &Arc<CoreServices>, ids: Vec<BookId>) -> Result<(), Error> {
        let count = ids.len();
        let job_repo = core.repository_service.job_repository().clone();
        transaction(&**core.repository_service.repository(), |tx| {
            let job_repo = job_repo.clone();
            let ids = ids.clone();
            Box::pin(async move {
                for book_id in ids {
                    job_repo.enqueue(tx, &EnrichBookFilesPayload { book_id }).await?;
                }
                Ok(())
            })
        })
        .await?;

        tracing::info!(count, "enqueued enrichment jobs (missing + stale)");

        // MOBI sweep: enqueue ConvertMobiPayload for books that have an
        // enriched EPUB but no enriched MOBI, when MOBI conversion is
        // enabled.
        if core.app_setting_service.mobi_enabled().await? {
            let batch_size = BookSweepPayload::default().batch_size;
            let book_repo = core.repository_service.book_repository().clone();
            let mobi_ids = read_only_transaction(&**core.repository_service.repository(), |tx| {
                Box::pin(async move { book_repo.find_book_ids_needing_mobi_conversion(tx, None, batch_size).await })
            })
            .await?;

            let mobi_count = mobi_ids.len();
            let job_repo = core.repository_service.job_repository().clone();
            transaction(&**core.repository_service.repository(), |tx| {
                let job_repo = job_repo.clone();
                let mobi_ids = mobi_ids.clone();
                Box::pin(async move {
                    for book_id in mobi_ids {
                        job_repo.enqueue(tx, &ConvertMobiPayload { book_id }).await?;
                    }
                    Ok(())
                })
            })
            .await?;

            tracing::info!(mobi_count, "enqueued MOBI conversion jobs");
        }

        Ok(())
    }
}

impl JobHandler for EnsureEnrichmentsHandler {
    const JOB_TYPE: &'static str = "health.ensure_enrichments";
    const DISPLAY_NAME: &'static str = "Ensure Enrichments";
    type Payload = BookSweepPayload;

    async fn handle(&self, payload: BookSweepPayload) -> Result<(), Error> {
        run_book_id_sweep(self, &self.core, payload).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_setting::repository::MockAppSettingRepository, book::repository::book::MockBookRepository, jobs::repository::MockJobRepository,
        repository::testing::default_repository_service_builder, test_support::*,
    };

    /// Returns a `MockAppSettingRepository` that reports MOBI disabled (returns
    /// `None`).
    fn mobi_disabled_app_setting_repo() -> MockAppSettingRepository {
        let mut repo = MockAppSettingRepository::new();
        repo.expect_get().returning(|_, _| Box::pin(std::future::ready(Ok(None))));
        repo
    }

    /// Returns a `MockAppSettingRepository` that reports MOBI enabled.
    fn mobi_enabled_app_setting_repo() -> MockAppSettingRepository {
        use crate::app_setting::AppSetting;
        let mut repo = MockAppSettingRepository::new();
        repo.expect_get().returning(|_, _| {
            let setting = AppSetting {
                key: "enrichment.mobi_enabled".into(),
                value: "true".into(),
            };
            Box::pin(std::future::ready(Ok(Some(setting))))
        });
        repo
    }

    fn fake_job() -> crate::jobs::Job {
        crate::jobs::Job {
            id: 1,
            job_type: String::new(),
            payload: serde_json::json!({}),
            status: crate::jobs::JobStatus::Pending,
            priority: 0,
            attempt: 0,
            max_attempts: 3,
            version: 0,
            scheduled_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            error_message: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn enqueues_jobs_for_books_needing_enrichment() {
        let mut book_repo = MockBookRepository::new();
        let mut job_repo = MockJobRepository::new();

        // Return two books needing enrichment; batch < batch_size so no
        // continuation.
        book_repo
            .expect_find_book_ids_needing_any_enrichment()
            .returning(|_, _, _| Box::pin(std::future::ready(Ok(vec![1, 5]))));

        // Expect exactly two enqueue calls (one per book), no delayed enqueue.
        // mobi_enabled = false, so no MOBI jobs enqueued.
        job_repo
            .expect_enqueue_raw()
            .times(2)
            .returning(|_, _, _, _| Box::pin(std::future::ready(Ok(fake_job()))));

        let repo_service = Arc::new(
            default_repository_service_builder()
                .app_setting_repository(Arc::new(mobi_disabled_app_setting_repo()))
                .book_repository(Arc::new(book_repo))
                .job_repository(Arc::new(job_repo))
                .build()
                .unwrap(),
        );

        let core = crate::create_services(
            default_external_services_builder().repository_service(repo_service).build().unwrap(),
            "test-secret",
        )
        .unwrap();

        let handler = EnsureEnrichmentsHandler::new(core);
        handler.handle(BookSweepPayload::default()).await.unwrap();
    }

    #[tokio::test]
    async fn noop_when_no_epub_enrichment_needed() {
        let mut book_repo = MockBookRepository::new();

        book_repo
            .expect_find_book_ids_needing_any_enrichment()
            .returning(|_, _, _| Box::pin(std::future::ready(Ok(vec![]))));

        let repo_service = Arc::new(
            default_repository_service_builder()
                .app_setting_repository(Arc::new(mobi_disabled_app_setting_repo()))
                .book_repository(Arc::new(book_repo))
                .build()
                .unwrap(),
        );

        let core = crate::create_services(
            default_external_services_builder().repository_service(repo_service).build().unwrap(),
            "test-secret",
        )
        .unwrap();

        let handler = EnsureEnrichmentsHandler::new(core);
        handler.handle(BookSweepPayload::default()).await.unwrap();
    }

    /// Verifies that
    /// `MockBookRepository::expect_find_book_ids_needing_mobi_conversion`
    /// compiles and behaves correctly — the real DB query is covered by
    /// integration tests.
    #[tokio::test]
    async fn mock_find_book_ids_needing_mobi_conversion_returns_expected_ids() {
        let mut book_repo = MockBookRepository::new();

        book_repo
            .expect_find_book_ids_needing_mobi_conversion()
            .returning(|_, _, _| Box::pin(std::future::ready(Ok(vec![3, 7]))));

        let repo_service = Arc::new(default_repository_service_builder().book_repository(Arc::new(book_repo)).build().unwrap());

        let core = crate::create_services(
            default_external_services_builder().repository_service(repo_service).build().unwrap(),
            "test-secret",
        )
        .unwrap();

        let book_repo = core.repository_service.book_repository().clone();
        let result = crate::repository::read_only_transaction(&**core.repository_service.repository(), |tx| {
            Box::pin(async move { book_repo.find_book_ids_needing_mobi_conversion(tx, None, 50).await })
        })
        .await
        .unwrap();

        assert_eq!(result, vec![3, 7]);
    }

    #[tokio::test]
    async fn full_batch_re_enqueues_continuation() {
        let mut book_repo = MockBookRepository::new();
        let mut job_repo = MockJobRepository::new();

        // Return exactly batch_size books so the sweep re-enqueues a
        // continuation.
        let batch_size = BookSweepPayload::default().batch_size;
        let ids: Vec<BookId> = (1..=batch_size).collect();
        book_repo
            .expect_find_book_ids_needing_any_enrichment()
            .returning(move |_, _, _| Box::pin(std::future::ready(Ok(ids.clone()))));

        // Expect one enqueue per book, plus one delayed enqueue for the
        // continuation. mobi_enabled = false, so no MOBI jobs enqueued.
        job_repo
            .expect_enqueue_raw()
            .returning(|_, _, _, _| Box::pin(std::future::ready(Ok(fake_job()))));
        job_repo
            .expect_enqueue_delayed()
            .once()
            .returning(|_, _, _, _, _| Box::pin(std::future::ready(Ok(fake_job()))));

        let repo_service = Arc::new(
            default_repository_service_builder()
                .app_setting_repository(Arc::new(mobi_disabled_app_setting_repo()))
                .book_repository(Arc::new(book_repo))
                .job_repository(Arc::new(job_repo))
                .build()
                .unwrap(),
        );

        let core = crate::create_services(
            default_external_services_builder().repository_service(repo_service).build().unwrap(),
            "test-secret",
        )
        .unwrap();

        let handler = EnsureEnrichmentsHandler::new(core);
        handler.handle(BookSweepPayload::default()).await.unwrap();
    }

    /// When mobi_enabled is true and books need MOBI conversion, enqueues
    /// `ConvertMobiPayload` jobs in addition to EPUB enrichment jobs.
    #[tokio::test]
    async fn mobi_enabled_enqueues_mobi_jobs() {
        let mut book_repo = MockBookRepository::new();
        let mut job_repo = MockJobRepository::new();

        // Two books need EPUB enrichment.
        book_repo
            .expect_find_book_ids_needing_any_enrichment()
            .returning(|_, _, _| Box::pin(std::future::ready(Ok(vec![1, 2]))));

        // One book needs MOBI conversion.
        book_repo
            .expect_find_book_ids_needing_mobi_conversion()
            .returning(|_, _, _| Box::pin(std::future::ready(Ok(vec![3]))));

        // 2 EPUB enqueue_raw + 1 MOBI enqueue_raw = 3 total.
        job_repo
            .expect_enqueue_raw()
            .times(3)
            .returning(|_, _, _, _| Box::pin(std::future::ready(Ok(fake_job()))));

        let repo_service = Arc::new(
            default_repository_service_builder()
                .app_setting_repository(Arc::new(mobi_enabled_app_setting_repo()))
                .book_repository(Arc::new(book_repo))
                .job_repository(Arc::new(job_repo))
                .build()
                .unwrap(),
        );

        let core = crate::create_services(
            default_external_services_builder().repository_service(repo_service).build().unwrap(),
            "test-secret",
        )
        .unwrap();

        let handler = EnsureEnrichmentsHandler::new(core);
        handler.handle(BookSweepPayload::default()).await.unwrap();
    }

    /// When mobi_enabled is false, no MOBI jobs are enqueued even if books
    /// would otherwise need conversion.
    #[tokio::test]
    async fn mobi_disabled_does_not_enqueue_mobi_jobs() {
        let mut book_repo = MockBookRepository::new();
        let mut job_repo = MockJobRepository::new();

        // Two books need EPUB enrichment.
        book_repo
            .expect_find_book_ids_needing_any_enrichment()
            .returning(|_, _, _| Box::pin(std::future::ready(Ok(vec![1, 2]))));

        // find_book_ids_needing_mobi_conversion must NOT be called.

        // Only 2 EPUB enqueue_raw calls; mobi_disabled so no MOBI enqueue.
        job_repo
            .expect_enqueue_raw()
            .times(2)
            .returning(|_, _, _, _| Box::pin(std::future::ready(Ok(fake_job()))));

        let repo_service = Arc::new(
            default_repository_service_builder()
                .app_setting_repository(Arc::new(mobi_disabled_app_setting_repo()))
                .book_repository(Arc::new(book_repo))
                .job_repository(Arc::new(job_repo))
                .build()
                .unwrap(),
        );

        let core = crate::create_services(
            default_external_services_builder().repository_service(repo_service).build().unwrap(),
            "test-secret",
        )
        .unwrap();

        let handler = EnsureEnrichmentsHandler::new(core);
        handler.handle(BookSweepPayload::default()).await.unwrap();
    }

    /// When mobi_enabled is true but `find_book_ids_needing_mobi_conversion`
    /// returns an empty list, only EPUB enrichment jobs are enqueued — no
    /// MOBI enqueue calls are made.
    #[tokio::test]
    async fn mobi_enabled_empty_mobi_result_enqueues_only_epub_jobs() {
        let mut book_repo = MockBookRepository::new();
        let mut job_repo = MockJobRepository::new();

        // Two books need EPUB enrichment.
        book_repo
            .expect_find_book_ids_needing_any_enrichment()
            .returning(|_, _, _| Box::pin(std::future::ready(Ok(vec![1, 2]))));

        // No books need MOBI conversion.
        book_repo
            .expect_find_book_ids_needing_mobi_conversion()
            .returning(|_, _, _| Box::pin(std::future::ready(Ok(vec![]))));

        // Only 2 EPUB enqueue_raw calls; MOBI list is empty so no MOBI enqueue.
        job_repo
            .expect_enqueue_raw()
            .times(2)
            .returning(|_, _, _, _| Box::pin(std::future::ready(Ok(fake_job()))));

        let repo_service = Arc::new(
            default_repository_service_builder()
                .app_setting_repository(Arc::new(mobi_enabled_app_setting_repo()))
                .book_repository(Arc::new(book_repo))
                .job_repository(Arc::new(job_repo))
                .build()
                .unwrap(),
        );

        let core = crate::create_services(
            default_external_services_builder().repository_service(repo_service).build().unwrap(),
            "test-secret",
        )
        .unwrap();

        let handler = EnsureEnrichmentsHandler::new(core);
        handler.handle(BookSweepPayload::default()).await.unwrap();
    }
}
