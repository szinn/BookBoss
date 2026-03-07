use std::sync::Arc;

use crate::{
    Error, RepositoryError,
    repository::RepositoryService,
    user::{NewUser, User, UserId, UserToken},
    with_read_only_transaction, with_transaction,
};

#[async_trait::async_trait]
pub trait UserService: Send + Sync {
    async fn add_user(&self, user: NewUser) -> Result<User, Error>;
    async fn update_user(&self, user: User) -> Result<User, Error>;
    async fn list_users(&self, start_id: Option<UserId>, page_size: Option<u64>) -> Result<Vec<User>, Error>;
    async fn delete_user(&self, id: UserId) -> Result<User, Error>;
    async fn find_by_id(&self, id: UserId) -> Result<Option<User>, Error>;
    async fn find_by_token(&self, token: UserToken) -> Result<Option<User>, Error>;
    async fn find_by_username(&self, username: &str) -> Result<Option<User>, Error>;
}

pub(crate) struct UserServiceImpl {
    repository_service: Arc<RepositoryService>,
}

impl UserServiceImpl {
    pub(crate) fn new(repository_service: Arc<RepositoryService>) -> Self {
        Self { repository_service }
    }
}

#[async_trait::async_trait]
impl UserService for UserServiceImpl {
    #[tracing::instrument(level = "trace", skip(self, user))]
    async fn add_user(&self, user: NewUser) -> Result<User, Error> {
        with_transaction!(self, user_repository, |tx| user_repository.add_user(tx, user).await)
    }

