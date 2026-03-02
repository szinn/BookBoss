use crate::{
    Error,
    book::{NewTag, Tag, TagId, TagToken},
    repository::Transaction,
};

#[async_trait::async_trait]
pub trait TagRepository: Send + Sync {
    async fn add_tag(&self, transaction: &dyn Transaction, tag: NewTag) -> Result<Tag, Error>;
    async fn update_tag(&self, transaction: &dyn Transaction, tag: Tag) -> Result<Tag, Error>;
    async fn find_by_id(&self, transaction: &dyn Transaction, id: TagId) -> Result<Option<Tag>, Error>;
    async fn find_by_token(&self, transaction: &dyn Transaction, token: &TagToken) -> Result<Option<Tag>, Error>;
    async fn find_by_name(&self, transaction: &dyn Transaction, name: &str) -> Result<Option<Tag>, Error>;
    async fn list_tags(&self, transaction: &dyn Transaction, start_id: Option<TagId>, page_size: Option<u64>) -> Result<Vec<Tag>, Error>;
}
