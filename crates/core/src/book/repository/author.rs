use crate::{
    Error,
    book::{Author, AuthorId, AuthorToken, NewAuthor},
    repository::Transaction,
};

#[async_trait::async_trait]
pub trait AuthorRepository: Send + Sync {
    async fn add_author(&self, transaction: &dyn Transaction, author: NewAuthor) -> Result<Author, Error>;
    async fn update_author(&self, transaction: &dyn Transaction, author: Author) -> Result<Author, Error>;
    async fn find_by_id(&self, transaction: &dyn Transaction, id: AuthorId) -> Result<Option<Author>, Error>;
    async fn find_by_token(&self, transaction: &dyn Transaction, token: &AuthorToken) -> Result<Option<Author>, Error>;
    async fn list_authors(&self, transaction: &dyn Transaction, start_id: Option<AuthorId>, page_size: Option<u64>) -> Result<Vec<Author>, Error>;
    async fn find_by_name(&self, transaction: &dyn Transaction, name: &str) -> Result<Option<Author>, Error>;
    async fn count_authors(&self, transaction: &dyn Transaction) -> Result<u64, Error>;
    async fn delete_author(&self, transaction: &dyn Transaction, author_id: AuthorId) -> Result<(), Error>;
}
