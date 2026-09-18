use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    CoreServices, Error, RepositoryError,
    book::{AuthorRole, BookId, FileFormat, FileRole, book_slug},
    format::{EBookFile, EnrichmentRequest},
    jobs::{Enqueueable, JobHandler},
    message::{MessageSeverity, NewSystemMessage},
    repository::{read_only_transaction, transaction},
    storage::{BookSidecar, SidecarAuthor, SidecarIdentifier, SidecarSeries},
};

/// Payload for the MOBI conversion job.
#[derive(Debug, Serialize, Deserialize)]
pub struct ConvertMobiPayload {
    pub book_id: BookId,
}

impl Enqueueable for ConvertMobiPayload {
    const JOB_TYPE: &'static str = "convert_mobi";
    const DEFAULT_PRIORITY: i16 = crate::jobs::PRIORITY_SWEEP;
}

pub struct ConvertMobiHandler {
    core: Arc<CoreServices>,
}

impl ConvertMobiHandler {
    #[must_use]
    pub fn new(core: Arc<CoreServices>) -> Self {
        Self { core }
    }
}

impl ConvertMobiHandler {
    async fn run(&self, book_id: BookId) -> Result<(), Error> {
        // ── 1. Check mobi_enabled setting
        // ─────────────────────────────────────
        let mobi_enabled = self.core.app_setting_service.mobi_enabled().await?;

        // ── 2. Load all book data in a single read transaction
        // ────────────────
        let repo = self.core.repository_service.clone();
        let (book, files, authors, identifiers, genres, tags, series_opt, publisher_opt) =
            read_only_transaction(&**self.core.repository_service.repository(), |tx| {
                let repo = repo.clone();
                Box::pin(async move {
                    let book_repo = repo.book_repository().clone();
                    let author_repo = repo.author_repository().clone();
                    let series_repo = repo.series_repository().clone();
                    let publisher_repo = repo.publisher_repository().clone();

                    let book = book_repo
                        .find_by_id(tx, book_id)
                        .await?
                        .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

                    let files = book_repo.files_for_book(tx, book_id).await?;
                    let book_author_links = book_repo.authors_for_book(tx, book_id).await?;
                    let identifiers = book_repo.identifiers_for_book(tx, book_id).await?;
                    let genres = book_repo.genres_for_book(tx, book_id).await?;
                    let tags = book_repo.tags_for_book(tx, book_id).await?;

                    let mut authors: Vec<(String, AuthorRole, i32)> = Vec::new();
                    for link in &book_author_links {
                        if let Some(author) = author_repo.find_by_id(tx, link.author_id).await? {
                            authors.push((author.name, link.role.clone(), link.sort_order));
                        }
                    }

                    let series_opt = if let Some(sid) = book.series_id {
                        series_repo.find_by_id(tx, sid).await?
                    } else {
                        None
                    };

                    let publisher_opt = if let Some(pid) = book.publisher_id {
                        publisher_repo.find_by_id(tx, pid).await?
                    } else {
                        None
                    };

                    Ok((book, files, authors, identifiers, genres, tags, series_opt, publisher_opt))
                })
            })
            .await?;

        // ── 2b. Bail if mobi disabled and no existing MOBI file
        // ─────────────── When mobi_enabled is false we still
        // re-convert if the book already has a MOBI file, keeping it in
        // sync with any metadata changes.
        let mobi_exists = files.iter().any(|f| f.file_role == FileRole::Enriched && f.format == FileFormat::Mobi);
        if !mobi_enabled && !mobi_exists {
            tracing::debug!(book_id, "MOBI conversion skipped: mobi_enabled is false and no existing MOBI file");
            return Ok(());
        }

        // ── 3. Find the enriched EPUB file
        // ────────────────────────────────────
        let Some(enriched_file) = files.iter().find(|f| f.file_role == FileRole::Enriched && f.format == FileFormat::Epub) else {
            tracing::warn!(book_id, "MOBI conversion skipped: no enriched EPUB found");
            return Ok(());
        };

        // ── 4. Resolve source path
        // ────────────────────────────────────────────
        let source_path = self.core.file_store.resolve(&enriched_file.path);

        // ── 5. Build sidecar from DB data
        // ─────────────────────────────────────
        let sidecar = BookSidecar {
            title: book.title.clone(),
            authors: authors
                .iter()
                .map(|(name, role, sort_order)| SidecarAuthor {
                    name: name.clone(),
                    role: role.clone(),
                    sort_order: *sort_order,
                    file_as: None,
                })
                .collect(),
            description: book.description.clone(),
            publisher: publisher_opt.map(|p| p.name),
            published_date: book.published_date,
            language: book.language.clone(),
            identifiers: identifiers
                .iter()
                .map(|id| SidecarIdentifier {
                    identifier_type: id.identifier_type.clone(),
                    value: id.value.clone(),
                })
                .collect(),
            series: series_opt.map(|s| SidecarSeries {
                name: s.name,
                number: book.series_number,
            }),
            genres: genres.iter().map(|g| g.name.clone()).collect(),
            tags: tags.iter().map(|t| t.name.clone()).collect(),
            page_count: book.page_count,
            status: book.status.clone(),
            metadata_source: book.metadata_source.clone(),
            files: vec![],
        };

        // ── 6. Resolve cover path
        // ─────────────────────────────────────────────
        let cover_path = book.has_cover.then(|| self.core.file_store.cover_path(book.token));

        // ── 7. Derive slug
        // ────────────────────────────────────────────────────
        let first_author_name = authors.first().map(|(name, _, _)| name.as_str());
        let slug = book_slug(&book.title, first_author_name);

        // ── 8. Create temp file for MOBI output
        // ───────────────────────────────
        let mobi_temp = tempfile::NamedTempFile::new().map_err(|e| Error::Infrastructure(format!("temp file: {e}")))?;
        let mobi_dest = mobi_temp.path().to_path_buf();

        // ── 9. Call FormatService to convert
        // ──────────────────────────────────
        let request = EnrichmentRequest {
            source: EBookFile {
                format: FileFormat::Epub,
                path: source_path,
            },
            sidecar,
            cover_path,
            outputs: vec![EBookFile {
                format: FileFormat::Mobi,
                path: mobi_dest.clone(),
            }],
        };

        self.core.format_service.enrich(&request).await?;

        // ── 10. Hash and size the MOBI output
        // ─────────────────────────────────
        let mobi_hash = bb_utils::hash::hash_file(&mobi_dest)
            .await
            .map_err(|e| Error::Infrastructure(format!("hash failed: {e}")))?;
        let mobi_size = tokio::fs::metadata(&mobi_dest)
            .await
            .map_err(|e| Error::Infrastructure(format!("metadata failed: {e}")))?
            .len() as i64;

        // ── 11. Move MOBI file into the library
        // ───────────────────────────────
        let mobi_path = self.core.file_store.store_book_file(book.token, &slug, FileFormat::Mobi, &mobi_dest).await?;

        // ── 12. Upsert book_file record
        // ───────────────────────────────────────
        let book_repo = self.core.repository_service.book_repository().clone();
        transaction(&**self.core.repository_service.repository(), |tx| {
            let book_repo = book_repo.clone();
            let mobi_hash = mobi_hash.clone();
            let mobi_path = mobi_path.clone();
            Box::pin(async move {
                book_repo.delete_book_file_by_role(tx, book_id, FileFormat::Mobi, FileRole::Enriched).await?;
                book_repo
                    .add_book_file(tx, book_id, FileFormat::Mobi, FileRole::Enriched, mobi_path, mobi_size, mobi_hash)
                    .await?;
                Ok(())
            })
        })
        .await?;

        tracing::info!(book_id, "MOBI conversion complete");
        Ok(())
    }
}

