use std::{path::PathBuf, sync::Arc};

use crate::{
    Error,
    book::{AuthorRole, BookStatus, IdentifierType, MetadataSource, NewAuthor, NewBook, NewPublisher, NewSeries},
    import::{ImportJob, ImportSource, ImportStatus},
    pipeline::{MetadataExtractor, MetadataProvider},
    repository::{RepositoryService, read_only_transaction, transaction},
    storage::{BookSidecar, LibraryStore, SidecarAuthor, SidecarFile, SidecarIdentifier, SidecarSeries},
};

#[async_trait::async_trait]
pub trait PipelineService: Send + Sync {
    /// Processes an import job through the full acquisition pipeline:
    /// dedup → extract → enrich → create book → stage files → write sidecar.
    ///
    /// Returns the updated import job with `NeedsReview` status and
    /// `candidate_book_id` set, or `Rejected` if the file is a duplicate.
    async fn process_job(&self, job: ImportJob) -> Result<ImportJob, Error>;
}

pub struct PipelineServiceImpl {
    repository_service: Arc<RepositoryService>,
    library_store: Arc<dyn LibraryStore>,
    extractor: Arc<dyn MetadataExtractor>,
    providers: Vec<Arc<dyn MetadataProvider>>,
}

impl PipelineServiceImpl {
    pub fn new(
        repository_service: Arc<RepositoryService>,
        library_store: Arc<dyn LibraryStore>,
        extractor: Arc<dyn MetadataExtractor>,
        providers: Vec<Arc<dyn MetadataProvider>>,
    ) -> Self {
        Self {
            repository_service,
            library_store,
            extractor,
            providers,
        }
    }
}

/// Detect a cover image filename from leading magic bytes.
fn detect_cover_filename(data: &[u8]) -> &'static str {
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "cover.png"
    } else if data.starts_with(&[0x47, 0x49, 0x46]) {
        "cover.gif"
    } else if data.len() >= 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
        "cover.webp"
    } else {
        "cover.jpg"
    }
}

/// Build a filesystem-safe slug from a title string.
fn slugify(s: &str) -> String {
    let raw: String = s.chars().map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    raw.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-")
}

