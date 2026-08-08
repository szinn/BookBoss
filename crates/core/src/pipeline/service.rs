use std::{path::PathBuf, sync::Arc};

use bb_utils::{
    similarity::{author_similarity, combined_score, title_similarity},
    string::normalize_name,
};

use crate::{
    Error,
    book::{AuthorRole, BookStatus, FileRole, IdentifierType, MetadataSource, NewAuthor, NewBook, NewGenre, NewPublisher, NewSeries, NewTag, book_slug},
    event::EventService,
    format::FormatService,
    import::{ImportJob, ImportJobId, ImportSource, ImportStatus},
    metadata::MetadataService,
    pipeline::ProviderBook,
    repository::{RepositoryService, read_only_transaction, transaction},
    storage::{BookSidecar, FileStoreService, SidecarAuthor, SidecarFile, SidecarIdentifier, SidecarSeries},
};

#[async_trait::async_trait]
#[cfg_attr(any(test, feature = "test-support"), mockall::automock)]
#[allow(unused_lifetimes, reason = "async_trait + mockall expansion emits a spurious 'life0 parameter")]
pub trait PipelineService: Send + Sync {
    /// Processes an import job through the full acquisition pipeline:
    /// dedup → extract → enrich → create book → stage files → write sidecar.
    ///
    /// Returns the updated import job with `NeedsReview` status and
    /// `candidate_book_id` set, or `Rejected` if the file is a duplicate.
    async fn process_job(&self, job: ImportJob) -> Result<ImportJob, Error>;

    /// Returns the human-readable names of all configured metadata providers,
    /// in priority order.
    fn list_provider_names(&self) -> Vec<&'static str>;
}

pub struct PipelineServiceImpl {
    repository_service: Arc<RepositoryService>,
    file_store: Arc<dyn FileStoreService>,
    format_service: Arc<dyn FormatService>,
    metadata_service: Arc<dyn MetadataService>,
    event_service: Arc<dyn EventService>,
}

impl PipelineServiceImpl {
    pub fn new(
        repository_service: Arc<RepositoryService>,
        file_store: Arc<dyn FileStoreService>,
        format_service: Arc<dyn FormatService>,
        metadata_service: Arc<dyn MetadataService>,
        event_service: Arc<dyn EventService>,
    ) -> Self {
        Self {
            repository_service,
            file_store,
            format_service,
            metadata_service,
            event_service,
        }
    }
}

/// Minimum title similarity score [0.0, 1.0] required for a provider result
/// to be accepted. Results below this threshold are discarded and embedded
/// metadata is used instead.
const MATCH_THRESHOLD: f32 = 0.5;

