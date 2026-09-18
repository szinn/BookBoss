use std::sync::Arc;

use crate::{
    Error, RepositoryError,
    book::{
        Author, AuthorId, AuthorToken, Book, BookAuthor, BookFile, BookHydrationData, BookId, BookIdentifier, BookQuery, BookToken, Genre, GenreToken,
        NewGenre, NewTag, Publisher, Series, SeriesId, SeriesToken, Tag, TagToken,
    },
    format::handler::EnrichBookFilesPayload,
    jobs::{JobService, JobServiceExt},
    repository::RepositoryService,
    with_read_only_transaction, with_transaction,
};

#[async_trait::async_trait]
pub trait BookService: Send + Sync {
    async fn list_books(
        &self,
        filter: &BookQuery,
        library_id: Option<crate::library::LibraryId>,
        offset: Option<u64>,
        page_size: Option<u64>,
    ) -> Result<Vec<Book>, Error>;
    async fn find_book_by_token(&self, token: BookToken) -> Result<Option<Book>, Error>;
    async fn find_books_by_ids(&self, ids: &[BookId]) -> Result<Vec<Book>, Error>;
    async fn authors_for_book(&self, book_id: BookId) -> Result<Vec<BookAuthor>, Error>;
    async fn files_for_book(&self, book_id: BookId) -> Result<Vec<BookFile>, Error>;
    async fn identifiers_for_book(&self, book_id: BookId) -> Result<Vec<BookIdentifier>, Error>;
    async fn list_authors(&self, start_id: Option<AuthorId>, page_size: Option<u64>) -> Result<Vec<Author>, Error>;
    async fn find_author_by_token(&self, token: AuthorToken) -> Result<Option<Author>, Error>;
    async fn list_series(&self, start_id: Option<SeriesId>, page_size: Option<u64>) -> Result<Vec<Series>, Error>;
    async fn find_series_by_token(&self, token: SeriesToken) -> Result<Option<Series>, Error>;
    async fn find_publisher_by_token(&self, token: crate::book::PublisherToken) -> Result<Option<crate::book::Publisher>, Error>;
    async fn genres_for_book(&self, book_id: BookId) -> Result<Vec<Genre>, Error>;
    async fn tags_for_book(&self, book_id: BookId) -> Result<Vec<Tag>, Error>;
    async fn list_all_genres(&self) -> Result<Vec<Genre>, Error>;
    async fn list_all_tags(&self) -> Result<Vec<Tag>, Error>;
    async fn create_genre(&self, name: String) -> Result<Genre, Error>;
    async fn create_tag(&self, name: String) -> Result<Tag, Error>;
    async fn delete_genre(&self, token: GenreToken) -> Result<(), Error>;
    async fn delete_tag(&self, token: TagToken) -> Result<(), Error>;
    async fn list_genres_with_counts(&self) -> Result<Vec<(Genre, u64, bool)>, Error>;
    async fn list_tags_with_counts(&self) -> Result<Vec<(Tag, u64, bool)>, Error>;
    async fn remove_unused_genres(&self) -> Result<u64, Error>;
    async fn remove_unused_tags(&self) -> Result<u64, Error>;
    async fn list_all_series(&self) -> Result<Vec<Series>, Error>;
    async fn list_all_authors(&self) -> Result<Vec<Author>, Error>;
    async fn list_all_publishers(&self) -> Result<Vec<Publisher>, Error>;
    async fn series_next_number(&self, series_name: &str) -> Result<u32, Error>;
    async fn count_books_for_author(&self, author_id: AuthorId) -> Result<u64, Error>;
    async fn count_books_for_series(&self, series_id: SeriesId) -> Result<u64, Error>;
    /// Fetches all data needed to hydrate a slice of books into view-models.
    ///
    /// Issues five batch repository queries within a single read-only
    /// transaction.
    ///
    /// # Parameters
    ///
    /// - `book_ids`: IDs of the books to hydrate.
    /// - `series_ids`: Deduplicated series IDs referenced by the caller's
    ///   already-loaded `Book` slice (i.e. `books.iter().filter_map(|b|
    ///   b.series_id)`). The caller supplies this to avoid an extra DB
    ///   round-trip; passing IDs inconsistent with `book_ids` will silently
    ///   produce missing series data.
    ///
    /// Returns `BookHydrationData::default()` immediately if `book_ids` is
    /// empty.
    async fn fetch_hydration_data(&self, book_ids: &[BookId], series_ids: &[SeriesId]) -> Result<BookHydrationData, Error>;
}

pub(crate) struct BookServiceImpl {
    repository_service: Arc<RepositoryService>,
    job_service: Arc<dyn JobService>,
}