#[async_trait::async_trait]
impl PipelineService for PipelineServiceImpl {
    #[tracing::instrument(level = "trace", skip(self))]
    async fn process_job(&self, mut job: ImportJob) -> Result<ImportJob, Error> {
        // Guard: only process jobs in Pending state. A duplicate queue entry
        // (e.g. from startup re-enqueue racing with a reset job) must not
        // overwrite a job that is already mid-flight or complete.
        if job.status != ImportStatus::Pending {
            tracing::debug!(import_job_id = job.id, status = ?job.status, "skipping import job not in pending state");
            return Ok(job);
        }

        // ── 1. Hash dedup: reject if file is already in the library ───────────
        {
            let book_repo = self.repository_service.book_repository().clone();
            let file_hash = job.file_hash.clone();
            let existing = read_only_transaction(&**self.repository_service.repository(), |tx| {
                Box::pin(async move { book_repo.find_file_by_hash(tx, &file_hash).await })
            })
            .await?;

            if existing.is_some() {
                let import_job_repo = self.repository_service.import_job_repository().clone();
                job.status = ImportStatus::Rejected;
                job.error_message = Some("File already exists in library".to_string());
                let j = job;
                return transaction(&**self.repository_service.repository(), |tx| {
                    Box::pin(async move { import_job_repo.update_job(tx, j).await })
                })
                .await;
            }
        }

        // ── 2. Mark Extracting ────────────────────────────────────────────────
        job = {
            let import_job_repo = self.repository_service.import_job_repository().clone();
            job.status = ImportStatus::Extracting;
            let j = job;
            transaction(&**self.repository_service.repository(), |tx| {
                Box::pin(async move { import_job_repo.update_job(tx, j).await })
            })
            .await?
        };

        // ── 3. Extract metadata from the e-book file ──────────────────────────
        let path: PathBuf = job.file_path.clone().into();
        let extracted = self.extractor.extract(&path, job.file_format.clone()).await?;

        // ── 4. Mark Identifying ───────────────────────────────────────────────
        job = {
            let import_job_repo = self.repository_service.import_job_repository().clone();
            job.status = ImportStatus::Identifying;
            let j = job;
            transaction(&**self.repository_service.repository(), |tx| {
                Box::pin(async move { import_job_repo.update_job(tx, j).await })
            })
            .await?
        };

        // ── 5. Enrich: try each provider in order, first success wins ─────────
        let (final_meta, cover_bytes, job_source) = {
            let mut result = None;
            for provider in &self.providers {
                if let Some(pb) = provider.enrich(&extracted).await? {
                    let mut metadata = pb.metadata;
                    // Preserve file-embedded identifiers not returned by the provider.
                    // This ensures the ISBN we searched with is always attached to the
                    // book even if the provider returns a different set of ISBNs.
                    if let Some(extracted_ids) = &extracted.identifiers {
                        let provider_ids = metadata.identifiers.get_or_insert_with(Vec::new);
                        let existing_types: std::collections::HashSet<IdentifierType> =
                            provider_ids.iter().map(|id| id.identifier_type.clone()).collect();
                        for id in extracted_ids {
                            if !existing_types.contains(&id.identifier_type) {
                                provider_ids.push(id.clone());
                            }
                        }
                    }
                    result = Some((metadata, pb.cover_bytes, pb.source));
                    break;
                }
            }
            result.unwrap_or((extracted, None, ImportSource::Embedded))
        };
        let job_source = Some(job_source);

        // ── 6. Resolve cover filename from magic bytes ─────────────────────────
        let cover_filename: Option<String> = cover_bytes.as_deref().map(|b| detect_cover_filename(b).to_string());

        // ── 7. Capture file size before the file is moved ─────────────────────
        let file_size = tokio::fs::metadata(&path).await.map(|m| m.len() as i64).unwrap_or(0);

        // ── 8. Determine title (fall back to filename stem) ───────────────────
        let title = final_meta
            .title
            .clone()
            .unwrap_or_else(|| path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown").to_string());

        // ── 9. Map ImportSource → MetadataSource for the Book record ──────────
        let book_metadata_source: Option<MetadataSource> = job_source.as_ref().map(|s| match s {
            ImportSource::Embedded => MetadataSource::Manual,
            ImportSource::OpenLibrary => MetadataSource::OpenLibrary,
            ImportSource::Hardcover => MetadataSource::Hardcover,
        });

        // ── 10. Pre-build sidecar sub-structures from final_meta ──────────────
        let sidecar_authors: Vec<SidecarAuthor> = final_meta
            .authors
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|a| SidecarAuthor {
                name: a.name.clone(),
                role: a.role.clone().unwrap_or(AuthorRole::Author),
                sort_order: a.sort_order,
                file_as: None,
            })
            .collect();

        let sidecar_identifiers: Vec<SidecarIdentifier> = final_meta
            .identifiers
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|i| SidecarIdentifier {
                identifier_type: i.identifier_type.clone(),
                value: i.value.clone(),
            })
            .collect();

        // ── 11. DB writes in a single transaction ──────────────────────────────
        let book_repo = self.repository_service.book_repository().clone();
        let author_repo = self.repository_service.author_repository().clone();
        let series_repo = self.repository_service.series_repository().clone();
        let publisher_repo = self.repository_service.publisher_repository().clone();
        let import_job_repo = self.repository_service.import_job_repository().clone();

        let fm = final_meta.clone();
        let bms = book_metadata_source.clone();
        let cover_fn = cover_filename.clone();
        let js = job_source.clone();
        let file_hash = job.file_hash.clone();
        let file_format = job.file_format.clone();
        let title_c = title.clone();
        let mut job_c = job;