/// Parse image dimensions from raw bytes by inspecting format-specific headers.
/// Supports PNG, GIF, WebP (VP8/VP8L/VP8X), and JPEG (SOF markers).
/// Returns `None` if the format is unrecognised or the header is truncated.
#[must_use]
pub fn image_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    // PNG: 8-byte signature followed by IHDR (width at 16, height at 20)
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) && data.len() >= 24 {
        let w = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let h = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
        return Some((w, h));
    }
    // GIF: "GIF87a" / "GIF89a" + 2-byte LE width + 2-byte LE height at offset 6
    if (data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a")) && data.len() >= 10 {
        let w = u32::from(u16::from_le_bytes([data[6], data[7]]));
        let h = u32::from(u16::from_le_bytes([data[8], data[9]]));
        return Some((w, h));
    }
    // WebP: RIFF....WEBP
    if data.len() >= 30 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP" {
        match &data[12..16] {
            b"VP8 " => {
                // Lossy: 14-bit LE width/height at offset 26/28 (mask 0x3FFF)
                let w = u32::from(u16::from_le_bytes([data[26], data[27]]) & 0x3FFF);
                let h = u32::from(u16::from_le_bytes([data[28], data[29]]) & 0x3FFF);
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
            if matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) && i + 8 < data.len() {
                let h = u32::from(u16::from_be_bytes([data[i + 5], data[i + 6]]));
                let w = u32::from(u16::from_be_bytes([data[i + 7], data[i + 8]]));
                return Some((w, h));
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
    image_dimensions(data).map_or(0, |(w, h)| w.min(h))
}

/// Priority order for breaking ties between providers with equal scores.
/// Lower value = preferred. Unknown providers fall back to `u8::MAX`.
fn provider_priority(name: &str) -> u8 {
    match name {
        "Hardcover" => 0,
        "Google Books" => 1,
        "Open Library" => 2,
        _ => u8::MAX,
    }
}

#[async_trait::async_trait]
impl PipelineService for PipelineServiceImpl {
    async fn process_job(&self, job: ImportJob) -> Result<ImportJob, Error> {
        // Guard: only process jobs in Pending state. A duplicate queue entry
        // (e.g. from startup re-enqueue racing with a reset job) must not
        // overwrite a job that is already mid-flight or complete.
        if job.status != ImportStatus::Pending {
            tracing::debug!(import_job_id = job.id, status = ?job.status, "skipping import job not in pending state");
            return Ok(job);
        }

        let job_id = job.id;
        match self.process_job_inner(job).await {
            Ok(job) => Ok(job),
            Err(e) => {
                // Best-effort: reset the import job to Error so it is not left
                // permanently stuck in Extracting or Identifying. Any secondary
                // failure here is logged and swallowed — we still return the
                // original error to the caller.
                self.try_mark_import_job_error(job_id, e.to_string()).await;
                Err(e)
            }
        }
    }

    fn list_provider_names(&self) -> Vec<&'static str> {
        self.metadata_service.list_provider_names()
    }
}

