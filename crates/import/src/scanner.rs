use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use bb_core::{
    Error,
    book::FileFormat,
    import::{ImportJobRepository, NewImportJob},
    jobs::{JobRepository, JobRepositoryExt},
    repository::{Repository, transaction},
};
use chrono::Utc;
use sha2::{Digest, Sha256};
use tokio_graceful_shutdown::{IntoSubsystem, SubsystemHandle};

use crate::handler::ProcessImportPayload;

pub struct LibraryScanner {
    watch_directory: PathBuf,
    poll_interval: Duration,
    repository: Arc<dyn Repository>,
    import_job_repo: Arc<dyn ImportJobRepository>,
    job_repo: Arc<dyn JobRepository>,
}

impl LibraryScanner {
    pub fn new(
        watch_directory: PathBuf,
        poll_interval: Duration,
        repository: Arc<dyn Repository>,
        import_job_repo: Arc<dyn ImportJobRepository>,
        job_repo: Arc<dyn JobRepository>,
    ) -> Self {
        Self {
            watch_directory,
            poll_interval,
            repository,
            import_job_repo,
            job_repo,
        }
    }

    async fn scan_once(&self) {
        let mut entries = match tokio::fs::read_dir(&self.watch_directory).await {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(directory = %self.watch_directory.display(), error = %e, "cannot read watch directory");
                return;
            }
        };

        loop {
            let entry = match entries.next_entry().await {
                Ok(Some(e)) => e,
                Ok(None) => break,
                Err(e) => {
                    tracing::warn!(error = %e, "error reading watch directory entry");
                    break;
                }
            };

            let path = entry.path();

            let file_type = match entry.file_type().await {
                Ok(ft) => ft,
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "cannot stat watch directory entry");
                    continue;
                }
            };

            if !file_type.is_file() {
                continue;
            }

            let Some(format) = detect_format(&path) else {
                tracing::debug!(path = %path.display(), "skipping unrecognised file extension");
                continue;
            };

            if let Err(e) = self.process_file(&path, format).await {
                tracing::warn!(path = %path.display(), error = %e, "failed to process file — skipping");
            }
        }
    }

    async fn process_file(&self, path: &Path, format: FileFormat) -> Result<(), Error> {
        let path_owned = path.to_owned();
        let hash = tokio::task::spawn_blocking(move || hash_file(&path_owned))
            .await
            .map_err(|e| Error::Infrastructure(format!("spawn_blocking join error: {e}")))?
            .map_err(|e| Error::Infrastructure(format!("file hashing failed: {e}")))?;

        let file_path_str = path.to_string_lossy().into_owned();
        let detected_at = Utc::now();

        let import_job_repo = self.import_job_repo.clone();
        let job_repo = self.job_repo.clone();
        let hash_clone = hash.clone();

        let repository = self.repository.clone();
        transaction(&*repository, |tx| {
            let import_job_repo = import_job_repo.clone();
            let job_repo = job_repo.clone();
            let file_path_str = file_path_str.clone();
            let hash = hash_clone.clone();

            Box::pin(async move {
                // Skip if this hash is already known (duplicate or already queued).
                if import_job_repo.find_by_hash(tx, &hash).await?.is_some() {
                    tracing::debug!(hash = %hash, "file already in import_jobs — skipping");
                    return Ok(());
                }

                let job = import_job_repo
                    .add_job(
                        tx,
                        NewImportJob {
                            file_path: file_path_str,
                            file_hash: hash,
                            file_format: format,
                            detected_at,
                        },
                    )
                    .await?;

                job_repo.enqueue(tx, &ProcessImportPayload { import_job_id: job.id }).await?;

                tracing::info!(import_job_token = job.token, "queued import job");
                Ok(())
            })
        })
        .await
    }
}

impl IntoSubsystem<Error> for LibraryScanner {
    async fn run(self, subsys: &mut SubsystemHandle) -> Result<(), Error> {
        tracing::info!(directory = %self.watch_directory.display(), "library scanner started");

        loop {
            tokio::select! {
                _ = subsys.on_shutdown_requested() => {
                    tracing::info!("LibraryScanner shutting down");
                    break;
                }
                _ = async {} => {
                    self.scan_once().await;
                    tokio::time::sleep(self.poll_interval).await;
                }
            }
        }

        Ok(())
    }
}

/// Detect the `FileFormat` from a file path's extension. Returns `None` for
/// unrecognised or missing extensions.
fn detect_format(path: &Path) -> Option<FileFormat> {
    match path.extension()?.to_str()? {
        "epub" => Some(FileFormat::Epub),
        "mobi" => Some(FileFormat::Mobi),
        "pdf" => Some(FileFormat::Pdf),
        "cbz" => Some(FileFormat::Cbz),
        "azw3" => Some(FileFormat::Azw3),
        _ => None,
    }
}

fn hash_file(path: &Path) -> std::io::Result<String> {
    use std::{
        fs::File,
        io::{BufReader, Read},
    };

    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];

    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn detect_format_known_extensions() {
        assert_eq!(detect_format(Path::new("book.epub")), Some(FileFormat::Epub));
        assert_eq!(detect_format(Path::new("book.mobi")), Some(FileFormat::Mobi));
        assert_eq!(detect_format(Path::new("book.pdf")), Some(FileFormat::Pdf));
        assert_eq!(detect_format(Path::new("book.cbz")), Some(FileFormat::Cbz));
        assert_eq!(detect_format(Path::new("book.azw3")), Some(FileFormat::Azw3));
    }

    #[test]
    fn detect_format_unknown_and_missing() {
        assert_eq!(detect_format(Path::new("book.txt")), None);
        assert_eq!(detect_format(Path::new("book.zip")), None);
        assert_eq!(detect_format(Path::new("no_extension")), None);
    }
}
