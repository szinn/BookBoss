use std::sync::Arc;

use crate::{
    Error,
    book::{Author, AuthorId, AuthorToken, Book, BookAuthor, BookFile, BookFilter, BookId, BookIdentifier, BookToken, Series, SeriesId, SeriesToken},
    repository::RepositoryService,
    with_read_only_transaction,
};

#[async_trait::async_trait]
pub trait BookService: Send + Sync {
    async fn list_books(&self, filter: &BookFilter, start_id: Option<BookId>, page_size: Option<u64>) -> Result<Vec<Book>, Error>;
    async fn find_book_by_token(&self, token: &BookToken) -> Result<Option<Book>, Error>;
    async fn authors_for_book(&self, book_id: BookId) -> Result<Vec<BookAuthor>, Error>;
    async fn files_for_book(&self, book_id: BookId) -> Result<Vec<BookFile>, Error>;
    async fn identifiers_for_book(&self, book_id: BookId) -> Result<Vec<BookIdentifier>, Error>;
    async fn list_authors(&self, start_id: Option<AuthorId>, page_size: Option<u64>) -> Result<Vec<Author>, Error>;
    async fn find_author_by_token(&self, token: &AuthorToken) -> Result<Option<Author>, Error>;
    async fn list_series(&self, start_id: Option<SeriesId>, page_size: Option<u64>) -> Result<Vec<Series>, Error>;
    async fn find_series_by_token(&self, token: &SeriesToken) -> Result<Option<Series>, Error>;
}

pub(crate) struct BookServiceImpl {
    repository_service: Arc<RepositoryService>,
}

impl BookServiceImpl {
    pub(crate) fn new(repository_service: Arc<RepositoryService>) -> Self {
        Self { repository_service }
    }
}

#[async_trait::async_trait]
impl BookService for BookServiceImpl {
    #[tracing::instrument(level = "trace", skip(self, filter))]
    async fn list_books(&self, filter: &BookFilter, start_id: Option<BookId>, page_size: Option<u64>) -> Result<Vec<Book>, Error> {
        let filter = filter.clone();
        with_read_only_transaction!(self, book_repository, |tx| book_repository.list_books(tx, &filter, start_id, page_size).await)
    }

