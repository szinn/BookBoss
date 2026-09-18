use std::{any::Any, pin::Pin, sync::Arc};

use derive_builder::Builder;

use crate::{
    Error,
    app_setting::AppSettingRepository,
    auth::SessionRepository,
    book::{AuthorRepository, BookRepository, GenreRepository, PublisherRepository, SeriesRepository, TagRepository},
    collection::CollectionRepository,
    device::DeviceRepository,
    import::ImportJobRepository,
    jobs::JobRepository,
    koreader::KoReaderDocumentHashRepository,
    library::LibraryRepository,
    message::SystemMessageRepository,
    reading::UserBookMetadataRepository,
    shelf::ShelfRepository,
    user::{UserRepository, UserSettingRepository},
};

#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct RepositoryService {
    repository: Arc<dyn Repository>,
    session_repository: Arc<dyn SessionRepository>,
    user_repository: Arc<dyn UserRepository>,
    user_setting_repository: Arc<dyn UserSettingRepository>,
    author_repository: Arc<dyn AuthorRepository>,
    series_repository: Arc<dyn SeriesRepository>,
    publisher_repository: Arc<dyn PublisherRepository>,
    genre_repository: Arc<dyn GenreRepository>,
    tag_repository: Arc<dyn TagRepository>,
    book_repository: Arc<dyn BookRepository>,
    import_job_repository: Arc<dyn ImportJobRepository>,
    job_repository: Arc<dyn JobRepository>,
    collection_repository: Arc<dyn CollectionRepository>,
    library_repository: Arc<dyn LibraryRepository>,
    shelf_repository: Arc<dyn ShelfRepository>,
    user_book_metadata_repository: Arc<dyn UserBookMetadataRepository>,
    device_repository: Arc<dyn DeviceRepository>,
    system_message_repository: Arc<dyn SystemMessageRepository>,
    koreader_document_hash_repository: Arc<dyn KoReaderDocumentHashRepository>,
    app_setting_repository: Arc<dyn AppSettingRepository>,
}

impl RepositoryService {
    /// Returns a reference to the main repository for transaction management.
    #[must_use]
    pub fn repository(&self) -> &Arc<dyn Repository> {
        &self.repository
    }

    /// Returns a reference to the session repository.
    #[must_use]
    pub fn session_repository(&self) -> &Arc<dyn SessionRepository> {
        &self.session_repository
    }

    /// Returns a reference to the user repository.
    #[must_use]
    pub fn user_repository(&self) -> &Arc<dyn UserRepository> {
        &self.user_repository
    }

    /// Returns a reference to the user setting repository.
    #[must_use]
    pub fn user_setting_repository(&self) -> &Arc<dyn UserSettingRepository> {
        &self.user_setting_repository
    }

    /// Returns a reference to the author repository.
    #[must_use]
    pub fn author_repository(&self) -> &Arc<dyn AuthorRepository> {
        &self.author_repository
    }

    /// Returns a reference to the series repository.
    #[must_use]
    pub fn series_repository(&self) -> &Arc<dyn SeriesRepository> {
        &self.series_repository
    }

    /// Returns a reference to the publisher repository.
    #[must_use]
    pub fn publisher_repository(&self) -> &Arc<dyn PublisherRepository> {
        &self.publisher_repository
    }

    /// Returns a reference to the genre repository.
    #[must_use]
    pub fn genre_repository(&self) -> &Arc<dyn GenreRepository> {
        &self.genre_repository
    }

    /// Returns a reference to the tag repository.
    #[must_use]
    pub fn tag_repository(&self) -> &Arc<dyn TagRepository> {
        &self.tag_repository
    }

    /// Returns a reference to the book repository.
    #[must_use]
    pub fn book_repository(&self) -> &Arc<dyn BookRepository> {
        &self.book_repository
    }

    /// Returns a reference to the import job repository.
    #[must_use]
    pub fn import_job_repository(&self) -> &Arc<dyn ImportJobRepository> {
        &self.import_job_repository
    }

    /// Returns a reference to the job repository.
    #[must_use]
    pub fn job_repository(&self) -> &Arc<dyn JobRepository> {
        &self.job_repository
    }

    /// Returns a reference to the collection repository.
    #[must_use]
    pub fn collection_repository(&self) -> &Arc<dyn CollectionRepository> {
        &self.collection_repository
    }

    /// Returns a reference to the library repository.
    #[must_use]
    pub fn library_repository(&self) -> &Arc<dyn LibraryRepository> {
        &self.library_repository
    }