impl JobHandler for ConvertMobiHandler {
    const JOB_TYPE: &'static str = "convert_mobi";
    const DISPLAY_NAME: &'static str = "Convert MOBI";
    type Payload = ConvertMobiPayload;

    async fn handle(&self, payload: ConvertMobiPayload) -> Result<(), Error> {
        let book_id = payload.book_id;
        let result = self.run(book_id).await;
        if let Err(ref e) = result {
            tracing::error!(book_id, error = %e, "convert_mobi failed");
            let repo = self.core.repository_service.clone();
            let title = read_only_transaction(&**self.core.repository_service.repository(), |tx| {
                let repo = repo.clone();
                Box::pin(async move { Ok(repo.book_repository().find_by_id(tx, book_id).await?.map(|b| b.title)) })
            })
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| format!("#{book_id}"));
            let _ = self
                .core
                .system_message_service
                .add_message(NewSystemMessage {
                    source_task: Self::JOB_TYPE.to_string(),
                    severity: MessageSeverity::Error,
                    message: format!("MOBI conversion failed for \"{title}\": {e}"),
                })
                .await;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app_setting::repository::MockAppSettingRepository,
        book::{Book, BookStatus, repository::book::MockBookRepository},
        message::repository::MockSystemMessageRepository,
        repository::testing::default_repository_service_builder,
        test_support::*,
    };

