use std::{sync::Arc, time::Duration};

use crate::{
    Error,
    auth::{NewSession, Session},
    repository::RepositoryService,
    user::User,
    with_read_only_transaction, with_transaction,
};

#[async_trait::async_trait]
pub trait AuthService: Send + Sync {
    async fn count(&self) -> Result<i64, Error>;
    async fn store(&self, session: NewSession) -> Result<Session, Error>;
    async fn load(&self, id: &str) -> Result<Option<Session>, Error>;
    async fn delete_by_id(&self, id: &str) -> Result<(), Error>;
    async fn exists(&self, id: &str) -> Result<bool, Error>;
    async fn delete_by_expiry(&self) -> Result<Vec<String>, Error>;
    async fn delete_all(&self) -> Result<(), Error>;
    async fn get_ids(&self) -> Result<Vec<String>, Error>;
    async fn is_valid_login(&self, username: &str, password: &str) -> Result<Option<User>, Error>;
}

pub(crate) struct AuthServiceImpl {
    repository_service: Arc<RepositoryService>,
}

impl AuthServiceImpl {
    pub(crate) fn new(repository_service: Arc<RepositoryService>) -> Self {
        Self { repository_service }
    }
}

#[async_trait::async_trait]
impl AuthService for AuthServiceImpl {
    async fn count(&self) -> Result<i64, Error> {
        with_transaction!(self, session_repository, |tx| session_repository.count(tx).await)
    }

    async fn store(&self, session: NewSession) -> Result<Session, Error> {
        with_transaction!(self, session_repository, |tx| session_repository.store(tx, session).await)
    }

    async fn load(&self, id: &str) -> Result<Option<Session>, Error> {
        let id = id.to_owned();
        with_transaction!(self, session_repository, |tx| session_repository.load(tx, &id).await)
    }
    async fn delete_by_id(&self, id: &str) -> Result<(), Error> {
        let id = id.to_owned();
        with_transaction!(self, session_repository, |tx| session_repository.delete_by_id(tx, &id).await)
    }
    async fn exists(&self, id: &str) -> Result<bool, Error> {
        let id = id.to_owned();
        with_transaction!(self, session_repository, |tx| session_repository.exists(tx, &id).await)
    }
    async fn delete_by_expiry(&self) -> Result<Vec<String>, Error> {
        with_transaction!(self, session_repository, |tx| session_repository.delete_by_expiry(tx).await)
    }
    async fn delete_all(&self) -> Result<(), Error> {
        with_transaction!(self, session_repository, |tx| session_repository.delete_all(tx).await)
    }
    async fn get_ids(&self) -> Result<Vec<String>, Error> {
        with_transaction!(self, session_repository, |tx| session_repository.get_ids(tx).await)
    }