    #[tracing::instrument(level = "trace", skip(self, token))]
    async fn find_book_by_token(&self, token: &BookToken) -> Result<Option<Book>, Error> {
        let token = *token;
        with_read_only_transaction!(self, book_repository, |tx| book_repository.find_by_token(tx, &token).await)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn authors_for_book(&self, book_id: BookId) -> Result<Vec<BookAuthor>, Error> {
        with_read_only_transaction!(self, book_repository, |tx| book_repository.authors_for_book(tx, book_id).await)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn files_for_book(&self, book_id: BookId) -> Result<Vec<BookFile>, Error> {
        with_read_only_transaction!(self, book_repository, |tx| book_repository.files_for_book(tx, book_id).await)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn identifiers_for_book(&self, book_id: BookId) -> Result<Vec<BookIdentifier>, Error> {
        with_read_only_transaction!(self, book_repository, |tx| book_repository.identifiers_for_book(tx, book_id).await)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn list_authors(&self, start_id: Option<AuthorId>, page_size: Option<u64>) -> Result<Vec<Author>, Error> {
        with_read_only_transaction!(self, author_repository, |tx| author_repository.list_authors(tx, start_id, page_size).await)
    }

    #[tracing::instrument(level = "trace", skip(self, token))]
    async fn find_author_by_token(&self, token: &AuthorToken) -> Result<Option<Author>, Error> {
        let token = *token;
        with_read_only_transaction!(self, author_repository, |tx| author_repository.find_by_token(tx, &token).await)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn list_series(&self, start_id: Option<SeriesId>, page_size: Option<u64>) -> Result<Vec<Series>, Error> {
        with_read_only_transaction!(self, series_repository, |tx| series_repository.list_series(tx, start_id, page_size).await)
    }

    #[tracing::instrument(level = "trace", skip(self, token))]
    async fn find_series_by_token(&self, token: &SeriesToken) -> Result<Option<Series>, Error> {
        let token = *token;
        with_read_only_transaction!(self, series_repository, |tx| series_repository.find_by_token(tx, &token).await)
    }
}

#[cfg(test)]
mod tests {
    use std::{any::Any, sync::Arc};

    use super::{BookService, BookServiceImpl};
    use crate::{
        Error, RepositoryError,
        auth::{NewSession, Session, repository::SessionRepository},
        book::{
            Author, AuthorId, AuthorRepository, AuthorRole, AuthorToken, Book, BookAuthor, BookFile, BookFilter, BookId, BookIdentifier, BookRepository,
            BookStatus, BookToken, FileFormat, Genre, GenreId, GenreRepository, GenreToken, IdentifierType, NewAuthor, NewBook, NewGenre, NewPublisher,
            NewSeries, NewTag, Publisher, PublisherId, PublisherRepository, PublisherToken, Series, SeriesId, SeriesRepository, SeriesToken, Tag, TagId,
            TagRepository, TagToken,
        },
        import::{ImportJob, ImportJobId, ImportJobRepository, ImportJobToken, ImportStatus, NewImportJob},
        jobs::{Job, JobRepository},
        repository::{Repository, RepositoryServiceBuilder, Transaction},
        user::{
            NewUser, NewUserSetting, User, UserId, UserSetting,
            repository::{UserRepository, UserSettingRepository},
        },
    };

    // ─── Mock Transaction ────────────────────────────────────────────────────

    struct MockTransaction;

    #[async_trait::async_trait]
    impl Transaction for MockTransaction {
        fn as_any(&self) -> &dyn Any {
            self
        }
        async fn commit(self: Box<Self>) -> Result<(), Error> {
            Ok(())
        }
        async fn rollback(self: Box<Self>) -> Result<(), Error> {
            Ok(())
        }
    }

    // ─── Mock Repository ─────────────────────────────────────────────────────

    struct MockRepository;

    #[async_trait::async_trait]
    impl Repository for MockRepository {
        async fn begin(&self) -> Result<Box<dyn Transaction>, Error> {
            Ok(Box::new(MockTransaction))
        }
        async fn begin_read_only(&self) -> Result<Box<dyn Transaction>, Error> {
            Ok(Box::new(MockTransaction))
        }
        async fn close(&self) -> Result<(), Error> {
            Ok(())
        }
    }

    // ─── Mock SessionRepository ──────────────────────────────────────────────

    struct MockSessionRepository;

    #[async_trait::async_trait]
    impl SessionRepository for MockSessionRepository {
        async fn count(&self, _: &dyn Transaction) -> Result<i64, Error> {
            unimplemented!()
        }
        async fn store(&self, _: &dyn Transaction, _: NewSession) -> Result<Session, Error> {
            unimplemented!()
        }
        async fn load(&self, _: &dyn Transaction, _: &str) -> Result<Option<Session>, Error> {
            unimplemented!()
        }
        async fn delete_by_id(&self, _: &dyn Transaction, _: &str) -> Result<(), Error> {
            unimplemented!()
        }
        async fn exists(&self, _: &dyn Transaction, _: &str) -> Result<bool, Error> {
            unimplemented!()
        }
        async fn delete_by_expiry(&self, _: &dyn Transaction) -> Result<Vec<String>, Error> {
            unimplemented!()
        }
        async fn delete_all(&self, _: &dyn Transaction) -> Result<(), Error> {
            unimplemented!()
        }
        async fn get_ids(&self, _: &dyn Transaction) -> Result<Vec<String>, Error> {
            unimplemented!()
        }
    }

    // ─── Mock UserRepository ─────────────────────────────────────────────────

    struct MockUserRepository;

    #[async_trait::async_trait]
    impl UserRepository for MockUserRepository {
        async fn add_user(&self, _: &dyn Transaction, _: NewUser) -> Result<User, Error> {
            unimplemented!()
        }
        async fn update_user(&self, _: &dyn Transaction, _: User) -> Result<User, Error> {
            unimplemented!()
        }
        async fn delete_user(&self, _: &dyn Transaction, _: User) -> Result<User, Error> {
            unimplemented!()
        }
        async fn list_users(&self, _: &dyn Transaction, _: Option<UserId>, _: Option<u64>) -> Result<Vec<User>, Error> {
            unimplemented!()
        }
        async fn find_by_id(&self, _: &dyn Transaction, _: UserId) -> Result<Option<User>, Error> {
            unimplemented!()
        }
        async fn find_by_username(&self, _: &dyn Transaction, _: &str) -> Result<Option<User>, Error> {
            unimplemented!()
        }
    }

    // ─── Mock UserSettingRepository ──────────────────────────────────────────

    struct MockUserSettingRepository;

    #[async_trait::async_trait]
    impl UserSettingRepository for MockUserSettingRepository {
        async fn get(&self, _: &dyn Transaction, _: UserId, _: &str) -> Result<Option<UserSetting>, Error> {
            unimplemented!()
        }
        async fn set(&self, _: &dyn Transaction, _: NewUserSetting) -> Result<UserSetting, Error> {
            unimplemented!()
        }
        async fn delete(&self, _: &dyn Transaction, _: UserId, _: &str) -> Result<(), Error> {
            unimplemented!()
        }
        async fn list_by_user(&self, _: &dyn Transaction, _: UserId) -> Result<Vec<UserSetting>, Error> {
            unimplemented!()
        }
    }

    // ─── Mock GenreRepository ────────────────────────────────────────────────

    struct MockGenreRepository;

    #[async_trait::async_trait]
    impl GenreRepository for MockGenreRepository {
        async fn add_genre(&self, _: &dyn Transaction, _: NewGenre) -> Result<Genre, Error> {
            unimplemented!()
        }
        async fn update_genre(&self, _: &dyn Transaction, _: Genre) -> Result<Genre, Error> {
            unimplemented!()
        }
        async fn find_by_id(&self, _: &dyn Transaction, _: GenreId) -> Result<Option<Genre>, Error> {
            unimplemented!()
        }
        async fn find_by_token(&self, _: &dyn Transaction, _: &GenreToken) -> Result<Option<Genre>, Error> {
            unimplemented!()
        }
        async fn find_by_name(&self, _: &dyn Transaction, _: &str) -> Result<Option<Genre>, Error> {
            unimplemented!()
        }
        async fn list_genres(&self, _: &dyn Transaction, _: Option<GenreId>, _: Option<u64>) -> Result<Vec<Genre>, Error> {
            unimplemented!()
        }
    }

    // ─── Mock PublisherRepository ────────────────────────────────────────────

    struct MockPublisherRepository;

    #[async_trait::async_trait]
    impl PublisherRepository for MockPublisherRepository {
        async fn add_publisher(&self, _: &dyn Transaction, _: NewPublisher) -> Result<Publisher, Error> {
            unimplemented!()
        }
        async fn update_publisher(&self, _: &dyn Transaction, _: Publisher) -> Result<Publisher, Error> {
            unimplemented!()
        }
        async fn find_by_id(&self, _: &dyn Transaction, _: PublisherId) -> Result<Option<Publisher>, Error> {
            unimplemented!()
        }
        async fn find_by_token(&self, _: &dyn Transaction, _: &PublisherToken) -> Result<Option<Publisher>, Error> {
            unimplemented!()
        }
        async fn list_publishers(&self, _: &dyn Transaction, _: Option<PublisherId>, _: Option<u64>) -> Result<Vec<Publisher>, Error> {
            unimplemented!()
        }
        async fn find_by_name(&self, _: &dyn Transaction, _: &str) -> Result<Option<Publisher>, Error> {
            unimplemented!()
        }
    }

    // ─── Mock TagRepository ──────────────────────────────────────────────────

    struct MockTagRepository;

    #[async_trait::async_trait]
    impl TagRepository for MockTagRepository {
        async fn add_tag(&self, _: &dyn Transaction, _: NewTag) -> Result<Tag, Error> {
            unimplemented!()
        }
        async fn update_tag(&self, _: &dyn Transaction, _: Tag) -> Result<Tag, Error> {
            unimplemented!()
        }
        async fn find_by_id(&self, _: &dyn Transaction, _: TagId) -> Result<Option<Tag>, Error> {
            unimplemented!()
        }
        async fn find_by_token(&self, _: &dyn Transaction, _: &TagToken) -> Result<Option<Tag>, Error> {
            unimplemented!()
        }
        async fn find_by_name(&self, _: &dyn Transaction, _: &str) -> Result<Option<Tag>, Error> {
            unimplemented!()
        }
        async fn list_tags(&self, _: &dyn Transaction, _: Option<TagId>, _: Option<u64>) -> Result<Vec<Tag>, Error> {
            unimplemented!()
        }
    }

    // ─── Mock AuthorRepository ───────────────────────────────────────────────

    struct MockAuthorRepository {
        list_authors_result: Option<Result<Vec<Author>, Error>>,
        find_by_token_result: Option<Result<Option<Author>, Error>>,
    }

    impl MockAuthorRepository {
        fn with_list_authors(result: Result<Vec<Author>, Error>) -> Self {
            Self {
                list_authors_result: Some(result),
                find_by_token_result: None,
            }
        }
        fn with_find_by_token(result: Result<Option<Author>, Error>) -> Self {
            Self {
                list_authors_result: None,
                find_by_token_result: Some(result),
            }
        }
    }

    #[async_trait::async_trait]
    impl AuthorRepository for MockAuthorRepository {
        async fn add_author(&self, _: &dyn Transaction, _: NewAuthor) -> Result<Author, Error> {
            unimplemented!()
        }
        async fn update_author(&self, _: &dyn Transaction, _: Author) -> Result<Author, Error> {
            unimplemented!()
        }
        async fn find_by_id(&self, _: &dyn Transaction, _: AuthorId) -> Result<Option<Author>, Error> {
            unimplemented!()
        }
        async fn find_by_token(&self, _: &dyn Transaction, _: &AuthorToken) -> Result<Option<Author>, Error> {
            self.find_by_token_result
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("find_by_token")))
        }
        async fn list_authors(&self, _: &dyn Transaction, _: Option<AuthorId>, _: Option<u64>) -> Result<Vec<Author>, Error> {
            self.list_authors_result
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("list_authors")))
        }
        async fn find_by_name(&self, _: &dyn Transaction, _: &str) -> Result<Option<Author>, Error> {
            unimplemented!()
        }
    }

    // ─── Mock SeriesRepository ───────────────────────────────────────────────

    struct MockSeriesRepository {
        list_series_result: Option<Result<Vec<Series>, Error>>,
        find_by_token_result: Option<Result<Option<Series>, Error>>,
    }

    impl MockSeriesRepository {
        fn with_list_series(result: Result<Vec<Series>, Error>) -> Self {
            Self {
                list_series_result: Some(result),
                find_by_token_result: None,
            }
        }
        fn with_find_by_token(result: Result<Option<Series>, Error>) -> Self {
            Self {
                list_series_result: None,
                find_by_token_result: Some(result),
            }
        }
    }

    #[async_trait::async_trait]
    impl SeriesRepository for MockSeriesRepository {
        async fn add_series(&self, _: &dyn Transaction, _: NewSeries) -> Result<Series, Error> {
            unimplemented!()
        }
        async fn update_series(&self, _: &dyn Transaction, _: Series) -> Result<Series, Error> {
            unimplemented!()
        }
        async fn find_by_id(&self, _: &dyn Transaction, _: SeriesId) -> Result<Option<Series>, Error> {
            unimplemented!()
        }
        async fn find_by_token(&self, _: &dyn Transaction, _: &SeriesToken) -> Result<Option<Series>, Error> {
            self.find_by_token_result
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("find_by_token")))
        }
        async fn list_series(&self, _: &dyn Transaction, _: Option<SeriesId>, _: Option<u64>) -> Result<Vec<Series>, Error> {
            self.list_series_result.clone().unwrap_or_else(|| Err(Error::MockNotConfigured("list_series")))
        }
        async fn find_by_name(&self, _: &dyn Transaction, _: &str) -> Result<Option<Series>, Error> {
            unimplemented!()
        }
    }

    // ─── Mock BookRepository ─────────────────────────────────────────────────

    struct MockBookRepository {
        list_books_result: Option<Result<Vec<Book>, Error>>,
        find_by_token_result: Option<Result<Option<Book>, Error>>,
        authors_for_book_result: Option<Result<Vec<BookAuthor>, Error>>,
        files_for_book_result: Option<Result<Vec<BookFile>, Error>>,
        identifiers_for_book_result: Option<Result<Vec<BookIdentifier>, Error>>,
    }

    impl MockBookRepository {
        fn new() -> Self {
            Self {
                list_books_result: None,
                find_by_token_result: None,
                authors_for_book_result: None,
                files_for_book_result: None,
                identifiers_for_book_result: None,
            }
        }
        fn with_list_books(mut self, result: Result<Vec<Book>, Error>) -> Self {
            self.list_books_result = Some(result);
            self
        }
        fn with_find_by_token(mut self, result: Result<Option<Book>, Error>) -> Self {
            self.find_by_token_result = Some(result);
            self
        }
        fn with_authors_for_book(mut self, result: Result<Vec<BookAuthor>, Error>) -> Self {
            self.authors_for_book_result = Some(result);
            self
        }
        fn with_files_for_book(mut self, result: Result<Vec<BookFile>, Error>) -> Self {
            self.files_for_book_result = Some(result);
            self
        }
        fn with_identifiers_for_book(mut self, result: Result<Vec<BookIdentifier>, Error>) -> Self {
            self.identifiers_for_book_result = Some(result);
            self
        }
    }

    #[async_trait::async_trait]
    impl BookRepository for MockBookRepository {
        async fn add_book(&self, _: &dyn Transaction, _: NewBook) -> Result<Book, Error> {
            unimplemented!()
        }
        async fn update_book(&self, _: &dyn Transaction, _: Book) -> Result<Book, Error> {
            unimplemented!()
        }
        async fn find_by_id(&self, _: &dyn Transaction, _: BookId) -> Result<Option<Book>, Error> {
            unimplemented!()
        }
        async fn find_by_token(&self, _: &dyn Transaction, _: &BookToken) -> Result<Option<Book>, Error> {
            self.find_by_token_result
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("find_by_token")))
        }
        async fn list_books(&self, _: &dyn Transaction, _: &BookFilter, _: Option<BookId>, _: Option<u64>) -> Result<Vec<Book>, Error> {
            self.list_books_result.clone().unwrap_or_else(|| Err(Error::MockNotConfigured("list_books")))
        }
        async fn authors_for_book(&self, _: &dyn Transaction, _: BookId) -> Result<Vec<BookAuthor>, Error> {
            self.authors_for_book_result
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("authors_for_book")))
        }
        async fn files_for_book(&self, _: &dyn Transaction, _: BookId) -> Result<Vec<BookFile>, Error> {
            self.files_for_book_result
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("files_for_book")))
        }
        async fn identifiers_for_book(&self, _: &dyn Transaction, _: BookId) -> Result<Vec<BookIdentifier>, Error> {
            self.identifiers_for_book_result
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("identifiers_for_book")))
        }
        async fn find_file_by_hash(&self, _: &dyn Transaction, _: &str) -> Result<Option<BookFile>, Error> {
            unimplemented!()
        }
        async fn add_book_file(&self, _: &dyn Transaction, _: BookId, _: FileFormat, _: i64, _: String) -> Result<BookFile, Error> {
            unimplemented!()
        }
        async fn add_book_author(&self, _: &dyn Transaction, _: BookId, _: AuthorId, _: AuthorRole, _: i32) -> Result<(), Error> {
            unimplemented!()
        }
        async fn add_book_identifier(&self, _: &dyn Transaction, _: BookId, _: IdentifierType, _: String) -> Result<(), Error> {
            unimplemented!()
        }
        async fn delete_book(&self, _: &dyn Transaction, _: BookId) -> Result<(), Error> {
            unimplemented!()
        }
    }

    struct MockJobRepository;
    #[async_trait::async_trait]
    impl JobRepository for MockJobRepository {
        async fn enqueue_raw(&self, _: &dyn Transaction, _: &str, _: serde_json::Value, _: i16) -> Result<Job, Error> {
            unimplemented!()
        }
        async fn claim_next(&self, _: &dyn Transaction) -> Result<Option<Job>, Error> {
            unimplemented!()
        }
        async fn complete(&self, _: &dyn Transaction, _: Job) -> Result<Job, Error> {
            unimplemented!()
        }
        async fn fail(&self, _: &dyn Transaction, _: Job, _: String) -> Result<Job, Error> {
            unimplemented!()
        }
        async fn reset_running_to_pending(&self, _: &dyn Transaction) -> Result<u64, Error> {
            unimplemented!()
        }
    }

    struct MockImportJobRepository;
    #[async_trait::async_trait]
    impl ImportJobRepository for MockImportJobRepository {
        async fn add_job(&self, _: &dyn Transaction, _: NewImportJob) -> Result<ImportJob, Error> {
            unimplemented!()
        }
        async fn update_job(&self, _: &dyn Transaction, _: ImportJob) -> Result<ImportJob, Error> {
            unimplemented!()
        }
        async fn find_by_id(&self, _: &dyn Transaction, _: ImportJobId) -> Result<Option<ImportJob>, Error> {
            unimplemented!()
        }
        async fn find_by_token(&self, _: &dyn Transaction, _: &ImportJobToken) -> Result<Option<ImportJob>, Error> {
            unimplemented!()
        }
        async fn find_by_hash(&self, _: &dyn Transaction, _: &str) -> Result<Option<ImportJob>, Error> {
            unimplemented!()
        }
        async fn list_by_status(&self, _: &dyn Transaction, _: ImportStatus, _: Option<ImportJobId>, _: Option<u64>) -> Result<Vec<ImportJob>, Error> {
            unimplemented!()
        }
        async fn reset_in_progress_to_pending(&self, _: &dyn Transaction) -> Result<u64, Error> {
            unimplemented!()
        }
        async fn delete_job(&self, _: &dyn Transaction, _: ImportJobId) -> Result<(), Error> {
            unimplemented!()
        }
    }

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
            RepositoryServiceBuilder::default()
                .repository(Arc::new(MockRepository) as Arc<dyn Repository>)
                .session_repository(Arc::new(MockSessionRepository) as Arc<dyn SessionRepository>)
                .user_repository(Arc::new(MockUserRepository) as Arc<dyn UserRepository>)
                .user_setting_repository(Arc::new(MockUserSettingRepository) as Arc<dyn UserSettingRepository>)
                .author_repository(Arc::new(author_repo) as Arc<dyn AuthorRepository>)
                .series_repository(Arc::new(series_repo) as Arc<dyn SeriesRepository>)
                .publisher_repository(Arc::new(MockPublisherRepository) as Arc<dyn PublisherRepository>)
                .genre_repository(Arc::new(MockGenreRepository) as Arc<dyn GenreRepository>)
                .tag_repository(Arc::new(MockTagRepository) as Arc<dyn TagRepository>)
                .book_repository(Arc::new(book_repo) as Arc<dyn BookRepository>)
                .import_job_repository(Arc::new(MockImportJobRepository) as Arc<dyn ImportJobRepository>)
                .job_repository(Arc::new(MockJobRepository) as Arc<dyn JobRepository>)
                .build()
                .expect("all fields provided"),
        );
        BookServiceImpl::new(repository_service)
    }

    fn default_service_with_book_repo(book_repo: MockBookRepository) -> BookServiceImpl {
        create_service(
            book_repo,
            MockAuthorRepository {
                list_authors_result: None,
                find_by_token_result: None,
            },
            MockSeriesRepository {
                list_series_result: None,
                find_by_token_result: None,
            },
        )
    }

    // ─── list_books ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_books_returns_results() {
        let books = vec![fake_book(1, "Dune"), fake_book(2, "Foundation")];
        let svc = default_service_with_book_repo(MockBookRepository::new().with_list_books(Ok(books)));

        let result = svc.list_books(&BookFilter::default(), None, None).await;

        assert!(result.is_ok());
        let list = result.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].title, "Dune");
        assert_eq!(list[1].title, "Foundation");
    }

    #[tokio::test]
    async fn test_list_books_returns_empty() {
        let svc = default_service_with_book_repo(MockBookRepository::new().with_list_books(Ok(vec![])));

        let result = svc.list_books(&BookFilter::default(), None, None).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_books_propagates_error() {
        let svc = default_service_with_book_repo(
            MockBookRepository::new().with_list_books(Err(Error::RepositoryError(RepositoryError::Database("db error".into())))),
        );

        let result = svc.list_books(&BookFilter::default(), None, None).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── find_book_by_token ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_book_by_token_found() {
        let book = fake_book(1, "Dune");
        let token = book.token;
        let svc = default_service_with_book_repo(MockBookRepository::new().with_find_by_token(Ok(Some(book))));

        let result = svc.find_book_by_token(&token).await;

        assert!(result.is_ok());
        let found = result.unwrap().unwrap();
        assert_eq!(found.id, 1);
        assert_eq!(found.title, "Dune");
    }

    #[tokio::test]
    async fn test_find_book_by_token_not_found() {
        let svc = default_service_with_book_repo(MockBookRepository::new().with_find_by_token(Ok(None)));

        let token = BookToken::generate();
        let result = svc.find_book_by_token(&token).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_find_book_by_token_propagates_error() {
        let svc = default_service_with_book_repo(
            MockBookRepository::new().with_find_by_token(Err(Error::RepositoryError(RepositoryError::Database("db error".into())))),
        );

        let token = BookToken::generate();
        let result = svc.find_book_by_token(&token).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── authors_for_book ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_authors_for_book_returns_results() {
        let authors = vec![BookAuthor::fake(1, 1, "author", 0), BookAuthor::fake(1, 2, "editor", 1)];
        let svc = default_service_with_book_repo(MockBookRepository::new().with_authors_for_book(Ok(authors)));

        let result = svc.authors_for_book(1).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_authors_for_book_returns_empty() {
        let svc = default_service_with_book_repo(MockBookRepository::new().with_authors_for_book(Ok(vec![])));

        let result = svc.authors_for_book(1).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_authors_for_book_propagates_error() {
        let svc = default_service_with_book_repo(
            MockBookRepository::new().with_authors_for_book(Err(Error::RepositoryError(RepositoryError::Database("db error".into())))),
        );

        let result = svc.authors_for_book(1).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── files_for_book ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_files_for_book_returns_results() {
        let files = vec![BookFile::fake(1, "epub"), BookFile::fake(1, "mobi")];
        let svc = default_service_with_book_repo(MockBookRepository::new().with_files_for_book(Ok(files)));

        let result = svc.files_for_book(1).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_files_for_book_returns_empty() {
        let svc = default_service_with_book_repo(MockBookRepository::new().with_files_for_book(Ok(vec![])));

        let result = svc.files_for_book(1).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_files_for_book_propagates_error() {
        let svc = default_service_with_book_repo(
            MockBookRepository::new().with_files_for_book(Err(Error::RepositoryError(RepositoryError::Database("db error".into())))),
        );

        let result = svc.files_for_book(1).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── identifiers_for_book ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_identifiers_for_book_returns_results() {
        let ids = vec![BookIdentifier::fake(1, "isbn13", "9780000000001")];
        let svc = default_service_with_book_repo(MockBookRepository::new().with_identifiers_for_book(Ok(ids)));

        let result = svc.identifiers_for_book(1).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_identifiers_for_book_propagates_error() {
        let svc = default_service_with_book_repo(
            MockBookRepository::new().with_identifiers_for_book(Err(Error::RepositoryError(RepositoryError::Database("db error".into())))),
        );

        let result = svc.identifiers_for_book(1).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── list_authors ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_authors_returns_results() {
        let authors = vec![fake_author(1, "Ursula K. Le Guin"), fake_author(2, "N.K. Jemisin")];
        let svc = create_service(
            MockBookRepository::new(),
            MockAuthorRepository::with_list_authors(Ok(authors)),
            MockSeriesRepository {
                list_series_result: None,
                find_by_token_result: None,
            },
        );

        let result = svc.list_authors(None, None).await;

        assert!(result.is_ok());
        let list = result.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Ursula K. Le Guin");
    }

    #[tokio::test]
    async fn test_list_authors_returns_empty() {
        let svc = create_service(
            MockBookRepository::new(),
            MockAuthorRepository::with_list_authors(Ok(vec![])),
            MockSeriesRepository {
                list_series_result: None,
                find_by_token_result: None,
            },
        );

        let result = svc.list_authors(None, None).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_authors_propagates_error() {
        let svc = create_service(
            MockBookRepository::new(),
            MockAuthorRepository::with_list_authors(Err(Error::RepositoryError(RepositoryError::Database("db error".into())))),
            MockSeriesRepository {
                list_series_result: None,
                find_by_token_result: None,
            },
        );

        let result = svc.list_authors(None, None).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── find_author_by_token ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_author_by_token_found() {
        let author = fake_author(1, "Brandon Sanderson");
        let token = author.token;
        let svc = create_service(
            MockBookRepository::new(),
            MockAuthorRepository::with_find_by_token(Ok(Some(author))),
            MockSeriesRepository {
                list_series_result: None,
                find_by_token_result: None,
            },
        );

        let result = svc.find_author_by_token(&token).await;

        assert!(result.is_ok());
        let found = result.unwrap().unwrap();
        assert_eq!(found.id, 1);
        assert_eq!(found.name, "Brandon Sanderson");
    }

    #[tokio::test]
    async fn test_find_author_by_token_not_found() {
        let svc = create_service(
            MockBookRepository::new(),
            MockAuthorRepository::with_find_by_token(Ok(None)),
            MockSeriesRepository {
                list_series_result: None,
                find_by_token_result: None,
            },
        );

        let result = svc.find_author_by_token(&AuthorToken::generate()).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    // ─── list_series ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_series_returns_results() {
        let series = vec![fake_series(1, "Stormlight Archive"), fake_series(2, "Mistborn")];
        let svc = create_service(
            MockBookRepository::new(),
            MockAuthorRepository {
                list_authors_result: None,
                find_by_token_result: None,
            },
            MockSeriesRepository::with_list_series(Ok(series)),
        );

        let result = svc.list_series(None, None).await;

        assert!(result.is_ok());
        let list = result.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Stormlight Archive");
    }

    #[tokio::test]
    async fn test_list_series_returns_empty() {
        let svc = create_service(
            MockBookRepository::new(),
            MockAuthorRepository {
                list_authors_result: None,
                find_by_token_result: None,
            },
            MockSeriesRepository::with_list_series(Ok(vec![])),
        );

        let result = svc.list_series(None, None).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_series_propagates_error() {
        let svc = create_service(
            MockBookRepository::new(),
            MockAuthorRepository {
                list_authors_result: None,
                find_by_token_result: None,
            },
            MockSeriesRepository::with_list_series(Err(Error::RepositoryError(RepositoryError::Database("db error".into())))),
        );

        let result = svc.list_series(None, None).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── find_series_by_token ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_series_by_token_found() {
        let series = fake_series(1, "Stormlight Archive");
        let token = series.token;
        let svc = create_service(
            MockBookRepository::new(),
            MockAuthorRepository {
                list_authors_result: None,
                find_by_token_result: None,
            },
            MockSeriesRepository::with_find_by_token(Ok(Some(series))),
        );

        let result = svc.find_series_by_token(&token).await;

        assert!(result.is_ok());
        let found = result.unwrap().unwrap();
        assert_eq!(found.id, 1);
        assert_eq!(found.name, "Stormlight Archive");
    }

    #[tokio::test]
    async fn test_find_series_by_token_not_found() {
        let svc = create_service(
            MockBookRepository::new(),
            MockAuthorRepository {
                list_authors_result: None,
                find_by_token_result: None,
            },
            MockSeriesRepository::with_find_by_token(Ok(None)),
        );

        let result = svc.find_series_by_token(&SeriesToken::generate()).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