    /// Builds a `CoreServices` with `mobi_enabled` returning `false` and no
    /// existing MOBI file for the book — verifies that the handler exits after
    /// loading files without attempting conversion.
    #[tokio::test]
    async fn skips_when_mobi_disabled_and_no_existing_mobi() {
        // mobi_enabled() calls AppSettingRepository::get — return None → false
        let mut app_setting_repo = MockAppSettingRepository::new();
        app_setting_repo.expect_get().returning(|_, _| Box::pin(std::future::ready(Ok(None))));

        // Book exists but has no MOBI file — handler should skip conversion
        let mut book_repo = MockBookRepository::new();
        book_repo
            .expect_find_by_id()
            .returning(|_, _| Box::pin(std::future::ready(Ok(Some(Book::fake(42, "Test Book", BookStatus::Available))))));
        book_repo.expect_files_for_book().returning(|_, _| Box::pin(std::future::ready(Ok(vec![]))));
        book_repo.expect_authors_for_book().returning(|_, _| Box::pin(std::future::ready(Ok(vec![]))));
        book_repo
            .expect_identifiers_for_book()
            .returning(|_, _| Box::pin(std::future::ready(Ok(vec![]))));
        book_repo.expect_genres_for_book().returning(|_, _| Box::pin(std::future::ready(Ok(vec![]))));
        book_repo.expect_tags_for_book().returning(|_, _| Box::pin(std::future::ready(Ok(vec![]))));

        let repo_service = Arc::new(
            default_repository_service_builder()
                .app_setting_repository(Arc::new(app_setting_repo))
                .book_repository(Arc::new(book_repo))
                .build()
                .unwrap(),
        );

        let core = crate::create_services(
            default_external_services_builder().repository_service(repo_service).build().unwrap(),
            "test-secret",
        )
        .unwrap();

        let handler = ConvertMobiHandler::new(core);
        let result = handler.handle(ConvertMobiPayload { book_id: 42 }).await;
        assert!(result.is_ok(), "handler should return Ok when mobi_enabled is false and no MOBI exists");
    }

    /// When `mobi_enabled` is true but the book is not found, the handler
    /// should return an error and post a system message.
    #[tokio::test]
    async fn posts_system_message_on_conversion_failure() {
        // mobi_enabled() → true
        let mut app_setting_repo = MockAppSettingRepository::new();
        app_setting_repo.expect_get().returning(|_, _| {
            use crate::app_setting::AppSetting;
            let setting = AppSetting {
                key: "enrichment.mobi_enabled".into(),
                value: "true".into(),
            };
            Box::pin(std::future::ready(Ok(Some(setting))))
        });

        // Book not found → run() returns an error
        let mut book_repo = MockBookRepository::new();
        book_repo.expect_find_by_id().returning(|_, _| Box::pin(std::future::ready(Ok(None))));

        let mut msg_repo = MockSystemMessageRepository::new();
        msg_repo.expect_add_message().once().returning(|_, msg| {
            assert_eq!(msg.severity, MessageSeverity::Error);
            assert!(msg.message.contains("42"), "message should include book_id");
            let msg = crate::message::SystemMessage {
                id: 1,
                source_task: msg.source_task,
                severity: msg.severity,
                message: msg.message,
                created_at: chrono::Utc::now(),
            };
            Box::pin(std::future::ready(Ok(msg)))
        });

        let repo_service = Arc::new(
            default_repository_service_builder()
                .app_setting_repository(Arc::new(app_setting_repo))
                .book_repository(Arc::new(book_repo))
                .system_message_repository(Arc::new(msg_repo))
                .build()
                .unwrap(),
        );

        let core = crate::create_services(
            default_external_services_builder().repository_service(repo_service).build().unwrap(),
            "test-secret",
        )
        .unwrap();

        let handler = ConvertMobiHandler::new(core);
        let result = handler.handle(ConvertMobiPayload { book_id: 42 }).await;
        assert!(result.is_err(), "handle should propagate the error");
    }
}