impl BookServiceImpl {
    pub(crate) fn new(repository_service: Arc<RepositoryService>, job_service: Arc<dyn JobService>) -> Self {
        Self {
            repository_service,
            job_service,
        }
    }
}

#[async_trait::async_trait]
impl BookService for BookServiceImpl {
    async fn list_books(
        &self,
        filter: &BookQuery,
        library_id: Option<crate::library::LibraryId>,
        offset: Option<u64>,
        page_size: Option<u64>,
    ) -> Result<Vec<Book>, Error> {
        let filter = filter.clone();
        with_read_only_transaction!(self, book_repository, |tx| book_repository
            .list_books(tx, &filter, library_id, offset, page_size)
            .await)
    }

    async fn find_book_by_token(&self, token: BookToken) -> Result<Option<Book>, Error> {
        with_read_only_transaction!(self, book_repository, |tx| book_repository.find_by_token(tx, token).await)
    }

    async fn find_books_by_ids(&self, ids: &[BookId]) -> Result<Vec<Book>, Error> {
        let ids = ids.to_vec();
        with_read_only_transaction!(self, book_repository, |tx| book_repository.find_by_ids(tx, &ids).await)
    }

    async fn authors_for_book(&self, book_id: BookId) -> Result<Vec<BookAuthor>, Error> {
        with_read_only_transaction!(self, book_repository, |tx| book_repository.authors_for_book(tx, book_id).await)
    }

    async fn files_for_book(&self, book_id: BookId) -> Result<Vec<BookFile>, Error> {
        with_read_only_transaction!(self, book_repository, |tx| book_repository.files_for_book(tx, book_id).await)
    }

    async fn identifiers_for_book(&self, book_id: BookId) -> Result<Vec<BookIdentifier>, Error> {
        with_read_only_transaction!(self, book_repository, |tx| book_repository.identifiers_for_book(tx, book_id).await)
    }

    async fn list_authors(&self, start_id: Option<AuthorId>, page_size: Option<u64>) -> Result<Vec<Author>, Error> {
        with_read_only_transaction!(self, author_repository, |tx| author_repository.list_authors(tx, start_id, page_size).await)
    }

    async fn find_author_by_token(&self, token: AuthorToken) -> Result<Option<Author>, Error> {
        with_read_only_transaction!(self, author_repository, |tx| author_repository.find_by_token(tx, token).await)
    }

    async fn list_series(&self, start_id: Option<SeriesId>, page_size: Option<u64>) -> Result<Vec<Series>, Error> {
        with_read_only_transaction!(self, series_repository, |tx| series_repository.list_series(tx, start_id, page_size).await)
    }

    async fn find_series_by_token(&self, token: SeriesToken) -> Result<Option<Series>, Error> {
        with_read_only_transaction!(self, series_repository, |tx| series_repository.find_by_token(tx, token).await)
    }

    async fn find_publisher_by_token(&self, token: crate::book::PublisherToken) -> Result<Option<crate::book::Publisher>, Error> {
        with_read_only_transaction!(self, publisher_repository, |tx| publisher_repository.find_by_token(tx, token).await)
    }

    async fn genres_for_book(&self, book_id: BookId) -> Result<Vec<Genre>, Error> {
        with_read_only_transaction!(self, book_repository, |tx| book_repository.genres_for_book(tx, book_id).await)
    }

    async fn tags_for_book(&self, book_id: BookId) -> Result<Vec<Tag>, Error> {
        with_read_only_transaction!(self, book_repository, |tx| book_repository.tags_for_book(tx, book_id).await)
    }

    async fn list_all_genres(&self) -> Result<Vec<Genre>, Error> {
        with_read_only_transaction!(self, genre_repository, |tx| genre_repository.list_all_genres(tx).await)
    }

    async fn list_all_tags(&self) -> Result<Vec<Tag>, Error> {
        with_read_only_transaction!(self, tag_repository, |tx| tag_repository.list_all_tags(tx).await)
    }

    async fn create_genre(&self, name: String) -> Result<Genre, Error> {
        with_transaction!(self, genre_repository, |tx| genre_repository.add_genre(tx, NewGenre { name }).await)
    }

    async fn create_tag(&self, name: String) -> Result<Tag, Error> {
        with_transaction!(self, tag_repository, |tx| tag_repository.add_tag(tx, NewTag { name }).await)
    }

