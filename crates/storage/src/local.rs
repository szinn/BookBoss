use std::path::{Path, PathBuf};

use async_trait::async_trait;
use bb_core::{
    Error,
    book::{BookToken, FileFormat},
    storage::{BookSidecar, LibraryStore},
};

pub struct LocalLibraryStore {
    library_path: PathBuf,
}

impl LocalLibraryStore {
    pub fn new(library_path: PathBuf) -> Self {
        Self { library_path }
    }

    fn book_dir(&self, token: &BookToken) -> PathBuf {
        self.library_path.join(token.to_string())
    }
}

fn format_ext(format: FileFormat) -> &'static str {
    match format {
        FileFormat::Epub => "epub",
        FileFormat::Mobi => "mobi",
        FileFormat::Azw3 => "azw3",
        FileFormat::Pdf => "pdf",
        FileFormat::Cbz => "cbz",
    }
}

fn io_err(e: impl ToString) -> Error {
    Error::Infrastructure(e.to_string())
}

#[async_trait]
impl LibraryStore for LocalLibraryStore {
    fn book_file_path(&self, token: &BookToken, slug: &str, format: FileFormat) -> PathBuf {
        self.book_dir(token).join(format!("{slug}.{}", format_ext(format)))
    }

    fn cover_path(&self, token: &BookToken) -> PathBuf {
        self.book_dir(token).join("cover.jpg")
    }

    fn metadata_path(&self, token: &BookToken) -> PathBuf {
        self.book_dir(token).join("metadata.opf")
    }

    async fn store_book_file(&self, token: &BookToken, slug: &str, format: FileFormat, source: &Path) -> Result<(), Error> {
        let book_dir = self.book_dir(token);
        tokio::fs::create_dir_all(&book_dir).await.map_err(io_err)?;
        let dest = book_dir.join(format!("{slug}.{}", format_ext(format)));
        // Try rename first (fast, same filesystem)
        if tokio::fs::rename(source, &dest).await.is_err() {
            // Fall back to copy then remove source
            tokio::fs::copy(source, &dest).await.map_err(io_err)?;
            let _ = tokio::fs::remove_file(source).await;
        }
        Ok(())
    }

    async fn store_cover(&self, token: &BookToken, data: &[u8]) -> Result<(), Error> {
        let book_dir = self.book_dir(token);
        tokio::fs::create_dir_all(&book_dir).await.map_err(io_err)?;
        let cover_path = self.cover_path(token);
        tokio::fs::write(cover_path, data).await.map_err(io_err)?;
        Ok(())
    }

    async fn store_metadata(&self, token: &BookToken, sidecar: &BookSidecar) -> Result<(), Error> {
        let book_dir = self.book_dir(token);
        tokio::fs::create_dir_all(&book_dir).await.map_err(io_err)?;
        let bytes = bb_formats::opf::write_sidecar(sidecar).map_err(|e| Error::Infrastructure(e.to_string()))?;
        let metadata_path = self.metadata_path(token);
        tokio::fs::write(metadata_path, bytes).await.map_err(io_err)?;
        Ok(())
    }

    async fn rename_book_files(&self, token: &BookToken, old_slug: &str, new_slug: &str) -> Result<(), Error> {
        let book_dir = self.book_dir(token);
        let prefix = format!("{old_slug}.");
        let mut entries = tokio::fs::read_dir(&book_dir).await.map_err(io_err)?;
        while let Some(entry) = entries.next_entry().await.map_err(io_err)? {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            if let Some(ext) = name.strip_prefix(&prefix) {
                let new_name = format!("{new_slug}.{ext}");
                tokio::fs::rename(entry.path(), book_dir.join(new_name)).await.map_err(io_err)?;
            }
        }
        Ok(())
    }

