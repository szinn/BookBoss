use bb_core::{
    Error, RepositoryError,
    book::BookId,
    import::{ImportJob, ImportJobId, ImportJobRepository, ImportJobToken, ImportStatus, NewImportJob},
    repository::Transaction,
};
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, ExprTrait, ModelTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, sea_query::Expr,
};

use crate::{
    entities::{import_jobs, prelude},
    error::handle_dberr,
    transaction::TransactionImpl,
};

// ── From impl ────────────────────────────────────────────────────────────────

impl From<import_jobs::Model> for ImportJob {
    fn from(m: import_jobs::Model) -> Self {
        let token = ImportJobToken::new(m.id as u64);
        Self {
            id: m.id as u64,
            version: m.version as u64,
            token,
            file_path: m.file_path,
            file_hash: m.file_hash,
            file_format: m.file_format.parse().expect("DB has unknown file format"),
            detected_at: m.detected_at.with_timezone(&Utc),
            status: m.status.parse().expect("DB has unknown import status"),
            candidate_book_id: m.candidate_book_id.map(|id| id as u64),
            metadata_source: m.metadata_source.as_deref().map(|s| s.parse().expect("DB has unknown import source")),
            error_message: m.error_message,
            reviewed_by: m.reviewed_by.map(|id| id as u64),
            reviewed_at: m.reviewed_at.map(|dt| dt.with_timezone(&Utc)),
            created_at: m.created_at.with_timezone(&Utc),
            updated_at: m.updated_at.with_timezone(&Utc),
        }
    }
}

// ── Adapter ──────────────────────────────────────────────────────────────────

pub(crate) struct ImportJobRepositoryAdapter;

impl ImportJobRepositoryAdapter {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl ImportJobRepository for ImportJobRepositoryAdapter {
    async fn add_job(&self, transaction: &dyn Transaction, job: NewImportJob) -> Result<ImportJob, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let token = ImportJobToken::generate();
        let now = Utc::now();

        let model = import_jobs::ActiveModel {
            id: Set(token.id() as i64),
            token: Set(token.to_string()),
            file_path: Set(job.file_path),
            file_hash: Set(job.file_hash),
            file_format: Set(job.file_format.as_str().to_owned()),
            detected_at: Set(job.detected_at.into()),
            status: Set(ImportStatus::Pending.to_string()),
            candidate_book_id: Set(None),
            metadata_source: Set(None),
            error_message: Set(None),
            reviewed_by: Set(None),
            reviewed_at: Set(None),
            version: Set(0),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };

        let model = model.insert(transaction).await.map_err(handle_dberr)?;
        Ok(model.into())
    }

    async fn update_job(&self, transaction: &dyn Transaction, job: ImportJob) -> Result<ImportJob, Error> {
        if job.id == 0 {
            return Err(Error::InvalidId(job.id));
        }
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let existing = prelude::ImportJobs::find_by_id(job.id as i64)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

        if existing.version != job.version as i64 {
            return Err(Error::RepositoryError(RepositoryError::Conflict));
        }

        let mut updater: import_jobs::ActiveModel = existing.into();

        updater.status = Set(job.status.to_string());
        updater.file_format = Set(job.file_format.as_str().to_owned());
        updater.candidate_book_id = Set(job.candidate_book_id.map(|id| id as i64));
        updater.metadata_source = Set(job.metadata_source.as_ref().map(std::string::ToString::to_string));
        updater.error_message = Set(job.error_message);
        updater.reviewed_by = Set(job.reviewed_by.map(|id| id as i64));
        updater.reviewed_at = Set(job.reviewed_at.map(Into::into));

        let result = updater.update(transaction).await.map_err(handle_dberr)?;
        Ok(result.into())
    }

