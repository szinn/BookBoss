use std::{path::PathBuf, sync::Arc};

use crate::{
    Error, RepositoryError,
    book::{AuthorRole, BookStatus, IdentifierType, MetadataSource, NewAuthor, NewBook, NewPublisher, NewSeries},
    import::{ImportJob, ImportJobToken, ImportSource, ImportStatus},
    pipeline::{
        MetadataExtractor, MetadataProvider,
        model::{BookEdit, ExtractedIdentifier, ExtractedMetadata},
    },
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

    /// Returns the human-readable names of all configured metadata providers,
    /// in priority order.
    fn list_provider_names(&self) -> Vec<&'static str>;

    /// Fetches metadata from a named provider using the current book data for
    /// the given import job as search context.
    ///
    /// Returns `None` when the provider finds no match or has insufficient
    /// data to query. Returns an error if the provider name is unknown or the
    /// job cannot be found.
    ///
    /// If the result includes cover bytes they are written to the temp cover
    /// store keyed by `job_token` for retrieval during `approve_job`.
    async fn fetch_from_provider(
        &self,
        job_token: &ImportJobToken,
        provider_name: &str,
        identifiers: Vec<(IdentifierType, String)>,
        temp_dir: &std::path::Path,
    ) -> Result<Option<crate::pipeline::ProviderBook>, Error>;

    /// Approves a `NeedsReview` import job, committing the reviewer's edits to
    /// the database, transitioning the book to `Available`, and marking the
    /// import job as `Approved`.
    ///
    /// File renames (when title or primary author changed) and sidecar rewrites
    /// are performed as part of this operation.
    async fn approve_job(&self, job_token: ImportJobToken, edit: BookEdit, temp_dir: &std::path::Path) -> Result<(), Error>;
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

/// Minimum side length (px) for a cover to be considered "good enough",
/// after which the pipeline stops querying further providers for a better one.
const GOOD_COVER_MIN_SIDE: u32 = 500;

/// Parse image dimensions from raw bytes by inspecting format-specific headers.
/// Supports PNG, GIF, WebP (VP8/VP8L/VP8X), and JPEG (SOF markers).
/// Returns `None` if the format is unrecognised or the header is truncated.
pub fn image_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    // PNG: 8-byte signature followed by IHDR (width at 16, height at 20)
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        if data.len() >= 24 {
            let w = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
            let h = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
            return Some((w, h));
        }
    }
    // GIF: "GIF87a" / "GIF89a" + 2-byte LE width + 2-byte LE height at offset 6
    if (data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a")) && data.len() >= 10 {
        let w = u16::from_le_bytes([data[6], data[7]]) as u32;
        let h = u16::from_le_bytes([data[8], data[9]]) as u32;
        return Some((w, h));
    }
    // WebP: RIFF....WEBP
    if data.len() >= 30 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
        match &data[12..16] {
            b"VP8 " => {
                // Lossy: 14-bit LE width/height at offset 26/28 (mask 0x3FFF)
                let w = (u16::from_le_bytes([data[26], data[27]]) & 0x3FFF) as u32;
                let h = (u16::from_le_bytes([data[28], data[29]]) & 0x3FFF) as u32;
                return Some((w, h));
            }
            b"VP8L" if data.len() >= 25 => {
                // Lossless: packed bits at offset 21
                let bits = u32::from_le_bytes([data[21], data[22], data[23], data[24]]);
                let w = (bits & 0x3FFF) + 1;
                let h = ((bits >> 14) & 0x3FFF) + 1;
                return Some((w, h));
            }
            b"VP8X" => {
                // Extended: canvas width-1 (3 bytes LE) at 24, height-1 at 27
                let w = u32::from_le_bytes([data[24], data[25], data[26], 0]) + 1;
                let h = u32::from_le_bytes([data[27], data[28], data[29], 0]) + 1;
                return Some((w, h));
            }
            _ => {}
        }
    }
    // JPEG: scan for SOF marker (0xFF 0xCx, excluding DHT/DAC/RST variants)
    if data.starts_with(&[0xFF, 0xD8]) {
        let mut i = 2usize;
        while i + 3 < data.len() {
            if data[i] != 0xFF {
                break;
            }
            let marker = data[i + 1];
            if matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
                if i + 8 < data.len() {
                    let h = u16::from_be_bytes([data[i + 5], data[i + 6]]) as u32;
                    let w = u16::from_be_bytes([data[i + 7], data[i + 8]]) as u32;
                    return Some((w, h));
                }
            }
            if i + 3 >= data.len() {
                break;
            }
            let len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
            if len < 2 {
                break;
            }
            i += 2 + len;
        }
    }
    None
}

