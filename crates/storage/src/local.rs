use std::path::{Path, PathBuf};

use async_trait::async_trait;
use bb_core::{
    Error,
    book::{BookToken, FileFormat},
    storage::FileStoreService,
};
use bb_utils::hash::hash_file;
use image::{ImageReader, codecs::jpeg::JpegEncoder, imageops::FilterType};

pub struct LocalFileStore {
    library_path: PathBuf,
}

impl LocalFileStore {
    #[must_use]
    pub fn new(library_path: PathBuf) -> Self {
        Self { library_path }
    }

    fn originals_dir(&self) -> PathBuf {
        self.library_path.join("Originals")
    }

    fn trash_dir(&self) -> PathBuf {
        self.library_path.join("Trash")
    }

    fn bookdrop_trash_dir(&self) -> PathBuf {
        self.library_path.join("Trash").join("Bookdrop")
    }

    fn book_dir(&self, token: BookToken) -> PathBuf {
        self.library_path.join(token.to_string())
    }

    fn book_file_path(&self, token: BookToken, slug: &str, format: &FileFormat) -> PathBuf {
        self.book_dir(token).join(format!("{slug}.{}", format.extension()))
    }

    /// Returns the library-root-relative path for a book file
    /// (e.g. `"BK_XXXXX/slug.epub"`).
    fn book_file_rel_path(token: BookToken, slug: &str, format: &FileFormat) -> String {
        format!("{}/{}.{}", token, slug, format.extension())
    }
}

#[allow(clippy::needless_pass_by_value, reason = "owned error needed for map_err ergonomics")]
fn io_err(e: impl ToString) -> Error {
    Error::Infrastructure(e.to_string())
}

/// Maps an I/O error on storage-system-level operations (root dir, book dir,
/// bookdrop dir) to `Error::StorageUnavailable`. Use this when the failure
/// indicates the storage mount is gone rather than a file being absent.
#[allow(clippy::needless_pass_by_value, reason = "owned error needed for map_err ergonomics")]
fn storage_unavailable(e: impl ToString) -> Error {
    Error::StorageUnavailable(e.to_string())
}

/// Creates all directories in `path`. Maps errors to
/// [`Error::StorageUnavailable`] since a failure here usually indicates a
/// missing or unmounted storage volume.
async fn ensure_dir(path: &Path) -> Result<(), Error> {
    tokio::fs::create_dir_all(path).await.map_err(storage_unavailable)
}

/// Converts any recognized image format to JPEG, resizing to fit within
/// 1024×1536 if needed, and re-encodes at quality 85.
///
/// Re-encoding strips all embedded metadata (EXIF, XMP, IPTC, ICC profiles)
/// and keeps covers small enough for Kobo firmware to decode reliably.
/// Falls back to the original bytes if decoding fails.
fn normalize_to_jpeg(data: &[u8]) -> Vec<u8> {
    const MAX_W: u32 = 1024;
    const MAX_H: u32 = 1536;
    const QUALITY: u8 = 85;

    let Ok(reader) = ImageReader::new(std::io::Cursor::new(data)).with_guessed_format() else {
        return data.to_vec();
    };
    let Ok(img) = reader.decode() else {
        return data.to_vec();
    };

    let img = if img.width() > MAX_W || img.height() > MAX_H {
        img.resize(MAX_W, MAX_H, FilterType::Lanczos3)
    } else {
        img
    };

    let mut out = Vec::new();
    if let Err(e) = JpegEncoder::new_with_quality(&mut out, QUALITY).encode_image(&img) {
        tracing::warn!(error = %e, "JPEG encoding failed during cover normalization; storing original bytes");
        return data.to_vec();
    }
    out
}

/// Generates a 256×384 thumbnail from cover image bytes.
///
/// Decodes `data` as any supported image format, resizes to fit within
/// 256×384 (preserving aspect ratio), and re-encodes as JPEG at quality 80.
/// Returns `None` if the input cannot be decoded or encoding fails.
fn generate_thumbnail_bytes(data: &[u8]) -> Option<Vec<u8>> {
    const THUMB_W: u32 = 256;
    const THUMB_H: u32 = 384;
    const QUALITY: u8 = 80;

    let reader = ImageReader::new(std::io::Cursor::new(data)).with_guessed_format().ok()?;
    let img = reader.decode().ok()?;

    let img = if img.width() > THUMB_W || img.height() > THUMB_H {
        img.resize(THUMB_W, THUMB_H, FilterType::Lanczos3)
    } else {
        img
    };

    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, QUALITY).encode_image(&img).ok()?;
    Some(out)
}

