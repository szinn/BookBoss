use std::sync::Arc;

use md5::{Digest, Md5};

use crate::{Error, book::BookId, repository::RepositoryService, with_read_only_transaction, with_transaction};

#[async_trait::async_trait]
pub trait KoReaderService: Send + Sync {
    /// Computes md5(filename) and md5(contents), inserts both as document
    /// hashes. Duplicate hashes (same hash + book_id) are silently ignored.
    async fn register_hashes(&self, book_id: BookId, filename: &str, contents: &[u8]) -> Result<(), Error>;

    /// Prefix-matches the incoming KOReader document digest against stored
    /// hashes. Returns the BookId for the first match, or None if not
    /// found.
    async fn find_book_by_digest(&self, digest_prefix: &str) -> Result<Option<BookId>, Error>;
}

pub struct KoReaderServiceImpl {
    repository_service: Arc<RepositoryService>,
}

impl KoReaderServiceImpl {
    pub fn new(repository_service: Arc<RepositoryService>) -> Self {
        Self { repository_service }
    }
}

fn md5_hex(data: &[u8]) -> String {
    let hash = Md5::digest(data);
    hash.iter().fold(String::with_capacity(32), |mut s, b| {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Replicates KOReader's `util.partialMD5` sampling strategy so that the
/// hash we store matches the document digest KOReader sends during sync.
///
/// The Lua formula is `lshift(1024, 2*i)` for i in -1..=10, using LuaJIT's
/// 32-bit integer arithmetic (equivalent to Java's `1024L << (2*i)` with
/// 64-bit wrapping). For i=-1 the shift count wraps to 62, causing 1024<<62
/// to overflow to 0, so the first chunk is always read from offset 0.
/// Subsequent offsets grow by 4× each step: 1024, 4096, 16384, 65536, …
///
/// The loop breaks on the first offset that meets or exceeds the file length —
/// unlike a skip-and-continue approach. All chunks are fed into a single
/// running MD5 accumulator (not concatenated then hashed).
fn partial_md5_hex(data: &[u8]) -> String {
    const CHUNK: usize = 1024;
    let mut md5 = Md5::new();
    for i in -1_i32..=10 {
        // (2*i).rem_euclid(64): maps -2 → 62, matching the LuaJIT/Java
        // signed-shift overflow that makes i=-1 land at offset 0.
        let shift = (2 * i).rem_euclid(64) as u32;
        let offset = (1024_u64).wrapping_shl(shift) as usize;
        if offset >= data.len() {
            break;
        }
        let end = (offset + CHUNK).min(data.len());
        md5.update(&data[offset..end]);
    }
    let hash = md5.finalize();
    hash.iter().fold(String::with_capacity(32), |mut s, b| {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
        s
    })
}

#[async_trait::async_trait]
impl KoReaderService for KoReaderServiceImpl {
    async fn register_hashes(&self, book_id: BookId, filename: &str, contents: &[u8]) -> Result<(), Error> {
        let filename_hash = md5_hex(filename.as_bytes());
        let contents_hash = md5_hex(contents);
        let partial_hash = partial_md5_hex(contents);
        let mut hashes = vec![filename_hash, contents_hash, partial_hash];
        hashes.sort_unstable();
        hashes.dedup();
        with_transaction!(self, koreader_document_hash_repository, |tx| {
            koreader_document_hash_repository.insert_hashes(tx, book_id, hashes).await
        })
    }

    async fn find_book_by_digest(&self, digest_prefix: &str) -> Result<Option<BookId>, Error> {
        let digest_prefix = digest_prefix.to_owned();
        with_read_only_transaction!(self, koreader_document_hash_repository, |tx| {
            koreader_document_hash_repository.find_book_by_digest_prefix(tx, &digest_prefix).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::koreader::repository::MockKoReaderDocumentHashRepository;

    fn create_service(mock: MockKoReaderDocumentHashRepository) -> KoReaderServiceImpl {
        let repository_service = Arc::new(
            crate::repository::testing::default_repository_service_builder()
                .koreader_document_hash_repository(Arc::new(mock))
                .build()
                .expect("all fields provided"),
        );
        KoReaderServiceImpl::new(repository_service)
    }

    #[tokio::test]
    async fn register_hashes_stores_filename_and_content_hashes() {
        let mut mock = MockKoReaderDocumentHashRepository::new();
        // "epub-bytes" is 10 bytes. partial_md5 reads from offset 0 (entire
        // file), so partial_hash == contents_hash. After dedup:
        // filename_hash + contents_hash = 2 distinct entries.
        mock.expect_insert_hashes()
            .withf(|_, book_id, hashes| *book_id == 1 && hashes.len() == 2)
            .returning(|_, _, _| Box::pin(async { Ok(()) }));
        let svc = create_service(mock);
        svc.register_hashes(1, "Foundation.epub", b"epub-bytes").await.unwrap();
    }

    #[tokio::test]
    async fn register_hashes_deduplicates_when_filename_and_content_hashes_match() {
        let mut mock = MockKoReaderDocumentHashRepository::new();
        // filename and contents are identical bytes → filename_hash ==
        // contents_hash == partial_hash (file < 1024 bytes, partial
        // reads full file). All three collapse to 1 entry after dedup.
        mock.expect_insert_hashes()
            .withf(|_, _, hashes| hashes.len() == 1)
            .returning(|_, _, _| Box::pin(async { Ok(()) }));
        let svc = create_service(mock);
        let data = b"Foundation.epub";
        svc.register_hashes(1, "Foundation.epub", data).await.unwrap();
    }

    #[tokio::test]
    async fn find_book_by_digest_returns_book_id_on_match() {
        let mut mock = MockKoReaderDocumentHashRepository::new();
        mock.expect_find_book_by_digest_prefix().returning(|_, _| Box::pin(async { Ok(Some(42)) }));
        let svc = create_service(mock);
        let result = svc.find_book_by_digest("abcd1234").await.unwrap();
        assert_eq!(result, Some(42));
    }

    #[tokio::test]
    async fn find_book_by_digest_returns_none_for_unknown_prefix() {
        let mut mock = MockKoReaderDocumentHashRepository::new();
        mock.expect_find_book_by_digest_prefix().returning(|_, _| Box::pin(async { Ok(None) }));
        let svc = create_service(mock);
        let result = svc.find_book_by_digest("0000ffff").await.unwrap();
        assert_eq!(result, None);
    }
}
