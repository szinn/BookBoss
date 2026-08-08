use std::sync::Arc;

use bb_utils::string::normalize_name;
use tracing::warn;

use crate::{
    Error, RepositoryError,
    book::{AuthorRole, Book, BookStatus, BookToken, FileRole, NewAuthor, NewGenre, NewPublisher, NewSeries, NewTag, book_slug},
    collection::BookEdit,
    event::EventService,
    filter::BookFilter,
    format::{FormatService, handler::EnrichBookFilesPayload},
    import::{ImportJobToken, ImportStatus},
    jobs::{JobService, JobServiceExt},
    library::ALL_BOOKS_LIBRARY_ID,
    repository::{RepositoryService, read_only_transaction, transaction},
    storage::{BookSidecar, FileStoreService, SidecarAuthor, SidecarFile, SidecarIdentifier, SidecarSeries},
};

pub struct CollectionStats {
    pub books: u64,
    pub authors: u64,
}

#[async_trait::async_trait]
#[cfg_attr(any(test, feature = "test-support"), mockall::automock)]
#[allow(unused_lifetimes, reason = "async_trait + mockall expansion emits a spurious 'life0 parameter")]
pub trait CollectionService: Send + Sync {
    /// Returns aggregate counts for the collection.
    async fn collection_stats(&self) -> Result<CollectionStats, Error>;

    /// Searches the library catalog using a [`BookFilter`].
    ///
    /// Only supports library-level (non-user-scoped) filter rules. Returns
    /// `Err` if the filter contains user-scoped rules such as `ReadStatus` —
    /// use a user-aware search method for those.
    ///
    /// When `library_id` is `Some`, results are scoped to books in that
    /// library. Pass `None` for full catalog access (admin use).
    async fn search_books(
        &self,
        filter: &BookFilter,
        library_id: Option<crate::library::LibraryId>,
        offset: Option<u64>,
        page_size: Option<u64>,
    ) -> Result<Vec<Book>, Error>;

    /// Permanently deletes a book and its files from the library.
    ///
    /// Removes all DB records (book, authors/identifiers join rows, and orphan
    /// authors with no remaining books) then deletes the book directory from
    /// the library store.
    async fn delete_book(&self, book_token: BookToken) -> Result<(), Error>;

    /// Approves a `NeedsReview` import job, committing the reviewer's edits to
    /// the database, transitioning the book to `Available`, and marking the
    /// import job as `Approved`.
    ///
    /// File renames (when title or primary author changed) and sidecar rewrites
    /// are performed as part of this operation.
    async fn approve_book(&self, job_token: ImportJobToken, reviewer_id: crate::user::UserId, edit: BookEdit, temp_dir: &std::path::Path) -> Result<(), Error>;

    /// Rejects a `NeedsReview` import job, cleaning up all associated
    /// artifacts: removes the library directory, deletes the candidate book
    /// record, and deletes the import job record so the file can be
    /// re-imported if dropped again.
    async fn reject_book(&self, job_token: ImportJobToken) -> Result<(), Error>;

    /// Saves edits directly to an existing library book without going through
    /// the import pipeline. Intended for editing books already in the library.
    ///
    /// `cover_key` is the key used to look up any fetched cover bytes in the
    /// temp store (written there by `fetch_from_provider`). File renames and
    /// sidecar rewrites are performed when the title or primary author changes.
    async fn edit_book(&self, book_token: BookToken, edit: BookEdit, cover_key: &str, temp_dir: &std::path::Path) -> Result<(), Error>;

    /// Replaces the cover image for a book (candidate or library book).
    ///
    /// Writes `cover_bytes` to the book's directory as `cover.jpg`, normalizing
    /// the image to JPEG in the storage layer. Sets `book.has_cover = true` in
    /// the database.
    async fn replace_cover(&self, book_token: BookToken, cover_bytes: Vec<u8>) -> Result<(), Error>;
}

pub struct CollectionServiceImpl {
    repository_service: Arc<RepositoryService>,
    file_store: Arc<dyn FileStoreService>,
    format_service: Arc<dyn FormatService>,
    job_service: Arc<dyn JobService>,
    event_service: Arc<dyn EventService>,
}

impl CollectionServiceImpl {
    pub(crate) fn new(
        repository_service: Arc<RepositoryService>,
        file_store: Arc<dyn FileStoreService>,
        format_service: Arc<dyn FormatService>,
        job_service: Arc<dyn JobService>,
        event_service: Arc<dyn EventService>,
    ) -> Self {
        Self {
            repository_service,
            file_store,
            format_service,
            job_service,
            event_service,
        }
    }
}

#[async_trait::async_trait]
impl CollectionService for CollectionServiceImpl {
    async fn collection_stats(&self) -> Result<CollectionStats, Error> {
        let library_repo = self.repository_service.collection_repository().clone();
        read_only_transaction(&**self.repository_service.repository(), |tx| {
            Box::pin(async move {
                let books = library_repo.count_available_books(tx).await?;
                let authors = library_repo.count_authors(tx).await?;
                Ok(CollectionStats { books, authors })
            })
        })
        .await
    }

    async fn search_books(
        &self,
        filter: &BookFilter,
        library_id: Option<crate::library::LibraryId>,
        offset: Option<u64>,
        page_size: Option<u64>,
    ) -> Result<Vec<Book>, Error> {
        if filter.contains_user_scoped_rules() {
            return Err(Error::Validation(
                "library search does not support user-scoped filter rules such as ReadStatus".to_string(),
            ));
        }
        let filter = filter.clone();
        let library_repo = self.repository_service.collection_repository().clone();
        read_only_transaction(&**self.repository_service.repository(), |tx| {
            Box::pin(async move { library_repo.books_for_filter(tx, &filter, 0, library_id, offset, page_size, None).await })
        })
        .await
    }