    #[tracing::instrument(level = "trace", skip(self, user))]
    async fn update_user(&self, user: User) -> Result<User, Error> {
        with_transaction!(self, user_repository, |tx| user_repository.update_user(tx, user).await)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn list_users(&self, start_id: Option<UserId>, page_size: Option<u64>) -> Result<Vec<User>, Error> {
        with_read_only_transaction!(self, user_repository, |tx| user_repository.list_users(tx, start_id, page_size).await)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn delete_user(&self, id: UserId) -> Result<User, Error> {
        with_transaction!(self, user_repository, |tx| {
            let user = user_repository
                .find_by_id(tx, id)
                .await?
                .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

            user_repository.delete_user(tx, user).await
        })
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn find_by_id(&self, id: UserId) -> Result<Option<User>, Error> {
        with_read_only_transaction!(self, user_repository, |tx| user_repository.find_by_id(tx, id).await)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn find_by_token(&self, token: UserToken) -> Result<Option<User>, Error> {
        with_read_only_transaction!(self, user_repository, |tx| user_repository.find_by_id(tx, token.id()).await)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn find_by_username(&self, username: &str) -> Result<Option<User>, Error> {
        let username = username.to_owned();
        with_read_only_transaction!(self, user_repository, |tx| user_repository.find_by_username(tx, &username).await)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        any::Any,
        collections::HashSet,
        sync::{Arc, Mutex},
    };

    use super::{UserService, UserServiceImpl};
    use crate::{
        Error, RepositoryError,
        auth::{NewSession, Session, repository::SessionRepository},
        book::{
            Author, AuthorId, AuthorRepository, AuthorRole, AuthorToken, Book, BookAuthor, BookFile, BookFilter, BookId, BookIdentifier, BookRepository,
            BookToken, FileFormat, Genre, GenreId, GenreRepository, GenreToken, IdentifierType, NewAuthor, NewBook, NewGenre, NewPublisher, NewSeries, NewTag,
            Publisher, PublisherId, PublisherRepository, PublisherToken, Series, SeriesId, SeriesRepository, SeriesToken, Tag, TagId, TagRepository, TagToken,
        },
        import::{ImportJob, ImportJobId, ImportJobRepository, ImportJobToken, ImportStatus, NewImportJob},
        jobs::{Job, JobRepository},
        repository::{Repository, RepositoryServiceBuilder, Transaction},
        user::{
            NewUser, NewUserSetting, User, UserId, UserSetting, UserToken,
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

    // ─── Mock SessionRepository ──────────────────────────────────────────────

    struct MockSessionRepository;

    #[async_trait::async_trait]
    impl SessionRepository for MockSessionRepository {
        async fn count(&self, _tx: &dyn Transaction) -> Result<i64, Error> {
            unimplemented!()
        }
        async fn store(&self, _tx: &dyn Transaction, _session: NewSession) -> Result<Session, Error> {
            unimplemented!()
        }
        async fn load(&self, _tx: &dyn Transaction, _id: &str) -> Result<Option<Session>, Error> {
            unimplemented!()
        }
        async fn delete_by_id(&self, _tx: &dyn Transaction, _id: &str) -> Result<(), Error> {
            unimplemented!()
        }
        async fn exists(&self, _tx: &dyn Transaction, _id: &str) -> Result<bool, Error> {
            unimplemented!()
        }
        async fn delete_by_expiry(&self, _tx: &dyn Transaction) -> Result<Vec<String>, Error> {
            unimplemented!()
        }
        async fn delete_all(&self, _tx: &dyn Transaction) -> Result<(), Error> {
            unimplemented!()
        }
        async fn get_ids(&self, _tx: &dyn Transaction) -> Result<Vec<String>, Error> {
            unimplemented!()
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

    // ─── Mock UserRepository ─────────────────────────────────────────────────

    #[derive(Default)]
    struct MockUserRepository {
        add_user_result: Mutex<Option<Result<User, Error>>>,
        update_user_result: Mutex<Option<Result<User, Error>>>,
        delete_user_result: Mutex<Option<Result<User, Error>>>,
        find_by_id_result: Mutex<Option<Result<Option<User>, Error>>>,
        find_by_username_result: Mutex<Option<Result<Option<User>, Error>>>,
        list_users_result: Mutex<Option<Result<Vec<User>, Error>>>,
    }

    impl MockUserRepository {
        fn with_add_user_result(self, result: Result<User, Error>) -> Self {
            *self.add_user_result.lock().unwrap() = Some(result);
            self
        }

        fn with_update_user_result(self, result: Result<User, Error>) -> Self {
            *self.update_user_result.lock().unwrap() = Some(result);
            self
        }

        fn with_delete_user_result(self, result: Result<User, Error>) -> Self {
            *self.delete_user_result.lock().unwrap() = Some(result);
            self
        }

        fn with_find_by_id_result(self, result: Result<Option<User>, Error>) -> Self {
            *self.find_by_id_result.lock().unwrap() = Some(result);
            self
        }

        fn with_find_by_username_result(self, result: Result<Option<User>, Error>) -> Self {
            *self.find_by_username_result.lock().unwrap() = Some(result);
            self
        }

        fn with_list_users_result(self, result: Result<Vec<User>, Error>) -> Self {
            *self.list_users_result.lock().unwrap() = Some(result);
            self
        }
    }

    #[async_trait::async_trait]
    impl UserRepository for MockUserRepository {
        async fn add_user(&self, _tx: &dyn Transaction, _user: NewUser) -> Result<User, Error> {
            self.add_user_result
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("add_user")))
        }

        async fn update_user(&self, _tx: &dyn Transaction, _user: User) -> Result<User, Error> {
            self.update_user_result
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("update_user")))
        }

        async fn delete_user(&self, _tx: &dyn Transaction, _user: User) -> Result<User, Error> {
            self.delete_user_result
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("delete_user")))
        }

        async fn list_users(&self, _tx: &dyn Transaction, _start_id: Option<UserId>, _page_size: Option<u64>) -> Result<Vec<User>, Error> {
            self.list_users_result
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("list_users")))
        }

        async fn find_by_id(&self, _tx: &dyn Transaction, _id: UserId) -> Result<Option<User>, Error> {
            self.find_by_id_result
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("find_by_id")))
        }

