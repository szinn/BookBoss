//! Test fixtures for integration tests.
//!
//! Two book-insertion helpers, mirroring how production builds book ↔ library
//! relationships:
//!
//! - [`insert_book`] inserts the book and adds it to the ALL_BOOKS virtual
//!   library. This matches what `CollectionService::add_book` does in
//!   production and is what most tests want.
//! - [`insert_book_into_library`] inserts the book into a specific library
//!   *only* (no ALL_BOOKS membership). Use this when a test needs to construct
//!   a book that lives in a non-default library so that library-scoped queries
//!   (e.g. smart shelves bound to a personal library) include or exclude it
//!   explicitly.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use bb_core::{
    Error,
    book::{Author, AuthorId, AuthorRole, Book, BookId, BookStatus, BookToken, FileFormat, NewAuthor, NewBook},
    format::FormatService,
    import::{ImportJob, ImportJobId, ImportStatus, NewImportJob},
    pipeline::ExtractedMetadata,
    repository::{RepositoryService, transaction},
    storage::{BookSidecar, FileStoreService},
    user::{NewUser, User},
};
use chrono::Utc;

// ── Silent library store
// ──────────────────────────────────────────────────────

/// A `FileStoreService` implementation that silently succeeds all operations.
/// Used in integration tests for code paths that touch the store.
pub struct SilentFileStore;

#[async_trait]
impl FileStoreService for SilentFileStore {
    fn resolve(&self, _relative_path: &str) -> PathBuf {
        PathBuf::new()
    }
    fn cover_path(&self, _token: BookToken) -> PathBuf {
        PathBuf::new()
    }
    fn thumbnail_path(&self, _token: BookToken) -> PathBuf {
        PathBuf::new()
    }
    fn metadata_path(&self, _token: BookToken) -> PathBuf {
        PathBuf::new()
    }
    async fn source_file_exists(&self, _path: &Path) -> bool {
        true
    }

    async fn store_original_file(&self, _source_hash: &str, original_filename: &str, _source: &Path) -> Result<String, Error> {
        Ok(format!("Originals/{original_filename}"))
    }
    async fn store_book_file(&self, token: BookToken, slug: &str, _format: FileFormat, _source: &Path) -> Result<String, Error> {
        Ok(format!("{token}/{slug}.epub"))
    }
    async fn store_cover(&self, _token: BookToken, _data: &[u8]) -> Result<(), Error> {
        Ok(())
    }
    async fn backfill_thumbnail(&self, _token: BookToken) -> Result<(), Error> {
        Ok(())
    }
    async fn rename_book_files(&self, _token: BookToken, _old_slug: &str, _new_slug: &str) -> Result<(), Error> {
        Ok(())
    }
    async fn copy_to_trash(&self, _token: BookToken, _file_name: &str) -> Result<(), Error> {
        Ok(())
    }
    async fn copy_to_bookdrop_trash(&self, _source: &Path) -> Result<(), Error> {
        Ok(())
    }
    async fn delete_book(&self, _token: BookToken) -> Result<(), Error> {
        Ok(())
    }
    async fn delete_original_file(&self, _relative_path: &str) -> Result<(), Error> {
        Ok(())
    }
    async fn list_files(&self, _path: &Path) -> Result<Vec<PathBuf>, Error> {
        Ok(Vec::new())
    }
}

pub fn silent_file_store() -> Arc<dyn FileStoreService> {
    Arc::new(SilentFileStore)
}

// ── Stub format service
// ──────────────────────────────────────────────────

/// A `FormatService` that returns fixed metadata and silently succeeds all
/// write operations. Used to test the pipeline without real e-book files.
pub struct StubFormatService {
    pub metadata: ExtractedMetadata,
}