    async fn delete_book(&self, book_token: BookToken) -> Result<(), Error> {
        let book_repo = self.repository_service.book_repository().clone();
        let author_repo = self.repository_service.author_repository().clone();
        let job_repo = self.repository_service.import_job_repository().clone();

        let (original_filenames, enriched_filename) = transaction(&**self.repository_service.repository(), |tx| {
            let br = book_repo.clone();
            let ar = author_repo.clone();
            let jr = job_repo.clone();
            Box::pin(async move {
                let book = br
                    .find_by_token(tx, book_token)
                    .await?
                    .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

                let author_links = br.authors_for_book(tx, book.id).await?;
                let author_ids: Vec<u64> = author_links.iter().map(|a| a.author_id).collect();

                // Collect file paths before deleting records.
                let files = br.files_for_book(tx, book.id).await?;
                let original_filenames: Vec<String> = files.iter().filter(|f| f.file_role == FileRole::Original).map(|f| f.path.clone()).collect();
                let enriched_filename: Option<String> = files
                    .iter()
                    .find(|f| f.file_role == FileRole::Enriched)
                    .and_then(|f| std::path::Path::new(&f.path).file_name().and_then(|n| n.to_str()).map(String::from));

                // Delete the originating import job so the file can be re-imported.
                if let Some(job) = jr.find_by_candidate_book_id(tx, book.id).await? {
                    jr.delete_job(tx, job.id).await?;
                }

                br.delete_book_authors(tx, book.id).await?;
                br.delete_book_identifiers(tx, book.id).await?;
                br.delete_book(tx, book.id).await?;

                for author_id in author_ids {
                    if br.count_books_for_author(tx, author_id).await? == 0 {
                        ar.delete_author(tx, author_id).await?;
                    }
                }

                Ok((original_filenames, enriched_filename))
            })
        })
        .await?;

        // Best-effort: copy the enriched file to Trash before deleting.
        if let Some(ref file_name) = enriched_filename
            && let Err(e) = self.file_store.copy_to_trash(book_token, file_name).await
        {
            warn!(book_token = %book_token, file_name, error = %e, "failed to copy enriched file to Trash");
        }

        self.file_store.delete_book(book_token).await?;
        for filename in original_filenames {
            self.file_store.delete_original_file(&filename).await?;
        }

        Ok(())
    }

    async fn approve_book(&self, job_token: ImportJobToken, reviewer_id: crate::user::UserId, edit: BookEdit, temp_dir: &std::path::Path) -> Result<(), Error> {
        // ── 1. Load job and guard status ──────────────────────────────────────
        let import_job_repo = self.repository_service.import_job_repository().clone();
        let job = read_only_transaction(&**self.repository_service.repository(), |tx| {
            Box::pin(async move { import_job_repo.find_by_token(tx, job_token).await })
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

        let old_slug = book_slug(&book.title, old_first_author_name.as_deref());

        // ── 3. Read temp cover if requested ───────────────────────────────────
        let cover_data: Option<Vec<u8>> = if edit.use_fetched_cover {
            let cover_path = temp_dir.join("bookboss-covers").join(job_token.to_string());
            match tokio::fs::read(&cover_path).await {
                Ok(data) => Some(data),
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
        let genre_repo = self.repository_service.genre_repository().clone();
        let tag_repo = self.repository_service.tag_repository().clone();
        let import_job_repo2 = self.repository_service.import_job_repository().clone();
        let library_repo = self.repository_service.library_repository().clone();
        let job_id = job.id;
        let has_new_cover = cover_data.is_some();
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
                if has_new_cover {
                    updated_book.has_cover = true;
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
                    #[expect(
                        clippy::cast_possible_truncation,
                        clippy::cast_possible_wrap,
                        reason = "author list index; books have far fewer authors than i32::MAX"
                    )]
                    let sort_order = sort_order as i32;
                    book_repo2.add_book_author(tx, book_id, author.id, AuthorRole::Author, sort_order).await?;
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

                // Replace genres. Dedupe by resolved genre id: find_by_name is case-
                // insensitive while the trimmed input preserves case, so two source
                // strings differing only in case resolve to the same row and would
                // violate (book_id, genre_id).
                book_repo2.delete_book_genres(tx, book_id).await?;
                let mut seen_genre_ids = std::collections::HashSet::new();
                for name in &edit_c.genres {
                    let name = name.trim();
                    if name.is_empty() {
                        continue;
                    }
                    let genre = match genre_repo.find_by_name(tx, name).await? {
                        Some(g) => g,
                        None => genre_repo.add_genre(tx, NewGenre { name: name.to_string() }).await?,
                    };
                    if !seen_genre_ids.insert(genre.id) {
                        continue;
                    }
                    book_repo2.add_book_genre(tx, book_id, genre.id).await?;
                }

                // Replace tags. Same case-insensitive find_by_name caveat as genres above.
                book_repo2.delete_book_tags(tx, book_id).await?;
                let mut seen_tag_ids = std::collections::HashSet::new();
                for name in &edit_c.tags {
                    let name = name.trim();
                    if name.is_empty() {
                        continue;
                    }
                    let tag = match tag_repo.find_by_name(tx, name).await? {
                        Some(t) => t,
                        None => tag_repo.add_tag(tx, NewTag { name: name.to_string() }).await?,
                    };
                    if !seen_tag_ids.insert(tag.id) {
                        continue;
                    }
                    book_repo2.add_book_tag(tx, book_id, tag.id).await?;
                }

                // Add book to the All Books virtual library
                library_repo.add_book_to_library(tx, ALL_BOOKS_LIBRARY_ID, book_id).await?;

                // Mark import job approved
                import_job_repo2.approve_job(tx, job_id, reviewer_id).await?;

                Ok(())
            })
        })
        .await?;

        // ── 5. Store fetched cover ────────────────────────────────────────────
        if let Some(cover_bytes) = cover_data {
            self.file_store.store_cover(book.token, &cover_bytes).await?;
            // Clean up temp file
            let cover_path = temp_dir.join("bookboss-covers").join(job_token.to_string());
            let _ = tokio::fs::remove_file(&cover_path).await;
        }

        // ── 6. Rename book files if slug changed ──────────────────────────────
        let new_first_author = edit.authors.first().map(|a| normalize_name(a));
        let new_slug = book_slug(&normalize_name(&edit.title), new_first_author.as_deref());

        if old_slug != new_slug {
            self.file_store.rename_book_files(book.token, &old_slug, &new_slug).await?;

            // Update DB paths for enriched files to match the new slug.
            let book_repo_rename = self.repository_service.book_repository().clone();
            let old_slug_c = old_slug.clone();
            let new_slug_c = new_slug.clone();
            transaction(&**self.repository_service.repository(), |tx| {
                let br = book_repo_rename.clone();
                let old = old_slug_c.clone();
                let new = new_slug_c.clone();
                Box::pin(async move { br.update_enriched_paths(tx, book_id, &old, &new).await })
            })
            .await?;
        }

