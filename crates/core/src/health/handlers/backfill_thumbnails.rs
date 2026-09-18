use std::sync::Arc;

use crate::{
    CoreServices, Error,
    book::{BookId, BookToken},
    jobs::{BookIdSweep, BookSweepPayload, JobHandler, run_book_id_sweep},
    repository::read_only_transaction,
};

pub struct BackfillThumbnailsHandler {
    core: Arc<CoreServices>,
}

impl BackfillThumbnailsHandler {
    #[must_use]
    pub fn new(core: Arc<CoreServices>) -> Self {
        Self { core }
    }
}

#[async_trait::async_trait]
impl BookIdSweep for BackfillThumbnailsHandler {
    fn job_type(&self) -> &'static str {
        Self::JOB_TYPE
    }

    fn continuation_delay(&self) -> chrono::Duration {
        chrono::Duration::zero()
    }

    async fn fetch_batch(&self, core: &CoreServices, after_id: Option<BookId>, batch_size: u64) -> Result<Vec<BookId>, Error> {
        let book_repo = core.repository_service.book_repository().clone();
        read_only_transaction(&**core.repository_service.repository(), |tx| {
            Box::pin(async move { book_repo.find_book_ids_with_cover_for_sweep(tx, after_id, batch_size).await })
        })
        .await
    }

    async fn process_batch(&self, core: &Arc<CoreServices>, ids: Vec<BookId>) -> Result<(), Error> {
        // BookToken is deterministically derived from BookId — no DB lookup
        // needed. fetch_batch already restricts to books with covers,
        // so has_cover is guaranteed.
        for id in &ids {
            core.file_store.backfill_thumbnail(BookToken::new(*id)).await?;
        }
        tracing::info!(total = ids.len(), "thumbnail backfill batch complete");
        Ok(())
    }
}

impl JobHandler for BackfillThumbnailsHandler {
    const JOB_TYPE: &'static str = "health.backfill_thumbnails";
    const DISPLAY_NAME: &'static str = "Backfill Thumbnails";
    type Payload = BookSweepPayload;

    async fn handle(&self, payload: BookSweepPayload) -> Result<(), Error> {
        run_book_id_sweep(self, &self.core, payload).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        book::repository::book::MockBookRepository, repository::testing::default_repository_service_builder, storage::MockFileStoreService, test_support::*,
    };

    fn make_core(book_repo: MockBookRepository, store: MockFileStoreService) -> Arc<CoreServices> {
        let repo_service = Arc::new(default_repository_service_builder().book_repository(Arc::new(book_repo)).build().unwrap());
        crate::create_services(
            default_external_services_builder()
                .repository_service(repo_service)
                .file_store(Arc::new(store))
                .build()
                .unwrap(),
            "test-secret",
        )
        .unwrap()
    }

    #[tokio::test]
    async fn calls_backfill_thumbnail_for_each_id_in_batch() {
        let mut book_repo = MockBookRepository::new();
        let mut store = MockFileStoreService::new();

        book_repo
            .expect_find_book_ids_with_cover_for_sweep()
            .returning(|_, _, _| Box::pin(async { Ok(vec![1u64, 2u64]) }));

        store.expect_backfill_thumbnail().times(2).returning(|_| Box::pin(async { Ok(()) }));

        let core = make_core(book_repo, store);
        let handler = BackfillThumbnailsHandler::new(core);
        handler.handle(BookSweepPayload::default()).await.unwrap();
    }

    #[tokio::test]
    async fn noop_when_no_books_with_cover() {
        let mut book_repo = MockBookRepository::new();
        let store = MockFileStoreService::new();

        book_repo
            .expect_find_book_ids_with_cover_for_sweep()
            .returning(|_, _, _| Box::pin(async { Ok(vec![]) }));

        let core = make_core(book_repo, store);
        let handler = BackfillThumbnailsHandler::new(core);
        handler.handle(BookSweepPayload::default()).await.unwrap();
    }
}