    async fn delete_book(&self, token: &BookToken) -> Result<(), Error> {
        let book_dir = self.book_dir(token);
        match tokio::fs::remove_dir_all(&book_dir).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io_err(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use bb_core::{
        book::{BookStatus, BookToken, FileFormat},
        storage::{BookSidecar, LibraryStore},
    };
    use tempfile::tempdir;

    use super::LocalLibraryStore;

    fn test_store(library_path: std::path::PathBuf) -> LocalLibraryStore {
        LocalLibraryStore::new(library_path)
    }

    fn test_token() -> BookToken {
        BookToken::new(1)
    }

    fn minimal_sidecar() -> BookSidecar {
        BookSidecar {
            title: "Test Book".to_string(),
            authors: vec![],
            description: None,
            publisher: None,
            published_date: None,
            language: None,
            identifiers: vec![],
            series: None,
            genres: vec![],
            tags: vec![],
            rating: None,
            status: BookStatus::Incoming,
            metadata_source: None,
            files: vec![],
        }
    }

    #[tokio::test]
    async fn store_book_file_creates_at_expected_path() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());
        let token = test_token();

        // Create a source file to move
        let source = dir.path().join("source.epub");
        tokio::fs::write(&source, b"epub content").await.unwrap();

        store.store_book_file(&token, "my-book", FileFormat::Epub, &source).await.unwrap();

        let expected = store.book_file_path(&token, "my-book", FileFormat::Epub);
        assert!(expected.exists(), "book file should exist at {expected:?}");
        let contents = tokio::fs::read(&expected).await.unwrap();
        assert_eq!(contents, b"epub content");
    }

    #[tokio::test]
    async fn store_cover_writes_to_cover_jpg() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());
        let token = test_token();

        let data = b"fake jpeg bytes";
        store.store_cover(&token, data).await.unwrap();

        let cover = store.cover_path(&token);
        assert!(cover.exists(), "cover.jpg should exist");
        let contents = tokio::fs::read(&cover).await.unwrap();
        assert_eq!(contents, data);
    }

    #[tokio::test]
    async fn store_metadata_writes_parseable_opf() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());
        let token = test_token();
        let sidecar = minimal_sidecar();

        store.store_metadata(&token, &sidecar).await.unwrap();

        let meta_path = store.metadata_path(&token);
        assert!(meta_path.exists(), "metadata.opf should exist");
        let bytes = tokio::fs::read(&meta_path).await.unwrap();
        let parsed = bb_formats::opf::parse_sidecar(&bytes).expect("should be parseable OPF");
        assert_eq!(parsed.title, sidecar.title);
    }

    #[tokio::test]
    async fn rename_book_files_renames_correctly() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());
        let token = test_token();

        // Create book dir and some files
        let book_dir = dir.path().join(token.to_string());
        tokio::fs::create_dir_all(&book_dir).await.unwrap();
        tokio::fs::write(book_dir.join("old-slug.epub"), b"epub").await.unwrap();
        tokio::fs::write(book_dir.join("old-slug.pdf"), b"pdf").await.unwrap();
        tokio::fs::write(book_dir.join("cover.jpg"), b"cover").await.unwrap();

        store.rename_book_files(&token, "old-slug", "new-slug").await.unwrap();

        assert!(book_dir.join("new-slug.epub").exists(), "epub should be renamed");
        assert!(book_dir.join("new-slug.pdf").exists(), "pdf should be renamed");
        assert!(!book_dir.join("old-slug.epub").exists(), "old epub should not exist");
        assert!(!book_dir.join("old-slug.pdf").exists(), "old pdf should not exist");
        // Non-matching file unchanged
        assert!(book_dir.join("cover.jpg").exists(), "cover.jpg should be untouched");
    }

    #[tokio::test]
    async fn delete_book_removes_directory() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());
        let token = test_token();

        let book_dir = dir.path().join(token.to_string());
        tokio::fs::create_dir_all(&book_dir).await.unwrap();
        tokio::fs::write(book_dir.join("test.epub"), b"data").await.unwrap();

        store.delete_book(&token).await.unwrap();
        assert!(!book_dir.exists(), "book dir should be removed");

        // Second call is a no-op
        store.delete_book(&token).await.unwrap();
    }
}