    /// Returns a reference to the shelf repository.
    #[must_use]
    pub fn shelf_repository(&self) -> &Arc<dyn ShelfRepository> {
        &self.shelf_repository
    }

    /// Returns a reference to the user book metadata repository.
    #[must_use]
    pub fn user_book_metadata_repository(&self) -> &Arc<dyn UserBookMetadataRepository> {
        &self.user_book_metadata_repository
    }

    /// Returns a reference to the device repository.
    #[must_use]
    pub fn device_repository(&self) -> &Arc<dyn DeviceRepository> {
        &self.device_repository
    }

    /// Returns a reference to the system message repository.
    #[must_use]
    pub fn system_message_repository(&self) -> &Arc<dyn SystemMessageRepository> {
        &self.system_message_repository
    }

    /// Returns a reference to the KOReader document hash repository.
    #[must_use]
    pub fn koreader_document_hash_repository(&self) -> &Arc<dyn KoReaderDocumentHashRepository> {
        &self.koreader_document_hash_repository
    }

    /// Returns a reference to the app setting repository.
    #[must_use]
    pub fn app_setting_repository(&self) -> &Arc<dyn AppSettingRepository> {
        &self.app_setting_repository
    }
}

#[async_trait::async_trait]
pub trait Transaction: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    async fn commit(self: Box<Self>) -> Result<(), Error>;
    async fn rollback(self: Box<Self>) -> Result<(), Error>;
}

/// Execute an async operation within a read-write transaction.
///
/// Clones one or more repositories, begins a transaction, executes the body,
/// and commits on success or rolls back on error.
///
/// # Examples
/// ```ignore
/// // Single repository
/// with_transaction!(self, user_repository, |tx| {
///     user_repository.add_user(tx, user).await
/// })
///
/// // Multiple repositories
/// with_transaction!(self, user_repository, order_repository, |tx| {
///     let user = user_repository.add_user(tx, user).await?;
///     order_repository.create_order(tx, user.id, order).await
/// })
/// ```
#[macro_export]
macro_rules! with_transaction {
    ($self:expr, $($repo:ident),+ , |$tx:ident| $body:expr) => {{
        $(let $repo = $self.repository_service.$repo().clone();)+
        $crate::repository::transaction(&**$self.repository_service.repository(), |$tx| Box::pin(async move { $body })).await
    }};
}

/// Execute an async operation within a read-only transaction.
///
/// Clones one or more repositories and executes the body within a read-only
/// transaction.
///
/// # Examples
/// ```ignore
/// // Single repository
/// with_read_only_transaction!(self, user_repository, |tx| {
///     user_repository.find_by_id(tx, id).await
/// })
///
/// // Multiple repositories
/// with_read_only_transaction!(self, user_repository, order_repository, |tx| {
///     let user = user_repository.find_by_id(tx, id).await?;
///     let orders = order_repository.find_by_user(tx, user.id).await?;
///     Ok((user, orders))
/// })
/// ```
#[macro_export]
macro_rules! with_read_only_transaction {
    ($self:expr, $($repo:ident),+ , |$tx:ident| $body:expr) => {{
        $(let $repo = $self.repository_service.$repo().clone();)+
        $crate::repository::read_only_transaction(&**$self.repository_service.repository(), |$tx| Box::pin(async move { $body })).await
    }};
}

#[async_trait::async_trait]
#[cfg_attr(test, mockall::automock)]
#[allow(unused_lifetimes, reason = "Generated by mockall")]
pub trait Repository: Send + Sync {
    async fn begin(&self) -> Result<Box<dyn Transaction>, Error>;
    async fn begin_read_only(&self) -> Result<Box<dyn Transaction>, Error>;
    async fn close(&self) -> Result<(), Error>;
    /// Verify the DB connection is alive. Used by `ResilienceWrapper::check()`.
    async fn ping(&self) -> Result<(), Error>;
}

/// Execute a closure within a transaction, automatically committing on success
/// or rolling back on error.
///
/// # Example
/// ```ignore
/// let result = transaction(&*repository, |tx| Box::pin(async move {
///     // do stuff with tx
///     Ok(result)
/// })).await?;
/// ```
pub async fn transaction<F, T>(repository: &dyn Repository, callback: F) -> Result<T, Error>
where
    F: for<'c> FnOnce(&'c dyn Transaction) -> Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'c>> + Send,
    T: Send,
{
    let tx = repository.begin().await?;
    match callback(&*tx).await {
        Ok(result) => {
            tx.commit().await?;
            Ok(result)
        }
        Err(e) => {
            // Best effort rollback - if it fails, we still return the original
            // error
            let _ = tx.rollback().await;
            Err(e)
        }
    }
}