    async fn delete_genre(&self, token: GenreToken) -> Result<(), Error> {
        let book_ids = with_transaction!(self, genre_repository, book_repository, |tx| {
            let genre = genre_repository.find_by_token(tx, token).await?;
            let Some(genre) = genre else {
                return Err(Error::RepositoryError(RepositoryError::NotFound));
            };
            let book_ids = book_repository.available_book_ids_for_genre(tx, genre.id).await?;
            for &book_id in &book_ids {
                book_repository.update_sidecar_fingerprint(tx, book_id, None).await?;
            }
            genre_repository.delete_genre(tx, genre.id).await?;
            Ok(book_ids)
        })?;
        for book_id in book_ids {
            self.job_service.enqueue(&EnrichBookFilesPayload { book_id }).await?;
        }
        Ok(())
    }

    async fn delete_tag(&self, token: TagToken) -> Result<(), Error> {
        let book_ids = with_transaction!(self, tag_repository, book_repository, |tx| {
            let tag = tag_repository.find_by_token(tx, token).await?;
            let Some(tag) = tag else {
                return Err(Error::RepositoryError(RepositoryError::NotFound));
            };
            let book_ids = book_repository.available_book_ids_for_tag(tx, tag.id).await?;
            for &book_id in &book_ids {
                book_repository.update_sidecar_fingerprint(tx, book_id, None).await?;
            }
            tag_repository.delete_tag(tx, tag.id).await?;
            Ok(book_ids)
        })?;
        for book_id in book_ids {
            self.job_service.enqueue(&EnrichBookFilesPayload { book_id }).await?;
        }
        Ok(())
    }

    async fn list_genres_with_counts(&self) -> Result<Vec<(Genre, u64, bool)>, Error> {
        with_read_only_transaction!(self, genre_repository, |tx| genre_repository.list_genres_with_counts(tx).await)
    }

    async fn list_tags_with_counts(&self) -> Result<Vec<(Tag, u64, bool)>, Error> {
        with_read_only_transaction!(self, tag_repository, |tx| tag_repository.list_tags_with_counts(tx).await)
    }

    async fn remove_unused_genres(&self) -> Result<u64, Error> {
        with_transaction!(self, genre_repository, |tx| genre_repository.delete_unused_genres(tx).await)
    }

    async fn remove_unused_tags(&self) -> Result<u64, Error> {
        with_transaction!(self, tag_repository, |tx| tag_repository.delete_unused_tags(tx).await)
    }

    async fn list_all_series(&self) -> Result<Vec<Series>, Error> {
        with_read_only_transaction!(self, series_repository, |tx| series_repository.list_all_series(tx).await)
    }

    async fn list_all_authors(&self) -> Result<Vec<Author>, Error> {
        with_read_only_transaction!(self, author_repository, |tx| author_repository.list_all_authors(tx).await)
    }

    async fn list_all_publishers(&self) -> Result<Vec<Publisher>, Error> {
        with_read_only_transaction!(self, publisher_repository, |tx| publisher_repository.list_all_publishers(tx).await)
    }

    async fn series_next_number(&self, series_name: &str) -> Result<u32, Error> {
        let name = series_name.to_string();
        let series = with_read_only_transaction!(self, series_repository, |tx| series_repository.find_by_name(tx, &name).await)?;
        let Some(series) = series else {
            return Ok(1);
        };
        let series_id = series.id;
        let max = with_read_only_transaction!(self, series_repository, |tx| series_repository
            .max_series_number_for_series(tx, series_id)
            .await)?;
        let next = match max {
            Some(d) => {
                use rust_decimal::prelude::ToPrimitive;
                let floor = d.floor().to_u32().unwrap_or(0);
                floor + 1
            }
            None => 1,
        };
        Ok(next)
    }

    async fn count_books_for_author(&self, author_id: AuthorId) -> Result<u64, Error> {
        with_read_only_transaction!(self, book_repository, |tx| book_repository.count_books_for_author(tx, author_id).await)
    }

    async fn count_books_for_series(&self, series_id: SeriesId) -> Result<u64, Error> {
        with_read_only_transaction!(self, series_repository, |tx| series_repository.count_books_for_series(tx, series_id).await)
    }