#[async_trait]
impl FileStoreService for LocalFileStore {
    fn resolve(&self, relative_path: &str) -> PathBuf {
        self.library_path.join(relative_path)
    }

    fn cover_path(&self, token: BookToken) -> PathBuf {
        self.book_dir(token).join("cover.jpg")
    }

    fn thumbnail_path(&self, token: BookToken) -> PathBuf {
        self.book_dir(token).join("thumb.jpg")
    }

    fn metadata_path(&self, token: BookToken) -> PathBuf {
        self.book_dir(token).join("metadata.opf")
    }

    async fn source_file_exists(&self, path: &Path) -> bool {
        path.exists()
    }

    async fn store_original_file(&self, source_hash: &str, original_filename: &str, source: &Path) -> Result<String, Error> {
        let originals_dir = self.originals_dir();
        ensure_dir(&originals_dir).await?;

        let preferred = originals_dir.join(original_filename);

        // Check for collision with an existing file.
        if tokio::fs::try_exists(&preferred).await.map_err(io_err)? {
            let existing_hash = hash_file(&preferred).await.map_err(|e| io_err(e.to_string()))?;
            if existing_hash == source_hash {
                // Same content already stored — idempotent, nothing to do.
                return Ok(format!("Originals/{original_filename}"));
            }
            // Different content: fall back to a hash-prefixed name.
            let hash_prefix = &source_hash[..8.min(source_hash.len())];
            let filename = {
                let path = std::path::Path::new(original_filename);
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(original_filename);
                let ext = path.extension().and_then(|s| s.to_str());
                match ext {
                    Some(e) => format!("{stem}_{hash_prefix}.{e}"),
                    None => format!("{stem}_{hash_prefix}"),
                }
            };
            let dest = originals_dir.join(&filename);
            tokio::fs::copy(source, &dest).await.map_err(io_err)?;
            return Ok(format!("Originals/{filename}"));
        }

        // No collision — copy to the preferred path.
        tokio::fs::copy(source, &preferred).await.map_err(io_err)?;
        Ok(format!("Originals/{original_filename}"))
    }

    async fn store_book_file(&self, token: BookToken, slug: &str, format: FileFormat, source: &Path) -> Result<String, Error> {
        let book_dir = self.book_dir(token);
        ensure_dir(&book_dir).await?;
        let dest = self.book_file_path(token, slug, &format);
        // Try rename first (fast, same filesystem)
        if tokio::fs::rename(source, &dest).await.is_err() {
            // Fall back to copy then remove source
            tokio::fs::copy(source, &dest).await.map_err(io_err)?;
            let _ = tokio::fs::remove_file(source).await;
        }
        Ok(Self::book_file_rel_path(token, slug, &format))
    }

    async fn store_cover(&self, token: BookToken, data: &[u8]) -> Result<(), Error> {
        let book_dir = self.book_dir(token);
        ensure_dir(&book_dir).await?;
        let cover_path = self.cover_path(token);

        // Normalize all recognized image formats to JPEG: resize to fit within
        // 1024×1536 if needed, re-encode at quality 85, strip all metadata
        // (EXIF, XMP, IPTC, ICC). Non-image data (unrecognized magic bytes) is
        // stored as-is. Falls back to original bytes on any decode error.
        let is_recognized_image = data.starts_with(&[0xFF, 0xD8])
            || data.starts_with(&[0x89, 0x50, 0x4E, 0x47])
            || data.starts_with(&[0x47, 0x49, 0x46])
            || (data.len() >= 12 && data.starts_with(b"RIFF") && data.get(8..12) == Some(b"WEBP"));

        let bytes = if is_recognized_image { normalize_to_jpeg(data) } else { data.to_vec() };

        tokio::fs::write(cover_path, &bytes).await.map_err(io_err)?;

        // Generate and write thumbnail alongside the full-size cover.
        if let Some(thumb_bytes) = generate_thumbnail_bytes(&bytes) {
            let thumb_path = self.thumbnail_path(token);
            if let Err(e) = tokio::fs::write(&thumb_path, thumb_bytes).await {
                tracing::warn!(error = %e, "failed to write thumbnail for book {token}");
            }
        }

        Ok(())
    }

