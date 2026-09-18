use std::{collections::HashMap, sync::Arc};

use crate::{
    CoreServices, Error,
    book::{BookId, BookStatus, FileRole},
    format::handler::EnrichBookFilesPayload,
    jobs::{JobHandler, JobRepositoryExt},
    message::{MessageSeverity, NewSystemMessage},
    repository::{read_only_transaction, transaction},
};

pub struct VerifyFileIntegrityHandler {
    core: Arc<CoreServices>,
}

impl VerifyFileIntegrityHandler {
    #[must_use]
    pub fn new(core: Arc<CoreServices>) -> Self {
        Self { core }
    }
}

impl JobHandler for VerifyFileIntegrityHandler {
    const JOB_TYPE: &'static str = "health.verify_file_integrity";
    const DISPLAY_NAME: &'static str = "Verify Library File Integrity";
    type Payload = serde_json::Value;

    async fn handle(&self, _payload: serde_json::Value) -> Result<(), Error> {
        let book_repo = self.core.repository_service.book_repository().clone();

        let all_files = read_only_transaction(&**self.core.repository_service.repository(), |tx| {
            let book_repo = book_repo.clone();
            Box::pin(async move { book_repo.list_all_book_files(tx).await })
        })
        .await?;

        // Collect missing files grouped by book_id.
        let mut missing_by_book: HashMap<BookId, Vec<(FileRole, crate::book::FileFormat)>> = HashMap::new();

        for file in &all_files {
            let abs_path = self.core.file_store.resolve(&file.path);
            if !abs_path.exists() {
                missing_by_book
                    .entry(file.book_id)
                    .or_default()
                    .push((file.file_role.clone(), file.format.clone()));
            }
        }

        if missing_by_book.is_empty() {
            tracing::info!(total = all_files.len(), "all library files verified");
            self.core
                .system_message_service
                .add_message(NewSystemMessage {
                    source_task: Self::JOB_TYPE.to_string(),
                    severity: MessageSeverity::Info,
                    message: format!("All {} library file(s) verified on disk", all_files.len()),
                })
                .await?;
            return Ok(());
        }

        let mut deleted_count = 0usize;
        let mut recovered_count = 0usize;

        for (book_id, missing_files) in missing_by_book {
            // Load the book record for title and token.
            let book = {
                let book_repo = book_repo.clone();
                read_only_transaction(&**self.core.repository_service.repository(), |tx| {
                    Box::pin(async move { book_repo.find_by_id(tx, book_id).await })
                })
                .await?
            };

            let Some(book) = book else {
                tracing::warn!(book_id, "book with missing files no longer in DB, skipping");
                continue;
            };

            let original_missing = missing_files.iter().any(|(role, _)| *role == FileRole::Original);
            let missing_enriched: Vec<crate::book::FileFormat> = missing_files
                .iter()
                .filter(|(role, _)| *role == FileRole::Enriched)
                .map(|(_, fmt)| fmt.clone())
                .collect();

            if original_missing {
                let title = book.title.clone();
                if book.status == BookStatus::Incoming {
                    // Candidate book awaiting admin review — do not
                    // auto-delete. The review screen will
                    // show an error and only allow rejection.
                    tracing::warn!(book_id, title, "import job has missing original file — admin review required");
                    self.core
                        .system_message_service
                        .add_message(NewSystemMessage {
                            source_task: Self::JOB_TYPE.to_string(),
                            severity: MessageSeverity::Warning,
                            message: format!("Import job for \"{title}\" has a missing original file — admin review required."),
                        })
                        .await?;
                } else {
                    // Library book with missing original — unrecoverable,
                    // delete it.
                    let token = book.token;
                    if let Err(e) = self.core.collection_service.delete_book(token).await {
                        tracing::error!(book_id, title, error = %e, "failed to delete unrecoverable book");
                    } else {
                        tracing::warn!(book_id, title, "deleted unrecoverable book (original file missing)");
                    }
                    self.core
                        .system_message_service
                        .add_message(NewSystemMessage {
                            source_task: Self::JOB_TYPE.to_string(),
                            severity: MessageSeverity::Warning,
                            message: format!("Book \"{title}\" had a missing original file and could not be recovered. It has been deleted."),
                        })
                        .await?;
                    deleted_count += 1;
                }
            } else if !missing_enriched.is_empty() {
                // Recoverable: remove stale enriched records and re-enqueue.
                let title = book.title.clone();
                let job_repo = self.core.repository_service.job_repository().clone();
                transaction(&**self.core.repository_service.repository(), |tx| {
                    let book_repo = book_repo.clone();
                    let job_repo = job_repo.clone();
                    let missing_enriched = missing_enriched.clone();
                    Box::pin(async move {
                        for fmt in &missing_enriched {
                            book_repo.delete_book_file_by_role(tx, book_id, fmt.clone(), FileRole::Enriched).await?;
                        }
                        job_repo.enqueue(tx, &EnrichBookFilesPayload { book_id }).await?;
                        Ok(())
                    })
                })
                .await?;
                tracing::warn!(book_id, title, "queued re-enrichment for book with missing enriched files");
                self.core
                    .system_message_service
                    .add_message(NewSystemMessage {
                        source_task: Self::JOB_TYPE.to_string(),
                        severity: MessageSeverity::Warning,
                        message: format!("Book \"{title}\" had missing enriched files. Regenerating from original."),
                    })
                    .await?;
                recovered_count += 1;

                // Also warn if the cover file is missing.
                if book.has_cover {
                    let cover_abs = self.core.file_store.cover_path(book.token);
                    if !cover_abs.exists() {
                        self.core
                            .system_message_service
                            .add_message(NewSystemMessage {
                                source_task: Self::JOB_TYPE.to_string(),
                                severity: MessageSeverity::Warning,
                                message: format!("The cover image for \"{title}\" may be incorrect — the cover file is missing."),
                            })
                            .await?;
                    }
                }
            }
        }

        tracing::warn!(
            deleted_count,
            recovered_count,
            total = deleted_count + recovered_count,
            "file integrity check found missing files"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::{
        book::{Book, BookFile, FileFormat, repository::book::MockBookRepository},
        import::repository::import_job::MockImportJobRepository,
        jobs::repository::MockJobRepository,
        message::repository::MockSystemMessageRepository,
        repository::testing::default_repository_service_builder,
        storage::MockFileStoreService,
        test_support::*,
    };

    fn fake_book_file(book_id: BookId, role: FileRole, format: FileFormat, path: &str) -> BookFile {
        BookFile {
            book_id,
            format,
            file_role: role,
            path: path.to_string(),
            file_size: 1000,
            file_hash: "hash".to_string(),
            created_at: chrono::Utc::now(),
        }
    }

    fn fake_enqueue_job() -> crate::jobs::Job {
        crate::jobs::Job {
            id: 1,
            job_type: String::new(),
            payload: serde_json::json!({}),
            status: crate::jobs::JobStatus::Pending,
            priority: 0,
            attempt: 0,
            max_attempts: 3,
            version: 0,
            scheduled_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            error_message: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn reports_info_when_all_files_exist() {
        let mut book_repo = MockBookRepository::new();
        let mut msg_repo = MockSystemMessageRepository::new();
        let mut store = MockFileStoreService::new();

        book_repo.expect_list_all_book_files().returning(|_| {
            Box::pin(std::future::ready(Ok(vec![fake_book_file(
                1,
                FileRole::Original,
                FileFormat::Epub,
                "BK_abc/book.epub",
            )])))
        });

        store
            .expect_resolve()
            .returning(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"));

        msg_repo.expect_add_message().returning(|_, msg| {
            assert_eq!(msg.severity, MessageSeverity::Info);
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
            default_external_services_builder()
                .repository_service(repo_service)
                .file_store(Arc::new(store))
                .build()
                .unwrap(),
            "test-secret",
        )
        .unwrap();

        VerifyFileIntegrityHandler::new(core).handle(serde_json::json!({})).await.unwrap();
    }

    #[tokio::test]
    async fn recoverable_missing_enriched_deletes_records_and_enqueues() {
        let book_id: BookId = 1;
        let book = Book::fake(book_id, "Test Book", BookStatus::Available);
        let original_path = "Originals/test-book.epub";
        let enriched_epub_path = "BK_1/test-book.epub";
        let enriched_kepub_path = "BK_1/test-book.kepub.epub";

        let mut book_repo = MockBookRepository::new();
        book_repo.expect_list_all_book_files().returning(move |_| {
            Box::pin(std::future::ready(Ok(vec![
                fake_book_file(book_id, FileRole::Original, FileFormat::Epub, original_path),
                fake_book_file(book_id, FileRole::Enriched, FileFormat::Epub, enriched_epub_path),
                fake_book_file(book_id, FileRole::Enriched, FileFormat::Kepub, enriched_kepub_path),
            ])))
        });

        book_repo.expect_find_by_id().returning(move |_, _| {
            let b = book.clone();
            Box::pin(async move { Ok(Some(b)) })
        });

        book_repo
            .expect_delete_book_file_by_role()
            .times(2)
            .returning(|_, _, _, _| Box::pin(async { Ok(()) }));

        let mut job_repo = MockJobRepository::new();
        job_repo
            .expect_enqueue_raw()
            .once()
            .returning(|_, _, _, _| Box::pin(std::future::ready(Ok(fake_enqueue_job()))));

        let mut msg_repo = MockSystemMessageRepository::new();
        msg_repo.expect_add_message().once().returning(|_, msg| {
            assert_eq!(msg.severity, MessageSeverity::Warning);
            assert!(msg.message.contains("Regenerating"), "expected recovery message, got: {}", msg.message);
            let msg = crate::message::SystemMessage {
                id: 1,
                source_task: msg.source_task,
                severity: msg.severity,
                message: msg.message,
                created_at: chrono::Utc::now(),
            };
            Box::pin(std::future::ready(Ok(msg)))
        });

        // original path exists; enriched paths do not
        let mut store = MockFileStoreService::new();
        store.expect_resolve().returning(move |p| {
            if p.contains("Originals") {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")
            } else {
                PathBuf::from("/nonexistent/path")
            }
        });

        let repo_service = Arc::new(
            default_repository_service_builder()
                .book_repository(Arc::new(book_repo))
                .job_repository(Arc::new(job_repo))
                .system_message_repository(Arc::new(msg_repo))
                .build()
                .unwrap(),
        );

        let core = crate::create_services(
            default_external_services_builder()
                .repository_service(repo_service)
                .file_store(Arc::new(store))
                .build()
                .unwrap(),
            "test-secret",
        )
        .unwrap();

        VerifyFileIntegrityHandler::new(core).handle(serde_json::json!({})).await.unwrap();
    }

    #[tokio::test]
    async fn missing_cover_adds_additional_warning() {
        let book_id: BookId = 1;
        let mut book = Book::fake(book_id, "Test Book", BookStatus::Available);
        book.has_cover = true;
        let original_path = "Originals/test-book.epub";
        let enriched_path = "BK_1/test-book.epub";

        let mut book_repo = MockBookRepository::new();
        book_repo.expect_list_all_book_files().returning(move |_| {
            Box::pin(std::future::ready(Ok(vec![
                fake_book_file(book_id, FileRole::Original, FileFormat::Epub, original_path),
                fake_book_file(book_id, FileRole::Enriched, FileFormat::Epub, enriched_path),
            ])))
        });

        book_repo.expect_find_by_id().returning(move |_, _| {
            let b = book.clone();
            Box::pin(async move { Ok(Some(b)) })
        });

        book_repo
            .expect_delete_book_file_by_role()
            .once()
            .returning(|_, _, _, _| Box::pin(async { Ok(()) }));

        let mut job_repo = MockJobRepository::new();
        job_repo
            .expect_enqueue_raw()
            .once()
            .returning(|_, _, _, _| Box::pin(std::future::ready(Ok(fake_enqueue_job()))));

        let mut msg_repo = MockSystemMessageRepository::new();
        // Expect two warnings: one recovery + one cover warning
        msg_repo.expect_add_message().times(2).returning(|_, msg| {
            assert_eq!(msg.severity, MessageSeverity::Warning);
            let msg = crate::message::SystemMessage {
                id: 1,
                source_task: msg.source_task,
                severity: msg.severity,
                message: msg.message,
                created_at: chrono::Utc::now(),
            };
            Box::pin(std::future::ready(Ok(msg)))
        });

        // original exists; enriched and cover missing
        let mut store = MockFileStoreService::new();
        store.expect_resolve().returning(move |p| {
            if p.contains("Originals") {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")
            } else {
                PathBuf::from("/nonexistent/path")
            }
        });
        store.expect_cover_path().returning(|_| PathBuf::from("/nonexistent/cover.jpg"));

        let repo_service = Arc::new(
            default_repository_service_builder()
                .book_repository(Arc::new(book_repo))
                .job_repository(Arc::new(job_repo))
                .system_message_repository(Arc::new(msg_repo))
                .build()
                .unwrap(),
        );

        let core = crate::create_services(
            default_external_services_builder()
                .repository_service(repo_service)
                .file_store(Arc::new(store))
                .build()
                .unwrap(),
            "test-secret",
        )
        .unwrap();

        VerifyFileIntegrityHandler::new(core).handle(serde_json::json!({})).await.unwrap();
    }

    #[tokio::test]
    async fn unrecoverable_book_is_deleted() {
        let book_id: BookId = 1;
        let book = Book::fake(book_id, "Lost Book", BookStatus::Available);
        let original_path = "Originals/lost-book.epub";

        let mut book_repo = MockBookRepository::new();
        book_repo.expect_list_all_book_files().returning(move |_| {
            Box::pin(std::future::ready(Ok(vec![fake_book_file(
                book_id,
                FileRole::Original,
                FileFormat::Epub,
                original_path,
            )])))
        });

        book_repo.expect_find_by_id().returning(move |_, _| {
            let b = book.clone();
            Box::pin(async move { Ok(Some(b)) })
        });

        // CollectionServiceImpl::delete_book internals
        book_repo.expect_find_by_token().returning(move |_, _| {
            let b = Book::fake(book_id, "Lost Book", BookStatus::Available);
            Box::pin(async move { Ok(Some(b)) })
        });
        book_repo.expect_authors_for_book().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        book_repo.expect_files_for_book().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        book_repo.expect_delete_book_authors().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_delete_book_identifiers().returning(|_, _| Box::pin(async { Ok(()) }));
        book_repo.expect_delete_book().returning(|_, _| Box::pin(async { Ok(()) }));

        let mut import_job_repo = MockImportJobRepository::new();
        import_job_repo
            .expect_find_by_candidate_book_id()
            .returning(|_, _| Box::pin(async { Ok(None) }));

        let mut msg_repo = MockSystemMessageRepository::new();
        msg_repo.expect_add_message().once().returning(|_, msg| {
            assert_eq!(msg.severity, MessageSeverity::Warning);
            assert!(msg.message.contains("deleted"), "expected deletion message, got: {}", msg.message);
            let msg = crate::message::SystemMessage {
                id: 1,
                source_task: msg.source_task,
                severity: msg.severity,
                message: msg.message,
                created_at: chrono::Utc::now(),
            };
            Box::pin(std::future::ready(Ok(msg)))
        });

        let mut store = MockFileStoreService::new();
        store.expect_resolve().returning(|_| PathBuf::from("/nonexistent/path"));
        store.expect_delete_book().returning(|_| Box::pin(async { Ok(()) }));

        let repo_service = Arc::new(
            default_repository_service_builder()
                .book_repository(Arc::new(book_repo))
                .import_job_repository(Arc::new(import_job_repo))
                .system_message_repository(Arc::new(msg_repo))
                .build()
                .unwrap(),
        );

        let core = crate::create_services(
            default_external_services_builder()
                .repository_service(repo_service)
                .file_store(Arc::new(store))
                .build()
                .unwrap(),
            "test-secret",
        )
        .unwrap();

        VerifyFileIntegrityHandler::new(core).handle(serde_json::json!({})).await.unwrap();
    }
}