    #[tracing::instrument(level = "trace", skip(self, password))]
    async fn is_valid_login(&self, username: &str, password: &str) -> Result<Option<User>, Error> {
        let username = username.to_owned();
        let password = password.to_owned();
        let user = with_read_only_transaction!(self, user_repository, |tx| user_repository.find_by_username(tx, &username).await)?;
        match user {
            Some(user) if user.check_password(&password) => Ok(Some(user)),
            Some(_) => Ok(None),
            None => {
                // Delay when the user isn't found to normalise response time and
                // prevent username enumeration via timing attacks.
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        any::Any,
        collections::HashSet,
        sync::{Arc, Mutex},
    };

    use chrono::Utc;

    use super::{AuthService, AuthServiceImpl};
    use crate::{
        Error, RepositoryError,
        auth::{NewSession, Session, SessionBuilder, repository::SessionRepository},
        book::{
            Author, AuthorId, AuthorToken, AuthorRepository, NewAuthor,
            Genre, GenreId, GenreToken, GenreRepository, NewGenre,
            Publisher, PublisherId, PublisherToken, PublisherRepository, NewPublisher,
            Series, SeriesId, SeriesToken, SeriesRepository, NewSeries,
            Tag, TagId, TagToken, TagRepository, NewTag,
        },
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

    // ─── Mock UserRepository ─────────────────────────────────────────────────

    #[derive(Default)]
    struct MockUserRepository {
        find_by_username_result: Mutex<Option<Result<Option<User>, Error>>>,
    }

    impl MockUserRepository {
        fn with_find_by_username_result(self, result: Result<Option<User>, Error>) -> Self {
            *self.find_by_username_result.lock().unwrap() = Some(result);
            self
        }
    }

    #[async_trait::async_trait]
    impl UserRepository for MockUserRepository {
        async fn add_user(&self, _tx: &dyn Transaction, _user: NewUser) -> Result<User, Error> {
            Err(Error::MockNotConfigured("add_user"))
        }

        async fn update_user(&self, _tx: &dyn Transaction, _user: User) -> Result<User, Error> {
            Err(Error::MockNotConfigured("update_user"))
        }

        async fn delete_user(&self, _tx: &dyn Transaction, _user: User) -> Result<User, Error> {
            Err(Error::MockNotConfigured("delete_user"))
        }

        async fn list_users(&self, _tx: &dyn Transaction, _start_id: Option<UserId>, _page_size: Option<u64>) -> Result<Vec<User>, Error> {
            Err(Error::MockNotConfigured("list_users"))
        }

        async fn find_by_id(&self, _tx: &dyn Transaction, _id: UserId) -> Result<Option<User>, Error> {
            Err(Error::MockNotConfigured("find_by_id"))
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

    // ─── Mock SessionRepository ───────────────────────────────────────────────

    #[derive(Default)]
    struct MockSessionRepository {
        count_result: Mutex<Option<Result<i64, Error>>>,
        store_result: Mutex<Option<Result<Session, Error>>>,
        load_result: Mutex<Option<Result<Option<Session>, Error>>>,
        delete_by_id_result: Mutex<Option<Result<(), Error>>>,
        exists_result: Mutex<Option<Result<bool, Error>>>,
        delete_by_expiry_result: Mutex<Option<Result<Vec<String>, Error>>>,
        delete_all_result: Mutex<Option<Result<(), Error>>>,
        get_ids_result: Mutex<Option<Result<Vec<String>, Error>>>,
    }

    impl MockSessionRepository {
        fn with_count_result(self, result: Result<i64, Error>) -> Self {
            *self.count_result.lock().unwrap() = Some(result);
            self
        }

        fn with_store_result(self, result: Result<Session, Error>) -> Self {
            *self.store_result.lock().unwrap() = Some(result);
            self
        }

        fn with_load_result(self, result: Result<Option<Session>, Error>) -> Self {
            *self.load_result.lock().unwrap() = Some(result);
            self
        }

        fn with_delete_by_id_result(self, result: Result<(), Error>) -> Self {
            *self.delete_by_id_result.lock().unwrap() = Some(result);
            self
        }

        fn with_exists_result(self, result: Result<bool, Error>) -> Self {
            *self.exists_result.lock().unwrap() = Some(result);
            self
        }

        fn with_delete_by_expiry_result(self, result: Result<Vec<String>, Error>) -> Self {
            *self.delete_by_expiry_result.lock().unwrap() = Some(result);
            self
        }

        fn with_delete_all_result(self, result: Result<(), Error>) -> Self {
            *self.delete_all_result.lock().unwrap() = Some(result);
            self
        }

        fn with_get_ids_result(self, result: Result<Vec<String>, Error>) -> Self {
            *self.get_ids_result.lock().unwrap() = Some(result);
            self
        }
    }

    #[async_trait::async_trait]
    impl SessionRepository for MockSessionRepository {
        async fn count(&self, _tx: &dyn Transaction) -> Result<i64, Error> {
            self.count_result
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("count")))
        }

        async fn store(&self, _tx: &dyn Transaction, _session: NewSession) -> Result<Session, Error> {
            self.store_result
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("store")))
        }

        async fn load(&self, _tx: &dyn Transaction, _id: &str) -> Result<Option<Session>, Error> {
            self.load_result
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("load")))
        }

