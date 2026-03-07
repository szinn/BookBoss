use std::sync::Arc;

use crate::{
    Error, RepositoryError,
    book::BookToken,
    repository::{RepositoryService, read_only_transaction, transaction},
    storage::LibraryStore,
};

pub struct LibraryStats {
    pub books: u64,
    pub authors: u64,
}

#[async_trait::async_trait]
pub trait LibraryService: Send + Sync {
    /// Returns aggregate counts for the library.
    async fn library_stats(&self) -> Result<LibraryStats, Error>;

    /// Permanently deletes a book and its files from the library.
    ///
    /// Removes all DB records (book, authors/identifiers join rows, and orphan
    /// authors with no remaining books) then deletes the book directory from
    /// the library store.
    async fn delete_book(&self, book_token: &BookToken) -> Result<(), Error>;
}

pub struct LibraryServiceImpl {
    repository_service: Arc<RepositoryService>,
    library_store: Arc<dyn LibraryStore>,
}

impl LibraryServiceImpl {
    pub(crate) fn new(repository_service: Arc<RepositoryService>, library_store: Arc<dyn LibraryStore>) -> Self {
        Self {
            repository_service,
            library_store,
        }
    }
}

#[async_trait::async_trait]
impl LibraryService for LibraryServiceImpl {
    async fn library_stats(&self) -> Result<LibraryStats, Error> {
        let book_repo = self.repository_service.book_repository().clone();
        let author_repo = self.repository_service.author_repository().clone();

        read_only_transaction(&**self.repository_service.repository(), |tx| {
            Box::pin(async move {
                let books = book_repo.count_available_books(tx).await?;
                let authors = author_repo.count_authors(tx).await?;
                Ok(LibraryStats { books, authors })
            })
        })
        .await
    }

    async fn delete_book(&self, book_token: &BookToken) -> Result<(), Error> {
        let token = *book_token;
        let book_repo = self.repository_service.book_repository().clone();
        let author_repo = self.repository_service.author_repository().clone();
        let job_repo = self.repository_service.import_job_repository().clone();

        transaction(&**self.repository_service.repository(), |tx| {
            let br = book_repo.clone();
            let ar = author_repo.clone();
            let jr = job_repo.clone();
            Box::pin(async move {
                let book = br.find_by_token(tx, &token).await?.ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

                let author_links = br.authors_for_book(tx, book.id).await?;
                let author_ids: Vec<u64> = author_links.iter().map(|a| a.author_id).collect();

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

                Ok(())
            })
        })
        .await?;

        self.library_store.delete_book(&token).await?;

        Ok(())
    }
}
