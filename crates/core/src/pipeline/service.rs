use std::{path::PathBuf, sync::Arc};

use crate::{
    Error, RepositoryError,
    book::{AuthorRole, BookStatus, IdentifierType, MetadataSource, NewAuthor, NewBook, NewPublisher, NewSeries},
    import::{ImportJob, ImportJobToken, ImportSource, ImportStatus},
    pipeline::{MetadataExtractor, MetadataProvider, model::ExtractedMetadata},
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

    /// Rejects a NeedsReview import job, cleaning up all associated artifacts:
    /// removes the library directory, deletes the candidate book record, and
    /// deletes the import job record so the file can be re-imported if dropped
    /// again.
    async fn reject_job(&self, job_token: ImportJobToken) -> Result<(), Error>;
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

/// Normalize a name string: trim edges and collapse interior whitespace runs
/// to a single space. Ensures "A  B" and "A B" resolve to the same author.
fn normalize_name(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Build a filesystem-safe slug from a title string.
fn slugify(s: &str) -> String {
    let raw: String = s.chars().map(|c| if c.is_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    raw.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-")
}

#[async_trait::async_trait]
impl PipelineService for PipelineServiceImpl {
    #[tracing::instrument(level = "trace", skip(self, job), fields(jobToken = %job.token))]
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

        // ── 5. Enrich: providers called lazily; stop once metadata + cover found
        //
        // Metadata comes from the first provider that returns a match.
        // Cover is sought from the same provider first; if it has none, remaining
        // providers are called in order until one supplies a cover. Providers
        // after the first are skipped entirely once cover is also satisfied.
        // The embedded EPUB cover is the final fallback if no provider has one.
        let (final_meta, cover_bytes, job_source) = {
            let mut meta: Option<(ExtractedMetadata, ImportSource)> = None;
            let mut cover: Option<Vec<u8>> = None;

            for provider in &self.providers {
                let need_cover = cover.is_none();
                let need_meta = meta.is_none();

                if !need_meta && !need_cover {
                    break;
                }

                if let Some(pb) = provider.enrich(&extracted).await? {
                    if need_meta {
                        meta = Some((pb.metadata, pb.source));
                    }
                    if need_cover {
                        cover = pb.cover_bytes;
                    }
                }
            }

            match meta {
                Some((mut metadata, source)) => {
                    // Preserve file-embedded identifiers not returned by the provider.
                    if let Some(extracted_ids) = &extracted.identifiers {
                        let provider_ids = metadata.identifiers.get_or_insert_with(Vec::new);
                        let existing_types: std::collections::HashSet<IdentifierType> = provider_ids.iter().map(|id| id.identifier_type.clone()).collect();
                        for id in extracted_ids {
                            if !existing_types.contains(&id.identifier_type) {
                                provider_ids.push(id.clone());
                            }
                        }
                    }
                    let cover = cover.or_else(|| extracted.cover_bytes.clone());
                    (metadata, cover, source)
                }
                None => {
                    let embedded_cover = extracted.cover_bytes.clone();
                    (extracted, embedded_cover, ImportSource::Embedded)
                }
            }
        };
        let job_source = Some(job_source);

        // ── 6. Resolve cover filename from magic bytes ─────────────────────────
        let cover_filename: Option<String> = cover_bytes.as_deref().map(|b| detect_cover_filename(b).to_string());

        // ── 7. Capture file size before the file is moved ─────────────────────
        let file_size = tokio::fs::metadata(&path).await.map(|m| m.len() as i64).unwrap_or(0);

        // ── 8. Determine title (fall back to filename stem) ───────────────────
        let title = normalize_name(
            &final_meta
                .title
                .clone()
                .unwrap_or_else(|| path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown").to_string()),
        );

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
                    Some(name) => {
                        let name = normalize_name(name);
                        match publisher_repo.find_by_name(tx, &name).await? {
                            Some(p) => Some(p.id),
                            None => Some(publisher_repo.add_publisher(tx, NewPublisher { name }).await?.id),
                        }
                    }
                    None => None,
                };

                // Find or create series
                let (series_id, series_number) = match &fm.series_name {
                    Some(name) => {
                        let name = normalize_name(name);
                        let s = match series_repo.find_by_name(tx, &name).await? {
                            Some(s) => s,
                            None => series_repo.add_series(tx, NewSeries { name, description: None }).await?,
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
                    let name = normalize_name(&a.name);
                    let author = match author_repo.find_by_name(tx, &name).await? {
                        Some(ex) => ex,
                        None => author_repo.add_author(tx, NewAuthor { name, bio: None }).await?,
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
            let author_slug = final_meta.authors.as_deref().and_then(|a| a.first()).map(|a| slugify(&a.name));
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

    #[tracing::instrument(level = "trace", skip(self), fields(jobToken = %job_token))]
    async fn reject_job(&self, job_token: ImportJobToken) -> Result<(), Error> {
        let import_job_repo = self.repository_service.import_job_repository().clone();
        let job = read_only_transaction(&**self.repository_service.repository(), |tx| {
            Box::pin(async move { import_job_repo.find_by_token(tx, &job_token).await })
        })
        .await?
        .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

        if job.status != ImportStatus::NeedsReview {
            return Err(Error::Validation(format!("cannot reject job with status {:?}", job.status)));
        }

        // Clean up library files and book record if a candidate book was staged.
        if let Some(book_id) = job.candidate_book_id {
            let book_repo = self.repository_service.book_repository().clone();
            let book = read_only_transaction(&**self.repository_service.repository(), |tx| {
                Box::pin(async move { book_repo.find_by_id(tx, book_id).await })
            })
            .await?;

            if let Some(book) = book {
                // Remove the library directory — idempotent if already missing.
                self.library_store.delete_book(&book.token).await?;

                // Delete the book record (cascades to book_authors, book_files,
                // book_identifiers).
                let book_repo = self.repository_service.book_repository().clone();
                transaction(&**self.repository_service.repository(), |tx| {
                    Box::pin(async move { book_repo.delete_book(tx, book_id).await })
                })
                .await?;
            }
        }

        // Delete the import job so the scanner can re-import the file if dropped again.
        let import_job_repo = self.repository_service.import_job_repository().clone();
        let job_id = job.id;
        transaction(&**self.repository_service.repository(), |tx| {
            Box::pin(async move { import_job_repo.delete_job(tx, job_id).await })
        })
        .await?;

        Ok(())
    }
}