    async fn find_by_id(&self, transaction: &dyn Transaction, id: ImportJobId) -> Result<Option<ImportJob>, Error> {
        if id == 0 {
            return Err(Error::InvalidId(id));
        }
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::ImportJobs::find_by_id(id as i64)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .map(Into::into))
    }

    async fn find_by_token(&self, transaction: &dyn Transaction, token: ImportJobToken) -> Result<Option<ImportJob>, Error> {
        self.find_by_id(transaction, token.id()).await
    }

    async fn find_active_by_hash(&self, transaction: &dyn Transaction, file_hash: &str) -> Result<Option<ImportJob>, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let active_statuses = [
            ImportStatus::Pending.to_string(),
            ImportStatus::Extracting.to_string(),
            ImportStatus::Identifying.to_string(),
            ImportStatus::NeedsReview.to_string(),
        ];

        Ok(prelude::ImportJobs::find()
            .filter(import_jobs::Column::FileHash.eq(file_hash))
            .filter(import_jobs::Column::Status.is_in(active_statuses))
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .map(Into::into))
    }

    async fn list_by_status(
        &self,
        transaction: &dyn Transaction,
        status: ImportStatus,
        start_id: Option<ImportJobId>,
        page_size: Option<u64>,
    ) -> Result<Vec<ImportJob>, Error> {
        if let Some(page_size) = page_size
            && page_size < 1
        {
            return Err(Error::InvalidPageSize(page_size));
        }

        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let mut query = prelude::ImportJobs::find()
            .filter(import_jobs::Column::Status.eq(status.as_str()))
            .order_by_asc(import_jobs::Column::Id);

        if let Some(start_id) = start_id {
            query = query.filter(import_jobs::Column::Id.gte(start_id as i64));
        }

        if let Some(page_size) = page_size {
            query = query.limit(page_size);
        }

        let rows = query.all(transaction).await.map_err(handle_dberr)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn list_all_by_status(&self, transaction: &dyn Transaction, status: ImportStatus) -> Result<Vec<ImportJob>, Error> {
        let db_tx = TransactionImpl::get_db_transaction(transaction)?;
        let rows = prelude::ImportJobs::find()
            .filter(import_jobs::Column::Status.eq(status.as_str()))
            .order_by_asc(import_jobs::Column::Id)
            .all(db_tx)
            .await
            .map_err(handle_dberr)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn count_by_status(&self, transaction: &dyn Transaction, status: ImportStatus) -> Result<u64, Error> {
        let db_tx = TransactionImpl::get_db_transaction(transaction)?;
        let count = prelude::ImportJobs::find()
            .filter(import_jobs::Column::Status.eq(status.as_str()))
            .count(db_tx)
            .await
            .map_err(handle_dberr)?;
        Ok(count)
    }

    async fn reset_in_progress_to_pending(&self, transaction: &dyn Transaction) -> Result<u64, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;
        let now = Utc::now();

        let result = prelude::ImportJobs::update_many()
            .col_expr(import_jobs::Column::Status, Expr::value("pending"))
            .col_expr(import_jobs::Column::Version, Expr::col(import_jobs::Column::Version).add(1))
            .col_expr(import_jobs::Column::UpdatedAt, Expr::value(now.fixed_offset()))
            .filter(import_jobs::Column::Status.is_in(["extracting", "identifying"]))
            .exec(transaction)
            .await
            .map_err(handle_dberr)?;

        Ok(result.rows_affected)
    }

    async fn find_by_candidate_book_id(&self, transaction: &dyn Transaction, book_id: BookId) -> Result<Option<ImportJob>, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::ImportJobs::find()
            .filter(import_jobs::Column::CandidateBookId.eq(book_id as i64))
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .map(Into::into))
    }

    async fn delete_job(&self, transaction: &dyn Transaction, job_id: ImportJobId) -> Result<(), Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        if let Some(existing) = prelude::ImportJobs::find_by_id(job_id as i64).one(transaction).await.map_err(handle_dberr)? {
            existing.delete(transaction).await.map_err(handle_dberr)?;
        }

        Ok(())
    }

    async fn approve_job(&self, transaction: &dyn Transaction, job_id: ImportJobId, reviewer_id: bb_core::user::UserId) -> Result<(), Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;
        let now = Utc::now();

        let existing = prelude::ImportJobs::find_by_id(job_id as i64)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

        let mut updater: import_jobs::ActiveModel = existing.into();
        updater.status = Set(ImportStatus::Approved.to_string());
        updater.reviewed_by = Set(Some(reviewer_id as i64));
        updater.reviewed_at = Set(Some(now.into()));
        updater.update(transaction).await.map_err(handle_dberr)?;

        Ok(())
    }

    async fn delete_old_terminal_jobs(&self, transaction: &dyn Transaction, cutoff: DateTime<Utc>) -> Result<u64, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let result = prelude::ImportJobs::delete_many()
            .filter(import_jobs::Column::Status.is_in([ImportStatus::Approved.as_str(), ImportStatus::Rejected.as_str()]))
            .filter(import_jobs::Column::UpdatedAt.lt(cutoff.fixed_offset()))
            .exec(transaction)
            .await
            .map_err(handle_dberr)?;

        Ok(result.rows_affected)
    }

    async fn find_stale_non_terminal_jobs(&self, transaction: &dyn Transaction, cutoff: DateTime<Utc>) -> Result<Vec<ImportJob>, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let rows = prelude::ImportJobs::find()
            .filter(import_jobs::Column::Status.is_in([
                ImportStatus::Pending.as_str(),
                ImportStatus::Extracting.as_str(),
                ImportStatus::Identifying.as_str(),
                ImportStatus::NeedsReview.as_str(),
            ]))
            .filter(import_jobs::Column::UpdatedAt.lt(cutoff.fixed_offset()))
            .all(transaction)
            .await
            .map_err(handle_dberr)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bb_core::{
        Error, RepositoryError,
        book::FileFormat,
        import::{ImportJob, ImportStatus, NewImportJob},
        repository::RepositoryService,
        user::NewUser,
    };
    use chrono::Utc;
    use sea_orm::Database;

    use crate::create_repository_service;

    async fn setup() -> Arc<RepositoryService> {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        create_repository_service(db).await.unwrap()
    }

    fn new_user() -> NewUser {
        use std::collections::HashSet;
        NewUser::new("tester", "hash", "tester@example.com", HashSet::new(), "Tester", false).unwrap()
    }

    fn new_job(file_path: &str) -> NewImportJob {
        NewImportJob {
            file_path: file_path.to_owned(),
            file_hash: format!("hash_{file_path}"),
            file_format: bb_core::book::FileFormat::Epub,
            detected_at: Utc::now(),
        }
    }

    // ─── add_job ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_add_job_success() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc.import_job_repository().add_job(&*tx, new_job("/watch/dune.epub")).await;

        assert!(result.is_ok());
        let job = result.unwrap();
        assert_ne!(job.id, 0);
        assert_eq!(job.file_path, "/watch/dune.epub");
        assert_eq!(job.file_format, bb_core::book::FileFormat::Epub);
        assert_eq!(job.status, ImportStatus::Pending);
        assert_eq!(job.token.id(), job.id);
    }

    // ─── find_by_id ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_id_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let inserted = svc.import_job_repository().add_job(&*tx, new_job("/watch/dune.epub")).await.unwrap();
        let result = svc.import_job_repository().find_by_id(&*tx, inserted.id).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().unwrap().id, inserted.id);
    }

    #[tokio::test]
    async fn test_find_by_id_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(svc.import_job_repository().find_by_id(&*tx, 999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_find_by_id_zero_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(matches!(svc.import_job_repository().find_by_id(&*tx, 0).await, Err(Error::InvalidId(0))));
    }

    // ─── find_by_token ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_token_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let inserted = svc.import_job_repository().add_job(&*tx, new_job("/watch/dune.epub")).await.unwrap();
        let result = svc.import_job_repository().find_by_token(&*tx, inserted.token).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().unwrap().id, inserted.id);
    }

    // ─── find_active_by_hash
    // ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_active_by_hash_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let inserted = svc.import_job_repository().add_job(&*tx, new_job("/watch/dune.epub")).await.unwrap();
        let result = svc.import_job_repository().find_active_by_hash(&*tx, &inserted.file_hash).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().unwrap().id, inserted.id);
    }

    #[tokio::test]
    async fn test_find_active_by_hash_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(
            svc.import_job_repository()
                .find_active_by_hash(&*tx, "nonexistent_hash")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn test_find_active_by_hash_ignores_terminal_statuses() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        // Insert a job and move it to Rejected (a terminal status)
        let inserted = svc.import_job_repository().add_job(&*tx, new_job("/watch/rejected.epub")).await.unwrap();
        let rejected = ImportJob {
            status: ImportStatus::Rejected,
            ..inserted
        };
        svc.import_job_repository().update_job(&*tx, rejected).await.unwrap();

        // find_active_by_hash must not return it
        let result = svc
            .import_job_repository()
            .find_active_by_hash(&*tx, "hash_/watch/rejected.epub")
            .await
            .unwrap();
        assert!(result.is_none(), "expected None for a Rejected job, got {result:?}");
    }

    // ─── list_by_status ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_by_status_returns_matching() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.import_job_repository().add_job(&*tx, new_job("/watch/a.epub")).await.unwrap();
        svc.import_job_repository().add_job(&*tx, new_job("/watch/b.epub")).await.unwrap();

        let results = svc
            .import_job_repository()
            .list_by_status(&*tx, ImportStatus::Pending, None, None)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|j| j.status == ImportStatus::Pending));
    }

    #[tokio::test]
    async fn test_list_by_status_filters_by_status() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.import_job_repository().add_job(&*tx, new_job("/watch/a.epub")).await.unwrap();

        let pending = svc
            .import_job_repository()
            .list_by_status(&*tx, ImportStatus::Pending, None, None)
            .await
            .unwrap();
        let approved = svc
            .import_job_repository()
            .list_by_status(&*tx, ImportStatus::Approved, None, None)
            .await
            .unwrap();

        assert_eq!(pending.len(), 1);
        assert!(approved.is_empty());
    }

    #[tokio::test]
    async fn test_list_by_status_start_id_filters() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.import_job_repository().add_job(&*tx, new_job("/watch/a.epub")).await.unwrap();
        svc.import_job_repository().add_job(&*tx, new_job("/watch/b.epub")).await.unwrap();

        let all = svc
            .import_job_repository()
            .list_by_status(&*tx, ImportStatus::Pending, None, None)
            .await
            .unwrap();
        assert_eq!(all.len(), 2);

        let result = svc
            .import_job_repository()
            .list_by_status(&*tx, ImportStatus::Pending, Some(all[1].id), None)
            .await
            .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, all[1].id);
    }

    #[tokio::test]
    async fn test_list_by_status_page_size_zero_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(matches!(
            svc.import_job_repository().list_by_status(&*tx, ImportStatus::Pending, None, Some(0)).await,
            Err(Error::InvalidPageSize(0))
        ));
    }

    // ─── update_job ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_update_job_success() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut job = svc.import_job_repository().add_job(&*tx, new_job("/watch/dune.epub")).await.unwrap();

        job.status = ImportStatus::Extracting;
        let result = svc.import_job_repository().update_job(&*tx, job).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().status, ImportStatus::Extracting);
    }

    #[tokio::test]
    async fn test_update_job_increments_version() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut job = svc.import_job_repository().add_job(&*tx, new_job("/watch/dune.epub")).await.unwrap();
        let version_before = job.version;

        job.status = ImportStatus::Extracting;
        let updated = svc.import_job_repository().update_job(&*tx, job).await.unwrap();

        assert_eq!(updated.version, version_before + 1);
    }

    #[tokio::test]
    async fn test_update_job_version_conflict() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut job = svc.import_job_repository().add_job(&*tx, new_job("/watch/dune.epub")).await.unwrap();
        job.version = 99;
        job.status = ImportStatus::Extracting;

        assert!(matches!(
            svc.import_job_repository().update_job(&*tx, job).await,
            Err(Error::RepositoryError(RepositoryError::Conflict))
        ));
    }

    #[tokio::test]
    async fn test_update_job_not_found() {
        use bb_core::import::ImportJobToken;

        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let job = ImportJob {
            id: 999,
            version: 1,
            token: ImportJobToken::new(999),
            file_path: "/watch/ghost.epub".to_owned(),
            file_hash: "ghosthash".to_owned(),
            file_format: FileFormat::Epub,
            detected_at: Utc::now(),
            status: ImportStatus::Pending,
            candidate_book_id: None,
            metadata_source: None,
            error_message: None,
            reviewed_by: None,
            reviewed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(matches!(
            svc.import_job_repository().update_job(&*tx, job).await,
            Err(Error::RepositoryError(RepositoryError::NotFound))
        ));
    }

    #[tokio::test]
    async fn test_update_job_zero_id_returns_error() {
        use bb_core::import::ImportJobToken;

        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let job = ImportJob {
            id: 0,
            version: 1,
            token: ImportJobToken::new(1),
            file_path: "/watch/dune.epub".to_owned(),
            file_hash: "hash".to_owned(),
            file_format: FileFormat::Epub,
            detected_at: Utc::now(),
            status: ImportStatus::Pending,
            candidate_book_id: None,
            metadata_source: None,
            error_message: None,
            reviewed_by: None,
            reviewed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert!(matches!(svc.import_job_repository().update_job(&*tx, job).await, Err(Error::InvalidId(0))));
    }

    // ─── reset_in_progress_to_pending ────────────────────────────────────────

    #[tokio::test]
    async fn test_reset_in_progress_to_pending_resets_extracting_and_identifying() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut job_a = svc.import_job_repository().add_job(&*tx, new_job("/watch/a.epub")).await.unwrap();
        let mut job_b = svc.import_job_repository().add_job(&*tx, new_job("/watch/b.epub")).await.unwrap();
        let job_c = svc.import_job_repository().add_job(&*tx, new_job("/watch/c.epub")).await.unwrap();

        job_a.status = ImportStatus::Extracting;
        svc.import_job_repository().update_job(&*tx, job_a).await.unwrap();

        job_b.status = ImportStatus::Identifying;
        svc.import_job_repository().update_job(&*tx, job_b).await.unwrap();

        let reset = svc.import_job_repository().reset_in_progress_to_pending(&*tx).await.unwrap();
        assert_eq!(reset, 2);

        let pending = svc
            .import_job_repository()
            .list_by_status(&*tx, ImportStatus::Pending, None, None)
            .await
            .unwrap();
        assert_eq!(pending.len(), 3);

        // job_c (already pending) must not be affected
        let _ = job_c;
    }

    #[tokio::test]
    async fn test_reset_in_progress_to_pending_returns_zero_when_none() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.import_job_repository().add_job(&*tx, new_job("/watch/a.epub")).await.unwrap();

        let reset = svc.import_job_repository().reset_in_progress_to_pending(&*tx).await.unwrap();
        assert_eq!(reset, 0);
    }

    // ─── delete_old_terminal_jobs ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_old_terminal_jobs_deletes_approved_before_cutoff() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let user = svc.user_repository().add_user(&*tx, new_user()).await.unwrap();
        let job = svc.import_job_repository().add_job(&*tx, new_job("/watch/old.epub")).await.unwrap();
        svc.import_job_repository().approve_job(&*tx, job.id, user.id).await.unwrap();

        // Cutoff in the future — everything is "old".
        let cutoff = Utc::now() + chrono::Duration::hours(1);
        let deleted = svc.import_job_repository().delete_old_terminal_jobs(&*tx, cutoff).await.unwrap();

        assert_eq!(deleted, 1);
    }

    #[tokio::test]
    async fn test_delete_old_terminal_jobs_does_not_delete_pending() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.import_job_repository().add_job(&*tx, new_job("/watch/pending.epub")).await.unwrap();

        let cutoff = Utc::now() + chrono::Duration::hours(1);
        let deleted = svc.import_job_repository().delete_old_terminal_jobs(&*tx, cutoff).await.unwrap();

        assert_eq!(deleted, 0);
    }

    #[tokio::test]
    async fn test_delete_old_terminal_jobs_does_not_delete_recent() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let user = svc.user_repository().add_user(&*tx, new_user()).await.unwrap();
        let job = svc.import_job_repository().add_job(&*tx, new_job("/watch/recent.epub")).await.unwrap();
        svc.import_job_repository().approve_job(&*tx, job.id, user.id).await.unwrap();

        // Cutoff in the past — nothing is old enough.
        let cutoff = Utc::now() - chrono::Duration::hours(1);
        let deleted = svc.import_job_repository().delete_old_terminal_jobs(&*tx, cutoff).await.unwrap();

        assert_eq!(deleted, 0);
    }

    // ─── find_stale_non_terminal_jobs ─────────────────────────────────────────

    #[tokio::test]
    async fn test_find_stale_non_terminal_jobs_returns_old_pending() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.import_job_repository().add_job(&*tx, new_job("/watch/stale.epub")).await.unwrap();

        // Cutoff in the future — everything is "stale".
        let cutoff = Utc::now() + chrono::Duration::hours(1);
        let stale = svc.import_job_repository().find_stale_non_terminal_jobs(&*tx, cutoff).await.unwrap();

        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].status, ImportStatus::Pending);
    }

    #[tokio::test]
    async fn test_find_stale_non_terminal_jobs_ignores_approved() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let user = svc.user_repository().add_user(&*tx, new_user()).await.unwrap();
        let job = svc.import_job_repository().add_job(&*tx, new_job("/watch/approved.epub")).await.unwrap();
        svc.import_job_repository().approve_job(&*tx, job.id, user.id).await.unwrap();

        let cutoff = Utc::now() + chrono::Duration::hours(1);
        let stale = svc.import_job_repository().find_stale_non_terminal_jobs(&*tx, cutoff).await.unwrap();

        assert!(stale.is_empty());
    }

    #[tokio::test]
    async fn test_find_stale_non_terminal_jobs_ignores_recent() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.import_job_repository().add_job(&*tx, new_job("/watch/recent.epub")).await.unwrap();

        // Cutoff in the past — nothing is stale enough.
        let cutoff = Utc::now() - chrono::Duration::hours(1);
        let stale = svc.import_job_repository().find_stale_non_terminal_jobs(&*tx, cutoff).await.unwrap();

        assert!(stale.is_empty());
    }
}