    async fn backfill_thumbnail(&self, token: BookToken) -> Result<(), Error> {
        let thumb_path = self.thumbnail_path(token);

        // Idempotent: skip if thumbnail already exists.
        if tokio::fs::try_exists(&thumb_path).await.unwrap_or(false) {
            return Ok(());
        }

        let cover_path = self.cover_path(token);
        let cover_bytes = match tokio::fs::read(&cover_path).await {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!("cover.jpg missing during thumbnail backfill, skipping");
                return Ok(());
            }
            Err(e) => return Err(io_err(e)),
        };

        let Some(thumb_bytes) = generate_thumbnail_bytes(&cover_bytes) else {
            tracing::warn!("could not decode cover.jpg for thumbnail backfill");
            return Ok(());
        };

        if let Err(e) = tokio::fs::write(&thumb_path, thumb_bytes).await {
            tracing::warn!(error = %e, "failed to write thumbnail during backfill");
        }

        Ok(())
    }

    async fn rename_book_files(&self, token: BookToken, old_slug: &str, new_slug: &str) -> Result<(), Error> {
        let book_dir = self.book_dir(token);
        let prefix = format!("{old_slug}.");
        let mut entries = tokio::fs::read_dir(&book_dir).await.map_err(storage_unavailable)?;
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

    async fn copy_to_trash(&self, token: BookToken, file_name: &str) -> Result<(), Error> {
        let source = self.book_dir(token).join(file_name);
        let trash_dir = self.trash_dir();
        ensure_dir(&trash_dir).await?;
        let dest = trash_dir.join(file_name);
        tokio::fs::copy(&source, &dest).await.map_err(io_err)?;
        Ok(())
    }

    async fn copy_to_bookdrop_trash(&self, source: &Path) -> Result<(), Error> {
        let trash_dir = self.bookdrop_trash_dir();
        ensure_dir(&trash_dir).await?;
        let file_name = source.file_name().ok_or_else(|| io_err("source path has no filename"))?;
        let dest = trash_dir.join(file_name);
        tokio::fs::copy(source, &dest).await.map_err(io_err)?;
        Ok(())
    }

    async fn delete_book(&self, token: BookToken) -> Result<(), Error> {
        let book_dir = self.book_dir(token);
        match tokio::fs::remove_dir_all(&book_dir).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io_err(e)),
        }
    }

    async fn delete_original_file(&self, relative_path: &str) -> Result<(), Error> {
        let path = self.resolve(relative_path);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io_err(e)),
        }
    }

    async fn list_files(&self, path: &Path) -> Result<Vec<PathBuf>, Error> {
        let mut entries = tokio::fs::read_dir(path).await.map_err(storage_unavailable)?;
        let mut files = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(io_err)? {
            if entry.file_type().await.map_err(io_err)?.is_file() {
                files.push(entry.path());
            }
        }
        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use bb_core::{
        book::{BookToken, FileFormat},
        storage::FileStoreService,
    };
    use tempfile::tempdir;

    use super::LocalFileStore;

    fn test_store(library_path: std::path::PathBuf) -> LocalFileStore {
        LocalFileStore::new(library_path)
    }

    fn test_token() -> BookToken {
        BookToken::new(1)
    }

    #[tokio::test]
    async fn store_book_file_creates_at_expected_path() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());
        let token = test_token();

        // Create a source file to move
        let source = dir.path().join("source.epub");
        tokio::fs::write(&source, b"epub content").await.unwrap();

        let rel_path = store.store_book_file(token, "my-book", FileFormat::Epub, &source).await.unwrap();

        let expected = store.resolve(&rel_path);
        assert!(expected.exists(), "book file should exist at {expected:?}");
        let contents = tokio::fs::read(&expected).await.unwrap();
        assert_eq!(contents, b"epub content");
    }

    #[tokio::test]
    async fn store_book_file_returns_correct_relative_path() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());
        let token = test_token();

        let source = dir.path().join("source.epub");
        tokio::fs::write(&source, b"epub content").await.unwrap();

        let rel_path = store.store_book_file(token, "my-book", FileFormat::Epub, &source).await.unwrap();

        assert_eq!(rel_path, format!("{token}/my-book.epub"));
    }

    #[tokio::test]
    async fn store_original_file_returns_relative_path() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());

        let source = dir.path().join("original.epub");
        tokio::fs::write(&source, b"epub content").await.unwrap();

        let rel_path = store.store_original_file("fakehash", "original.epub", &source).await.unwrap();

        assert_eq!(rel_path, "Originals/original.epub");
        assert!(store.resolve(&rel_path).exists());
    }

    #[tokio::test]
    async fn store_cover_writes_to_cover_jpg() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());
        let token = test_token();

        let data = b"fake jpeg bytes";
        store.store_cover(token, data).await.unwrap();

        let cover = store.cover_path(token);
        assert!(cover.exists(), "cover.jpg should exist");
        let contents = tokio::fs::read(&cover).await.unwrap();
        assert_eq!(contents, data);
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

        store.rename_book_files(token, "old-slug", "new-slug").await.unwrap();

        assert!(book_dir.join("new-slug.epub").exists(), "epub should be renamed");
        assert!(book_dir.join("new-slug.pdf").exists(), "pdf should be renamed");
        assert!(!book_dir.join("old-slug.epub").exists(), "old epub should not exist");
        assert!(!book_dir.join("old-slug.pdf").exists(), "old pdf should not exist");
        // Non-matching file unchanged
        assert!(book_dir.join("cover.jpg").exists(), "cover.jpg should be untouched");
    }

    #[tokio::test]
    async fn copy_to_trash_copies_file() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());
        let token = test_token();

        let book_dir = dir.path().join(token.to_string());
        tokio::fs::create_dir_all(&book_dir).await.unwrap();
        tokio::fs::write(book_dir.join("my-book.epub"), b"enriched epub").await.unwrap();

        store.copy_to_trash(token, "my-book.epub").await.unwrap();

        let trash_file = dir.path().join("Trash").join("my-book.epub");
        assert!(trash_file.exists(), "file should exist in Trash/");
        let contents = tokio::fs::read(&trash_file).await.unwrap();
        assert_eq!(contents, b"enriched epub");
    }

    #[tokio::test]
    async fn copy_to_trash_overwrites_existing() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());
        let token = test_token();

        let book_dir = dir.path().join(token.to_string());
        tokio::fs::create_dir_all(&book_dir).await.unwrap();

        // Write an old version in Trash
        let trash_dir = dir.path().join("Trash");
        tokio::fs::create_dir_all(&trash_dir).await.unwrap();
        tokio::fs::write(trash_dir.join("my-book.epub"), b"old version").await.unwrap();

        // Write the new version in the book dir
        tokio::fs::write(book_dir.join("my-book.epub"), b"new version").await.unwrap();

        store.copy_to_trash(token, "my-book.epub").await.unwrap();

        let contents = tokio::fs::read(trash_dir.join("my-book.epub")).await.unwrap();
        assert_eq!(contents, b"new version");
    }

    #[tokio::test]
    async fn list_files_returns_only_files() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());

        let scan_dir = dir.path().join("scan");
        tokio::fs::create_dir_all(&scan_dir).await.unwrap();
        tokio::fs::write(scan_dir.join("book.epub"), b"epub").await.unwrap();
        tokio::fs::write(scan_dir.join("book.pdf"), b"pdf").await.unwrap();
        tokio::fs::create_dir_all(scan_dir.join("subdir")).await.unwrap();

        let mut files = store.list_files(&scan_dir).await.unwrap();
        files.sort();

        assert_eq!(files.len(), 2);
        assert!(files[0].ends_with("book.epub"));
        assert!(files[1].ends_with("book.pdf"));
    }

    #[tokio::test]
    async fn list_files_empty_directory() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());

        let scan_dir = dir.path().join("empty");
        tokio::fs::create_dir_all(&scan_dir).await.unwrap();

        let files = store.list_files(&scan_dir).await.unwrap();
        assert!(files.is_empty());
    }

    #[tokio::test]
    async fn list_files_nonexistent_directory_returns_error() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());

        let result = store.list_files(&dir.path().join("nope")).await;
        result.unwrap_err();
    }

    #[tokio::test]
    async fn delete_book_removes_directory() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());
        let token = test_token();

        let book_dir = dir.path().join(token.to_string());
        tokio::fs::create_dir_all(&book_dir).await.unwrap();
        tokio::fs::write(book_dir.join("test.epub"), b"data").await.unwrap();

        store.delete_book(token).await.unwrap();
        assert!(!book_dir.exists(), "book dir should be removed");

        // Second call is a no-op
        store.delete_book(token).await.unwrap();
    }

    #[tokio::test]
    async fn copy_to_bookdrop_trash_copies_to_trash_bookdrop() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());

        // Source file lives outside the library (simulates a bookdrop path)
        let source_dir = tempdir().unwrap();
        let source = source_dir.path().join("dead-line.epub");
        tokio::fs::write(&source, b"epub content").await.unwrap();

        store.copy_to_bookdrop_trash(&source).await.unwrap();

        let dest = dir.path().join("Trash").join("Bookdrop").join("dead-line.epub");
        assert!(dest.exists(), "file should be at Trash/Bookdrop/dead-line.epub");
        let contents = tokio::fs::read(&dest).await.unwrap();
        assert_eq!(contents, b"epub content");
        // Source is unmodified — caller removes it
        assert!(source.exists(), "source should still exist after copy");
    }

    #[tokio::test]
    async fn copy_to_bookdrop_trash_overwrites_existing() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());

        // Pre-populate Trash/Bookdrop with an old version
        let trash_dir = dir.path().join("Trash").join("Bookdrop");
        tokio::fs::create_dir_all(&trash_dir).await.unwrap();
        tokio::fs::write(trash_dir.join("book.epub"), b"old version").await.unwrap();

        let source_dir = tempdir().unwrap();
        let source = source_dir.path().join("book.epub");
        tokio::fs::write(&source, b"new version").await.unwrap();

        store.copy_to_bookdrop_trash(&source).await.unwrap();

        let contents = tokio::fs::read(trash_dir.join("book.epub")).await.unwrap();
        assert_eq!(contents, b"new version");
    }

    // ── normalize_to_jpeg helpers
    // ──────────────────────────────────────────────

    fn tiny_jpeg(width: u32, height: u32) -> Vec<u8> {
        use image::{ImageBuffer, Rgb, codecs::jpeg::JpegEncoder};
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(width, height);
        let mut out = Vec::new();
        JpegEncoder::new_with_quality(&mut out, 85).encode_image(&img).unwrap();
        out
    }

    fn tiny_png(width: u32, height: u32) -> Vec<u8> {
        use image::{ImageBuffer, ImageFormat, Rgb};
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(width, height);
        let mut out = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut out), ImageFormat::Png).unwrap();
        out
    }

    // ── normalize_to_jpeg tests
    // ────────────────────────────────────────────────

    #[test]
    fn normalize_to_jpeg_jpeg_passthrough() {
        let input = tiny_jpeg(10, 10);
        let output = super::normalize_to_jpeg(&input);
        // Output should start with JPEG magic bytes
        assert_eq!(&output[..2], &[0xFF, 0xD8], "output should be JPEG");
    }

    #[test]
    fn normalize_to_jpeg_png_converts_to_jpeg() {
        let input = tiny_png(10, 10);
        // Confirm input is PNG, not already JPEG
        assert_eq!(&input[..4], &[0x89, 0x50, 0x4E, 0x47]);
        let output = super::normalize_to_jpeg(&input);
        assert_eq!(&output[..2], &[0xFF, 0xD8], "PNG should be converted to JPEG");
    }

    #[test]
    fn normalize_to_jpeg_oversized_image_is_resized() {
        use image::ImageReader;
        // 2048×3072 is double the 1024×1536 limit
        let input = tiny_png(2048, 3072);
        let output = super::normalize_to_jpeg(&input);
        // Decode the output to check its dimensions
        let decoded = ImageReader::new(std::io::Cursor::new(&output)).with_guessed_format().unwrap().decode().unwrap();
        assert!(decoded.width() <= 1024, "width {} exceeds 1024", decoded.width());
        assert!(decoded.height() <= 1536, "height {} exceeds 1536", decoded.height());
    }

    #[test]
    fn normalize_to_jpeg_unrecognized_bytes_returned_unchanged() {
        let input = b"this is not an image at all";
        let output = super::normalize_to_jpeg(input);
        assert_eq!(output.as_slice(), input, "unrecognized bytes should pass through unchanged");
    }

    #[test]
    fn normalize_to_jpeg_corrupt_png_returned_unchanged() {
        // PNG magic header followed by garbage — decode will fail
        let mut input = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        input.extend_from_slice(b"this is garbage data that will fail to decode");
        let output = super::normalize_to_jpeg(&input);
        assert_eq!(output, input, "corrupt PNG should be returned unchanged");
    }

    // ── generate_thumbnail_bytes tests
    // ────────────────────────────────────────

    #[test]
    fn generate_thumbnail_bytes_returns_jpeg_from_jpeg() {
        let input = tiny_jpeg(500, 750);
        let output = super::generate_thumbnail_bytes(&input).expect("should produce output");
        assert_eq!(&output[..2], &[0xFF, 0xD8], "output should be JPEG");
    }

    #[test]
    fn generate_thumbnail_bytes_resizes_oversized_image() {
        use image::ImageReader;
        let input = tiny_png(1024, 1536); // exactly at the full-cover limit
        let output = super::generate_thumbnail_bytes(&input).expect("should produce thumbnail");
        let decoded = ImageReader::new(std::io::Cursor::new(&output)).with_guessed_format().unwrap().decode().unwrap();
        assert!(decoded.width() <= 256, "width {} exceeds 256", decoded.width());
        assert!(decoded.height() <= 384, "height {} exceeds 384", decoded.height());
    }

    #[test]
    fn generate_thumbnail_bytes_returns_none_for_invalid_bytes() {
        let output = super::generate_thumbnail_bytes(b"not an image at all");
        assert!(output.is_none());
    }

    #[tokio::test]
    async fn store_cover_generates_thumb_jpg_alongside_cover() {
        use image::{ImageBuffer, Rgb, codecs::jpeg::JpegEncoder};
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());
        let token = test_token();

        // Build a minimal valid JPEG so normalize_to_jpeg succeeds.
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(10, 10);
        let mut jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpeg, 85).encode_image(&img).unwrap();

        store.store_cover(token, &jpeg).await.unwrap();

        let thumb = store.thumbnail_path(token);
        assert!(thumb.exists(), "thumb.jpg should have been written alongside cover.jpg");
        let bytes = tokio::fs::read(&thumb).await.unwrap();
        assert_eq!(&bytes[..2], &[0xFF, 0xD8], "thumb should be JPEG");
    }

    #[tokio::test]
    async fn backfill_thumbnail_writes_thumb_when_missing() {
        use image::{ImageBuffer, Rgb, codecs::jpeg::JpegEncoder};

        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());
        let token = test_token();

        // Write a valid JPEG as cover.jpg
        let book_dir = dir.path().join(token.to_string());
        tokio::fs::create_dir_all(&book_dir).await.unwrap();
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(10, 10);
        let mut jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpeg, 85).encode_image(&img).unwrap();
        tokio::fs::write(book_dir.join("cover.jpg"), &jpeg).await.unwrap();

        store.backfill_thumbnail(token).await.unwrap();

        let thumb = store.thumbnail_path(token);
        assert!(thumb.exists(), "thumb.jpg should have been created");
        let bytes = tokio::fs::read(&thumb).await.unwrap();
        assert_eq!(&bytes[..2], &[0xFF, 0xD8], "thumb should be JPEG");
    }

    #[tokio::test]
    async fn backfill_thumbnail_is_noop_when_thumb_exists() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());
        let token = test_token();

        let book_dir = dir.path().join(token.to_string());
        tokio::fs::create_dir_all(&book_dir).await.unwrap();
        // Pre-populate thumb.jpg with sentinel bytes
        tokio::fs::write(book_dir.join("thumb.jpg"), b"sentinel").await.unwrap();

        store.backfill_thumbnail(token).await.unwrap();

        // Sentinel bytes should be unchanged
        let bytes = tokio::fs::read(book_dir.join("thumb.jpg")).await.unwrap();
        assert_eq!(bytes, b"sentinel", "thumb.jpg should not have been overwritten");
    }

    #[tokio::test]
    async fn backfill_thumbnail_is_noop_when_cover_missing() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());
        let token = test_token();

        // No cover.jpg exists — should return Ok without creating thumb.jpg
        let result = store.backfill_thumbnail(token).await;
        assert!(result.is_ok(), "should succeed even when cover is missing");

        let thumb = store.thumbnail_path(token);
        assert!(!thumb.exists(), "thumb.jpg should not be created when cover is missing");
    }

    #[tokio::test]
    async fn source_file_exists_returns_true_for_existing_file() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());

        let file = dir.path().join("book.epub");
        tokio::fs::write(&file, b"content").await.unwrap();

        assert!(store.source_file_exists(&file).await);
    }

    #[tokio::test]
    async fn source_file_exists_returns_false_for_missing_file() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());

        assert!(!store.source_file_exists(&dir.path().join("missing.epub")).await);
    }

    #[tokio::test]
    async fn delete_original_file_removes_file_and_is_idempotent() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());

        // Create a file in the Originals directory
        let originals = dir.path().join("Originals");
        tokio::fs::create_dir_all(&originals).await.unwrap();
        tokio::fs::write(originals.join("book.epub"), b"content").await.unwrap();

        store.delete_original_file("Originals/book.epub").await.unwrap();
        assert!(!originals.join("book.epub").exists(), "file should have been deleted");

        // Second call on a missing file must be a no-op (NotFound → Ok(()))
        store.delete_original_file("Originals/book.epub").await.unwrap();
    }

    #[tokio::test]
    async fn store_original_file_idempotent_same_content() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());

        let source = dir.path().join("book.epub");
        tokio::fs::write(&source, b"epub content").await.unwrap();

        // Compute the real hash so the idempotency branch is exercised.
        let real_hash = bb_utils::hash::hash_file(&source).await.unwrap();

        // First store — no collision, copies to Originals/book.epub.
        let path1 = store.store_original_file(&real_hash, "book.epub", &source).await.unwrap();
        assert_eq!(path1, "Originals/book.epub");

        // Second store with same hash — should return same path without
        // creating a duplicate.
        let path2 = store.store_original_file(&real_hash, "book.epub", &source).await.unwrap();
        assert_eq!(path2, "Originals/book.epub");

        // Exactly one file should exist in Originals/
        let originals = dir.path().join("Originals");
        let mut entries = tokio::fs::read_dir(&originals).await.unwrap();
        let mut count = 0;
        while entries.next_entry().await.unwrap().is_some() {
            count += 1;
        }
        assert_eq!(count, 1, "should not have created a duplicate");
    }

    #[tokio::test]
    async fn store_original_file_collision_different_content() {
        let dir = tempdir().unwrap();
        let store = test_store(dir.path().to_path_buf());

        // Store file A — no collision yet.
        let source_a = dir.path().join("book-a.epub");
        tokio::fs::write(&source_a, b"content-A").await.unwrap();
        let hash_a = bb_utils::hash::hash_file(&source_a).await.unwrap();
        store.store_original_file(&hash_a, "book.epub", &source_a).await.unwrap();

        // Store file B under the same desired filename "book.epub" — hash
        // differs → collision path.
        let source_b = dir.path().join("book-b.epub");
        tokio::fs::write(&source_b, b"content-B-different").await.unwrap();
        let hash_b = bb_utils::hash::hash_file(&source_b).await.unwrap();
        let path_b = store.store_original_file(&hash_b, "book.epub", &source_b).await.unwrap();

        // The returned path should include the first 8 chars of hash_b as a
        // suffix.
        let hash_prefix = &hash_b[..8];
        assert!(
            path_b.contains(hash_prefix),
            "collision path {path_b:?} should contain hash prefix {hash_prefix:?}"
        );
        assert!(path_b.ends_with(".epub"), "should keep the .epub extension");
        assert_ne!(path_b, "Originals/book.epub", "should not overwrite the original");

        // Both files should now exist in Originals/
        assert!(store.resolve("Originals/book.epub").exists(), "original file should still exist");
        assert!(store.resolve(&path_b).exists(), "collision file should exist at {path_b:?}");
    }
}
