use crate::{
    Error,
    book::{Book, BookId},
    repository::Transaction,
    shelf::{BookShelf, NewShelf, Shelf, ShelfFilter, ShelfId, ShelfToken},
    user::UserId,
};

#[async_trait::async_trait]
pub trait ShelfRepository: Send + Sync {
    // Shelf CRUD
    async fn add_shelf(&self, transaction: &dyn Transaction, shelf: NewShelf) -> Result<Shelf, Error>;
    async fn update_shelf(&self, transaction: &dyn Transaction, shelf: Shelf) -> Result<Shelf, Error>;
    async fn delete_shelf(&self, transaction: &dyn Transaction, shelf: Shelf) -> Result<(), Error>;
    async fn find_by_id(&self, transaction: &dyn Transaction, id: ShelfId) -> Result<Option<Shelf>, Error>;
    async fn find_by_token(&self, transaction: &dyn Transaction, token: &ShelfToken) -> Result<Option<Shelf>, Error>;
    async fn list_for_user(&self, transaction: &dyn Transaction, owner_id: UserId) -> Result<Vec<Shelf>, Error>;

    // Manual shelf book management
    async fn add_book_to_shelf(&self, transaction: &dyn Transaction, book_shelf: BookShelf) -> Result<BookShelf, Error>;
    async fn remove_book_from_shelf(&self, transaction: &dyn Transaction, shelf_id: ShelfId, book_id: BookId) -> Result<(), Error>;
    async fn books_for_shelf(
        &self,
        transaction: &dyn Transaction,
        shelf_id: ShelfId,
        start_id: Option<BookId>,
        page_size: Option<u64>,
    ) -> Result<Vec<BookShelf>, Error>;

    // Smart shelf queries
    async fn books_for_filter(
        &self,
        transaction: &dyn Transaction,
        filter: &ShelfFilter,
        user_id: UserId,
        start_id: Option<BookId>,
        page_size: Option<u64>,
    ) -> Result<Vec<Book>, Error>;
    async fn count_for_filter(&self, transaction: &dyn Transaction, filter: &ShelfFilter, user_id: UserId) -> Result<u64, Error>;
}
