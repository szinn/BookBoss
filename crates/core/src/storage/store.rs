use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::{
    Error,
    book::{BookToken, FileFormat},
    storage::BookSidecar,
};

#[async_trait]
pub trait LibraryStore: Send + Sync {
    // ── Path derivation (sync, no I/O) ──────────────────────────────────────

    /// Returns the full path to a book's file:
    /// `{library}/{token}/{slug}.{ext}`.
    fn book_file_path(&self, token: &BookToken, slug: &str, format: FileFormat) -> PathBuf;

    /// Returns the path to a book's cover image: `{library}/{token}/cover.jpg`.
    fn cover_path(&self, token: &BookToken) -> PathBuf;

    /// Returns the path to a book's sidecar: `{library}/{token}/metadata.opf`.
    fn metadata_path(&self, token: &BookToken) -> PathBuf;

    // ── Filesystem I/O (async) ───────────────────────────────────────────────

    /// Moves or copies the source file into the book's directory.
    async fn store_book_file(&self, token: &BookToken, slug: &str, format: FileFormat, source: &Path) -> Result<(), Error>;

    /// Writes raw bytes as the book's cover image.
    async fn store_cover(&self, token: &BookToken, data: &[u8]) -> Result<(), Error>;

    /// Serialises `sidecar` and writes it as `metadata.opf` in the book's
    /// directory.
    async fn store_metadata(&self, token: &BookToken, sidecar: &BookSidecar) -> Result<(), Error>;

    /// Renames all `{old_slug}.*` files in the book's directory to
    /// `{new_slug}.*`.
    async fn rename_book_files(&self, token: &BookToken, old_slug: &str, new_slug: &str) -> Result<(), Error>;

    /// Removes the book's entire directory and all its contents.
    async fn delete_book(&self, token: &BookToken) -> Result<(), Error>;
}