/// Returns the minimum side of an image's dimensions, used for quality
/// comparison.
fn cover_min_side(data: &[u8]) -> u32 {
    image_dimensions(data).map(|(w, h)| w.min(h)).unwrap_or(0)
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

        // ── 5. Enrich: find metadata + best-quality cover across all providers
        //
        // The embedded EPUB cover seeds `best_cover` before providers are tried.
        // Each provider cover replaces the current best if its min-side is larger.
        // The loop stops early once metadata is found AND the best cover's min-side
        // meets GOOD_COVER_MIN_SIDE; otherwise every provider is visited so we end
        // up with the highest-resolution cover available.
        let (final_meta, cover_bytes, job_source) = {
            let mut meta: Option<(ExtractedMetadata, ImportSource)> = None;
            let mut best_cover: Option<Vec<u8>> = extracted.cover_bytes.clone();
            let mut best_min_side: u32 = best_cover.as_deref().map(cover_min_side).unwrap_or(0);

            for provider in &self.providers {
                let cover_good_enough = best_min_side >= GOOD_COVER_MIN_SIDE;
                if meta.is_some() && cover_good_enough {
                    break;
                }

                if let Some(pb) = provider.enrich(&extracted).await? {
                    if meta.is_none() {
                        meta = Some((pb.metadata, pb.source));
                    }
                    if let Some(provider_cover) = pb.cover_bytes {
                        let provider_min_side = cover_min_side(&provider_cover);
                        if provider_min_side > best_min_side {
                            best_min_side = provider_min_side;
                            best_cover = Some(provider_cover);
                        }
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
                    (metadata, best_cover, source)
                }
                None => (extracted, best_cover, ImportSource::Embedded),
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

    fn list_provider_names(&self) -> Vec<&'static str> {
        self.providers.iter().map(|p| p.name()).collect()
    }

    #[tracing::instrument(level = "trace", skip(self, identifiers, temp_dir), fields(jobToken = %job_token, provider = provider_name))]
    async fn fetch_from_provider(
        &self,
        job_token: &ImportJobToken,
        provider_name: &str,
        identifiers: Vec<(IdentifierType, String)>,
        temp_dir: &std::path::Path,
    ) -> Result<Option<crate::pipeline::ProviderBook>, Error> {
        let provider = self
            .providers
            .iter()
            .find(|p| p.name() == provider_name)
            .ok_or_else(|| Error::Validation(format!("unknown provider: {provider_name}")))?
            .clone();

        // Load the job to locate the candidate book for its title.
        let import_job_repo = self.repository_service.import_job_repository().clone();
        let jt = job_token.clone();
        let job = read_only_transaction(&**self.repository_service.repository(), |tx| {
            Box::pin(async move { import_job_repo.find_by_token(tx, &jt).await })
        })
        .await?
        .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

        // Use caller-supplied identifiers; load only the title from the DB.
        let title = if let Some(book_id) = job.candidate_book_id {
            let book_repo = self.repository_service.book_repository().clone();
            read_only_transaction(&**self.repository_service.repository(), |tx| {
                Box::pin(async move { book_repo.find_by_id(tx, book_id).await })
            })
            .await?
            .map(|b| b.title)
        } else {
            None
        };

        let extracted = ExtractedMetadata {
            title,
            identifiers: if identifiers.is_empty() {
                None
            } else {
                Some(
                    identifiers
                        .into_iter()
                        .map(|(identifier_type, value)| ExtractedIdentifier { identifier_type, value })
                        .collect(),
                )
            },
            ..Default::default()
        };

        let result = provider.enrich(&extracted).await?;

        // Persist cover bytes to temp store so approve_job can access them
        // without a large round-trip through the frontend.
        if let Some(pb) = &result {
            if let Some(cover) = &pb.cover_bytes {
                let cover_dir = temp_dir.join("bookboss-covers");
                tokio::fs::create_dir_all(&cover_dir)
                    .await
                    .map_err(|e| Error::Infrastructure(format!("failed to create temp cover dir: {e}")))?;
                let cover_path = cover_dir.join(job_token.to_string());
                tokio::fs::write(&cover_path, cover)
                    .await
                    .map_err(|e| Error::Infrastructure(format!("failed to write temp cover: {e}")))?;
            }
        }

        Ok(result)
    }

    #[tracing::instrument(level = "trace", skip(self, edit, temp_dir), fields(jobToken = %job_token))]
    async fn approve_job(&self, job_token: ImportJobToken, edit: BookEdit, temp_dir: &std::path::Path) -> Result<(), Error> {
        // ── 1. Load job and guard status ──────────────────────────────────────
        let import_job_repo = self.repository_service.import_job_repository().clone();
        let jt = job_token.clone();
        let job = read_only_transaction(&**self.repository_service.repository(), |tx| {
            Box::pin(async move { import_job_repo.find_by_token(tx, &jt).await })
        })
        .await?
        .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

        if job.status != ImportStatus::NeedsReview {
            return Err(Error::Validation(format!("cannot approve job with status {:?}", job.status)));
        }

        let book_id = job
            .candidate_book_id
            .ok_or_else(|| Error::Validation("import job has no candidate book".into()))?;

        // ── 2. Load current book and first author for old-slug computation ─────
        let book_repo = self.repository_service.book_repository().clone();
        let (book, old_authors) = read_only_transaction(&**self.repository_service.repository(), |tx| {
            let br = book_repo.clone();
            Box::pin(async move {
                let book = br.find_by_id(tx, book_id).await?.ok_or(Error::RepositoryError(RepositoryError::NotFound))?;
                let authors = br.authors_for_book(tx, book_id).await?;
                Ok::<_, Error>((book, authors))
            })
        })
        .await?;

        let old_first_author_id = old_authors.iter().min_by_key(|a| a.sort_order).map(|a| a.author_id);

        let old_first_author_name = if let Some(author_id) = old_first_author_id {
            let author_repo = self.repository_service.author_repository().clone();
            read_only_transaction(&**self.repository_service.repository(), |tx| {
                Box::pin(async move { author_repo.find_by_id(tx, author_id).await.map(|a| a.map(|a| a.name)) })
            })
            .await?
        } else {
            None
        };

        let old_slug = match &old_first_author_name {
            Some(name) => format!("{}-{}", slugify(name), slugify(&book.title)),
            None => slugify(&book.title),
        };

        // ── 3. Read temp cover if requested ───────────────────────────────────
        let cover_data: Option<(Vec<u8>, String)> = if edit.use_fetched_cover {
            let cover_path = temp_dir.join("bookboss-covers").join(job_token.to_string());
            match tokio::fs::read(&cover_path).await {
                Ok(data) => {
                    let filename = detect_cover_filename(&data).to_string();
                    Some((data, filename))
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    tracing::warn!(
                        job_token = %job_token,
                        "use_fetched_cover requested but no temp cover file found; skipping"
                    );
                    None
                }
                Err(e) => return Err(Error::Infrastructure(format!("failed to read temp cover: {e}"))),
            }
        } else {
            None
        };

        // ── 4. DB transaction: update book + approve job ──────────────────────
        let book_repo2 = self.repository_service.book_repository().clone();
        let author_repo = self.repository_service.author_repository().clone();
        let series_repo = self.repository_service.series_repository().clone();
        let publisher_repo = self.repository_service.publisher_repository().clone();
        let import_job_repo2 = self.repository_service.import_job_repository().clone();
        let job_id = job.id;
        let cover_filename = cover_data.as_ref().map(|(_, f)| f.clone());
        let edit_c = edit.clone();
        let book_version = book.version;

        transaction(&**self.repository_service.repository(), |tx| {
            Box::pin(async move {
                // Resolve series
                let (series_id, series_number) = match &edit_c.series_name {
                    Some(name) if !name.is_empty() => {
                        let name = normalize_name(name);
                        let s = match series_repo.find_by_name(tx, &name).await? {
                            Some(s) => s,
                            None => series_repo.add_series(tx, NewSeries { name, description: None }).await?,
                        };
                        (Some(s.id), edit_c.series_number)
                    }
                    _ => (None, None),
                };

                // Resolve publisher
                let publisher_id = match &edit_c.publisher_name {
                    Some(name) if !name.is_empty() => {
                        let name = normalize_name(name);
                        match publisher_repo.find_by_name(tx, &name).await? {
                            Some(p) => Some(p.id),
                            None => Some(publisher_repo.add_publisher(tx, NewPublisher { name }).await?.id),
                        }
                    }
                    _ => None,
                };

                // Update book record
                let mut updated_book = book_repo2
                    .find_by_id(tx, book_id)
                    .await?
                    .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;
                if updated_book.version != book_version {
                    return Err(Error::RepositoryError(RepositoryError::Conflict));
                }
                updated_book.title = normalize_name(&edit_c.title);
                updated_book.description = edit_c.description;
                updated_book.published_date = edit_c.published_date;
                updated_book.language = edit_c.language;
                updated_book.series_id = series_id;
                updated_book.series_number = series_number;
                updated_book.publisher_id = publisher_id;
                updated_book.page_count = edit_c.page_count;
                updated_book.status = BookStatus::Available;
                if let Some(filename) = cover_filename {
                    updated_book.cover_path = Some(filename);
                }
                book_repo2.update_book(tx, updated_book).await?;

                // Replace authors
                book_repo2.delete_book_authors(tx, book_id).await?;
                for (sort_order, name) in edit_c.authors.iter().enumerate() {
                    let name = normalize_name(name);
                    if name.is_empty() {
                        continue;
                    }
                    let author = match author_repo.find_by_name(tx, &name).await? {
                        Some(a) => a,
                        None => author_repo.add_author(tx, NewAuthor { name, bio: None }).await?,
                    };
                    book_repo2
                        .add_book_author(tx, book_id, author.id, AuthorRole::Author, sort_order as i32)
                        .await?;
                }

                // Replace identifiers (deduplicate by type, keep first)
                book_repo2.delete_book_identifiers(tx, book_id).await?;
                let mut seen_types = std::collections::HashSet::new();
                for (id_type, value) in &edit_c.identifiers {
                    if value.is_empty() {
                        continue;
                    }
                    if seen_types.insert(id_type.clone()) {
                        book_repo2.add_book_identifier(tx, book_id, id_type.clone(), value.clone()).await?;
                    }
                }

                // Mark import job approved
                import_job_repo2.approve_job(tx, job_id).await?;

                Ok(())
            })
        })
        .await?;

        // ── 5. Store fetched cover ────────────────────────────────────────────
        if let Some((cover_bytes, cover_filename)) = cover_data {
            self.library_store.store_cover(&book.token, &cover_filename, &cover_bytes).await?;
            // Clean up temp file
            let cover_path = temp_dir.join("bookboss-covers").join(job_token.to_string());
            let _ = tokio::fs::remove_file(&cover_path).await;
        }

        // ── 6. Rename book files if slug changed ──────────────────────────────
        let new_first_author = edit.authors.first().map(|a| normalize_name(a));
        let new_slug = match &new_first_author {
            Some(name) if !name.is_empty() => {
                format!("{}-{}", slugify(name), slugify(&normalize_name(&edit.title)))
            }
            _ => slugify(&normalize_name(&edit.title)),
        };

        if old_slug != new_slug {
            self.library_store.rename_book_files(&book.token, &old_slug, &new_slug).await?;
        }

        // ── 7. Rewrite metadata sidecar ───────────────────────────────────────
        let sidecar_authors: Vec<SidecarAuthor> = edit
            .authors
            .iter()
            .filter(|n| !n.is_empty())
            .enumerate()
            .map(|(i, name)| SidecarAuthor {
                name: normalize_name(name),
                role: AuthorRole::Author,
                sort_order: i as i32,
                file_as: None,
            })
            .collect();

        let sidecar_identifiers: Vec<SidecarIdentifier> = {
            let mut seen = std::collections::HashSet::new();
            edit.identifiers
                .iter()
                .filter(|(t, v)| !v.is_empty() && seen.insert(t.clone()))
                .map(|(t, v)| SidecarIdentifier {
                    identifier_type: t.clone(),
                    value: v.clone(),
                })
                .collect()
        };

        let book_file = {
            let book_repo3 = self.repository_service.book_repository().clone();
            read_only_transaction(&**self.repository_service.repository(), |tx| {
                Box::pin(async move { book_repo3.files_for_book(tx, book_id).await })
            })
            .await?
            .into_iter()
            .next()
        };

        let sidecar = BookSidecar {
            title: normalize_name(&edit.title),
            authors: sidecar_authors,
            description: edit.description,
            publisher: edit.publisher_name,
            published_date: edit.published_date,
            language: edit.language,
            identifiers: sidecar_identifiers,
            series: edit.series_name.filter(|n| !n.is_empty()).map(|name| SidecarSeries {
                name,
                number: edit.series_number,
            }),
            genres: vec![],
            tags: vec![],
            rating: None,
            status: BookStatus::Available,
            metadata_source: None,
            files: book_file
                .map(|f| {
                    vec![SidecarFile {
                        format: f.format,
                        hash: f.file_hash,
                    }]
                })
                .unwrap_or_default(),
        };
        self.library_store.store_metadata(&book.token, &sidecar).await?;

        Ok(())
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