    async fn fetch_hydration_data(&self, book_ids: &[BookId], series_ids: &[SeriesId]) -> Result<BookHydrationData, Error> {
        if book_ids.is_empty() {
            return Ok(BookHydrationData::default());
        }
        let book_ids = book_ids.to_vec();
        let series_ids = series_ids.to_vec();

        with_read_only_transaction!(self, book_repository, author_repository, series_repository, |tx| {
            let book_authors = book_repository.book_authors_for_books(tx, &book_ids).await?;

            let author_ids: Vec<AuthorId> = {
                let mut ids: Vec<AuthorId> = book_authors.iter().map(|ba| ba.author_id).collect();
                ids.sort_unstable();
                ids.dedup();
                ids
            };

            let authors = author_repository.find_by_ids(tx, &author_ids).await?;
            let series = series_repository.find_by_ids(tx, &series_ids).await?;
            let genres = book_repository.book_genres_for_books(tx, &book_ids).await?;
            let tags = book_repository.book_tags_for_books(tx, &book_ids).await?;

            Ok(BookHydrationData {
                book_authors,
                authors,
                series,
                genres,
                tags,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{BookService, BookServiceImpl};
    use crate::{
        Error, RepositoryError,
        book::{
            Author, AuthorId, AuthorToken, Book, BookAuthor, BookFile, BookId, BookIdentifier, BookQuery, BookStatus, BookToken, Genre, GenreToken, Series,
            SeriesId, SeriesToken, Tag, TagToken,
            repository::{author::MockAuthorRepository, book::MockBookRepository, series::MockSeriesRepository},
        },
        jobs::service::MockJobService,
    };

    // ─── Helpers ─────────────────────────────────────────────────────────────

    fn fake_book(id: BookId, title: &str) -> Book {
        Book::fake(id, title, BookStatus::Available)
    }

    fn fake_author(id: AuthorId, name: &str) -> Author {
        Author::fake(id, name)
    }

    fn fake_series(id: SeriesId, name: &str) -> Series {
        Series::fake(id, name)
    }

    fn create_service(book_repo: MockBookRepository, author_repo: MockAuthorRepository, series_repo: MockSeriesRepository) -> BookServiceImpl {
        let repository_service = Arc::new(
            crate::repository::testing::default_repository_service_builder()
                .author_repository(Arc::new(author_repo))
                .series_repository(Arc::new(series_repo))
                .book_repository(Arc::new(book_repo))
                .build()
                .expect("all fields provided"),
        );
        BookServiceImpl::new(repository_service, Arc::new(MockJobService::new()))
    }

    fn default_service_with_book_repo(book_repo: MockBookRepository) -> BookServiceImpl {
        create_service(book_repo, MockAuthorRepository::new(), MockSeriesRepository::new())
    }

    // ─── list_books ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_books_returns_results() {
        let books = vec![fake_book(1, "Dune"), fake_book(2, "Foundation")];
        let mut book_repo = MockBookRepository::new();
        book_repo.expect_list_books().returning(move |_, _, _, _, _| {
            let books = books.clone();
            Box::pin(async move { Ok(books) })
        });
        let svc = default_service_with_book_repo(book_repo);

        let result = svc.list_books(&BookQuery::default(), None, None, None).await;

        assert!(result.is_ok());
        let list = result.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].title, "Dune");
        assert_eq!(list[1].title, "Foundation");
    }

    #[tokio::test]
    async fn test_list_books_returns_empty() {
        let mut book_repo = MockBookRepository::new();
        book_repo.expect_list_books().returning(|_, _, _, _, _| Box::pin(async { Ok(vec![]) }));
        let svc = default_service_with_book_repo(book_repo);

        let result = svc.list_books(&BookQuery::default(), None, None, None).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_books_propagates_error() {
        let mut book_repo = MockBookRepository::new();
        book_repo
            .expect_list_books()
            .returning(|_, _, _, _, _| Box::pin(async { Err(Error::RepositoryError(RepositoryError::Database("db error".into()))) }));
        let svc = default_service_with_book_repo(book_repo);

        let result = svc.list_books(&BookQuery::default(), None, None, None).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── find_book_by_token ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_book_by_token_found() {
        let book = fake_book(1, "Dune");
        let token = book.token;
        let mut book_repo = MockBookRepository::new();
        book_repo.expect_find_by_token().returning(move |_, _| {
            let book = book.clone();
            Box::pin(async move { Ok(Some(book)) })
        });
        let svc = default_service_with_book_repo(book_repo);

        let result = svc.find_book_by_token(token).await;

        assert!(result.is_ok());
        let found = result.unwrap().unwrap();
        assert_eq!(found.id, 1);
        assert_eq!(found.title, "Dune");
    }

    #[tokio::test]
    async fn test_find_book_by_token_not_found() {
        let mut book_repo = MockBookRepository::new();
        book_repo.expect_find_by_token().returning(|_, _| Box::pin(async { Ok(None) }));
        let svc = default_service_with_book_repo(book_repo);

        let token = BookToken::generate();
        let result = svc.find_book_by_token(token).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_find_book_by_token_propagates_error() {
        let mut book_repo = MockBookRepository::new();
        book_repo
            .expect_find_by_token()
            .returning(|_, _| Box::pin(async { Err(Error::RepositoryError(RepositoryError::Database("db error".into()))) }));
        let svc = default_service_with_book_repo(book_repo);

        let token = BookToken::generate();
        let result = svc.find_book_by_token(token).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── authors_for_book ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_authors_for_book_returns_results() {
        let authors = vec![BookAuthor::fake(1, 1, "author", 0), BookAuthor::fake(1, 2, "editor", 1)];
        let mut book_repo = MockBookRepository::new();
        book_repo.expect_authors_for_book().returning(move |_, _| {
            let authors = authors.clone();
            Box::pin(async move { Ok(authors) })
        });
        let svc = default_service_with_book_repo(book_repo);

        let result = svc.authors_for_book(1).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_authors_for_book_returns_empty() {
        let mut book_repo = MockBookRepository::new();
        book_repo.expect_authors_for_book().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        let svc = default_service_with_book_repo(book_repo);

        let result = svc.authors_for_book(1).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_authors_for_book_propagates_error() {
        let mut book_repo = MockBookRepository::new();
        book_repo
            .expect_authors_for_book()
            .returning(|_, _| Box::pin(async { Err(Error::RepositoryError(RepositoryError::Database("db error".into()))) }));
        let svc = default_service_with_book_repo(book_repo);

        let result = svc.authors_for_book(1).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── files_for_book ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_files_for_book_returns_results() {
        let files = vec![BookFile::fake(1, "epub"), BookFile::fake(1, "mobi")];
        let mut book_repo = MockBookRepository::new();
        book_repo.expect_files_for_book().returning(move |_, _| {
            let files = files.clone();
            Box::pin(async move { Ok(files) })
        });
        let svc = default_service_with_book_repo(book_repo);

        let result = svc.files_for_book(1).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_files_for_book_returns_empty() {
        let mut book_repo = MockBookRepository::new();
        book_repo.expect_files_for_book().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        let svc = default_service_with_book_repo(book_repo);

        let result = svc.files_for_book(1).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_files_for_book_propagates_error() {
        let mut book_repo = MockBookRepository::new();
        book_repo
            .expect_files_for_book()
            .returning(|_, _| Box::pin(async { Err(Error::RepositoryError(RepositoryError::Database("db error".into()))) }));
        let svc = default_service_with_book_repo(book_repo);

        let result = svc.files_for_book(1).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── identifiers_for_book ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_identifiers_for_book_returns_results() {
        let ids = vec![BookIdentifier::fake(1, "isbn13", "9780000000001")];
        let mut book_repo = MockBookRepository::new();
        book_repo.expect_identifiers_for_book().returning(move |_, _| {
            let ids = ids.clone();
            Box::pin(async move { Ok(ids) })
        });
        let svc = default_service_with_book_repo(book_repo);

        let result = svc.identifiers_for_book(1).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_identifiers_for_book_propagates_error() {
        let mut book_repo = MockBookRepository::new();
        book_repo
            .expect_identifiers_for_book()
            .returning(|_, _| Box::pin(async { Err(Error::RepositoryError(RepositoryError::Database("db error".into()))) }));
        let svc = default_service_with_book_repo(book_repo);

        let result = svc.identifiers_for_book(1).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── list_authors ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_authors_returns_results() {
        let authors = vec![fake_author(1, "Ursula K. Le Guin"), fake_author(2, "N.K. Jemisin")];
        let mut author_repo = MockAuthorRepository::new();
        author_repo.expect_list_authors().returning(move |_, _, _| {
            let authors = authors.clone();
            Box::pin(async move { Ok(authors) })
        });
        let svc = create_service(MockBookRepository::new(), author_repo, MockSeriesRepository::new());

        let result = svc.list_authors(None, None).await;

        assert!(result.is_ok());
        let list = result.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Ursula K. Le Guin");
    }

    #[tokio::test]
    async fn test_list_authors_returns_empty() {
        let mut author_repo = MockAuthorRepository::new();
        author_repo.expect_list_authors().returning(|_, _, _| Box::pin(async { Ok(vec![]) }));
        let svc = create_service(MockBookRepository::new(), author_repo, MockSeriesRepository::new());

        let result = svc.list_authors(None, None).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_authors_propagates_error() {
        let mut author_repo = MockAuthorRepository::new();
        author_repo
            .expect_list_authors()
            .returning(|_, _, _| Box::pin(async { Err(Error::RepositoryError(RepositoryError::Database("db error".into()))) }));
        let svc = create_service(MockBookRepository::new(), author_repo, MockSeriesRepository::new());

        let result = svc.list_authors(None, None).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── find_author_by_token ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_author_by_token_found() {
        let author = fake_author(1, "Brandon Sanderson");
        let token = author.token;
        let mut author_repo = MockAuthorRepository::new();
        author_repo.expect_find_by_token().returning(move |_, _| {
            let author = author.clone();
            Box::pin(async move { Ok(Some(author)) })
        });
        let svc = create_service(MockBookRepository::new(), author_repo, MockSeriesRepository::new());

        let result = svc.find_author_by_token(token).await;

        assert!(result.is_ok());
        let found = result.unwrap().unwrap();
        assert_eq!(found.id, 1);
        assert_eq!(found.name, "Brandon Sanderson");
    }

    #[tokio::test]
    async fn test_find_author_by_token_not_found() {
        let mut author_repo = MockAuthorRepository::new();
        author_repo.expect_find_by_token().returning(|_, _| Box::pin(async { Ok(None) }));
        let svc = create_service(MockBookRepository::new(), author_repo, MockSeriesRepository::new());

        let result = svc.find_author_by_token(AuthorToken::generate()).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    // ─── list_series ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_series_returns_results() {
        let series = vec![fake_series(1, "Stormlight Archive"), fake_series(2, "Mistborn")];
        let mut series_repo = MockSeriesRepository::new();
        series_repo.expect_list_series().returning(move |_, _, _| {
            let series = series.clone();
            Box::pin(async move { Ok(series) })
        });
        let svc = create_service(MockBookRepository::new(), MockAuthorRepository::new(), series_repo);

        let result = svc.list_series(None, None).await;

        assert!(result.is_ok());
        let list = result.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Stormlight Archive");
    }

    #[tokio::test]
    async fn test_list_series_returns_empty() {
        let mut series_repo = MockSeriesRepository::new();
        series_repo.expect_list_series().returning(|_, _, _| Box::pin(async { Ok(vec![]) }));
        let svc = create_service(MockBookRepository::new(), MockAuthorRepository::new(), series_repo);

        let result = svc.list_series(None, None).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_series_propagates_error() {
        let mut series_repo = MockSeriesRepository::new();
        series_repo
            .expect_list_series()
            .returning(|_, _, _| Box::pin(async { Err(Error::RepositoryError(RepositoryError::Database("db error".into()))) }));
        let svc = create_service(MockBookRepository::new(), MockAuthorRepository::new(), series_repo);

        let result = svc.list_series(None, None).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── find_series_by_token ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_series_by_token_found() {
        let series = fake_series(1, "Stormlight Archive");
        let token = series.token;
        let mut series_repo = MockSeriesRepository::new();
        series_repo.expect_find_by_token().returning(move |_, _| {
            let series = series.clone();
            Box::pin(async move { Ok(Some(series)) })
        });
        let svc = create_service(MockBookRepository::new(), MockAuthorRepository::new(), series_repo);

        let result = svc.find_series_by_token(token).await;

        assert!(result.is_ok());
        let found = result.unwrap().unwrap();
        assert_eq!(found.id, 1);
        assert_eq!(found.name, "Stormlight Archive");
    }

    #[tokio::test]
    async fn test_find_series_by_token_not_found() {
        let mut series_repo = MockSeriesRepository::new();
        series_repo.expect_find_by_token().returning(|_, _| Box::pin(async { Ok(None) }));
        let svc = create_service(MockBookRepository::new(), MockAuthorRepository::new(), series_repo);

        let result = svc.find_series_by_token(SeriesToken::generate()).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    // ─── count_books_for_author ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_count_books_for_author_returns_count() {
        let mut book_repo = MockBookRepository::new();
        book_repo.expect_count_books_for_author().returning(|_, _| Box::pin(async { Ok(5) }));
        let svc = default_service_with_book_repo(book_repo);

        let result = svc.count_books_for_author(1).await;

        assert_eq!(result.unwrap(), 5);
    }

    #[tokio::test]
    async fn test_count_books_for_author_propagates_error() {
        let mut book_repo = MockBookRepository::new();
        book_repo
            .expect_count_books_for_author()
            .returning(|_, _| Box::pin(async { Err(Error::RepositoryError(RepositoryError::Database("db error".into()))) }));
        let svc = default_service_with_book_repo(book_repo);

        let result = svc.count_books_for_author(1).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── count_books_for_series ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_count_books_for_series_returns_count() {
        let mut series_repo = MockSeriesRepository::new();
        series_repo.expect_count_books_for_series().returning(|_, _| Box::pin(async { Ok(3) }));
        let svc = create_service(MockBookRepository::new(), MockAuthorRepository::new(), series_repo);

        let result = svc.count_books_for_series(1).await;

        assert_eq!(result.unwrap(), 3);
    }

    #[tokio::test]
    async fn test_count_books_for_series_propagates_error() {
        let mut series_repo = MockSeriesRepository::new();
        series_repo
            .expect_count_books_for_series()
            .returning(|_, _| Box::pin(async { Err(Error::RepositoryError(RepositoryError::Database("db error".into()))) }));
        let svc = create_service(MockBookRepository::new(), MockAuthorRepository::new(), series_repo);

        let result = svc.count_books_for_series(1).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── delete_genre ────────────────────────────────────────────────────────

    fn fake_genre(name: &str) -> Genre {
        use chrono::Utc;
        let token = GenreToken::generate();
        let id = token.id();
        Genre {
            id,
            version: 1,
            token,
            name: name.to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn fake_tag(name: &str) -> Tag {
        use chrono::Utc;
        let token = TagToken::generate();
        let id = token.id();
        Tag {
            id,
            version: 1,
            token,
            name: name.to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_delete_genre_enqueues_enrichment_for_affected_books() {
        use crate::{book::repository::genre::MockGenreRepository, repository::testing::default_repository_service_builder};

        let genre = fake_genre("Fantasy");
        let token = genre.token;

        let mut genre_repo = MockGenreRepository::new();
        let g = genre.clone();
        genre_repo.expect_find_by_token().returning(move |_, _| {
            let g = g.clone();
            Box::pin(async move { Ok(Some(g)) })
        });
        genre_repo.expect_delete_genre().returning(|_, _| Box::pin(async { Ok(()) }));

        let mut book_repo = MockBookRepository::new();
        book_repo
            .expect_available_book_ids_for_genre()
            .returning(|_, _| Box::pin(async { Ok(vec![1u64, 2u64]) }));
        book_repo
            .expect_update_sidecar_fingerprint()
            .times(2)
            .returning(|_, _, _| Box::pin(async { Ok(()) }));

        let mut job_svc = MockJobService::new();
        job_svc.expect_enqueue_raw().times(2).returning(|_, _, _| Box::pin(async { Ok(()) }));

        let repository_service = Arc::new(
            default_repository_service_builder()
                .book_repository(Arc::new(book_repo))
                .genre_repository(Arc::new(genre_repo))
                .build()
                .expect("all fields provided"),
        );
        let svc = BookServiceImpl::new(repository_service, Arc::new(job_svc));

        svc.delete_genre(token).await.unwrap();
    }

    #[tokio::test]
    async fn test_delete_tag_enqueues_enrichment_for_affected_books() {
        use crate::{book::repository::tag::MockTagRepository, repository::testing::default_repository_service_builder};

        let tag = fake_tag("Space Opera");
        let token = tag.token;

        let mut tag_repo = MockTagRepository::new();
        let t = tag.clone();
        tag_repo.expect_find_by_token().returning(move |_, _| {
            let t = t.clone();
            Box::pin(async move { Ok(Some(t)) })
        });
        tag_repo.expect_delete_tag().returning(|_, _| Box::pin(async { Ok(()) }));

        let mut book_repo = MockBookRepository::new();
        book_repo
            .expect_available_book_ids_for_tag()
            .returning(|_, _| Box::pin(async { Ok(vec![5u64]) }));
        book_repo
            .expect_update_sidecar_fingerprint()
            .times(1)
            .returning(|_, _, _| Box::pin(async { Ok(()) }));

        let mut job_svc = MockJobService::new();
        job_svc.expect_enqueue_raw().times(1).returning(|_, _, _| Box::pin(async { Ok(()) }));

        let repository_service = Arc::new(
            default_repository_service_builder()
                .book_repository(Arc::new(book_repo))
                .tag_repository(Arc::new(tag_repo))
                .build()
                .expect("all fields provided"),
        );
        let svc = BookServiceImpl::new(repository_service, Arc::new(job_svc));

        svc.delete_tag(token).await.unwrap();
    }

    // ─── fetch_hydration_data
    // ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_fetch_hydration_data_empty_book_ids_returns_default() {
        let svc = create_service(MockBookRepository::new(), MockAuthorRepository::new(), MockSeriesRepository::new());
        let result = svc.fetch_hydration_data(&[], &[]).await;
        assert!(result.is_ok());
        let data = result.unwrap();
        assert!(data.book_authors.is_empty());
        assert!(data.authors.is_empty());
        assert!(data.series.is_empty());
        assert!(data.genres.is_empty());
        assert!(data.tags.is_empty());
    }

    #[tokio::test]
    async fn test_fetch_hydration_data_returns_populated_data() {
        let book_author = BookAuthor::fake(1, 10, "author", 0);
        let author = fake_author(10, "Frank Herbert");
        let series = fake_series(5, "Dune Chronicles");
        let genre = fake_genre("SciFi");
        let tag = fake_tag("epic");

        let ba = book_author.clone();
        let mut book_repo = MockBookRepository::new();
        book_repo.expect_book_authors_for_books().returning(move |_, _| {
            let ba = ba.clone();
            Box::pin(async move { Ok(vec![ba]) })
        });

        let expected_genres = vec![(1u64, genre.clone())];
        let eg = expected_genres.clone();
        book_repo.expect_book_genres_for_books().returning(move |_, _| {
            let eg = eg.clone();
            Box::pin(async move { Ok(eg) })
        });

        let expected_tags = vec![(1u64, tag.clone())];
        let et = expected_tags.clone();
        book_repo.expect_book_tags_for_books().returning(move |_, _| {
            let et = et.clone();
            Box::pin(async move { Ok(et) })
        });

        let a = author.clone();
        let mut author_repo = MockAuthorRepository::new();
        author_repo.expect_find_by_ids().returning(move |_, _| {
            let a = a.clone();
            Box::pin(async move { Ok(vec![a]) })
        });

        let s = series.clone();
        let mut series_repo = MockSeriesRepository::new();
        series_repo.expect_find_by_ids().returning(move |_, _| {
            let s = s.clone();
            Box::pin(async move { Ok(vec![s]) })
        });

        let svc = create_service(book_repo, author_repo, series_repo);
        let result = svc.fetch_hydration_data(&[1], &[5]).await;

        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.book_authors.len(), 1);
        assert_eq!(data.authors.len(), 1);
        assert_eq!(data.series.len(), 1);
        assert_eq!(data.genres.len(), 1);
        assert_eq!(data.tags.len(), 1);
        assert_eq!(data.book_authors[0].book_id, 1);
        assert_eq!(data.book_authors[0].author_id, 10);
        assert_eq!(data.authors[0].name, "Frank Herbert");
        assert_eq!(data.series[0].name, "Dune Chronicles");
        assert_eq!(data.genres[0].0, 1u64);
        assert_eq!(data.genres[0].1.name, "SciFi");
        assert_eq!(data.tags[0].0, 1u64);
        assert_eq!(data.tags[0].1.name, "epic");
    }

    #[tokio::test]
    async fn test_fetch_hydration_data_deduplicates_author_ids() {
        // Two BookAuthor rows for the same author_id (10) — find_by_ids should
        // be called with a deduplicated slice of length 1, not 2.
        let ba1 = BookAuthor::fake(1, 10, "author", 0);
        let ba2 = BookAuthor::fake(2, 10, "author", 0);
        let author = fake_author(10, "Frank Herbert");

        let mut book_repo = MockBookRepository::new();
        book_repo.expect_book_authors_for_books().returning(move |_, _| {
            let (ba1, ba2) = (ba1.clone(), ba2.clone());
            Box::pin(async move { Ok(vec![ba1, ba2]) })
        });
        book_repo.expect_book_genres_for_books().returning(|_, _| Box::pin(async { Ok(vec![]) }));
        book_repo.expect_book_tags_for_books().returning(|_, _| Box::pin(async { Ok(vec![]) }));

        let a = author.clone();
        let mut author_repo = MockAuthorRepository::new();
        author_repo.expect_find_by_ids().returning(move |_, ids| {
            assert_eq!(ids.len(), 1, "find_by_ids should receive deduplicated author IDs");
            let a = a.clone();
            Box::pin(async move { Ok(vec![a]) })
        });

        let mut series_repo = MockSeriesRepository::new();
        series_repo.expect_find_by_ids().returning(|_, _| Box::pin(async { Ok(vec![]) }));

        let svc = create_service(book_repo, author_repo, series_repo);
        let result = svc.fetch_hydration_data(&[1, 2], &[]).await;

        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.book_authors.len(), 2);
        assert_eq!(data.authors.len(), 1);
        assert_eq!(data.authors[0].name, "Frank Herbert");
    }

    #[tokio::test]
    async fn test_fetch_hydration_data_propagates_repo_error() {
        let mut book_repo = MockBookRepository::new();
        book_repo
            .expect_book_authors_for_books()
            .returning(|_, _| Box::pin(async { Err(Error::RepositoryError(RepositoryError::Database("db error".into()))) }));

        let svc = create_service(book_repo, MockAuthorRepository::new(), MockSeriesRepository::new());
        let result = svc.fetch_hydration_data(&[1], &[]).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }
}
