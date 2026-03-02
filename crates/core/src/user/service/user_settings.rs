use std::sync::Arc;

use crate::{
    Error,
    repository::RepositoryService,
    user::{NewUserSetting, UserId, UserSetting},
    with_read_only_transaction, with_transaction,
};

#[async_trait::async_trait]
pub trait UserSettingService: Send + Sync {
    async fn get(&self, user_id: UserId, key: &str) -> Result<Option<UserSetting>, Error>;
    async fn set(&self, user_id: UserId, key: &str, value: &str) -> Result<UserSetting, Error>;
    async fn delete(&self, user_id: UserId, key: &str) -> Result<(), Error>;
    async fn list_by_user(&self, user_id: UserId) -> Result<Vec<UserSetting>, Error>;
}

pub(crate) struct UserSettingServiceImpl {
    repository_service: Arc<RepositoryService>,
}

impl UserSettingServiceImpl {
    pub(crate) fn new(repository_service: Arc<RepositoryService>) -> Self {
        Self { repository_service }
    }
}

#[async_trait::async_trait]
impl UserSettingService for UserSettingServiceImpl {
    #[tracing::instrument(level = "trace", skip(self))]
    async fn get(&self, user_id: UserId, key: &str) -> Result<Option<UserSetting>, Error> {
        let key = key.to_owned();
        with_read_only_transaction!(self, user_setting_repository, |tx| user_setting_repository.get(tx, user_id, &key).await)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn set(&self, user_id: UserId, key: &str, value: &str) -> Result<UserSetting, Error> {
        let setting = NewUserSetting {
            user_id,
            key: key.to_owned(),
            value: value.to_owned(),
        };
        with_transaction!(self, user_setting_repository, |tx| user_setting_repository.set(tx, setting).await)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn delete(&self, user_id: UserId, key: &str) -> Result<(), Error> {
        let key = key.to_owned();
        with_transaction!(self, user_setting_repository, |tx| user_setting_repository.delete(tx, user_id, &key).await)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn list_by_user(&self, user_id: UserId) -> Result<Vec<UserSetting>, Error> {
        with_read_only_transaction!(self, user_setting_repository, |tx| user_setting_repository.list_by_user(tx, user_id).await)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        any::Any,
        sync::{Arc, Mutex},
    };

    use super::{UserSettingService, UserSettingServiceImpl};
    use crate::{
        Error, RepositoryError,
        auth::{NewSession, Session, repository::SessionRepository},
        book::{
            Author, AuthorId, AuthorToken, AuthorRepository, NewAuthor,
            Book, BookAuthor, BookFile, BookFilter, BookId, BookIdentifier, BookRepository, BookToken, NewBook,
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

    // ─── Mock UserRepository ─────────────────────────────────────────────────

    struct MockUserRepository;

    #[async_trait::async_trait]
    impl UserRepository for MockUserRepository {
        async fn add_user(&self, _tx: &dyn Transaction, _user: NewUser) -> Result<User, Error> {
            unimplemented!()
        }
        async fn update_user(&self, _tx: &dyn Transaction, _user: User) -> Result<User, Error> {
            unimplemented!()
        }
        async fn delete_user(&self, _tx: &dyn Transaction, _user: User) -> Result<User, Error> {
            unimplemented!()
        }
        async fn list_users(&self, _tx: &dyn Transaction, _start_id: Option<UserId>, _page_size: Option<u64>) -> Result<Vec<User>, Error> {
            unimplemented!()
        }
        async fn find_by_id(&self, _tx: &dyn Transaction, _id: UserId) -> Result<Option<User>, Error> {
            unimplemented!()
        }
        async fn find_by_username(&self, _tx: &dyn Transaction, _username: &str) -> Result<Option<User>, Error> {
            unimplemented!()
        }
    }

    // ─── Mock UserSettingRepository ──────────────────────────────────────────

    #[derive(Default)]
    struct MockUserSettingRepository {
        get_result: Mutex<Option<Result<Option<UserSetting>, Error>>>,
        set_result: Mutex<Option<Result<UserSetting, Error>>>,
        delete_result: Mutex<Option<Result<(), Error>>>,
        list_by_user_result: Mutex<Option<Result<Vec<UserSetting>, Error>>>,
    }

    impl MockUserSettingRepository {
        fn with_get_result(self, result: Result<Option<UserSetting>, Error>) -> Self {
            *self.get_result.lock().unwrap() = Some(result);
            self
        }

        fn with_set_result(self, result: Result<UserSetting, Error>) -> Self {
            *self.set_result.lock().unwrap() = Some(result);
            self
        }

        fn with_delete_result(self, result: Result<(), Error>) -> Self {
            *self.delete_result.lock().unwrap() = Some(result);
            self
        }

        fn with_list_by_user_result(self, result: Result<Vec<UserSetting>, Error>) -> Self {
            *self.list_by_user_result.lock().unwrap() = Some(result);
            self
        }
    }

    #[async_trait::async_trait]
    impl UserSettingRepository for MockUserSettingRepository {
        async fn get(&self, _tx: &dyn Transaction, _user_id: UserId, _key: &str) -> Result<Option<UserSetting>, Error> {
            self.get_result.lock().unwrap().clone().unwrap_or_else(|| Err(Error::MockNotConfigured("get")))
        }

        async fn set(&self, _tx: &dyn Transaction, _setting: NewUserSetting) -> Result<UserSetting, Error> {
            self.set_result.lock().unwrap().clone().unwrap_or_else(|| Err(Error::MockNotConfigured("set")))
        }

        async fn delete(&self, _tx: &dyn Transaction, _user_id: UserId, _key: &str) -> Result<(), Error> {
            self.delete_result
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("delete")))
        }

        async fn list_by_user(&self, _tx: &dyn Transaction, _user_id: UserId) -> Result<Vec<UserSetting>, Error> {
            self.list_by_user_result
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Err(Error::MockNotConfigured("list_by_user")))
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

    struct MockBookRepository;
    #[async_trait::async_trait]
    impl BookRepository for MockBookRepository {
        async fn add_book(&self, _: &dyn Transaction, _: NewBook) -> Result<Book, Error> { unimplemented!() }
        async fn update_book(&self, _: &dyn Transaction, _: Book) -> Result<Book, Error> { unimplemented!() }
        async fn find_by_id(&self, _: &dyn Transaction, _: BookId) -> Result<Option<Book>, Error> { unimplemented!() }
        async fn find_by_token(&self, _: &dyn Transaction, _: &BookToken) -> Result<Option<Book>, Error> { unimplemented!() }
        async fn list_books(&self, _: &dyn Transaction, _: &BookFilter, _: Option<BookId>, _: Option<u64>) -> Result<Vec<Book>, Error> { unimplemented!() }
        async fn authors_for_book(&self, _: &dyn Transaction, _: BookId) -> Result<Vec<BookAuthor>, Error> { unimplemented!() }
        async fn files_for_book(&self, _: &dyn Transaction, _: BookId) -> Result<Vec<BookFile>, Error> { unimplemented!() }
        async fn identifiers_for_book(&self, _: &dyn Transaction, _: BookId) -> Result<Vec<BookIdentifier>, Error> { unimplemented!() }
    }

    // ─── Helper ──────────────────────────────────────────────────────────────

    fn fake_setting(user_id: UserId, key: &str, value: &str) -> UserSetting {
        UserSetting {
            user_id,
            key: key.to_owned(),
            value: value.to_owned(),
        }
    }

    fn create_service(mock: MockUserSettingRepository) -> UserSettingServiceImpl {
        let repository_service = Arc::new(
            RepositoryServiceBuilder::default()
                .repository(Arc::new(MockRepository) as Arc<dyn Repository>)
                .session_repository(Arc::new(MockSessionRepository) as Arc<dyn SessionRepository>)
                .user_repository(Arc::new(MockUserRepository) as Arc<dyn UserRepository>)
                .user_setting_repository(Arc::new(mock) as Arc<dyn UserSettingRepository>)
                .author_repository(Arc::new(MockAuthorRepository) as Arc<dyn AuthorRepository>)
                .series_repository(Arc::new(MockSeriesRepository) as Arc<dyn SeriesRepository>)
                .publisher_repository(Arc::new(MockPublisherRepository) as Arc<dyn PublisherRepository>)
                .genre_repository(Arc::new(MockGenreRepository) as Arc<dyn GenreRepository>)
                .tag_repository(Arc::new(MockTagRepository) as Arc<dyn TagRepository>)
                .book_repository(Arc::new(MockBookRepository) as Arc<dyn BookRepository>)
                .build()
                .expect("all fields provided"),
        );
        UserSettingServiceImpl::new(repository_service)
    }

    // ─── get ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_get_returns_none_when_not_found() {
        let svc = create_service(MockUserSettingRepository::default().with_get_result(Ok(None)));

        let result = svc.get(1, "some-key").await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_get_returns_setting_when_found() {
        let expected = fake_setting(1, "theme", "dark");
        let svc = create_service(MockUserSettingRepository::default().with_get_result(Ok(Some(expected))));

        let result = svc.get(1, "theme").await;

        assert!(result.is_ok());
        let setting = result.unwrap().unwrap();
        assert_eq!(setting.user_id, 1);
        assert_eq!(setting.key, "theme");
        assert_eq!(setting.value, "dark");
    }

    #[tokio::test]
    async fn test_get_propagates_error() {
        let svc =
            create_service(MockUserSettingRepository::default().with_get_result(Err(Error::RepositoryError(RepositoryError::Database("db error".into())))));

        let result = svc.get(1, "theme").await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── set ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_set_returns_setting_on_success() {
        let expected = fake_setting(1, "theme", "dark");
        let svc = create_service(MockUserSettingRepository::default().with_set_result(Ok(expected)));

        let result = svc.set(1, "theme", "dark").await;

        assert!(result.is_ok());
        let setting = result.unwrap();
        assert_eq!(setting.user_id, 1);
        assert_eq!(setting.key, "theme");
        assert_eq!(setting.value, "dark");
    }

    #[tokio::test]
    async fn test_set_propagates_error() {
        let svc =
            create_service(MockUserSettingRepository::default().with_set_result(Err(Error::RepositoryError(RepositoryError::Database("db error".into())))));

        let result = svc.set(1, "theme", "dark").await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── delete ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_returns_ok_on_success() {
        let svc = create_service(MockUserSettingRepository::default().with_delete_result(Ok(())));

        let result = svc.delete(1, "theme").await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_delete_propagates_error() {
        let svc =
            create_service(MockUserSettingRepository::default().with_delete_result(Err(Error::RepositoryError(RepositoryError::Database("db error".into())))));

        let result = svc.delete(1, "theme").await;

        assert!(matches!(result, Err(Error::RepositoryError(RepositoryError::Database(_)))));
    }

    // ─── list_by_user ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_by_user_returns_empty() {
        let svc = create_service(MockUserSettingRepository::default().with_list_by_user_result(Ok(vec![])));

        let result = svc.list_by_user(1).await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_by_user_returns_multiple() {
        let settings = vec![fake_setting(1, "theme", "dark"), fake_setting(1, "lang", "en")];
        let svc = create_service(MockUserSettingRepository::default().with_list_by_user_result(Ok(settings)));

        let result = svc.list_by_user(1).await;

        assert!(result.is_ok());
        let list = result.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].key, "theme");
        assert_eq!(list[1].key, "lang");
    }
}