        // ── 7. Rewrite metadata sidecar ───────────────────────────────────────
        let sidecar_authors: Vec<SidecarAuthor> = edit
            .authors
            .iter()
            .filter(|n| !n.is_empty())
            .enumerate()
            .map(|(i, name)| {
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_possible_wrap,
                    reason = "author list index; books have far fewer authors than i32::MAX"
                )]
                let sort_order = i as i32;
                SidecarAuthor {
                    name: normalize_name(name),
                    role: AuthorRole::Author,
                    sort_order,
                    file_as: None,
                }
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
            genres: edit.genres.iter().map(|g| g.trim().to_string()).filter(|g| !g.is_empty()).collect(),
            tags: edit.tags.iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect(),
            page_count: edit.page_count,
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
        let sidecar_path = self.file_store.metadata_path(book.token);
        self.format_service.write_sidecar(&sidecar_path, &sidecar).await?;

        // ── 8. Enqueue book file enrichment ─────────────────────────────────
        self.job_service.enqueue(&EnrichBookFilesPayload { book_id }).await?;

        self.event_service.notify_incoming_changed();
        self.event_service.notify_jobs_changed();

        Ok(())
    }

    async fn reject_book(&self, job_token: ImportJobToken) -> Result<(), Error> {
        let import_job_repo = self.repository_service.import_job_repository().clone();
        let job = read_only_transaction(&**self.repository_service.repository(), |tx| {
            Box::pin(async move { import_job_repo.find_by_token(tx, job_token).await })
        })
        .await?
        .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

        if job.status != ImportStatus::NeedsReview {
            return Err(Error::Validation(format!("cannot reject job with status {:?}", job.status)));
        }

        // Collect cleanup targets, then delete DB records in one transaction.
        let (book_token, original_filenames) = if let Some(book_id) = job.candidate_book_id {
            let book_repo = self.repository_service.book_repository().clone();
            let author_repo = self.repository_service.author_repository().clone();

            transaction(&**self.repository_service.repository(), |tx| {
                Box::pin(async move {
                    let Some(book) = book_repo.find_by_id(tx, book_id).await? else {
                        return Ok((None, vec![]));
                    };

                    let author_links = book_repo.authors_for_book(tx, book.id).await?;
                    let author_ids: Vec<u64> = author_links.iter().map(|a| a.author_id).collect();

                    // Collect original file paths before the records are deleted.
                    let original_filenames: Vec<String> = book_repo
                        .files_for_book(tx, book.id)
                        .await?
                        .into_iter()
                        .filter(|f| f.file_role == FileRole::Original)
                        .map(|f| f.path)
                        .collect();

                    book_repo.delete_book_genres(tx, book.id).await?;
                    book_repo.delete_book_tags(tx, book.id).await?;
                    book_repo.delete_book_authors(tx, book.id).await?;
                    book_repo.delete_book_identifiers(tx, book.id).await?;
                    book_repo.delete_book(tx, book.id).await?;

                    // Orphan author cleanup.
                    for author_id in author_ids {
                        if book_repo.count_books_for_author(tx, author_id).await? == 0 {
                            author_repo.delete_author(tx, author_id).await?;
                        }
                    }

                    Ok((Some(book.token), original_filenames))
                })
            })
            .await?
        } else {
            (None, vec![])
        };

        // Delete the import job — candidate_book_id was SET NULL by FK when the
        // book record was deleted above, so the job is still findable by ID.
        let import_job_repo = self.repository_service.import_job_repository().clone();
        let job_id = job.id;
        transaction(&**self.repository_service.repository(), |tx| {
            Box::pin(async move { import_job_repo.delete_job(tx, job_id).await })
        })
        .await?;

        // Delete library files after all DB work is done.
        if let Some(token) = book_token {
            self.file_store.delete_book(token).await?;
        }
        for filename in original_filenames {
            self.file_store.delete_original_file(&filename).await?;
        }

        self.event_service.notify_incoming_changed();

        Ok(())
    }

    async fn edit_book(&self, book_token: BookToken, edit: BookEdit, cover_key: &str, temp_dir: &std::path::Path) -> Result<(), Error> {
        // ── 1. Load book and compute old slug for rename detection ─────────────
        let book_repo = self.repository_service.book_repository().clone();
        let (book, old_authors) = read_only_transaction(&**self.repository_service.repository(), |tx| {
            let br = book_repo.clone();
            Box::pin(async move {
                let book = br
                    .find_by_token(tx, book_token)
                    .await?
                    .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;
                let book_id = book.id;
                let authors = br.authors_for_book(tx, book_id).await?;
                Ok::<_, Error>((book, authors))
            })
        })
        .await?;

        let book_id = book.id;

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

        let old_slug = book_slug(&book.title, old_first_author_name.as_deref());

        // ── 2. Read temp cover if requested ───────────────────────────────────
        let cover_data: Option<Vec<u8>> = if edit.use_fetched_cover {
            let cover_path = temp_dir.join("bookboss-covers").join(cover_key);
            match tokio::fs::read(&cover_path).await {
                Ok(data) => Some(data),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    tracing::warn!(cover_key, "use_fetched_cover requested but no temp cover file found; skipping");
                    None
                }
                Err(e) => return Err(Error::Infrastructure(format!("failed to read temp cover: {e}"))),
            }
        } else {
            None
        };

        // ── 3. DB transaction: update book ────────────────────────────────────
        let book_repo2 = self.repository_service.book_repository().clone();
        let author_repo = self.repository_service.author_repository().clone();
        let series_repo = self.repository_service.series_repository().clone();
        let publisher_repo = self.repository_service.publisher_repository().clone();
        let genre_repo = self.repository_service.genre_repository().clone();
        let tag_repo = self.repository_service.tag_repository().clone();
        let has_new_cover = cover_data.is_some();
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
                if has_new_cover {
                    updated_book.has_cover = true;
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
                    #[expect(
                        clippy::cast_possible_truncation,
                        clippy::cast_possible_wrap,
                        reason = "author list index; books have far fewer authors than i32::MAX"
                    )]
                    let sort_order = sort_order as i32;
                    book_repo2.add_book_author(tx, book_id, author.id, AuthorRole::Author, sort_order).await?;
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

                // Replace genres. Dedupe by resolved genre id: find_by_name is case-
                // insensitive while the trimmed input preserves case, so two source
                // strings differing only in case resolve to the same row and would
                // violate (book_id, genre_id).
                book_repo2.delete_book_genres(tx, book_id).await?;
                let mut seen_genre_ids = std::collections::HashSet::new();
                for name in &edit_c.genres {
                    let name = name.trim();
                    if name.is_empty() {
                        continue;
                    }
                    let genre = match genre_repo.find_by_name(tx, name).await? {
                        Some(g) => g,
                        None => genre_repo.add_genre(tx, NewGenre { name: name.to_string() }).await?,
                    };
                    if !seen_genre_ids.insert(genre.id) {
                        continue;
                    }
                    book_repo2.add_book_genre(tx, book_id, genre.id).await?;
                }

                // Replace tags. Same case-insensitive find_by_name caveat as genres above.
                book_repo2.delete_book_tags(tx, book_id).await?;
                let mut seen_tag_ids = std::collections::HashSet::new();
                for name in &edit_c.tags {
                    let name = name.trim();
                    if name.is_empty() {
                        continue;
                    }
                    let tag = match tag_repo.find_by_name(tx, name).await? {
                        Some(t) => t,
                        None => tag_repo.add_tag(tx, NewTag { name: name.to_string() }).await?,
                    };
                    if !seen_tag_ids.insert(tag.id) {
                        continue;
                    }
                    book_repo2.add_book_tag(tx, book_id, tag.id).await?;
                }

                Ok(())
            })
        })
        .await?;

        // ── 4. Store fetched cover ─────────────────────────────────────────────
        if let Some(cover_bytes) = cover_data {
            self.file_store.store_cover(book.token, &cover_bytes).await?;
            let cover_path = temp_dir.join("bookboss-covers").join(cover_key);
            let _ = tokio::fs::remove_file(&cover_path).await;
        }

        // ── 5. Rename book files if slug changed ──────────────────────────────
        let new_first_author = edit.authors.first().map(|a| normalize_name(a));
        let new_slug = book_slug(&normalize_name(&edit.title), new_first_author.as_deref());

        if old_slug != new_slug {
            self.file_store.rename_book_files(book.token, &old_slug, &new_slug).await?;

            // Update DB paths for enriched files to match the new slug.
            let book_repo_rename = self.repository_service.book_repository().clone();
            let old_slug_c = old_slug.clone();
            let new_slug_c = new_slug.clone();
            transaction(&**self.repository_service.repository(), |tx| {
                let br = book_repo_rename.clone();
                let old = old_slug_c.clone();
                let new = new_slug_c.clone();
                Box::pin(async move { br.update_enriched_paths(tx, book_id, &old, &new).await })
            })
            .await?;
        }

        // ── 6. Rewrite metadata sidecar ───────────────────────────────────────
        let sidecar_authors: Vec<SidecarAuthor> = edit
            .authors
            .iter()
            .filter(|n| !n.is_empty())
            .enumerate()
            .map(|(i, name)| {
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_possible_wrap,
                    reason = "author list index; books have far fewer authors than i32::MAX"
                )]
                let sort_order = i as i32;
                SidecarAuthor {
                    name: normalize_name(name),
                    role: AuthorRole::Author,
                    sort_order,
                    file_as: None,
                }
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
            genres: edit.genres.iter().map(|g| g.trim().to_string()).filter(|g| !g.is_empty()).collect(),
            tags: edit.tags.iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect(),
            page_count: edit.page_count,
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
        let sidecar_path = self.file_store.metadata_path(book.token);
        self.format_service.write_sidecar(&sidecar_path, &sidecar).await?;

        // ── 7. Enqueue book file enrichment ─────────────────────────────────
        self.job_service.enqueue(&EnrichBookFilesPayload { book_id }).await?;

        self.event_service.notify_jobs_changed();

        Ok(())
    }

    async fn replace_cover(&self, book_token: BookToken, cover_bytes: Vec<u8>) -> Result<(), Error> {
        // ── 1. Find book ──────────────────────────────────────────────────────
        let book_repo = self.repository_service.book_repository().clone();
        let book = read_only_transaction(&**self.repository_service.repository(), |tx| {
            Box::pin(async move { book_repo.find_by_token(tx, book_token).await })
        })
        .await?
        .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

        let book_id = book.id;
        let book_version = book.version;

        // ── 2. Store cover (normalization happens in the storage layer) ───────
        self.file_store.store_cover(book_token, &cover_bytes).await?;

        // ── 3. Set has_cover = true in DB ────────────────────────────────────
        let book_repo2 = self.repository_service.book_repository().clone();
        transaction(&**self.repository_service.repository(), |tx| {
            Box::pin(async move {
                let mut updated_book = book_repo2
                    .find_by_id(tx, book_id)
                    .await?
                    .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;
                if updated_book.version != book_version {
                    return Err(Error::RepositoryError(RepositoryError::Conflict));
                }
                updated_book.has_cover = true;
                book_repo2.update_book(tx, updated_book).await
            })
        })
        .await?;

        // ── 4. Enqueue book file enrichment ─────────────────────────────────
        self.job_service.enqueue(&EnrichBookFilesPayload { book_id }).await?;

        self.event_service.notify_jobs_changed();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{CollectionService, CollectionServiceImpl};
    use crate::{
        Error, RepositoryError,
        book::{
            AuthorId, BookAuthor, BookFile, BookId, BookStatus, BookToken, FileRole,
            repository::{author::MockAuthorRepository, book::MockBookRepository},
        },
        collection::MockCollectionRepository,
        import::{ImportJob, ImportJobId, ImportJobToken, ImportStatus, repository::import_job::MockImportJobRepository},
        library::MockLibraryRepository,
        storage::store::MockFileStoreService,
        test_support::{nop_event_service, nop_format_service, nop_job_service},
    };

    // ─── Service builder ──────────────────────────────────────────────────────

    fn create_service(
        book_repo: MockBookRepository,
        author_repo: MockAuthorRepository,
        job_repo: MockImportJobRepository,
        collection_repo: MockCollectionRepository,
        file_store: MockFileStoreService,
    ) -> CollectionServiceImpl {
        create_service_with_library_repo(book_repo, author_repo, job_repo, collection_repo, MockLibraryRepository::new(), file_store)
    }

    fn create_service_with_library_repo(
        book_repo: MockBookRepository,
        author_repo: MockAuthorRepository,
        job_repo: MockImportJobRepository,
        collection_repo: MockCollectionRepository,
        library_repo: MockLibraryRepository,
        file_store: MockFileStoreService,
    ) -> CollectionServiceImpl {
        let repository_service = Arc::new(
            crate::repository::testing::default_repository_service_builder()
                .author_repository(Arc::new(author_repo))
                .book_repository(Arc::new(book_repo))
                .import_job_repository(Arc::new(job_repo))
                .collection_repository(Arc::new(collection_repo))
                .library_repository(Arc::new(library_repo))
                .build()
                .expect("all fields provided"),
        );
        CollectionServiceImpl::new(
            repository_service,
            Arc::new(file_store),
            nop_format_service(),
            nop_job_service(),
            nop_event_service(),
        )
    }

    fn fake_book_with_id(id: BookId) -> crate::book::Book {
        crate::book::Book::fake(id, "Test Book", BookStatus::Available)
    }

    fn fake_book_author_link(book_id: BookId, author_id: AuthorId) -> BookAuthor {
        BookAuthor::fake(book_id, author_id, "Author", 0)
    }

    fn fake_import_job(id: ImportJobId, book_id: BookId) -> ImportJob {
        ImportJob {
            id,
            version: 1,
            token: ImportJobToken::new(id),
            file_path: "/watch/test.epub".to_owned(),
            file_hash: "abc123".to_owned(),
            file_format: crate::book::FileFormat::Epub,
            detected_at: chrono::Utc::now(),
            status: ImportStatus::Approved,
            candidate_book_id: Some(book_id),
            metadata_source: None,
            error_message: None,
            reviewed_by: None,
            reviewed_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    // ─── collection_stats ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn collection_stats_returns_counts() {
        let mut library_repo = MockCollectionRepository::new();
        library_repo.expect_count_available_books().returning(|_| Box::pin(async { Ok(5) }));
        library_repo.expect_count_authors().returning(|_| Box::pin(async { Ok(3) }));

        let svc = create_service(
            MockBookRepository::new(),
            MockAuthorRepository::new(),
            MockImportJobRepository::new(),
            library_repo,
            MockFileStoreService::new(),
        );

        let stats = svc.collection_stats().await.unwrap();

        assert_eq!(stats.books, 5);
        assert_eq!(stats.authors, 3);
    }

    #[tokio::test]
    async fn collection_stats_returns_zeroes_when_empty() {
        let mut library_repo = MockCollectionRepository::new();
        library_repo.expect_count_available_books().returning(|_| Box::pin(async { Ok(0) }));
        library_repo.expect_count_authors().returning(|_| Box::pin(async { Ok(0) }));

        let svc = create_service(
            MockBookRepository::new(),
            MockAuthorRepository::new(),
            MockImportJobRepository::new(),
            library_repo,
            MockFileStoreService::new(),
        );

        let stats = svc.collection_stats().await.unwrap();

        assert_eq!(stats.books, 0);
        assert_eq!(stats.authors, 0);
    }

    // ─── delete_book ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn delete_book_returns_not_found_when_book_missing() {
        let mut book_repo = MockBookRepository::new();
        book_repo.expect_find_by_token().returning(|_, _| Box::pin(async { Ok(None) }));

        let svc = create_service(
            book_repo,
            MockAuthorRepository::new(),
            MockImportJobRepository::new(),
            MockCollectionRepository::new(),
            MockFileStoreService::new(),
        );
        let token = BookToken::new(99);

        let result = svc.delete_book(token).await;

        assert!(
            matches!(result, Err(Error::RepositoryError(RepositoryError::NotFound))),
            "expected NotFound, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn delete_book_succeeds_with_no_authors_no_import_job() {
        let book_id: BookId = 1;
        let book = fake_book_with_id(book_id);

        let mut book_repo = MockBookRepository::new();
        book_repo.expect_find_by_token().returning(move |_, _| {
            let b = book.clone();
            Box::pin(async move { Ok(Some(b)) })
        });
        book_repo.expect_authors_for_book().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        book_repo.expect_files_for_book().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        book_repo.expect_delete_book_authors().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_delete_book_identifiers().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_delete_book().returning(|_, _| Box::pin(async { Ok(()) }));

        let mut job_repo = MockImportJobRepository::new();
        job_repo.expect_find_by_candidate_book_id().returning(|_, _| Box::pin(async { Ok(None) }));

        let mut store = MockFileStoreService::new();
        store.expect_delete_book().returning(|_| Box::pin(async { Ok(()) }));

        let svc = create_service(book_repo, MockAuthorRepository::new(), job_repo, MockCollectionRepository::new(), store);
        let token = BookToken::new(book_id);

        svc.delete_book(token).await.unwrap();
    }

    #[tokio::test]
    async fn delete_book_removes_orphan_author() {
        let book_id: BookId = 1;
        let author_id: AuthorId = 42;
        let book = fake_book_with_id(book_id);
        let link = fake_book_author_link(book_id, author_id);

        let mut book_repo = MockBookRepository::new();
        book_repo.expect_find_by_token().returning(move |_, _| {
            let b = book.clone();
            Box::pin(async move { Ok(Some(b)) })
        });
        book_repo.expect_authors_for_book().returning(move |_, _| {
            let l = link.clone();
            Box::pin(async move { Ok(vec![l]) })
        });
        book_repo.expect_files_for_book().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        book_repo.expect_delete_book_authors().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_delete_book_identifiers().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_delete_book().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_count_books_for_author().returning(|_, _| Box::pin(async { Ok(0) })); // no remaining books → orphan

        let mut author_repo = MockAuthorRepository::new();
        author_repo
            .expect_delete_author()
            .withf(move |_, id| *id == author_id)
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));

        let mut job_repo = MockImportJobRepository::new();
        job_repo.expect_find_by_candidate_book_id().returning(|_, _| Box::pin(async { Ok(None) }));

        let mut store = MockFileStoreService::new();
        store.expect_delete_book().returning(|_| Box::pin(async { Ok(()) }));

        let svc = create_service(book_repo, author_repo, job_repo, MockCollectionRepository::new(), store);
        let token = BookToken::new(book_id);

        svc.delete_book(token).await.unwrap();
        // `.times(1)` on `delete_author` verifies the orphan was deleted when
        // the mock is dropped
    }

    #[tokio::test]
    async fn delete_book_preserves_author_with_other_books() {
        let book_id: BookId = 1;
        let author_id: AuthorId = 42;
        let book = fake_book_with_id(book_id);
        let link = fake_book_author_link(book_id, author_id);

        let mut book_repo = MockBookRepository::new();
        book_repo.expect_find_by_token().returning(move |_, _| {
            let b = book.clone();
            Box::pin(async move { Ok(Some(b)) })
        });
        book_repo.expect_authors_for_book().returning(move |_, _| {
            let l = link.clone();
            Box::pin(async move { Ok(vec![l]) })
        });
        book_repo.expect_files_for_book().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        book_repo.expect_delete_book_authors().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_delete_book_identifiers().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_delete_book().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_count_books_for_author().returning(|_, _| Box::pin(async { Ok(1) })); // still has 1 other book → not an orphan

        // No expectation set on delete_author — mockall will panic if it is called
        let author_repo = MockAuthorRepository::new();

        let mut job_repo = MockImportJobRepository::new();
        job_repo.expect_find_by_candidate_book_id().returning(|_, _| Box::pin(async { Ok(None) }));

        let mut store = MockFileStoreService::new();
        store.expect_delete_book().returning(|_| Box::pin(async { Ok(()) }));

        let svc = create_service(book_repo, author_repo, job_repo, MockCollectionRepository::new(), store);
        let token = BookToken::new(book_id);

        svc.delete_book(token).await.unwrap();
    }

    #[tokio::test]
    async fn delete_book_removes_linked_import_job() {
        let book_id: BookId = 1;
        let job_id: ImportJobId = 99;
        let book = fake_book_with_id(book_id);
        let job = fake_import_job(job_id, book_id);

        let mut book_repo = MockBookRepository::new();
        book_repo.expect_find_by_token().returning(move |_, _| {
            let b = book.clone();
            Box::pin(async move { Ok(Some(b)) })
        });
        book_repo.expect_authors_for_book().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        book_repo.expect_files_for_book().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        book_repo.expect_delete_book_authors().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_delete_book_identifiers().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_delete_book().returning(|_, _| Box::pin(async { Ok(()) }));

        let mut job_repo = MockImportJobRepository::new();
        job_repo.expect_find_by_candidate_book_id().returning(move |_, _| {
            let j = job.clone();
            Box::pin(async move { Ok(Some(j)) })
        });
        job_repo
            .expect_delete_job()
            .withf(move |_, id| *id == job_id)
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));

        let mut store = MockFileStoreService::new();
        store.expect_delete_book().returning(|_| Box::pin(async { Ok(()) }));

        let svc = create_service(book_repo, MockAuthorRepository::new(), job_repo, MockCollectionRepository::new(), store);
        let token = BookToken::new(book_id);

        svc.delete_book(token).await.unwrap();
        // `.times(1)` on `delete_job` verifies the linked job was deleted when
        // the mock is dropped
    }

    #[tokio::test]
    async fn delete_book_deletes_original_files_from_store() {
        let book_id: BookId = 1;
        let book = fake_book_with_id(book_id);
        let mut file = BookFile::fake(book_id, "epub");
        file.path = "Originals/test.epub".to_owned();

        let mut book_repo = MockBookRepository::new();
        book_repo.expect_find_by_token().returning(move |_, _| {
            let b = book.clone();
            Box::pin(async move { Ok(Some(b)) })
        });
        book_repo.expect_authors_for_book().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        book_repo.expect_files_for_book().returning(move |_, _| {
            let f = file.clone();
            Box::pin(async move { Ok(vec![f]) })
        });
        book_repo.expect_delete_book_authors().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_delete_book_identifiers().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_delete_book().returning(|_, _| Box::pin(async { Ok(()) }));

        let mut job_repo = MockImportJobRepository::new();
        job_repo.expect_find_by_candidate_book_id().returning(|_, _| Box::pin(async { Ok(None) }));

        let mut store = MockFileStoreService::new();
        store.expect_delete_book().returning(|_| Box::pin(async { Ok(()) }));
        store
            .expect_delete_original_file()
            .withf(|path| path == "Originals/test.epub")
            .times(1)
            .returning(|_| Box::pin(async { Ok(()) }));

        let svc = create_service(book_repo, MockAuthorRepository::new(), job_repo, MockCollectionRepository::new(), store);
        let token = BookToken::new(book_id);

        svc.delete_book(token).await.unwrap();
        // `.times(1)` on `delete_original_file` verifies the file was removed
        // from the store
    }

    #[tokio::test]
    async fn delete_book_copies_enriched_file_to_trash() {
        let book_id: BookId = 1;
        let book = fake_book_with_id(book_id);

        let mut enriched = BookFile::fake(book_id, "epub");
        enriched.file_role = FileRole::Enriched;
        enriched.path = "BK_00001/my-book.epub".to_owned();

        let mut book_repo = MockBookRepository::new();
        book_repo.expect_find_by_token().returning(move |_, _| {
            let b = book.clone();
            Box::pin(async move { Ok(Some(b)) })
        });
        book_repo.expect_authors_for_book().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        book_repo.expect_files_for_book().returning(move |_, _| {
            let f = enriched.clone();
            Box::pin(async move { Ok(vec![f]) })
        });
        book_repo.expect_delete_book_authors().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_delete_book_identifiers().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_delete_book().returning(|_, _| Box::pin(async { Ok(()) }));

        let mut job_repo = MockImportJobRepository::new();
        job_repo.expect_find_by_candidate_book_id().returning(|_, _| Box::pin(async { Ok(None) }));

        let mut store = MockFileStoreService::new();
        store
            .expect_copy_to_trash()
            .withf(|_, name| name == "my-book.epub")
            .times(1)
            .returning(|_, _| Box::pin(async { Ok(()) }));
        store.expect_delete_book().returning(|_| Box::pin(async { Ok(()) }));

        let svc = create_service(book_repo, MockAuthorRepository::new(), job_repo, MockCollectionRepository::new(), store);
        let token = BookToken::new(book_id);

        svc.delete_book(token).await.unwrap();
        // `.times(1)` on `copy_to_trash` verifies it was called
    }

    #[tokio::test]
    async fn delete_book_skips_trash_when_no_enriched_file() {
        let book_id: BookId = 1;
        let book = fake_book_with_id(book_id);

        let mut original = BookFile::fake(book_id, "epub");
        original.path = "Originals/test.epub".to_owned();

        let mut book_repo = MockBookRepository::new();
        book_repo.expect_find_by_token().returning(move |_, _| {
            let b = book.clone();
            Box::pin(async move { Ok(Some(b)) })
        });
        book_repo.expect_authors_for_book().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        book_repo.expect_files_for_book().returning(move |_, _| {
            let f = original.clone();
            Box::pin(async move { Ok(vec![f]) })
        });
        book_repo.expect_delete_book_authors().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_delete_book_identifiers().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_delete_book().returning(|_, _| Box::pin(async { Ok(()) }));

        let mut job_repo = MockImportJobRepository::new();
        job_repo.expect_find_by_candidate_book_id().returning(|_, _| Box::pin(async { Ok(None) }));

        let mut store = MockFileStoreService::new();
        // No expectation on copy_to_trash — mockall panics if it is called
        store.expect_delete_book().returning(|_| Box::pin(async { Ok(()) }));
        store.expect_delete_original_file().returning(|_| Box::pin(async { Ok(()) }));

        let svc = create_service(book_repo, MockAuthorRepository::new(), job_repo, MockCollectionRepository::new(), store);
        let token = BookToken::new(book_id);

        svc.delete_book(token).await.unwrap();
    }

    #[tokio::test]
    async fn delete_book_succeeds_when_trash_copy_fails() {
        let book_id: BookId = 1;
        let book = fake_book_with_id(book_id);

        let mut enriched = BookFile::fake(book_id, "epub");
        enriched.file_role = FileRole::Enriched;
        enriched.path = "BK_00001/my-book.epub".to_owned();

        let mut book_repo = MockBookRepository::new();
        book_repo.expect_find_by_token().returning(move |_, _| {
            let b = book.clone();
            Box::pin(async move { Ok(Some(b)) })
        });
        book_repo.expect_authors_for_book().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        book_repo.expect_files_for_book().returning(move |_, _| {
            let f = enriched.clone();
            Box::pin(async move { Ok(vec![f]) })
        });
        book_repo.expect_delete_book_authors().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_delete_book_identifiers().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_delete_book().returning(|_, _| Box::pin(async { Ok(()) }));

        let mut job_repo = MockImportJobRepository::new();
        job_repo.expect_find_by_candidate_book_id().returning(|_, _| Box::pin(async { Ok(None) }));

        let mut store = MockFileStoreService::new();
        store
            .expect_copy_to_trash()
            .returning(|_, _| Box::pin(async { Err(crate::Error::Infrastructure("disk full".to_owned())) }));
        store.expect_delete_book().returning(|_| Box::pin(async { Ok(()) }));

        let svc = create_service(book_repo, MockAuthorRepository::new(), job_repo, MockCollectionRepository::new(), store);
        let token = BookToken::new(book_id);

        // Should succeed despite trash copy failure
        svc.delete_book(token).await.unwrap();
    }

    // ─── replace_cover ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn replace_cover_enqueues_enrichment() {
        use crate::jobs::service::MockJobService;

        let book_id: BookId = 42;
        let book = fake_book_with_id(book_id);

        let mut book_repo = MockBookRepository::new();
        let book_for_token = book.clone();
        book_repo.expect_find_by_token().returning(move |_, _| {
            let b = book_for_token.clone();
            Box::pin(async move { Ok(Some(b)) })
        });
        let book_for_id = book.clone();
        book_repo.expect_find_by_id().returning(move |_, _| {
            let b = book_for_id.clone();
            Box::pin(async move { Ok(Some(b)) })
        });
        book_repo.expect_update_book().returning(|_, b| Box::pin(async move { Ok(b) }));

        let mut store = MockFileStoreService::new();
        store.expect_store_cover().returning(|_, _| Box::pin(async { Ok(()) }));

        let mut job_svc = MockJobService::new();
        job_svc.expect_enqueue_raw().once().returning(|_, _, _| Box::pin(async { Ok(()) }));

        let repository_service = Arc::new(
            crate::repository::testing::default_repository_service_builder()
                .book_repository(Arc::new(book_repo))
                .import_job_repository(Arc::new(MockImportJobRepository::new()))
                .collection_repository(Arc::new(MockCollectionRepository::new()))
                .build()
                .expect("all fields provided"),
        );
        let svc = CollectionServiceImpl::new(
            repository_service,
            Arc::new(store),
            nop_format_service(),
            Arc::new(job_svc),
            nop_event_service(),
        );

        svc.replace_cover(BookToken::new(book_id), vec![0u8; 4]).await.unwrap();
    }

    // ─── search_books ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn search_books_rejects_user_scoped_filter() {
        use crate::filter::{BookFilter, FilterReadStatus, FilterRule, SetOp};
        let svc = create_service(
            MockBookRepository::new(),
            MockAuthorRepository::new(),
            MockImportJobRepository::new(),
            MockCollectionRepository::new(),
            MockFileStoreService::new(),
        );
        let filter = BookFilter::Rule(FilterRule::ReadStatus {
            op: SetOp::IncludesAny,
            values: vec![FilterReadStatus::Active],
        });
        let result = svc.search_books(&filter, None, None, None).await;
        assert!(matches!(result, Err(Error::Validation(_))));
    }

    // ─── approve_book ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn approve_book_rejects_non_needs_review_status() {
        use crate::collection::BookEdit;
        let token = ImportJobToken::new(42);
        let mut import_mock = MockImportJobRepository::new();
        import_mock.expect_find_by_token().returning(move |_, _| {
            let job = ImportJob {
                id: 42,
                version: 1,
                token: ImportJobToken::new(42),
                file_path: "/watch/test.epub".to_owned(),
                file_hash: "abc".to_owned(),
                file_format: crate::book::FileFormat::Epub,
                detected_at: chrono::Utc::now(),
                status: ImportStatus::Approved,
                candidate_book_id: None,
                metadata_source: None,
                error_message: None,
                reviewed_by: None,
                reviewed_at: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            Box::pin(async move { Ok(Some(job)) })
        });
        let svc = create_service(
            MockBookRepository::new(),
            MockAuthorRepository::new(),
            import_mock,
            MockCollectionRepository::new(),
            MockFileStoreService::new(),
        );
        let edit = BookEdit {
            title: "Test".to_owned(),
            description: None,
            published_date: None,
            language: None,
            series_name: None,
            series_number: None,
            publisher_name: None,
            page_count: None,
            authors: vec![],
            identifiers: vec![],
            use_fetched_cover: false,
            genres: vec![],
            tags: vec![],
        };
        let result = svc.approve_book(token, 1, edit, std::path::Path::new("/tmp")).await;
        assert!(matches!(result, Err(Error::Validation(_))));
    }

    #[tokio::test]
    async fn approve_book_adds_book_to_all_books_library() {
        use crate::{
            book::repository::{genre::MockGenreRepository, publisher::MockPublisherRepository, series::MockSeriesRepository, tag::MockTagRepository},
            collection::BookEdit,
            format::MockFormatService,
            jobs::service::MockJobService,
            library::ALL_BOOKS_LIBRARY_ID,
        };

        let book_id: BookId = 7;
        let job_id: ImportJobId = 11;
        let book = crate::book::Book::fake(book_id, "My Book", BookStatus::Available);
        let job_token = ImportJobToken::new(job_id);

        // ── Import job: NeedsReview ──────────────────────────────────────────
        let mut import_mock = MockImportJobRepository::new();
        import_mock.expect_find_by_token().returning(move |_, _| {
            let job = ImportJob {
                id: job_id,
                version: 1,
                token: ImportJobToken::new(job_id),
                file_path: "/watch/test.epub".to_owned(),
                file_hash: "abc".to_owned(),
                file_format: crate::book::FileFormat::Epub,
                detected_at: chrono::Utc::now(),
                status: ImportStatus::NeedsReview,
                candidate_book_id: Some(book_id),
                metadata_source: None,
                error_message: None,
                reviewed_by: None,
                reviewed_at: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            Box::pin(async move { Ok(Some(job)) })
        });
        import_mock.expect_approve_job().returning(|_, _, _| Box::pin(async { Ok(()) }));

        // ── Book repo ────────────────────────────────────────────────────────
        let mut book_repo = MockBookRepository::new();
        let b_for_id = book.clone();
        book_repo.expect_find_by_id().returning(move |_, _| {
            let b = b_for_id.clone();
            Box::pin(async move { Ok(Some(b)) })
        });
        let b_for_token = book.clone();
        book_repo.expect_find_by_token().returning(move |_, _| {
            let b = b_for_token.clone();
            Box::pin(async move { Ok(Some(b)) })
        });
        book_repo.expect_authors_for_book().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        book_repo.expect_update_book().returning(|_, b| Box::pin(async move { Ok(b) }));
        book_repo.expect_delete_book_authors().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_delete_book_identifiers().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_delete_book_genres().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_delete_book_tags().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_files_for_book().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        book_repo.expect_update_enriched_paths().returning(|_, _, _, _| Box::pin(async { Ok(()) }));

        // ── Library repo: verify add_book_to_library is called correctly ─────
        let mut library_mock = MockLibraryRepository::new();
        library_mock
            .expect_add_book_to_library()
            .withf(move |_, lib_id, bk_id| *lib_id == ALL_BOOKS_LIBRARY_ID && *bk_id == book_id)
            .times(1)
            .returning(|_, _, _| Box::pin(async { Ok(()) }));

        // ── File store ───────────────────────────────────────────────────────
        let mut store = MockFileStoreService::new();
        store.expect_metadata_path().returning(|_| std::path::PathBuf::from("/tmp/meta.opf"));

        // ── Format service ───────────────────────────────────────────────────
        let mut format_svc = MockFormatService::new();
        format_svc.expect_write_sidecar().returning(|_, _| Box::pin(async { Ok(()) }));

        // ── Job service ──────────────────────────────────────────────────────
        let mut job_svc = MockJobService::new();
        job_svc.expect_enqueue_raw().once().returning(|_, _, _| Box::pin(async { Ok(()) }));

        let repository_service = Arc::new(
            crate::repository::testing::default_repository_service_builder()
                .book_repository(Arc::new(book_repo))
                .author_repository(Arc::new(MockAuthorRepository::new()))
                .series_repository(Arc::new(MockSeriesRepository::new()))
                .publisher_repository(Arc::new(MockPublisherRepository::new()))
                .genre_repository(Arc::new(MockGenreRepository::new()))
                .tag_repository(Arc::new(MockTagRepository::new()))
                .import_job_repository(Arc::new(import_mock))
                .library_repository(Arc::new(library_mock))
                .build()
                .expect("all fields provided"),
        );

        let svc = CollectionServiceImpl::new(
            repository_service,
            Arc::new(store),
            Arc::new(format_svc),
            Arc::new(job_svc),
            nop_event_service(),
        );

        let edit = BookEdit {
            title: "My Book".to_owned(),
            description: None,
            published_date: None,
            language: None,
            series_name: None,
            series_number: None,
            publisher_name: None,
            page_count: None,
            authors: vec![],
            identifiers: vec![],
            use_fetched_cover: false,
            genres: vec![],
            tags: vec![],
        };

        svc.approve_book(job_token, 1, edit, std::path::Path::new("/tmp")).await.unwrap();
        // `.times(1)` on `add_book_to_library` verifies it was called when the
        // mock is dropped
    }

    // ─── reject_book ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn reject_book_rejects_non_needs_review_status() {
        let token = ImportJobToken::new(43);
        let mut import_mock = MockImportJobRepository::new();
        import_mock.expect_find_by_token().returning(move |_, _| {
            let job = ImportJob {
                id: 43,
                version: 1,
                token: ImportJobToken::new(43),
                file_path: "/watch/test.epub".to_owned(),
                file_hash: "abc".to_owned(),
                file_format: crate::book::FileFormat::Epub,
                detected_at: chrono::Utc::now(),
                status: ImportStatus::Approved,
                candidate_book_id: None,
                metadata_source: None,
                error_message: None,
                reviewed_by: None,
                reviewed_at: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            Box::pin(async move { Ok(Some(job)) })
        });
        let svc = create_service(
            MockBookRepository::new(),
            MockAuthorRepository::new(),
            import_mock,
            MockCollectionRepository::new(),
            MockFileStoreService::new(),
        );
        let result = svc.reject_book(token).await;
        assert!(matches!(result, Err(Error::Validation(_))));
    }
}