impl PipelineServiceImpl {
    async fn process_job_inner(&self, mut job: ImportJob) -> Result<ImportJob, Error> {
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

        // Guard: if the file has disappeared since the job was queued, move it
        // to Error immediately rather than propagating an IO error that would
        // cause the worker to retry indefinitely.
        if !self.file_store.source_file_exists(&path).await {
            let import_job_repo = self.repository_service.import_job_repository().clone();
            job.status = ImportStatus::Error;
            job.error_message = Some(format!("file no longer exists at {}", path.display()));
            let j = job;
            return transaction(&**self.repository_service.repository(), |tx| {
                Box::pin(async move { import_job_repo.update_job(tx, j).await })
            })
            .await;
        }

        let (detected_format, extracted) = self.format_service.extract_metadata(&path).await?;

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

        // ── 5. Enrich: query all providers concurrently, score results by title
        //    similarity, and pick the highest-scoring match above the threshold.
        //
        // If the file already contains spinnaker:metadata it was previously
        // enriched by BookBoss. Treat the embedded metadata as authoritative
        // and skip all external providers.
        //
        // Cover selection: pick the largest image from any provider result
        // (subtask 5 will refine this to only consider confident providers).
        let (final_meta, cover_bytes, job_source) = {
            let embedded_cover = extracted.cover_bytes.clone();

            if extracted.has_spinnaker_metadata {
                (extracted, embedded_cover, ImportSource::Embedded)
            } else {
                // Spawn one task per provider and run all concurrently.
                let mut join_set = tokio::task::JoinSet::new();
                for provider in self.metadata_service.providers() {
                    let ex = extracted.clone();
                    join_set.spawn(async move {
                        let name = provider.name();
                        let result = provider.enrich(&ex).await;
                        (name, result)
                    });
                }

                // Collect successful results; log and discard errors.
                let mut provider_results: Vec<(&'static str, ProviderBook)> = Vec::new();
                while let Some(join_result) = join_set.join_next().await {
                    match join_result {
                        Ok((name, Ok(Some(pb)))) => provider_results.push((name, pb)),
                        Ok((name, Ok(None))) => tracing::debug!(provider = name, "provider returned no result"),
                        Ok((name, Err(e))) => tracing::warn!(provider = name, error = %e, "provider error during enrichment"),
                        Err(e) => tracing::warn!(error = %e, "provider task panicked during enrichment"),
                    }
                }

                // Score each result against the embedded title and primary author.
                // Scores are stored in a parallel vec so they can be reused for
                // both metadata selection and cover selection below.
                let extracted_title = extracted.title.as_deref();
                let extracted_author = extracted
                    .authors
                    .as_deref()
                    .and_then(|a| a.iter().min_by_key(|a| a.sort_order))
                    .map(|a| a.name.as_str());
                let scores: Vec<f32> = provider_results
                    .iter()
                    .map(|(name, pb)| {
                        let t_score = match (extracted_title, pb.metadata.title.as_deref()) {
                            (Some(ext), Some(prov)) => title_similarity(ext, prov),
                            _ => 0.0,
                        };
                        let a_score = match (
                            extracted_author,
                            pb.metadata.authors.as_deref().and_then(|a| a.iter().min_by_key(|a| a.sort_order)),
                        ) {
                            (Some(ext), Some(prov)) => author_similarity(ext, &prov.name),
                            _ => 0.5,
                        };
                        let score = combined_score(t_score, a_score);
                        tracing::debug!(provider = name, title_score = t_score, author_score = a_score, score, "provider result scored");
                        score
                    })
                    .collect();

                // Pick the highest-scoring result above MATCH_THRESHOLD,
                // breaking ties by provider priority.
                let mut best_idx: Option<usize> = None;
                let mut best_score: f32 = -1.0;
                let mut best_priority = u8::MAX;
                for (i, (name, _)) in provider_results.iter().enumerate() {
                    let score = scores[i];
                    if score >= MATCH_THRESHOLD {
                        let priority = provider_priority(name);
                        #[allow(
                            clippy::float_cmp,
                            reason = "scores are computed by the same deterministic formula; exact equality is intentional"
                        )]
                        let is_better = score > best_score || (score == best_score && priority < best_priority);
                        if is_better {
                            best_score = score;
                            best_priority = priority;
                            best_idx = Some(i);
                        }
                    }
                }

                // Cover: pick the largest image from the embedded EPUB cover and
                // any provider that scored above MATCH_THRESHOLD. Covers from
                // providers that failed to match are excluded.
                let mut best_cover = embedded_cover;
                let mut best_min_side = best_cover.as_deref().map_or(0, cover_min_side);
                let mut cover_source: Option<&str> = None;
                for ((name, pb), &score) in provider_results.iter().zip(scores.iter()) {
                    if score >= MATCH_THRESHOLD
                        && let Some(cover) = &pb.cover_bytes
                    {
                        let min_side = cover_min_side(cover);
                        if min_side > best_min_side {
                            best_min_side = min_side;
                            best_cover = Some(cover.clone());
                            cover_source = Some(name);
                        }
                    }
                }
                tracing::debug!(provider = cover_source, min_side = best_min_side, "cover selected");

                if let Some(idx) = best_idx {
                    let (source_name, pb) = provider_results.swap_remove(idx);
                    tracing::debug!(provider = source_name, score = best_score, "selected provider result");
                    let source = pb.source;
                    let mut metadata = pb.metadata;
                    // Preserve file-embedded fields not returned by the provider.
                    if let Some(extracted_ids) = &extracted.identifiers {
                        let provider_ids = metadata.identifiers.get_or_insert_with(Vec::new);
                        let existing_types: std::collections::HashSet<IdentifierType> = provider_ids.iter().map(|id| id.identifier_type.clone()).collect();
                        for id in extracted_ids {
                            if !existing_types.contains(&id.identifier_type) {
                                provider_ids.push(id.clone());
                            }
                        }
                    }
                    if metadata.publisher.is_none() {
                        metadata.publisher.clone_from(&extracted.publisher);
                    }
                    if metadata.language.is_none() {
                        metadata.language.clone_from(&extracted.language);
                    }
                    if metadata.genres.is_empty() {
                        metadata.genres.clone_from(&extracted.genres);
                    }
                    if metadata.tags.is_empty() {
                        metadata.tags.clone_from(&extracted.tags);
                    }
                    if metadata.page_count.is_none() {
                        metadata.page_count = extracted.page_count;
                    }
                    (metadata, best_cover, source)
                } else {
                    tracing::debug!("no provider result met match threshold; using embedded metadata");
                    (extracted, best_cover, ImportSource::Embedded)
                }
            }
        };
        let job_source = Some(job_source);

