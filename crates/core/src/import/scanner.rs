use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use chrono::Utc;
use tokio::sync::mpsc;
use tokio_graceful_shutdown::{IntoSubsystem, SubsystemBuilder, SubsystemHandle};

use crate::{
    Error,
    format::FormatService,
    import::{
        ImportJobService,
        service::{FileQueueStatus, ImportJobServiceImpl},
    },
    message::{MessageSeverity, NewSystemMessage, SystemMessageService},
    repository::RepositoryService,
    storage::FileStoreService,
};

// ── Channel types
// ─────────────────────────────────────────────────────────────

enum ScanCommand {
    ScanOnce,
}

/// Cloneable handle for triggering an on-demand bookdrop scan.
///
/// `trigger()` is non-blocking: if a scan is already queued (channel full),
/// the call is silently dropped — no pile-up of redundant scans.
#[derive(Clone)]
pub(super) struct ScanTrigger {
    tx: mpsc::Sender<ScanCommand>,
}

impl ScanTrigger {
    pub(super) fn trigger(&self) {
        let _ = self.tx.try_send(ScanCommand::ScanOnce);
    }
}

fn create_scan_channel() -> (ScanTrigger, mpsc::Receiver<ScanCommand>) {
    let (tx, rx) = mpsc::channel(1);
    (ScanTrigger { tx }, rx)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Returns the uppercased extension string if `path` has a file extension that
/// belongs to a known e-book format that BookBoss does not currently support
/// for import (MOBI, AZW3, PDF, CBZ). Returns `None` for truly unrecognised
/// extensions so the scanner can distinguish "wrong format" from "not an
/// e-book".
fn unsupported_ebook_extension(path: &std::path::Path) -> Option<&str> {
    match path.extension()?.to_str()? {
        ext @ ("mobi" | "azw3" | "pdf" | "cbz") => Some(ext),
        _ => None,
    }
}

// ── ScanWorker ───────────────────────────────────────────────────────────────

/// Executes bookdrop scans when commanded via the channel.
struct ScanWorker {
    bookdrop_path: PathBuf,
    scan_rx: mpsc::Receiver<ScanCommand>,
    import_job_service: Arc<dyn ImportJobService>,
    file_store: Arc<dyn FileStoreService>,
    format_service: Arc<dyn FormatService>,
    system_message_service: Arc<dyn SystemMessageService>,
}

impl ScanWorker {
    async fn scan_once(&self) {
        let files = match self.file_store.list_files(&self.bookdrop_path).await {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(path = %self.bookdrop_path.display(), error = %e, "cannot read bookdrop directory");
                return;
            }
        };

        for path in files {
            let Some(file_format) = self.format_service.detect_format(&path) else {
                if let Some(ext) = unsupported_ebook_extension(&path) {
                    let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    let message = format!(
                        r#""{file_name}" is a {ext} file. Only EPUB files can be imported. Removed from bookdrop."#,
                        ext = ext.to_uppercase()
                    );
                    tracing::info!(%message, "unsupported format removed from bookdrop");
                    if let Err(e) = self
                        .system_message_service
                        .add_message(NewSystemMessage {
                            source_task: "bookdrop.scanner".into(),
                            severity: MessageSeverity::Warning,
                            message,
                        })
                        .await
                    {
                        tracing::warn!(error = %e, "failed to post unsupported-format system message");
                    }
                    self.move_to_bookdrop_trash(&path).await;
                } else {
                    tracing::debug!(path = %path.display(), "skipping unrecognised file extension");
                }
                continue;
            };

            if let Err(e) = self.process_file(&path, file_format).await {
                tracing::warn!(path = %path.display(), error = %e, "failed to process file — skipping");
            }
        }
    }

    /// Copies `path` to `Trash/Bookdrop/` then deletes the original.
    /// Both steps are best-effort: failures are logged but not propagated.
    async fn move_to_bookdrop_trash(&self, path: &Path) {
        if let Err(e) = self.file_store.copy_to_bookdrop_trash(path).await {
            tracing::warn!(path = %path.display(), error = %e, "failed to copy file to Trash/Bookdrop");
        }
        if let Err(e) = tokio::fs::remove_file(path).await {
            tracing::warn!(path = %path.display(), error = %e, "failed to remove bookdrop file after trash copy");
        }
    }

    async fn process_file(&self, path: &Path, file_format: crate::book::FileFormat) -> Result<(), Error> {
        let hash = bb_utils::hash::hash_file(path)
            .await
            .map_err(|e| Error::Infrastructure(format!("file hashing failed: {e}")))?;

        let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let file_path_str = path.to_string_lossy().into_owned();
        let detected_at = Utc::now();

        let status = self.import_job_service.queue_file_if_new(file_path_str, hash, file_format, detected_at).await?;

        match status {
            FileQueueStatus::DuplicateLibraryFile { title, author } => {
                let message = format!(r#""{file_name}" is already in your library – {author} / {title}. Moved to Trash."#);
                tracing::info!(%message, "duplicate bookdrop file trashed");
                if let Err(e) = self
                    .system_message_service
                    .add_message(NewSystemMessage {
                        source_task: "bookdrop.scanner".into(),
                        severity: MessageSeverity::Info,
                        message,
                    })
                    .await
                {
                    tracing::warn!(error = %e, "failed to post duplicate-library system message");
                }
                self.move_to_bookdrop_trash(path).await;
            }
            FileQueueStatus::ActivelyProcessing => {
                // The pipeline worker is currently processing this exact file.
                // Leave it in place so store_original_file and store_book_file
                // can still access it.
                tracing::debug!(path = %path.display(), "file is actively being processed — skipping");
            }
            FileQueueStatus::DuplicateIncomingQueue => {
                let message = format!(r#""{file_name}" is already in the Incoming Review list. Moved to Trash."#);
                tracing::info!(%message, "duplicate bookdrop file trashed");
                if let Err(e) = self
                    .system_message_service
                    .add_message(NewSystemMessage {
                        source_task: "bookdrop.scanner".into(),
                        severity: MessageSeverity::Info,
                        message,
                    })
                    .await
                {
                    tracing::warn!(error = %e, "failed to post duplicate-queue system message");
                }
                self.move_to_bookdrop_trash(path).await;
            }
            FileQueueStatus::Queued => {}
        }

        Ok(())
    }
}

impl IntoSubsystem<Error> for ScanWorker {
    async fn run(mut self, subsys: &mut SubsystemHandle) -> Result<(), Error> {
        tracing::info!(directory = %self.bookdrop_path.display(), "scan worker started");

        loop {
            tokio::select! {
                () = subsys.on_shutdown_requested() => {
                    tracing::info!("ScanWorker shutting down...");
                    break;
                }
                cmd = self.scan_rx.recv() => {
                    match cmd {
                        Some(ScanCommand::ScanOnce) => self.scan_once().await,
                        None => {
                            tracing::warn!("ScanWorker channel closed — shutting down");
                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

// ── BookdropScanner ──────────────────────────────────────────────────────────

/// Fires a `ScanOnce` command on a fixed timer interval.
///
/// Decoupled from scan execution: the actual scan is performed by `ScanWorker`.
struct BookdropScanner {
    poll_interval: Duration,
    scan_trigger: ScanTrigger,
}

impl IntoSubsystem<Error> for BookdropScanner {
    async fn run(self, subsys: &mut SubsystemHandle) -> Result<(), Error> {
        tracing::info!("bookdrop scanner timer started");

        let mut counter: u32 = 0;

        loop {
            tokio::select! {
                () = subsys.on_shutdown_requested() => {
                    tracing::info!("BookdropScanner shutting down...");
                    break;
                }
                () = async {} => {
                    if counter == 0 {
                        self.scan_trigger.trigger();
                    }
                    counter += 1;
                    #[expect(clippy::cast_possible_truncation, reason = "poll interval in seconds fits in u32; no sane interval exceeds ~136 years")]
                    let poll_secs = self.poll_interval.as_secs() as u32;
                    if counter >= poll_secs {
                        counter = 0;
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }

        Ok(())
    }
}

// ── BookdropScanSubsystem ────────────────────────────────────────────────────

/// Owns the bookdrop scan wiring: startup recovery, the scan worker, and the
/// periodic trigger timer.
pub(crate) struct BookdropScanSubsystem {
    import_job_service: Arc<dyn ImportJobService>,
    scan_worker: ScanWorker,
    scanner: BookdropScanner,
}

impl IntoSubsystem<Error> for BookdropScanSubsystem {
    async fn run(self, subsys: &mut SubsystemHandle) -> Result<(), Error> {
        self.import_job_service.recover_on_startup().await?;

        subsys.start(SubsystemBuilder::new("ScanWorker", self.scan_worker.into_subsystem()));
        subsys.start(SubsystemBuilder::new("BookdropScanner", self.scanner.into_subsystem()));

        tracing::info!("BookdropScanSubsystem started");

        subsys.on_shutdown_requested().await;
        tracing::info!("BookdropScanSubsystem shutting down...");

        Ok(())
    }
}

/// Creates an [`ImportJobService`] and its paired [`BookdropScanSubsystem`].
///
/// The scan channel and trigger wiring are internal implementation details —
/// callers never see `ScanTrigger` or `ScanReceiver`.
#[must_use]
pub(crate) fn create_bookdrop_scan_subsystem(
    repository_service: Arc<RepositoryService>,
    file_store: Arc<dyn FileStoreService>,
    format_service: Arc<dyn FormatService>,
    system_message_service: Arc<dyn SystemMessageService>,
    bookdrop_path: PathBuf,
    scan_interval: Duration,
) -> (Arc<dyn ImportJobService>, BookdropScanSubsystem) {
    let (trigger, scan_rx) = create_scan_channel();
    let import_job_service: Arc<dyn ImportJobService> = Arc::new(ImportJobServiceImpl::new(
        repository_service,
        Some(trigger.clone()),
        Some(bookdrop_path.clone()),
    ));
    let scan_worker = ScanWorker {
        bookdrop_path,
        scan_rx,
        import_job_service: import_job_service.clone(),
        file_store,
        format_service,
        system_message_service,
    };
    let scanner = BookdropScanner {
        poll_interval: scan_interval,
        scan_trigger: trigger,
    };
    let subsystem = BookdropScanSubsystem {
        import_job_service: import_job_service.clone(),
        scan_worker,
        scanner,
    };
    (import_job_service, subsystem)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tempfile::tempdir;
    use tokio::sync::mpsc;

    use super::{ScanCommand, ScanWorker};
    use crate::{
        book::FileFormat,
        format::MockFormatService,
        import::service::{FileQueueStatus, MockImportJobService},
        message::{MessageSeverity, service::MockSystemMessageService},
        storage::MockFileStoreService,
    };

    fn make_worker(import_svc: MockImportJobService, file_store: MockFileStoreService, msg_svc: MockSystemMessageService) -> ScanWorker {
        let (_tx, scan_rx) = mpsc::channel::<ScanCommand>(1);
        ScanWorker {
            bookdrop_path: std::path::PathBuf::from("/tmp"),
            scan_rx,
            import_job_service: Arc::new(import_svc),
            file_store: Arc::new(file_store),
            format_service: Arc::new(MockFormatService::new()),
            system_message_service: Arc::new(msg_svc),
        }
    }

    /// Returns a `MockFileStoreService` that expects exactly one
    /// `copy_to_bookdrop_trash` call and returns `Ok(())`.
    fn expect_one_bookdrop_trash_copy() -> MockFileStoreService {
        let mut store = MockFileStoreService::new();
        store.expect_copy_to_bookdrop_trash().once().returning(|_| Box::pin(async { Ok(()) }));
        store
    }

    fn ok_system_message(id: u64, source_task: String, severity: MessageSeverity, message: String) -> crate::message::SystemMessage {
        crate::message::SystemMessage {
            id,
            source_task,
            severity,
            message,
            created_at: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_process_file_duplicate_library_file_moves_to_trash() {
        let dir = tempdir().expect("tempdir");
        let file_path = dir.path().join("dune.epub");
        tokio::fs::write(&file_path, b"fake epub content").await.expect("write file");

        let mut import_svc = MockImportJobService::new();
        import_svc.expect_queue_file_if_new().returning(|_, _, _, _| {
            Box::pin(async {
                Ok(FileQueueStatus::DuplicateLibraryFile {
                    title: "Dune".into(),
                    author: "Frank Herbert".into(),
                })
            })
        });

        let mut msg_svc = MockSystemMessageService::new();
        msg_svc
            .expect_add_message()
            .withf(|msg| {
                msg.severity == MessageSeverity::Info
                    && msg.message.contains("dune.epub")
                    && msg.message.contains("Frank Herbert")
                    && msg.message.contains("Dune")
                    && msg.message.contains("Moved to Trash")
            })
            .once()
            .returning(|msg| Box::pin(async move { Ok(ok_system_message(1, msg.source_task, msg.severity, msg.message)) }));

        let worker = make_worker(import_svc, expect_one_bookdrop_trash_copy(), msg_svc);
        worker.process_file(&file_path, FileFormat::Epub).await.expect("process_file");

        assert!(!file_path.exists(), "duplicate file should have been removed from bookdrop");
    }

    #[tokio::test]
    async fn test_process_file_actively_processing_does_not_remove_file() {
        let dir = tempdir().expect("tempdir");
        let file_path = dir.path().join("in-flight.epub");
        tokio::fs::write(&file_path, b"fake epub content").await.expect("write file");

        let mut import_svc = MockImportJobService::new();
        import_svc
            .expect_queue_file_if_new()
            .returning(|_, _, _, _| Box::pin(async { Ok(FileQueueStatus::ActivelyProcessing) }));

        // No copy_to_bookdrop_trash calls expected
        let worker = make_worker(import_svc, MockFileStoreService::new(), MockSystemMessageService::new());
        worker.process_file(&file_path, FileFormat::Epub).await.expect("process_file");

        assert!(file_path.exists(), "in-flight file must not be deleted");
    }

    #[tokio::test]
    async fn test_process_file_duplicate_incoming_queue_moves_to_trash() {
        let dir = tempdir().expect("tempdir");
        let file_path = dir.path().join("already-queued.epub");
        tokio::fs::write(&file_path, b"fake epub content").await.expect("write file");

        let mut import_svc = MockImportJobService::new();
        import_svc
            .expect_queue_file_if_new()
            .returning(|_, _, _, _| Box::pin(async { Ok(FileQueueStatus::DuplicateIncomingQueue) }));

        let mut msg_svc = MockSystemMessageService::new();
        msg_svc
            .expect_add_message()
            .withf(|msg| {
                msg.severity == MessageSeverity::Info
                    && msg.message.contains("already-queued.epub")
                    && msg.message.contains("Incoming Review")
                    && msg.message.contains("Moved to Trash")
            })
            .once()
            .returning(|msg| Box::pin(async move { Ok(ok_system_message(2, msg.source_task, msg.severity, msg.message)) }));

        let worker = make_worker(import_svc, expect_one_bookdrop_trash_copy(), msg_svc);
        worker.process_file(&file_path, FileFormat::Epub).await.expect("process_file");

        assert!(!file_path.exists(), "duplicate file should have been removed from bookdrop");
    }

    // ── unsupported_ebook_extension
    // ───────────────────────────────────────────

    #[test]
    fn unsupported_extension_recognised() {
        use std::path::Path;

        use super::unsupported_ebook_extension;

        assert_eq!(unsupported_ebook_extension(Path::new("book.mobi")), Some("mobi"));
        assert_eq!(unsupported_ebook_extension(Path::new("book.azw3")), Some("azw3"));
        assert_eq!(unsupported_ebook_extension(Path::new("book.pdf")), Some("pdf"));
        assert_eq!(unsupported_ebook_extension(Path::new("book.cbz")), Some("cbz"));
    }

    #[test]
    fn unsupported_extension_returns_none_for_supported_and_unknown() {
        use std::path::Path;

        use super::unsupported_ebook_extension;

        assert_eq!(unsupported_ebook_extension(Path::new("book.epub")), None);
        assert_eq!(unsupported_ebook_extension(Path::new("book.txt")), None);
        assert_eq!(unsupported_ebook_extension(Path::new("no_extension")), None);
    }
}