/// Execute a closure within a read-only transaction.
///
/// # Example
/// ```ignore
/// let result = read_only_transaction(&*repository, |tx| Box::pin(async move {
///     // do stuff with tx
///     Ok(result)
/// })).await?;
/// ```
pub async fn read_only_transaction<F, T>(repository: &dyn Repository, callback: F) -> Result<T, Error>
where
    F: for<'c> FnOnce(&'c dyn Transaction) -> Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'c>> + Send,
    T: Send,
{
    let tx = repository.begin_read_only().await?;
    callback(&*tx).await
}

/// Shared test helpers for building a fully-mocked `RepositoryService`.
///
/// All service test modules should use `default_repository_service_builder()`
/// instead of repeating the 16-field builder by hand.
#[cfg(test)]
pub(crate) mod testing {
    use std::{any::Any, sync::Arc};

    use super::{MockRepository, RepositoryServiceBuilder, Transaction};
    use crate::{
        Error,
        app_setting::repository::MockAppSettingRepository,
        auth::repository::MockSessionRepository,
        book::repository::{
            author::MockAuthorRepository, book::MockBookRepository, genre::MockGenreRepository, publisher::MockPublisherRepository,
            series::MockSeriesRepository, tag::MockTagRepository,
        },
        collection::MockCollectionRepository,
        device::repository::device::MockDeviceRepository,
        import::repository::import_job::MockImportJobRepository,
        jobs::repository::MockJobRepository,
        koreader::repository::MockKoReaderDocumentHashRepository,
        library::MockLibraryRepository,
        message::repository::MockSystemMessageRepository,
        reading::repository::user_book_metadata::MockUserBookMetadataRepository,
        shelf::repository::shelf::MockShelfRepository,
        user::repository::{user::MockUserRepository, user_settings::MockUserSettingRepository},
    };

    /// A no-op transaction for unit tests.
    pub(crate) struct MockTransaction;

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

    /// Returns a `MockRepository` pre-configured to open transactions.
    pub(crate) fn make_mock_repo() -> MockRepository {
        let mut r = MockRepository::new();
        r.expect_begin()
            .returning(|| Box::pin(async { Ok(Box::new(MockTransaction) as Box<dyn Transaction>) }));
        r.expect_begin_read_only()
            .returning(|| Box::pin(async { Ok(Box::new(MockTransaction) as Box<dyn Transaction>) }));
        r.expect_ping().returning(|| Box::pin(async { Ok(()) }));
        r
    }

    /// Returns a `RepositoryServiceBuilder` pre-populated with default mocks
    /// for all 17 repositories. Override individual fields for the repo(s)
    /// under test before calling `.build()`.
    pub(crate) fn default_repository_service_builder() -> RepositoryServiceBuilder {
        RepositoryServiceBuilder::default()
            .repository(Arc::new(make_mock_repo()))
            .session_repository(Arc::new(MockSessionRepository::new()))
            .user_repository(Arc::new(MockUserRepository::new()))
            .user_setting_repository(Arc::new(MockUserSettingRepository::new()))
            .author_repository(Arc::new(MockAuthorRepository::new()))
            .series_repository(Arc::new(MockSeriesRepository::new()))
            .publisher_repository(Arc::new(MockPublisherRepository::new()))
            .genre_repository(Arc::new(MockGenreRepository::new()))
            .tag_repository(Arc::new(MockTagRepository::new()))
            .book_repository(Arc::new(MockBookRepository::new()))
            .import_job_repository(Arc::new(MockImportJobRepository::new()))
            .job_repository(Arc::new(MockJobRepository::new()))
            .collection_repository(Arc::new(MockCollectionRepository::new()))
            .library_repository(Arc::new(MockLibraryRepository::new()))
            .shelf_repository(Arc::new(MockShelfRepository::new()))
            .user_book_metadata_repository(Arc::new(MockUserBookMetadataRepository::new()))
            .device_repository(Arc::new(MockDeviceRepository::new()))
            .system_message_repository(Arc::new(MockSystemMessageRepository::new()))
            .koreader_document_hash_repository(Arc::new(MockKoReaderDocumentHashRepository::new()))
            .app_setting_repository(Arc::new(MockAppSettingRepository::new()))
    }
}
