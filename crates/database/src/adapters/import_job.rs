use bb_core::{
    Error, RepositoryError,
    book::FileFormat,
    import::{ImportJob, ImportJobId, ImportJobRepository, ImportJobToken, ImportSource, ImportStatus, NewImportJob},
    repository::Transaction,
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use crate::{
    entities::{import_jobs, prelude},
    error::handle_dberr,
    transaction::TransactionImpl,
};

// ── String conversions ───────────────────────────────────────────────────────

fn import_status_to_str(s: &ImportStatus) -> &'static str {
    match s {
        ImportStatus::Pending => "pending",
        ImportStatus::Extracting => "extracting",
        ImportStatus::Identifying => "identifying",
        ImportStatus::NeedsReview => "needs_review",
        ImportStatus::Approved => "approved",
        ImportStatus::Rejected => "rejected",
        ImportStatus::Error => "error",
    }
}

fn str_to_import_status(s: &str) -> ImportStatus {
    match s {
        "pending" => ImportStatus::Pending,
        "extracting" => ImportStatus::Extracting,
        "identifying" => ImportStatus::Identifying,
        "needs_review" => ImportStatus::NeedsReview,
        "approved" => ImportStatus::Approved,
        "rejected" => ImportStatus::Rejected,
        "error" => ImportStatus::Error,
        other => panic!("unknown import status: {other}"),
    }
}

fn import_source_to_str(s: &ImportSource) -> &'static str {
    match s {
        ImportSource::Embedded => "embedded",
        ImportSource::Hardcover => "hardcover",
        ImportSource::OpenLibrary => "open_library",
    }
}

fn str_to_import_source(s: &str) -> ImportSource {
    match s {
        "embedded" => ImportSource::Embedded,
        "hardcover" => ImportSource::Hardcover,
        "open_library" => ImportSource::OpenLibrary,
        other => panic!("unknown import source: {other}"),
    }
}

fn file_format_to_str(f: &FileFormat) -> &'static str {
    match f {
        FileFormat::Epub => "epub",
        FileFormat::Mobi => "mobi",
        FileFormat::Azw3 => "azw3",
        FileFormat::Pdf => "pdf",
        FileFormat::Cbz => "cbz",
    }
}

fn str_to_file_format(s: &str) -> FileFormat {
    match s {
        "mobi" => FileFormat::Mobi,
        "azw3" => FileFormat::Azw3,
        "pdf" => FileFormat::Pdf,
        "cbz" => FileFormat::Cbz,
        _ => FileFormat::Epub,
    }
}

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
            file_format: str_to_file_format(&m.file_format),
            detected_at: m.detected_at.with_timezone(&Utc),
            status: str_to_import_status(&m.status),
            candidate_book_id: m.candidate_book_id.map(|id| id as u64),
            metadata_source: m.metadata_source.as_deref().map(str_to_import_source),
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
            file_format: Set(file_format_to_str(&job.file_format).to_owned()),
            detected_at: Set(job.detected_at.into()),
            status: Set(import_status_to_str(&ImportStatus::Pending).to_owned()),
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

        updater.status = Set(import_status_to_str(&job.status).to_owned());
        updater.candidate_book_id = Set(job.candidate_book_id.map(|id| id as i64));
        updater.metadata_source = Set(job.metadata_source.as_ref().map(import_source_to_str).map(str::to_owned));
        updater.error_message = Set(job.error_message);
        updater.reviewed_by = Set(job.reviewed_by.map(|id| id as i64));
        updater.reviewed_at = Set(job.reviewed_at.map(Into::into));

        let updated = updater.update(transaction).await.map_err(handle_dberr)?;
        Ok(updated.into())
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

    async fn find_by_token(&self, transaction: &dyn Transaction, token: &ImportJobToken) -> Result<Option<ImportJob>, Error> {
        self.find_by_id(transaction, token.id()).await
    }

    async fn find_by_hash(&self, transaction: &dyn Transaction, file_hash: &str) -> Result<Option<ImportJob>, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::ImportJobs::find()
            .filter(import_jobs::Column::FileHash.eq(file_hash))
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
        const DEFAULT_PAGE_SIZE: u64 = 50;
        const MAX_PAGE_SIZE: u64 = 50;

        if let Some(page_size) = page_size {
            if page_size < 1 {
                return Err(Error::InvalidPageSize(page_size));
            }
        }

        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let mut query = prelude::ImportJobs::find()
            .filter(import_jobs::Column::Status.eq(import_status_to_str(&status)))
            .order_by_asc(import_jobs::Column::Id);

        if let Some(start_id) = start_id {
            query = query.filter(import_jobs::Column::Id.gte(start_id as i64));
        }

        let page_size = page_size.unwrap_or(DEFAULT_PAGE_SIZE).min(MAX_PAGE_SIZE);
        query = query.limit(page_size);

        let rows = query.all(transaction).await.map_err(handle_dberr)?;
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
        import::{ImportJob, ImportJobRepository, ImportStatus, NewImportJob},
        repository::RepositoryService,
    };
    use chrono::Utc;
    use sea_orm::Database;

    use crate::create_repository_service;

    async fn setup() -> Arc<RepositoryService> {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        create_repository_service(db).await.unwrap()
    }

    fn new_job(file_path: &str) -> NewImportJob {
        NewImportJob {
            file_path: file_path.to_owned(),
            file_hash: format!("hash_{file_path}"),
            file_format: FileFormat::Epub,
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
        assert_eq!(job.file_format, FileFormat::Epub);
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
        let result = svc.import_job_repository().find_by_token(&*tx, &inserted.token).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().unwrap().id, inserted.id);
    }

    // ─── find_by_hash ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_hash_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let inserted = svc.import_job_repository().add_job(&*tx, new_job("/watch/dune.epub")).await.unwrap();
        let result = svc.import_job_repository().find_by_hash(&*tx, &inserted.file_hash).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().unwrap().id, inserted.id);
    }

    #[tokio::test]
    async fn test_find_by_hash_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(svc.import_job_repository().find_by_hash(&*tx, "nonexistent_hash").await.unwrap().is_none());
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
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        use bb_core::import::ImportJobToken;
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
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        use bb_core::import::ImportJobToken;
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
}
