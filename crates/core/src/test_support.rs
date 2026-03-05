use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;

use crate::{
    Error,
    book::{BookToken, FileFormat},
    import::ImportJob,
    pipeline::PipelineService,
    storage::{BookSidecar, LibraryStore},
};

/// No-op `LibraryStore` for use in tests and as a placeholder until
/// `LocalLibraryStore` is wired in during M3.8.
pub struct NopLibraryStore;

#[async_trait]
impl LibraryStore for NopLibraryStore {
    fn book_file_path(&self, _token: &BookToken, _slug: &str, _format: FileFormat) -> PathBuf {
        unimplemented!("NopLibraryStore")
    }
    fn cover_path(&self, _token: &BookToken, _filename: &str) -> PathBuf {
        unimplemented!("NopLibraryStore")
    }
    fn metadata_path(&self, _token: &BookToken) -> PathBuf {
        unimplemented!("NopLibraryStore")
    }
    async fn store_book_file(&self, _token: &BookToken, _slug: &str, _format: FileFormat, _source: &Path) -> Result<(), Error> {
        unimplemented!("NopLibraryStore")
    }
    async fn store_cover(&self, _token: &BookToken, _filename: &str, _data: &[u8]) -> Result<(), Error> {
        unimplemented!("NopLibraryStore")
    }
    async fn store_metadata(&self, _token: &BookToken, _sidecar: &BookSidecar) -> Result<(), Error> {
        unimplemented!("NopLibraryStore")
    }
    async fn rename_book_files(&self, _token: &BookToken, _old_slug: &str, _new_slug: &str) -> Result<(), Error> {
        unimplemented!("NopLibraryStore")
    }
    async fn delete_book(&self, _token: &BookToken) -> Result<(), Error> {
        unimplemented!("NopLibraryStore")
    }
}

pub fn nop_library_store() -> Arc<dyn LibraryStore> {
    Arc::new(NopLibraryStore)
}

pub struct NopPipelineService;

#[async_trait]
impl PipelineService for NopPipelineService {
    async fn process_job(&self, _job: ImportJob) -> Result<ImportJob, Error> {
        unimplemented!("NopPipelineService")
    }
}

pub fn nop_pipeline_service() -> Arc<dyn PipelineService> {
    Arc::new(NopPipelineService)
}
