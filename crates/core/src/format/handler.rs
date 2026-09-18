use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    CoreServices, Error, RepositoryError,
    book::{AuthorRole, BookId, FileFormat, FileRole, book_slug, compute_sidecar_fingerprint},
    format::{EBookFile, EnrichmentRequest},
    jobs::{Enqueueable, JobHandler},
    message::{MessageSeverity, NewSystemMessage},
    repository::{read_only_transaction, transaction},
    storage::{BookSidecar, SidecarAuthor, SidecarIdentifier, SidecarSeries},
};

/// Payload for the unified enrichment job. Replaces the old separate
/// `enrich_epub` and `convert_kepub` job types.
#[derive(Debug, Serialize, Deserialize)]
pub struct EnrichBookFilesPayload {
    pub book_id: BookId,
}

impl Enqueueable for EnrichBookFilesPayload {
    const JOB_TYPE: &'static str = "enrich_book_files";
    const DEFAULT_PRIORITY: i16 = crate::jobs::PRIORITY_NORMAL;
}

pub struct EnrichBookFilesHandler {
    core: Arc<CoreServices>,
}

impl EnrichBookFilesHandler {
    #[must_use]
    pub fn new(core: Arc<CoreServices>) -> Self {
        Self { core }
    }
}

impl EnrichBookFilesHandler {
    async fn run(&self, book_id: BookId) -> Result<(), Error> {
        // ── 1. Load all book data in a single read transaction
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

        // ── 1b. Compute fingerprint from loaded metadata ─────────────────────
        let fingerprint = {
            let mut author_names: Vec<&str> = authors.iter().map(|(name, _, _)| name.as_str()).collect();
            author_names.sort_unstable();
            let mut genre_names: Vec<&str> = genres.iter().map(|g| g.name.as_str()).collect();
            genre_names.sort_unstable();
            let mut tag_names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
            tag_names.sort_unstable();
            compute_sidecar_fingerprint(
                &book.title,
                &author_names,
                series_opt.as_ref().map(|s| s.name.as_str()),
                book.series_number.as_ref(),
                publisher_opt.as_ref().map(|p| p.name.as_str()),
                &genre_names,
                &tag_names,
                book.rating,
            )
        };

        // ── 2. Find the Original EPUB file ──────────────────────────────────
        let original_file = files
            .iter()
            .find(|f| f.file_role == FileRole::Original && f.format == FileFormat::Epub)
            .ok_or_else(|| Error::Infrastructure(format!("book {book_id}: no original epub file record")))?;

        let source_path = self.core.file_store.resolve(&original_file.path);
        tracing::debug!(book_id, file_path = %original_file.path, "starting enrichment");

        // ── 3. Build sidecar from DB data ───────────────────────────────────
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

        // ── 4. Resolve cover path ───────────────────────────────────────────
        let cover_path = book.has_cover.then(|| self.core.file_store.cover_path(book.token));

        // ── 5. Derive slug and output paths ─────────────────────────────────
        let first_author_name = authors.first().map(|(name, _, _)| name.as_str());
        let slug = book_slug(&book.title, first_author_name);

        // Create temp files for enrichment outputs — FormatService writes here,
        // then we move them into the library via FileStoreService.
        let epub_temp = tempfile::NamedTempFile::new().map_err(|e| Error::Infrastructure(format!("temp file: {e}")))?;
        let kepub_temp = tempfile::NamedTempFile::new().map_err(|e| Error::Infrastructure(format!("temp file: {e}")))?;

        let epub_dest = epub_temp.path().to_path_buf();
        let kepub_dest = kepub_temp.path().to_path_buf();

        // ── 6. Call FormatService to enrich ──────────────────────────────────
        let request = EnrichmentRequest {
            source: EBookFile {
                format: FileFormat::Epub,
                path: source_path,
            },
            sidecar,
            cover_path,
            outputs: vec![
                EBookFile {
                    format: FileFormat::Epub,
                    path: epub_dest.clone(),
                },
                EBookFile {
                    format: FileFormat::Kepub,
                    path: kepub_dest.clone(),
                },
            ],
        };

        self.core.format_service.enrich(&request).await?;

        // ── 7. Hash and size the enriched files ─────────────────────────────
        let epub_hash = bb_utils::hash::hash_file(&epub_dest)
            .await
            .map_err(|e| Error::Infrastructure(format!("hash failed: {e}")))?;
        let epub_size = tokio::fs::metadata(&epub_dest)
            .await
            .map_err(|e| Error::Infrastructure(format!("metadata failed: {e}")))?
            .len() as i64;

        let kepub_hash = bb_utils::hash::hash_file(&kepub_dest)
            .await
            .map_err(|e| Error::Infrastructure(format!("hash failed: {e}")))?;
        let kepub_size = tokio::fs::metadata(&kepub_dest)
            .await
            .map_err(|e| Error::Infrastructure(format!("metadata failed: {e}")))?
            .len() as i64;

        // ── 8. Move enriched files into the library ─────────────────────────
        let enriched_epub_path = self.core.file_store.store_book_file(book.token, &slug, FileFormat::Epub, &epub_dest).await?;

        let enriched_kepub_path = self.core.file_store.store_book_file(book.token, &slug, FileFormat::Kepub, &kepub_dest).await?;

        // ── 9. Upsert book_file records ─────────────────────────────────────
        let book_repo = self.core.repository_service.book_repository().clone();
        transaction(&**self.core.repository_service.repository(), |tx| {
            let book_repo = book_repo.clone();
            let epub_hash = epub_hash.clone();
            let enriched_epub_path = enriched_epub_path.clone();
            let kepub_hash = kepub_hash.clone();
            let enriched_kepub_path = enriched_kepub_path.clone();
            let fingerprint = fingerprint.clone();
            Box::pin(async move {
                // Enriched EPUB
                book_repo.delete_book_file_by_role(tx, book_id, FileFormat::Epub, FileRole::Enriched).await?;
                book_repo
                    .add_book_file(tx, book_id, FileFormat::Epub, FileRole::Enriched, enriched_epub_path, epub_size, epub_hash)
                    .await?;

                // Enriched KEPUB
                book_repo.delete_book_file_by_role(tx, book_id, FileFormat::Kepub, FileRole::Enriched).await?;
                book_repo
                    .add_book_file(tx, book_id, FileFormat::Kepub, FileRole::Enriched, enriched_kepub_path, kepub_size, kepub_hash)
                    .await?;

                // Record fingerprint — marks sidecar as current
                book_repo.update_sidecar_fingerprint(tx, book_id, Some(fingerprint)).await?;

                Ok(())
            })
        })
        .await?;

        tracing::info!(book_id, "book file enrichment complete (EPUB + KEPUB)");

        // ── 10. Enqueue MOBI conversion if needed ────────────────────────────
        // Enqueue when mobi_enabled (new conversion) OR an enriched MOBI
        // already exists (keep existing file up to date after metadata
        // changes).
        let mobi_exists = files.iter().any(|f| f.file_role == FileRole::Enriched && f.format == FileFormat::Mobi);
        if mobi_exists || self.core.app_setting_service.mobi_enabled().await? {
            use crate::{format::mobi_handler::ConvertMobiPayload, jobs::JobServiceExt};
            self.core.job_service.enqueue(&ConvertMobiPayload { book_id }).await?;
        }

        Ok(())
    }
}

