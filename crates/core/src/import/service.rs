use std::sync::Arc;

use chrono::Utc;

use crate::{
    Error,
    import::{ImportJob, ImportJobId, ImportJobToken, ImportStatus},
    repository::RepositoryService,
    user::UserId,
    with_read_only_transaction, with_transaction,
};

#[async_trait::async_trait]
pub trait ImportJobService: Send + Sync {
    async fn list_pending(&self, start_id: Option<ImportJobId>, page_size: Option<u64>) -> Result<Vec<ImportJob>, Error>;
    async fn find_by_token(&self, token: &ImportJobToken) -> Result<Option<ImportJob>, Error>;
    async fn approve_job(&self, job: ImportJob, reviewer_id: UserId) -> Result<ImportJob, Error>;
    async fn reject_job(&self, job: ImportJob, reviewer_id: UserId) -> Result<ImportJob, Error>;
}

pub(crate) struct ImportJobServiceImpl {
    repository_service: Arc<RepositoryService>,
}

impl ImportJobServiceImpl {
    pub(crate) fn new(repository_service: Arc<RepositoryService>) -> Self {
        Self { repository_service }
    }
}

#[async_trait::async_trait]
impl ImportJobService for ImportJobServiceImpl {
    #[tracing::instrument(level = "trace", skip(self))]
    async fn list_pending(&self, start_id: Option<ImportJobId>, page_size: Option<u64>) -> Result<Vec<ImportJob>, Error> {
        with_read_only_transaction!(self, import_job_repository, |tx| {
            import_job_repository.list_by_status(tx, ImportStatus::Pending, start_id, page_size).await
        })
    }

