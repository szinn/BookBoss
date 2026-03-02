use crate::{
    Error,
    book::{NewPublisher, Publisher, PublisherId, PublisherToken},
    repository::Transaction,
};

#[async_trait::async_trait]
pub trait PublisherRepository: Send + Sync {
    async fn add_publisher(&self, transaction: &dyn Transaction, publisher: NewPublisher) -> Result<Publisher, Error>;
    async fn update_publisher(&self, transaction: &dyn Transaction, publisher: Publisher) -> Result<Publisher, Error>;
    async fn find_by_id(&self, transaction: &dyn Transaction, id: PublisherId) -> Result<Option<Publisher>, Error>;
    async fn find_by_token(&self, transaction: &dyn Transaction, token: &PublisherToken) -> Result<Option<Publisher>, Error>;
    async fn list_publishers(&self, transaction: &dyn Transaction, start_id: Option<PublisherId>, page_size: Option<u64>) -> Result<Vec<Publisher>, Error>;
}