        let (book, updated_job) = transaction(&**self.repository_service.repository(), |tx| {
            Box::pin(async move {
                // Find or create publisher
                let publisher_id = match &fm.publisher {
                    Some(name) => match publisher_repo.find_by_name(tx, name).await? {
                        Some(p) => Some(p.id),
                        None => Some(publisher_repo.add_publisher(tx, NewPublisher { name: name.clone() }).await?.id),
                    },
                    None => None,
                };

                // Find or create series
                let (series_id, series_number) = match &fm.series_name {
                    Some(name) => {
                        let s = match series_repo.find_by_name(tx, name).await? {
                            Some(s) => s,
                            None => {
                                series_repo
                                    .add_series(
                                        tx,
                                        NewSeries {
                                            name: name.clone(),
                                            description: None,
                                        },
                                    )
                                    .await?
                            }
                        };
                        (Some(s.id), fm.series_number)
                    }
                    None => (None, None),
                };

                // Create the candidate book record
                let book = book_repo
                    .add_book(
                        tx,
                        NewBook {
                            title: title_c,
                            status: BookStatus::Incoming,
                            description: fm.description.clone(),
                            published_date: fm.published_date,
                            language: fm.language.clone(),
                            series_id,
                            series_number,
                            publisher_id,
                            page_count: None,
                            rating: None,
                            metadata_source: bms,
                            cover_path: cover_fn,
                        },
                    )
                    .await?;

                // Record the book file
                book_repo.add_book_file(tx, book.id, file_format, file_size, file_hash).await?;

                // Find or create each author, then link to book
                for a in fm.authors.as_deref().unwrap_or(&[]) {
                    let author = match author_repo.find_by_name(tx, &a.name).await? {
                        Some(ex) => ex,
                        None => {
                            author_repo
                                .add_author(
                                    tx,
                                    NewAuthor {
                                        name: a.name.clone(),
                                        bio: None,
                                    },
                                )
                                .await?
                        }
                    };
                    let role = a.role.clone().unwrap_or(AuthorRole::Author);
                    book_repo.add_book_author(tx, book.id, author.id, role, a.sort_order).await?;
                }

                // Add identifiers, deduplicating by type (keep first occurrence)
                let mut seen_types = std::collections::HashSet::new();
                for id in fm.identifiers.as_deref().unwrap_or(&[]) {
                    if seen_types.insert(id.identifier_type.clone()) {
                        book_repo.add_book_identifier(tx, book.id, id.identifier_type.clone(), id.value.clone()).await?;
                    }
                }

                // Advance import job to NeedsReview with candidate book linked
                job_c.status = ImportStatus::NeedsReview;
                job_c.candidate_book_id = Some(book.id);
                job_c.metadata_source = js;
                let updated_job = import_job_repo.update_job(tx, job_c).await?;

                Ok((book, updated_job))
            })
        })
        .await?;

        // ── 12. Store book file (moves it into the library directory) ──────────
        let slug = {
            let author_slug = final_meta
                .authors
                .as_deref()
                .and_then(|a| a.first())
                .map(|a| slugify(&a.name));
            match author_slug {
                Some(a) => format!("{a}-{}", slugify(&book.title)),
                None => slugify(&book.title),
            }
        };
        self.library_store
            .store_book_file(&book.token, &slug, updated_job.file_format.clone(), &path)
            .await?;

        // ── 13. Store cover image ──────────────────────────────────────────────
        if let (Some(filename), Some(data)) = (&cover_filename, &cover_bytes) {
            self.library_store.store_cover(&book.token, filename, data).await?;
        }

        // ── 14. Write metadata sidecar ────────────────────────────────────────
        let sidecar = BookSidecar {
            title: book.title.clone(),
            authors: sidecar_authors,
            description: final_meta.description,
            publisher: final_meta.publisher,
            published_date: final_meta.published_date,
            language: final_meta.language,
            identifiers: sidecar_identifiers,
            series: final_meta.series_name.map(|name| SidecarSeries {
                name,
                number: final_meta.series_number,
            }),
            genres: vec![],
            tags: vec![],
            rating: None,
            status: BookStatus::Incoming,
            metadata_source: book_metadata_source,
            files: vec![SidecarFile {
                format: updated_job.file_format.clone(),
                hash: updated_job.file_hash.clone(),
            }],
        };
        self.library_store.store_metadata(&book.token, &sidecar).await?;

        Ok(updated_job)
    }
}