    #[tracing::instrument(level = "trace", skip(self, token))]
    async fn find_by_token(&self, token: &ImportJobToken) -> Result<Option<ImportJob>, Error> {
        let token = *token;
        with_read_only_transaction!(self, import_job_repository, |tx| import_job_repository.find_by_token(tx, &token).await)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn approve_job(&self, job: ImportJob, reviewer_id: UserId) -> Result<ImportJob, Error> {
        if job.status != ImportStatus::NeedsReview {
            return Err(Error::Validation(format!("cannot approve job with status {:?}", job.status)));
        }
        let approved = ImportJob {
            status: ImportStatus::Approved,
            reviewed_by: Some(reviewer_id),
            reviewed_at: Some(Utc::now()),
            ..job
        };
        with_transaction!(self, import_job_repository, |tx| import_job_repository.update_job(tx, approved).await)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn reject_job(&self, job: ImportJob, reviewer_id: UserId) -> Result<ImportJob, Error> {
        if job.status != ImportStatus::NeedsReview {
            return Err(Error::Validation(format!("cannot reject job with status {:?}", job.status)));
        }
        let rejected = ImportJob {
            status: ImportStatus::Rejected,
            reviewed_by: Some(reviewer_id),
            reviewed_at: Some(Utc::now()),
            ..job
        };
        with_transaction!(self, import_job_repository, |tx| import_job_repository.update_job(tx, rejected).await)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        any::Any,
        sync::{Arc, Mutex},
    };

    use chrono::Utc;

    use super::{ImportJobService, ImportJobServiceImpl};
    use crate::{
        Error, RepositoryError,
        auth::{NewSession, Session, repository::SessionRepository},
        book::{
            Author, AuthorId, AuthorRepository, AuthorRole, AuthorToken, Book, BookAuthor, BookFile, BookFilter, BookId, BookIdentifier, BookRepository,
            BookToken, FileFormat, Genre, GenreId, GenreRepository, GenreToken, IdentifierType, NewAuthor, NewBook, NewGenre, NewPublisher, NewSeries, NewTag,
            Publisher, PublisherId, PublisherRepository, PublisherToken, Series, SeriesId, SeriesRepository, SeriesToken, Tag, TagId, TagRepository, TagToken,
        },
        import::{ImportJob, ImportJobId, ImportJobRepository, ImportJobToken, ImportStatus, NewImportJob},
        repository::{Repository, RepositoryServiceBuilder, Transaction},
        user::{
            NewUser, NewUserSetting, User, UserId, UserSetting,
            repository::{UserRepository, UserSettingRepository},
        },
    };

    // ─── Mock Transaction ─────────────────────────────────────────────────────

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

    // ─── Mock Repository ──────────────────────────────────────────────────────

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

    // ─── Mock ImportJobRepository ─────────────────────────────────────────────

    #[derive(Default)]
    struct MockImportJobRepository {
        list_by_status_result: Mutex<Option<Result<Vec<ImportJob>, Error>>>,
        find_by_token_result: Mutex<Option<Result<Option<ImportJob>, Error>>>,
        update_job_result: Mutex<Option<Result<ImportJob, Error>>>,
    }

    impl MockImportJobRepository {
        fn with_list_by_status_result(self, result: Result<Vec<ImportJob>, Error>) -> Self {
            *self.list_by_status_result.lock().unwrap() = Some(result);
            self
        }

        fn with_find_by_token_result(self, result: Result<Option<ImportJob>, Error>) -> Self {
            *self.find_by_token_result.lock().unwrap() = Some(result);
            self
        }

        fn with_update_job_result(self, result: Result<ImportJob, Error>) -> Self {
            *self.update_job_result.lock().unwrap() = Some(result);
            self
        }
    }

    #[async_trait::async_trait]
    impl ImportJobRepository for MockImportJobRepository {
        async fn add_job(&self, _: &dyn Transaction, _: NewImportJob) -> Result<ImportJob, Error> {
            unimplemented!()
        }

        async fn update_job(&self, _: &dyn Transaction, _: ImportJob) -> Result<ImportJob, Error> {
            self.update_job_result
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("update_job")))
        }

        async fn find_by_id(&self, _: &dyn Transaction, _: ImportJobId) -> Result<Option<ImportJob>, Error> {
            unimplemented!()
        }

        async fn find_by_token(&self, _: &dyn Transaction, _: &ImportJobToken) -> Result<Option<ImportJob>, Error> {
            self.find_by_token_result
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("find_by_token")))
        }

        async fn find_by_hash(&self, _: &dyn Transaction, _: &str) -> Result<Option<ImportJob>, Error> {
            unimplemented!()
        }

        async fn list_by_status(&self, _: &dyn Transaction, _: ImportStatus, _: Option<ImportJobId>, _: Option<u64>) -> Result<Vec<ImportJob>, Error> {
            self.list_by_status_result
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("list_by_status")))
        }
    }

    // ─── Stub repositories (unused by ImportJobService) ───────────────────────

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

    struct MockAuthorRepository;
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
            unimplemented!()
        }
        async fn list_authors(&self, _: &dyn Transaction, _: Option<AuthorId>, _: Option<u64>) -> Result<Vec<Author>, Error> {
            unimplemented!()
        }
        async fn find_by_name(&self, _: &dyn Transaction, _: &str) -> Result<Option<Author>, Error> {
            unimplemented!()
        }
    }

    struct MockSeriesRepository;
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
            unimplemented!()
        }
        async fn list_series(&self, _: &dyn Transaction, _: Option<SeriesId>, _: Option<u64>) -> Result<Vec<Series>, Error> {
            unimplemented!()
        }
        async fn find_by_name(&self, _: &dyn Transaction, _: &str) -> Result<Option<Series>, Error> {
            unimplemented!()
        }
    }

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

    struct MockBookRepository;
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
            unimplemented!()
        }
        async fn list_books(&self, _: &dyn Transaction, _: &BookFilter, _: Option<BookId>, _: Option<u64>) -> Result<Vec<Book>, Error> {
            unimplemented!()
        }
        async fn authors_for_book(&self, _: &dyn Transaction, _: BookId) -> Result<Vec<BookAuthor>, Error> {
            unimplemented!()
        }
        async fn files_for_book(&self, _: &dyn Transaction, _: BookId) -> Result<Vec<BookFile>, Error> {
            unimplemented!()
        }
        async fn identifiers_for_book(&self, _: &dyn Transaction, _: BookId) -> Result<Vec<BookIdentifier>, Error> {
            unimplemented!()
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
    }

    // ─── Helper ───────────────────────────────────────────────────────────────

    fn create_service(mock: MockImportJobRepository) -> ImportJobServiceImpl {
        let repository_service = Arc::new(
            RepositoryServiceBuilder::default()
                .repository(Arc::new(MockRepository) as Arc<dyn Repository>)
                .session_repository(Arc::new(MockSessionRepository) as Arc<dyn SessionRepository>)
                .user_repository(Arc::new(MockUserRepository) as Arc<dyn UserRepository>)
                .user_setting_repository(Arc::new(MockUserSettingRepository) as Arc<dyn UserSettingRepository>)
                .author_repository(Arc::new(MockAuthorRepository) as Arc<dyn AuthorRepository>)
                .series_repository(Arc::new(MockSeriesRepository) as Arc<dyn SeriesRepository>)
                .publisher_repository(Arc::new(MockPublisherRepository) as Arc<dyn PublisherRepository>)
                .genre_repository(Arc::new(MockGenreRepository) as Arc<dyn GenreRepository>)
                .tag_repository(Arc::new(MockTagRepository) as Arc<dyn TagRepository>)
                .book_repository(Arc::new(MockBookRepository) as Arc<dyn BookRepository>)
                .import_job_repository(Arc::new(mock) as Arc<dyn ImportJobRepository>)
                .build()
                .expect("all fields provided"),
        );
        ImportJobServiceImpl::new(repository_service)
    }

    fn fake_job(status: ImportStatus) -> ImportJob {
        let id: ImportJobId = 1;
        ImportJob {
            id,
            version: 1,
            token: ImportJobToken::new(id),
            file_path: "/watch/test.epub".to_owned(),
            file_hash: "abc123".to_owned(),
            file_format: crate::book::FileFormat::Epub,
            detected_at: Utc::now(),
            status,
            candidate_book_id: None,
            metadata_source: None,
            error_message: None,
            reviewed_by: None,
            reviewed_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    // ─── list_pending ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_pending_returns_jobs() {
        let jobs = vec![fake_job(ImportStatus::Pending)];
        let svc = create_service(MockImportJobRepository::default().with_list_by_status_result(Ok(jobs)));

        let result = svc.list_pending(None, None).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_list_pending_returns_empty() {
        let svc = create_service(MockImportJobRepository::default().with_list_by_status_result(Ok(vec![])));

        let result = svc.list_pending(None, None).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_pending_propagates_error() {
        let svc = create_service(
            MockImportJobRepository::default().with_list_by_status_result(Err(Error::RepositoryError(RepositoryError::Database("db error".into())))),
        );

        let result = svc.list_pending(None, None).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── find_by_token ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_token_found() {
        let job = fake_job(ImportStatus::Pending);
        let token = job.token;
        let svc = create_service(MockImportJobRepository::default().with_find_by_token_result(Ok(Some(job))));

        let result = svc.find_by_token(&token).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_find_by_token_not_found() {
        let token = ImportJobToken::new(99);
        let svc = create_service(MockImportJobRepository::default().with_find_by_token_result(Ok(None)));

        let result = svc.find_by_token(&token).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_find_by_token_propagates_error() {
        let token = ImportJobToken::new(1);
        let svc = create_service(
            MockImportJobRepository::default().with_find_by_token_result(Err(Error::RepositoryError(RepositoryError::Database("db error".into())))),
        );

        let result = svc.find_by_token(&token).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── approve_job ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_approve_job_success() {
        let job = fake_job(ImportStatus::NeedsReview);
        let approved = ImportJob {
            status: ImportStatus::Approved,
            ..job.clone()
        };
        let svc = create_service(MockImportJobRepository::default().with_update_job_result(Ok(approved)));

        let result = svc.approve_job(job, 1).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().status, ImportStatus::Approved);
    }

    #[tokio::test]
    async fn test_approve_job_wrong_status_returns_validation_error() {
        let svc = create_service(MockImportJobRepository::default());

        for status in [ImportStatus::Pending, ImportStatus::Approved, ImportStatus::Rejected, ImportStatus::Error] {
            let label = format!("{status:?}");
            let result = svc.approve_job(fake_job(status), 1).await;
            assert!(matches!(result, Err(Error::Validation(_))), "expected Validation error for {label}");
        }
    }

    #[tokio::test]
    async fn test_approve_job_propagates_update_error() {
        let job = fake_job(ImportStatus::NeedsReview);
        let svc = create_service(MockImportJobRepository::default().with_update_job_result(Err(Error::RepositoryError(RepositoryError::Conflict))));

        let result = svc.approve_job(job, 1).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Conflict))));
    }

    // ─── reject_job ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_reject_job_success() {
        let job = fake_job(ImportStatus::NeedsReview);
        let rejected = ImportJob {
            status: ImportStatus::Rejected,
            ..job.clone()
        };
        let svc = create_service(MockImportJobRepository::default().with_update_job_result(Ok(rejected)));

        let result = svc.reject_job(job, 1).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().status, ImportStatus::Rejected);
    }

    #[tokio::test]
    async fn test_reject_job_wrong_status_returns_validation_error() {
        let svc = create_service(MockImportJobRepository::default());

        for status in [ImportStatus::Pending, ImportStatus::Approved, ImportStatus::Rejected, ImportStatus::Error] {
            let label = format!("{status:?}");
            let result = svc.reject_job(fake_job(status), 1).await;
            assert!(matches!(result, Err(Error::Validation(_))), "expected Validation error for {label}");
        }
    }

    #[tokio::test]
    async fn test_reject_job_propagates_update_error() {
        let job = fake_job(ImportStatus::NeedsReview);
        let svc = create_service(MockImportJobRepository::default().with_update_job_result(Err(Error::RepositoryError(RepositoryError::Conflict))));

        let result = svc.reject_job(job, 1).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Conflict))));
    }
}
