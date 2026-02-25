use crate::{
    Error,
    repository::Transaction,
    user::{NewUser, User, UserId},
};

#[async_trait::async_trait]
pub trait UserRepository: Send + Sync {
    async fn add_user(&self, transaction: &dyn Transaction, user: NewUser) -> Result<User, Error>;
    async fn update_user(&self, transaction: &dyn Transaction, user: User) -> Result<User, Error>;
    async fn delete_user(&self, transaction: &dyn Transaction, user: User) -> Result<User, Error>;
    async fn list_users(&self, transaction: &dyn Transaction, start_id: Option<UserId>, page_size: Option<u64>) -> Result<Vec<User>, Error>;
    async fn find_by_id(&self, transaction: &dyn Transaction, id: UserId) -> Result<Option<User>, Error>;
    async fn find_by_username(&self, transaction: &dyn Transaction, username: &str) -> Result<Option<User>, Error>;
}
