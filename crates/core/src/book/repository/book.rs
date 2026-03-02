use crate::{
    Error,
    book::{Book, BookAuthor, BookFile, BookFilter, BookId, BookIdentifier, BookToken, NewBook},
    repository::Transaction,
};

#[async_trait::async_trait]
pub trait BookRepository: Send + Sync {
    async fn add_book(&self, transaction: &dyn Transaction, book: NewBook) -> Result<Book, Error>;
    async fn update_book(&self, transaction: &dyn Transaction, book: Book) -> Result<Book, Error>;
    async fn find_by_id(&self, transaction: &dyn Transaction, id: BookId) -> Result<Option<Book>, Error>;
    async fn find_by_token(&self, transaction: &dyn Transaction, token: &BookToken) -> Result<Option<Book>, Error>;
    async fn list_books(
        &self,
        transaction: &dyn Transaction,
        filter: &BookFilter,
        start_id: Option<BookId>,
        page_size: Option<u64>,
    ) -> Result<Vec<Book>, Error>;
    async fn authors_for_book(&self, transaction: &dyn Transaction, book_id: BookId) -> Result<Vec<BookAuthor>, Error>;
    async fn files_for_book(&self, transaction: &dyn Transaction, book_id: BookId) -> Result<Vec<BookFile>, Error>;
    async fn identifiers_for_book(&self, transaction: &dyn Transaction, book_id: BookId) -> Result<Vec<BookIdentifier>, Error>;
}