        // ── 7. Capture file size before the file is moved ─────────────────────
        #[expect(
            clippy::cast_possible_wrap,
            reason = "file size stored as i64 in DB; files in practice will not exceed i64::MAX bytes"
        )]
        let file_size = tokio::fs::metadata(&path).await.map_or(0, |m| m.len() as i64);

        // ── 7a. Store original file in Originals/ before the transaction ───────
        // Must happen before the DB transaction so the actual filename (possibly
        // collision-resolved) is available to pass into add_book_file.
        // Uses copy semantics — source file is preserved for store_book_file.
        let intended_filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();
        let actual_original_filename = self.file_store.store_original_file(&job.file_hash, &intended_filename, &path).await?;

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
            ImportSource::GoogleBooks => MetadataSource::GoogleBooks,
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
        let genre_repo = self.repository_service.genre_repository().clone();
        let tag_repo = self.repository_service.tag_repository().clone();
        let import_job_repo = self.repository_service.import_job_repository().clone();

        let fm = final_meta.clone();
        let bms = book_metadata_source.clone();
        let has_cover = cover_bytes.is_some();
        let js = job_source.clone();
        let file_hash = job.file_hash.clone();
        let file_format = detected_format.clone();
        let original_filename = actual_original_filename.clone();
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
                            page_count: fm.page_count,
                            rating: None,
                            metadata_source: bms,
                            has_cover,
                        },
                    )
                    .await?;

                // Record the book file — path set to the library-relative location
                // returned by store_original_file (updated to full relative path in M9.3).
                book_repo
                    .add_book_file(tx, book.id, file_format, FileRole::Original, original_filename, file_size, file_hash)
                    .await?;

                // Find or create each author, then link to book.
                // Dedupe by resolved author id: source metadata can list the same
                // person twice (e.g. duplicate <dc:creator> entries with/without a
                // role attribute, or providers returning duplicates), and the
                // book_authors PK is (book_id, author_id).
                let mut seen_author_ids = std::collections::HashSet::new();
                for a in fm.authors.as_deref().unwrap_or(&[]) {
                    let name = normalize_name(&a.name);
                    let author = match author_repo.find_by_name(tx, &name).await? {
                        Some(ex) => ex,
                        None => author_repo.add_author(tx, NewAuthor { name, bio: None }).await?,
                    };
                    if !seen_author_ids.insert(author.id) {
                        continue;
                    }
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

                // Add genres. Dedupe by resolved genre id: GenreRepository::find_by_name
                // is case-insensitive while normalize_name preserves case, so two source
                // strings differing only in case (e.g. Open Library subjects "Fiction"
                // and "FICTION") resolve to the same row and violate (book_id, genre_id).
                let mut seen_genre_ids = std::collections::HashSet::new();
                for name in &fm.genres {
                    let name = normalize_name(name);
                    if name.is_empty() {
                        continue;
                    }
                    let genre = match genre_repo.find_by_name(tx, &name).await? {
                        Some(g) => g,
                        None => genre_repo.add_genre(tx, NewGenre { name }).await?,
                    };
                    if !seen_genre_ids.insert(genre.id) {
                        continue;
                    }
                    book_repo.add_book_genre(tx, book.id, genre.id).await?;
                }

                // Add tags. Same case-insensitive find_by_name caveat as genres above.
                let mut seen_tag_ids = std::collections::HashSet::new();
                for name in &fm.tags {
                    let name = normalize_name(name);
                    if name.is_empty() {
                        continue;
                    }
                    let tag = match tag_repo.find_by_name(tx, &name).await? {
                        Some(t) => t,
                        None => tag_repo.add_tag(tx, NewTag { name }).await?,
                    };
                    if !seen_tag_ids.insert(tag.id) {
                        continue;
                    }
                    book_repo.add_book_tag(tx, book.id, tag.id).await?;
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
            let first_author = final_meta.authors.as_deref().and_then(|a| a.first()).map(|a| a.name.as_str());
            book_slug(&book.title, first_author)
        };
        self.file_store.store_book_file(book.token, &slug, detected_format.clone(), &path).await?;

        // ── 13. Store cover image ──────────────────────────────────────────────
        if let Some(data) = &cover_bytes {
            self.file_store.store_cover(book.token, data).await?;
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
            genres: final_meta.genres.clone(),
            tags: final_meta.tags.clone(),
            page_count: final_meta.page_count,
            status: BookStatus::Incoming,
            metadata_source: book_metadata_source,
            files: vec![SidecarFile {
                format: detected_format,
                hash: updated_job.file_hash.clone(),
            }],
        };
        let sidecar_path = self.file_store.metadata_path(book.token);
        self.format_service.write_sidecar(&sidecar_path, &sidecar).await?;

        self.event_service.notify_incoming_changed();

        Ok(updated_job)
    }

    /// Best-effort: if `process_job_inner` returned an error and the import job
    /// is still stuck in an intermediate state, reset it to `Error` so it does
    /// not appear as permanently in-progress. Any failure here is logged and
    /// swallowed — the caller's original error is still returned.
    async fn try_mark_import_job_error(&self, job_id: ImportJobId, message: String) {
        let import_job_repo = self.repository_service.import_job_repository().clone();
        let outcome = transaction(&**self.repository_service.repository(), |tx| {
            let import_job_repo = import_job_repo.clone();
            let message = message.clone();
            Box::pin(async move {
                let Ok(Some(mut job)) = import_job_repo.find_by_id(tx, job_id).await else {
                    return Ok(());
                };
                if matches!(job.status, ImportStatus::Extracting | ImportStatus::Identifying) {
                    job.status = ImportStatus::Error;
                    job.error_message = Some(message);
                    import_job_repo.update_job(tx, job).await?;
                }
                Ok::<(), Error>(())
            })
        })
        .await;
        if let Err(e) = outcome {
            tracing::warn!(import_job_id = job_id, error = %e, "failed to reset stuck import job to Error");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{cover_min_side, image_dimensions, provider_priority};

    // ── image_dimensions: PNG ─────────────────────────────────────────────────

    #[test]
    fn image_dimensions_png() {
        // PNG signature (8 bytes) + fake IHDR length/type (8 bytes) + width (4 BE) +
        // height (4 BE)
        let mut data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]; // PNG sig
        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x0D]); // IHDR length (unused by parser)
        data.extend_from_slice(b"IHDR"); // chunk type (unused by parser)
        data.extend_from_slice(&100u32.to_be_bytes()); // width = 100
        data.extend_from_slice(&200u32.to_be_bytes()); // height = 200
        assert_eq!(image_dimensions(&data), Some((100, 200)));
    }

    #[test]
    fn image_dimensions_truncated_png_returns_none() {
        // PNG magic but only 10 bytes — not enough to read width/height at offset 16–23
        let data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00];
        assert_eq!(image_dimensions(&data), None);
    }

    // ── image_dimensions: GIF ─────────────────────────────────────────────────

    #[test]
    fn image_dimensions_gif89a() {
        let mut data = b"GIF89a".to_vec();
        data.extend_from_slice(&80u16.to_le_bytes()); // width = 80
        data.extend_from_slice(&60u16.to_le_bytes()); // height = 60
        assert_eq!(image_dimensions(&data), Some((80, 60)));
    }

    // ── image_dimensions: WebP ────────────────────────────────────────────────

    #[test]
    fn image_dimensions_webp_vp8_lossy() {
        // RIFF....WEBPVP8 <4-byte chunk-size> <bitstream-start: 3 bytes> <w+h 2 bytes
        // each>
        let mut data = vec![0u8; 30];
        data[0..4].copy_from_slice(b"RIFF");
        data[8..12].copy_from_slice(b"WEBP");
        data[12..16].copy_from_slice(b"VP8 ");
        // Width at [26..28] LE, mask 0x3FFF; height at [28..30] LE, mask 0x3FFF
        let w: u16 = 0x0140; // 320
        let h: u16 = 0x00F0; // 240
        data[26..28].copy_from_slice(&w.to_le_bytes());
        data[28..30].copy_from_slice(&h.to_le_bytes());
        assert_eq!(image_dimensions(&data), Some((320, 240)));
    }

    #[test]
    fn image_dimensions_webp_vp8l_lossless() {
        // Outer WebP guard requires len >= 30; VP8L parser reads at offset 21..25
        let mut data = vec![0u8; 30];
        data[0..4].copy_from_slice(b"RIFF");
        data[8..12].copy_from_slice(b"WEBP");
        data[12..16].copy_from_slice(b"VP8L");
        // Bits at [21..25]: width-1 in low 14 bits, height-1 in next 14 bits
        let w = 100u32;
        let h = 50u32;
        let bits: u32 = (w - 1) | ((h - 1) << 14);
        data[21..25].copy_from_slice(&bits.to_le_bytes());
        assert_eq!(image_dimensions(&data), Some((w, h)));
    }

    // ── image_dimensions: JPEG ────────────────────────────────────────────────

    #[test]
    fn image_dimensions_jpeg_sof0() {
        // Minimal JPEG: SOI marker + SOF0 segment
        let mut data = vec![
            0xFF, 0xD8, // SOI
            0xFF, 0xC0, // SOF0 marker
            0x00, 0x11, // segment length = 17 (unused by parser after marker check)
            0x08, // precision (unused)
        ];
        data.extend_from_slice(&480u16.to_be_bytes()); // height at offset [i+5..i+7]
        data.extend_from_slice(&640u16.to_be_bytes()); // width  at offset [i+7..i+9]
        assert_eq!(image_dimensions(&data), Some((640, 480)));
    }

    // ── image_dimensions: edge cases ─────────────────────────────────────────

    #[test]
    fn image_dimensions_unrecognized_format_returns_none() {
        assert_eq!(image_dimensions(b"this is not an image"), None);
    }

    // ── provider_priority ────────────────────────────────────────────────────

    #[test]
    fn provider_priority_known_names() {
        assert_eq!(provider_priority("Hardcover"), 0);
        assert_eq!(provider_priority("Google Books"), 1);
        assert_eq!(provider_priority("Open Library"), 2);
    }

    #[test]
    fn provider_priority_unknown_name_returns_max() {
        assert_eq!(provider_priority("Some Unknown Provider"), u8::MAX);
    }

    // ── cover_min_side ───────────────────────────────────────────────────────

    #[test]
    fn cover_min_side_returns_smaller_dimension() {
        // Use a PNG header for a 100×200 image
        let mut data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        data.extend_from_slice(&[0x00; 8]); // padding to reach offset 16
        data.extend_from_slice(&100u32.to_be_bytes()); // width = 100
        data.extend_from_slice(&200u32.to_be_bytes()); // height = 200
        assert_eq!(cover_min_side(&data), 100);
    }

    #[test]
    fn cover_min_side_unknown_format_returns_zero() {
        assert_eq!(cover_min_side(b"garbage"), 0);
    }
}