        async fn delete_by_id(&self, _tx: &dyn Transaction, _id: &str) -> Result<(), Error> {
            self.delete_by_id_result
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("delete_by_id")))
        }

        async fn exists(&self, _tx: &dyn Transaction, _id: &str) -> Result<bool, Error> {
            self.exists_result
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("exists")))
        }

        async fn delete_by_expiry(&self, _tx: &dyn Transaction) -> Result<Vec<String>, Error> {
            self.delete_by_expiry_result
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("delete_by_expiry")))
        }

        async fn delete_all(&self, _tx: &dyn Transaction) -> Result<(), Error> {
            self.delete_all_result
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("delete_all")))
        }

        async fn get_ids(&self, _tx: &dyn Transaction) -> Result<Vec<String>, Error> {
            self.get_ids_result
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("get_ids")))
        }
    }

    // ─── Mock Book Repositories ──────────────────────────────────────────────

    struct MockAuthorRepository;
    #[async_trait::async_trait]
    impl AuthorRepository for MockAuthorRepository {
        async fn add_author(&self, _: &dyn Transaction, _: NewAuthor) -> Result<Author, Error> { unimplemented!() }
        async fn update_author(&self, _: &dyn Transaction, _: Author) -> Result<Author, Error> { unimplemented!() }
        async fn find_by_id(&self, _: &dyn Transaction, _: AuthorId) -> Result<Option<Author>, Error> { unimplemented!() }
        async fn find_by_token(&self, _: &dyn Transaction, _: &AuthorToken) -> Result<Option<Author>, Error> { unimplemented!() }
        async fn list_authors(&self, _: &dyn Transaction, _: Option<AuthorId>, _: Option<u64>) -> Result<Vec<Author>, Error> { unimplemented!() }
    }

    struct MockSeriesRepository;
    #[async_trait::async_trait]
    impl SeriesRepository for MockSeriesRepository {
        async fn add_series(&self, _: &dyn Transaction, _: NewSeries) -> Result<Series, Error> { unimplemented!() }
        async fn update_series(&self, _: &dyn Transaction, _: Series) -> Result<Series, Error> { unimplemented!() }
        async fn find_by_id(&self, _: &dyn Transaction, _: SeriesId) -> Result<Option<Series>, Error> { unimplemented!() }
        async fn find_by_token(&self, _: &dyn Transaction, _: &SeriesToken) -> Result<Option<Series>, Error> { unimplemented!() }
        async fn list_series(&self, _: &dyn Transaction, _: Option<SeriesId>, _: Option<u64>) -> Result<Vec<Series>, Error> { unimplemented!() }
    }

    struct MockPublisherRepository;
    #[async_trait::async_trait]
    impl PublisherRepository for MockPublisherRepository {
        async fn add_publisher(&self, _: &dyn Transaction, _: NewPublisher) -> Result<Publisher, Error> { unimplemented!() }
        async fn update_publisher(&self, _: &dyn Transaction, _: Publisher) -> Result<Publisher, Error> { unimplemented!() }
        async fn find_by_id(&self, _: &dyn Transaction, _: PublisherId) -> Result<Option<Publisher>, Error> { unimplemented!() }
        async fn find_by_token(&self, _: &dyn Transaction, _: &PublisherToken) -> Result<Option<Publisher>, Error> { unimplemented!() }
        async fn list_publishers(&self, _: &dyn Transaction, _: Option<PublisherId>, _: Option<u64>) -> Result<Vec<Publisher>, Error> { unimplemented!() }
    }

    struct MockGenreRepository;
    #[async_trait::async_trait]
    impl GenreRepository for MockGenreRepository {
        async fn add_genre(&self, _: &dyn Transaction, _: NewGenre) -> Result<Genre, Error> { unimplemented!() }
        async fn update_genre(&self, _: &dyn Transaction, _: Genre) -> Result<Genre, Error> { unimplemented!() }
        async fn find_by_id(&self, _: &dyn Transaction, _: GenreId) -> Result<Option<Genre>, Error> { unimplemented!() }
        async fn find_by_token(&self, _: &dyn Transaction, _: &GenreToken) -> Result<Option<Genre>, Error> { unimplemented!() }
        async fn find_by_name(&self, _: &dyn Transaction, _: &str) -> Result<Option<Genre>, Error> { unimplemented!() }
        async fn list_genres(&self, _: &dyn Transaction, _: Option<GenreId>, _: Option<u64>) -> Result<Vec<Genre>, Error> { unimplemented!() }
    }

    struct MockTagRepository;
    #[async_trait::async_trait]
    impl TagRepository for MockTagRepository {
        async fn add_tag(&self, _: &dyn Transaction, _: NewTag) -> Result<Tag, Error> { unimplemented!() }
        async fn update_tag(&self, _: &dyn Transaction, _: Tag) -> Result<Tag, Error> { unimplemented!() }
        async fn find_by_id(&self, _: &dyn Transaction, _: TagId) -> Result<Option<Tag>, Error> { unimplemented!() }
        async fn find_by_token(&self, _: &dyn Transaction, _: &TagToken) -> Result<Option<Tag>, Error> { unimplemented!() }
        async fn find_by_name(&self, _: &dyn Transaction, _: &str) -> Result<Option<Tag>, Error> { unimplemented!() }
        async fn list_tags(&self, _: &dyn Transaction, _: Option<TagId>, _: Option<u64>) -> Result<Vec<Tag>, Error> { unimplemented!() }
    }

    // ─── Helpers ─────────────────────────────────────────────────────────────

    fn create_service(mock: MockSessionRepository) -> AuthServiceImpl {
        let repository_service = Arc::new(
            RepositoryServiceBuilder::default()
                .repository(Arc::new(MockRepository) as Arc<dyn Repository>)
                .session_repository(Arc::new(mock) as Arc<dyn SessionRepository>)
                .user_repository(Arc::new(MockUserRepository::default()) as Arc<dyn UserRepository>)
                .user_setting_repository(Arc::new(MockUserSettingRepository) as Arc<dyn UserSettingRepository>)
                .author_repository(Arc::new(MockAuthorRepository) as Arc<dyn AuthorRepository>)
                .series_repository(Arc::new(MockSeriesRepository) as Arc<dyn SeriesRepository>)
                .publisher_repository(Arc::new(MockPublisherRepository) as Arc<dyn PublisherRepository>)
                .genre_repository(Arc::new(MockGenreRepository) as Arc<dyn GenreRepository>)
                .tag_repository(Arc::new(MockTagRepository) as Arc<dyn TagRepository>)
                .build()
                .expect("all fields provided"),
        );
        AuthServiceImpl::new(repository_service)
    }

    fn create_login_service(user_mock: MockUserRepository) -> AuthServiceImpl {
        let repository_service = Arc::new(
            RepositoryServiceBuilder::default()
                .repository(Arc::new(MockRepository) as Arc<dyn Repository>)
                .session_repository(Arc::new(MockSessionRepository::default()) as Arc<dyn SessionRepository>)
                .user_repository(Arc::new(user_mock) as Arc<dyn UserRepository>)
                .user_setting_repository(Arc::new(MockUserSettingRepository) as Arc<dyn UserSettingRepository>)
                .author_repository(Arc::new(MockAuthorRepository) as Arc<dyn AuthorRepository>)
                .series_repository(Arc::new(MockSeriesRepository) as Arc<dyn SeriesRepository>)
                .publisher_repository(Arc::new(MockPublisherRepository) as Arc<dyn PublisherRepository>)
                .genre_repository(Arc::new(MockGenreRepository) as Arc<dyn GenreRepository>)
                .tag_repository(Arc::new(MockTagRepository) as Arc<dyn TagRepository>)
                .build()
                .expect("all fields provided"),
        );
        AuthServiceImpl::new(repository_service)
    }

    fn fake_session(id: &str) -> Session {
        SessionBuilder::default()
            .id(id.to_owned())
            .session("data".to_owned())
            .expires_at(Utc::now() + chrono::Duration::hours(1))
            .build()
            .expect("valid session")
    }

    // ─── count ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_count_returns_value() {
        let svc = create_service(MockSessionRepository::default().with_count_result(Ok(3)));

        let result = svc.count().await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 3);
    }

    #[tokio::test]
    async fn test_count_propagates_error() {
        let svc = create_service(MockSessionRepository::default().with_count_result(Err(Error::RepositoryError(RepositoryError::Database("db error".into())))));

        let result = svc.count().await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── store ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_store_success() {
        let session = fake_session("sess-1");
        let svc = create_service(MockSessionRepository::default().with_store_result(Ok(session)));

        let new_session = NewSession::new("sess-1", "data", Utc::now() + chrono::Duration::hours(1)).unwrap();
        let result = svc.store(new_session).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, "sess-1");
    }

    #[tokio::test]
    async fn test_store_propagates_constraint_error() {
        let svc =
            create_service(MockSessionRepository::default().with_store_result(Err(Error::RepositoryError(RepositoryError::Constraint("duplicate id".into())))));

        let new_session = NewSession::new("sess-1", "data", Utc::now()).unwrap();
        let result = svc.store(new_session).await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Constraint(_)))));
    }

    // ─── load ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_load_found() {
        let session = fake_session("sess-1");
        let svc = create_service(MockSessionRepository::default().with_load_result(Ok(Some(session))));

        let result = svc.load("sess-1").await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().unwrap().id, "sess-1");
    }

    #[tokio::test]
    async fn test_load_not_found() {
        let svc = create_service(MockSessionRepository::default().with_load_result(Ok(None)));

        let result = svc.load("missing").await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    // ─── delete_by_id ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_by_id_success() {
        let svc = create_service(MockSessionRepository::default().with_delete_by_id_result(Ok(())));

        let result = svc.delete_by_id("sess-1").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_delete_by_id_propagates_error() {
        let svc = create_service(MockSessionRepository::default().with_delete_by_id_result(Err(Error::RepositoryError(RepositoryError::NotFound))));

        let result = svc.delete_by_id("missing").await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::NotFound))));
    }

    // ─── exists ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_exists_true() {
        let svc = create_service(MockSessionRepository::default().with_exists_result(Ok(true)));

        let result = svc.exists("sess-1").await;

        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_exists_false() {
        let svc = create_service(MockSessionRepository::default().with_exists_result(Ok(false)));

        let result = svc.exists("missing").await;

        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    // ─── delete_by_expiry ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_by_expiry_returns_deleted_ids() {
        let ids = vec!["sess-1".to_owned(), "sess-2".to_owned()];
        let svc = create_service(MockSessionRepository::default().with_delete_by_expiry_result(Ok(ids)));

        let result = svc.delete_by_expiry().await;

        assert!(result.is_ok());
        let deleted = result.unwrap();
        assert_eq!(deleted.len(), 2);
        assert_eq!(deleted[0], "sess-1");
        assert_eq!(deleted[1], "sess-2");
    }

    #[tokio::test]
    async fn test_delete_by_expiry_empty_when_none_expired() {
        let svc = create_service(MockSessionRepository::default().with_delete_by_expiry_result(Ok(vec![])));

        let result = svc.delete_by_expiry().await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // ─── delete_all ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_all_success() {
        let svc = create_service(MockSessionRepository::default().with_delete_all_result(Ok(())));

        let result = svc.delete_all().await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_delete_all_propagates_error() {
        let svc =
            create_service(MockSessionRepository::default().with_delete_all_result(Err(Error::RepositoryError(RepositoryError::Database("db error".into())))));

        let result = svc.delete_all().await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── get_ids ─────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_ids_returns_all() {
        let ids = vec!["sess-1".to_owned(), "sess-2".to_owned(), "sess-3".to_owned()];
        let svc = create_service(MockSessionRepository::default().with_get_ids_result(Ok(ids)));

        let result = svc.get_ids().await;

        assert!(result.is_ok());
        let list = result.unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0], "sess-1");
    }

    #[tokio::test]
    async fn test_get_ids_empty() {
        let svc = create_service(MockSessionRepository::default().with_get_ids_result(Ok(vec![])));

        let result = svc.get_ids().await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // ─── is_valid_login ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_is_valid_login_success() {
        let hash = User::encrypt_password("correct-password").unwrap();
        let user = User::fake(1, "alice", hash, "alice@example.com", HashSet::new());
        let svc = create_login_service(MockUserRepository::default().with_find_by_username_result(Ok(Some(user))));

        let result = svc.is_valid_login("alice", "correct-password").await;

        assert!(result.is_ok());
        let found = result.unwrap().unwrap();
        assert_eq!(found.id, 1);
        assert_eq!(found.username, "alice");
    }

    #[tokio::test]
    async fn test_is_valid_login_wrong_password() {
        let hash = User::encrypt_password("correct-password").unwrap();
        let user = User::fake(1, "alice", hash, "alice@example.com", HashSet::new());
        let svc = create_login_service(MockUserRepository::default().with_find_by_username_result(Ok(Some(user))));

        let result = svc.is_valid_login("alice", "wrong-password").await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_is_valid_login_user_not_found() {
        let svc = create_login_service(MockUserRepository::default().with_find_by_username_result(Ok(None)));

        let result = svc.is_valid_login("nobody", "password").await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_is_valid_login_propagates_error() {
        let svc = create_login_service(
            MockUserRepository::default().with_find_by_username_result(Err(Error::RepositoryError(RepositoryError::Database("db error".into())))),
        );

        let result = svc.is_valid_login("alice", "password").await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }
}
