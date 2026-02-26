use crate::{
    Error,
    auth::{NewSession, Session},
    repository::Transaction,
};

#[async_trait::async_trait]
pub trait SessionRepository: Send + Sync {
    async fn count(&self, transaction: &dyn Transaction) -> Result<i64, Error>;
    async fn store(&self, transaction: &dyn Transaction, session: NewSession) -> Result<Session, Error>;
    async fn load(&self, transaction: &dyn Transaction, id: &str) -> Result<Option<Session>, Error>;
    async fn delete_by_id(&self, transaction: &dyn Transaction, id: &str) -> Result<(), Error>;
    async fn exists(&self, transaction: &dyn Transaction, id: &str) -> Result<bool, Error>;
    async fn delete_by_expiry(&self, transaction: &dyn Transaction) -> Result<Vec<String>, Error>;
    async fn delete_all(&self, transaction: &dyn Transaction) -> Result<(), Error>;
    async fn get_ids(&self, transaction: &dyn Transaction) -> Result<Vec<String>, Error>;
}