        async fn find_by_username(&self, _tx: &dyn Transaction, _username: &str) -> Result<Option<User>, Error> {
            self.find_by_username_result
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("find_by_username")))
        }
    }

    // ─── Mock UserSettingRepository ──────────────────────────────────────────

    struct MockUserSettingRepository;

    #[async_trait::async_trait]
    impl UserSettingRepository for MockUserSettingRepository {
        async fn get(&self, _tx: &dyn Transaction, _user_id: UserId, _key: &str) -> Result<Option<UserSetting>, Error> {
            unimplemented!()
        }
        async fn set(&self, _tx: &dyn Transaction, _setting: NewUserSetting) -> Result<UserSetting, Error> {
            unimplemented!()
        }
        async fn delete(&self, _tx: &dyn Transaction, _user_id: UserId, _key: &str) -> Result<(), Error> {
            unimplemented!()
        }
        async fn list_by_user(&self, _tx: &dyn Transaction, _user_id: UserId) -> Result<Vec<UserSetting>, Error> {
            unimplemented!()
        }
    }

    // ─── Mock Book Repositories ──────────────────────────────────────────────

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
        async fn count_authors(&self, _: &dyn Transaction) -> Result<u64, Error> {
            unimplemented!()
        }
        async fn delete_author(&self, _: &dyn Transaction, _: AuthorId) -> Result<(), Error> {
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
        async fn delete_book(&self, _: &dyn Transaction, _: BookId) -> Result<(), Error> {
            unimplemented!()
        }
        async fn delete_book_authors(&self, _: &dyn Transaction, _: BookId) -> Result<(), Error> {
            unimplemented!()
        }
        async fn delete_book_identifiers(&self, _: &dyn Transaction, _: BookId) -> Result<(), Error> {
            unimplemented!()
        }
        async fn count_available_books(&self, _: &dyn Transaction) -> Result<u64, Error> {
            unimplemented!()
        }
        async fn count_books_for_author(&self, _: &dyn Transaction, _: AuthorId) -> Result<u64, Error> {
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
        async fn find_by_candidate_book_id(&self, _: &dyn Transaction, _: BookId) -> Result<Option<ImportJob>, Error> {
            unimplemented!()
        }
        async fn delete_job(&self, _: &dyn Transaction, _: ImportJobId) -> Result<(), Error> {
            unimplemented!()
        }
        async fn approve_job(&self, _: &dyn Transaction, _: ImportJobId) -> Result<(), Error> {
            unimplemented!()
        }
    }

    // ─── Helper ──────────────────────────────────────────────────────────────

    fn create_service(mock: MockUserRepository) -> UserServiceImpl {
        let repository_service = Arc::new(
            RepositoryServiceBuilder::default()
                .repository(Arc::new(MockRepository) as Arc<dyn Repository>)
                .session_repository(Arc::new(MockSessionRepository) as Arc<dyn SessionRepository>)
                .user_repository(Arc::new(mock) as Arc<dyn UserRepository>)
                .user_setting_repository(Arc::new(MockUserSettingRepository) as Arc<dyn UserSettingRepository>)
                .author_repository(Arc::new(MockAuthorRepository) as Arc<dyn AuthorRepository>)
                .series_repository(Arc::new(MockSeriesRepository) as Arc<dyn SeriesRepository>)
                .publisher_repository(Arc::new(MockPublisherRepository) as Arc<dyn PublisherRepository>)
                .genre_repository(Arc::new(MockGenreRepository) as Arc<dyn GenreRepository>)
                .tag_repository(Arc::new(MockTagRepository) as Arc<dyn TagRepository>)
                .book_repository(Arc::new(MockBookRepository) as Arc<dyn BookRepository>)
                .import_job_repository(Arc::new(MockImportJobRepository) as Arc<dyn ImportJobRepository>)
                .job_repository(Arc::new(MockJobRepository) as Arc<dyn JobRepository>)
                .build()
                .expect("all fields provided"),
        );
        UserServiceImpl::new(repository_service)
    }

    // ─── add_user ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_add_user_success() {
        let expected = User::fake(1, "alice", "hash", "alice@example.com", HashSet::new());
        let svc = create_service(MockUserRepository::default().with_add_user_result(Ok(expected)));

        let result = svc.add_user(NewUser::new("alice", "hash", "alice@example.com", HashSet::new()).unwrap()).await;

        assert!(result.is_ok());
        let user = result.unwrap();
        assert_eq!(user.id, 1);
        assert_eq!(user.username, "alice");
        assert_eq!(user.email_address.as_str(), "alice@example.com");
    }

    #[tokio::test]
    async fn test_add_user_propagates_constraint_error() {
        let svc = create_service(
            MockUserRepository::default().with_add_user_result(Err(Error::RepositoryError(RepositoryError::Constraint("duplicate email".into())))),
        );

        let result = svc.add_user(NewUser::new("alice", "hash", "alice@example.com", HashSet::new()).unwrap()).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Constraint(_)))));
    }

    // ─── update_user ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_update_user_success() {
        let updated = User::fake(1, "alice-updated", "newhash", "new@example.com", HashSet::new());
        let svc = create_service(MockUserRepository::default().with_update_user_result(Ok(updated)));

        let result = svc.update_user(User::fake(1, "alice", "hash", "alice@example.com", HashSet::new())).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().username, "alice-updated");
    }

    #[tokio::test]
    async fn test_update_user_not_found() {
        let svc = create_service(MockUserRepository::default().with_update_user_result(Err(Error::RepositoryError(RepositoryError::NotFound))));

        let result = svc.update_user(User::fake(999, "ghost", "hash", "ghost@example.com", HashSet::new())).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::NotFound))));
    }

    // ─── list_users ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_users_returns_all() {
        let users = vec![
            User::fake(1, "alice", "h1", "alice@example.com", HashSet::new()),
            User::fake(2, "bob", "h2", "bob@example.com", HashSet::new()),
        ];
        let svc = create_service(MockUserRepository::default().with_list_users_result(Ok(users)));

        let result = svc.list_users(None, None).await;

        assert!(result.is_ok());
        let list = result.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].username, "alice");
        assert_eq!(list[1].username, "bob");
    }

    #[tokio::test]
    async fn test_list_users_empty() {
        let svc = create_service(MockUserRepository::default().with_list_users_result(Ok(vec![])));

        let result = svc.list_users(None, None).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // ─── delete_user ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_user_success() {
        let user = User::fake(1, "alice", "hash", "alice@example.com", HashSet::new());
        let svc = create_service(
            MockUserRepository::default()
                .with_find_by_id_result(Ok(Some(user.clone())))
                .with_delete_user_result(Ok(user)),
        );

        let result = svc.delete_user(1).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, 1);
    }

    #[tokio::test]
    async fn test_delete_user_not_found() {
        let svc = create_service(MockUserRepository::default().with_find_by_id_result(Ok(None)));

        let result = svc.delete_user(999).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::NotFound))));
    }

    // ─── find_by_id ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_id_found() {
        let user = User::fake(1, "alice", "hash", "alice@example.com", HashSet::new());
        let svc = create_service(MockUserRepository::default().with_find_by_id_result(Ok(Some(user))));

        let result = svc.find_by_id(1).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().unwrap().username, "alice");
    }

    #[tokio::test]
    async fn test_find_by_id_not_found() {
        let svc = create_service(MockUserRepository::default().with_find_by_id_result(Ok(None)));

        let result = svc.find_by_id(999).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    // ─── find_by_token ───────────────────────────────────────────────────────
    // The service extracts token.id() and delegates to find_by_id, so we
    // configure find_by_id rather than a separate token mock.

    #[tokio::test]
    async fn test_find_by_token_found() {
        let user = User::fake(1, "alice", "hash", "alice@example.com", HashSet::new());
        let token = user.token;
        let svc =
            create_service(MockUserRepository::default().with_find_by_id_result(Ok(Some(User::fake(1, "alice", "hash", "alice@example.com", HashSet::new())))));

        let result = svc.find_by_token(token).await;

        assert!(result.is_ok());
        let found = result.unwrap().unwrap();
        assert_eq!(found.id, 1);
        assert_eq!(found.username, "alice");
    }

    #[tokio::test]
    async fn test_find_by_token_not_found() {
        let svc = create_service(MockUserRepository::default().with_find_by_id_result(Ok(None)));

        let result = svc.find_by_token(UserToken::generate()).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    // ─── find_by_username ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_username_found() {
        let user = User::fake(1, "alice", "hash", "alice@example.com", HashSet::new());
        let svc = create_service(MockUserRepository::default().with_find_by_username_result(Ok(Some(user))));

        let result = svc.find_by_username("alice").await;

        assert!(result.is_ok());
        let found = result.unwrap().unwrap();
        assert_eq!(found.id, 1);
        assert_eq!(found.username, "alice");
    }

    #[tokio::test]
    async fn test_find_by_username_not_found() {
        let svc = create_service(MockUserRepository::default().with_find_by_username_result(Ok(None)));

        let result = svc.find_by_username("nobody").await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_find_by_username_propagates_error() {
        let svc = create_service(MockUserRepository::default().with_find_by_username_result(Err(Error::RepositoryError(RepositoryError::NotFound))));

        let result = svc.find_by_username("alice").await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::NotFound))));
    }
}
