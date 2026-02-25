use std::{any::Any, future::Future, pin::Pin, sync::Arc};

use derive_builder::Builder;

use crate::{Error, user::UserRepository};

#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct RepositoryService {
    repository: Arc<dyn Repository>,
    user_repository: Arc<dyn UserRepository>,
}

impl RepositoryService {
    /// Returns a reference to the main repository for transaction management.
    pub fn repository(&self) -> &Arc<dyn Repository> {
        &self.repository
    }

    /// Returns a reference to the user repository.
    pub fn user_repository(&self) -> &Arc<dyn UserRepository> {
        &self.user_repository
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
pub trait Repository: Send + Sync {
    async fn begin(&self) -> Result<Box<dyn Transaction>, Error>;
    async fn begin_read_only(&self) -> Result<Box<dyn Transaction>, Error>;
    async fn close(&self) -> Result<(), Error>;
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
#[tracing::instrument(level = "trace", skip(repository, callback))]
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
            // Best effort rollback - if it fails, we still return the original error
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
#[tracing::instrument(level = "trace", skip(repository, callback))]
pub async fn read_only_transaction<F, T>(repository: &dyn Repository, callback: F) -> Result<T, Error>
where
    F: for<'c> FnOnce(&'c dyn Transaction) -> Pin<Box<dyn Future<Output = Result<T, Error>> + Send + 'c>> + Send,
    T: Send,
{
    let tx = repository.begin_read_only().await?;
    callback(&*tx).await
}