#[async_trait]
impl FormatService for StubFormatService {
    fn detect_format(&self, _path: &Path) -> Option<FileFormat> {
        Some(FileFormat::Epub)
    }
    async fn extract_metadata(&self, _path: &Path) -> Result<(FileFormat, ExtractedMetadata), Error> {
        Ok((FileFormat::Epub, self.metadata.clone()))
    }
    async fn enrich(&self, _request: &bb_core::format::EnrichmentRequest) -> Result<(), Error> {
        Ok(())
    }
    async fn write_sidecar(&self, _path: &Path, _sidecar: &BookSidecar) -> Result<(), Error> {
        Ok(())
    }
    async fn read_sidecar(&self, _path: &Path) -> Result<BookSidecar, Error> {
        Ok(BookSidecar {
            title: String::new(),
            authors: vec![],
            description: None,
            publisher: None,
            published_date: None,
            language: None,
            identifiers: vec![],
            series: None,
            genres: vec![],
            tags: vec![],
            page_count: None,
            status: BookStatus::Incoming,
            metadata_source: None,
            files: vec![],
        })
    }
    async fn read_raw_opf(&self, _path: &Path) -> Result<Option<String>, Error> {
        Ok(None)
    }
}

// ── Pipeline service factory
// ──────────────────────────────────────────────────

/// Builds a `CoreServices` backed by a real `PipelineServiceImpl` using:
/// - `StubFormatService` (returns provided metadata, no-op file ops)
/// - `SilentFileStore` (no real file I/O)
/// - No metadata providers (extracted metadata is used as-is)
pub fn pipeline_services(ctx: &crate::context::TestContext, metadata: ExtractedMetadata) -> Arc<bb_core::CoreServices> {
    let format_service: Arc<dyn FormatService> = Arc::new(StubFormatService { metadata });
    bb_core::create_services(
        bb_core::test_support::default_external_services_builder()
            .repository_service(ctx.repos.clone())
            .file_store(silent_file_store())
            .format_service(format_service)
            .build()
            .unwrap(),
        "test-encryption-secret",
    )
    .unwrap()
}

// ── Fixture helpers
// ───────────────────────────────────────────────────────────

pub async fn insert_book(repos: &RepositoryService, title: &str, status: BookStatus) -> Book {
    let book_repo = repos.book_repository().clone();
    let library_repo = repos.library_repository().clone();
    let title = title.to_owned();
    transaction(&**repos.repository(), |tx| {
        let book_repo = book_repo.clone();
        let library_repo = library_repo.clone();
        let title = title.clone();
        Box::pin(async move {
            let book = book_repo
                .add_book(
                    tx,
                    NewBook {
                        title,
                        status,
                        description: None,
                        published_date: None,
                        language: None,
                        series_id: None,
                        series_number: None,
                        publisher_id: None,
                        page_count: None,
                        rating: None,
                        metadata_source: None,
                        has_cover: false,
                    },
                )
                .await?;
            // Mirror CollectionService::add_book: every book lands in
            // ALL_BOOKS.
            library_repo.add_book_to_library(tx, bb_core::library::ALL_BOOKS_LIBRARY_ID, book.id).await?;
            Ok(book)
        })
    })
    .await
    .expect("insert_book fixture failed")
}

/// Insert a book into a specific library only (no ALL_BOOKS membership).
/// Unlike [`insert_book`], the caller controls library membership exactly.
#[allow(dead_code)]
pub async fn insert_book_into_library(repos: &RepositoryService, title: &str, status: BookStatus, library_id: bb_core::library::LibraryId) -> Book {
    let book_repo = repos.book_repository().clone();
    let library_repo = repos.library_repository().clone();
    let title = title.to_owned();
    transaction(&**repos.repository(), |tx| {
        let book_repo = book_repo.clone();
        let library_repo = library_repo.clone();
        let title = title.clone();
        Box::pin(async move {
            let book = book_repo
                .add_book(
                    tx,
                    NewBook {
                        title,
                        status,
                        description: None,
                        published_date: None,
                        language: None,
                        series_id: None,
                        series_number: None,
                        publisher_id: None,
                        page_count: None,
                        rating: None,
                        metadata_source: None,
                        has_cover: false,
                    },
                )
                .await?;
            library_repo.add_book_to_library(tx, library_id, book.id).await?;
            Ok(book)
        })
    })
    .await
    .expect("insert_book_into_library fixture failed")
}

