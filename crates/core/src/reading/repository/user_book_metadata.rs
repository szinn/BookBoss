use crate::{
    Error,
    book::BookId,
    reading::{ReadStatus, UserBookMetadata},
    repository::Transaction,
    user::UserId,
};

#[async_trait::async_trait]
pub trait UserBookMetadataRepository: Send + Sync {
    async fn upsert(&self, transaction: &dyn Transaction, metadata: UserBookMetadata) -> Result<UserBookMetadata, Error>;
    async fn find_by_user_and_book(&self, transaction: &dyn Transaction, user_id: UserId, book_id: BookId) -> Result<Option<UserBookMetadata>, Error>;
    async fn list_for_user(
        &self,
        transaction: &dyn Transaction,
        user_id: UserId,
        status: Option<ReadStatus>,
        start_book_id: Option<BookId>,
        page_size: Option<u64>,
    ) -> Result<Vec<UserBookMetadata>, Error>;
}