impl JobHandler for EnrichBookFilesHandler {
    const JOB_TYPE: &'static str = "enrich_book_files";
    const DISPLAY_NAME: &'static str = "Enrich Book Files";
    type Payload = EnrichBookFilesPayload;

    async fn handle(&self, payload: EnrichBookFilesPayload) -> Result<(), Error> {
        let book_id = payload.book_id;
        let result = self.run(book_id).await;
        if let Err(ref e) = result {
            tracing::error!(book_id, error = %e, "enrich_book_files failed");
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
                    message: format!("Enrichment failed for \"{title}\": {e}"),
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
        book::repository::book::MockBookRepository, message::repository::MockSystemMessageRepository, repository::testing::default_repository_service_builder,
        test_support::*,
    };

    #[tokio::test]
    async fn posts_system_message_on_enrichment_failure() {
        let mut book_repo = MockBookRepository::new();
        let mut msg_repo = MockSystemMessageRepository::new();

        // Book not found → run() returns an error
        book_repo.expect_find_by_id().returning(|_, _| Box::pin(std::future::ready(Ok(None))));

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

        let handler = EnrichBookFilesHandler::new(core);
        let result = handler.handle(EnrichBookFilesPayload { book_id: 42 }).await;
        assert!(result.is_err(), "handle should propagate the error");
    }
}