pub async fn insert_user(repos: &RepositoryService, username: &str) -> User {
    let user_repo = repos.user_repository().clone();
    let new_user = NewUser::new(
        username,
        "password123!",
        format!("{username}@example.com"),
        Default::default(),
        "Test User",
        false,
    )
    .expect("valid new user");
    transaction(&**repos.repository(), |tx| {
        let user_repo = user_repo.clone();
        Box::pin(async move { user_repo.add_user(tx, new_user).await })
    })
    .await
    .expect("insert_user fixture failed")
}

pub async fn insert_library(services: &bb_core::CoreServices, owner_id: bb_core::user::UserId, name: &str) -> bb_core::library::Library {
    services
        .library_service
        .create_personal_library_for_user(owner_id, name.to_string())
        .await
        .expect("insert_library fixture failed")
}

pub async fn insert_author(repos: &RepositoryService, name: &str) -> Author {
    let author_repo = repos.author_repository().clone();
    let name = name.to_owned();
    transaction(&**repos.repository(), |tx| {
        let author_repo = author_repo.clone();
        let name = name.clone();
        Box::pin(async move { author_repo.add_author(tx, NewAuthor { name, bio: None }).await })
    })
    .await
    .expect("insert_author fixture failed")
}

pub async fn link_book_author(repos: &RepositoryService, book_id: BookId, author_id: AuthorId) {
    let book_repo = repos.book_repository().clone();
    transaction(&**repos.repository(), |tx| {
        let book_repo = book_repo.clone();
        Box::pin(async move { book_repo.add_book_author(tx, book_id, author_id, AuthorRole::Author, 0).await })
    })
    .await
    .expect("link_book_author fixture failed");
}

pub async fn insert_import_job(repos: &RepositoryService, file_hash: &str) -> ImportJob {
    let job_repo = repos.import_job_repository().clone();
    let file_hash = file_hash.to_owned();
    transaction(&**repos.repository(), |tx| {
        let job_repo = job_repo.clone();
        let file_hash = file_hash.clone();
        Box::pin(async move {
            job_repo
                .add_job(
                    tx,
                    NewImportJob {
                        file_path: format!("/watch/{file_hash}.epub"),
                        file_hash,
                        file_format: bb_core::book::FileFormat::Epub,
                        detected_at: Utc::now(),
                    },
                )
                .await
        })
    })
    .await
    .expect("insert_import_job fixture failed")
}

pub async fn link_job_to_book(repos: &RepositoryService, job: ImportJob, book_id: BookId) -> ImportJob {
    let tx = repos.repository().begin().await.expect("begin tx");
    let result = repos
        .import_job_repository()
        .update_job(
            &*tx,
            ImportJob {
                candidate_book_id: Some(book_id),
                ..job
            },
        )
        .await
        .expect("update_job (link_job_to_book)");
    tx.commit().await.expect("commit tx");
    result
}

pub async fn set_job_status(repos: &RepositoryService, job: ImportJob, status: ImportStatus) -> ImportJob {
    let tx = repos.repository().begin().await.expect("begin tx");
    let result = repos
        .import_job_repository()
        .update_job(&*tx, ImportJob { status, ..job })
        .await
        .expect("update_job (set_job_status)");
    tx.commit().await.expect("commit tx");
    result
}

pub async fn find_book_by_id(repos: &RepositoryService, id: BookId) -> Option<Book> {
    let tx = repos.repository().begin_read_only().await.expect("begin read-only tx");
    repos.book_repository().find_by_id(&*tx, id).await.expect("find_by_id")
}

pub async fn find_job_by_id(repos: &RepositoryService, id: ImportJobId) -> Option<ImportJob> {
    let tx = repos.repository().begin_read_only().await.expect("begin read-only tx");
    repos.import_job_repository().find_by_id(&*tx, id).await.expect("find_by_id")
}
