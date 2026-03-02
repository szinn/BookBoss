use crate::book::BookId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentifierType {
    Isbn10,
    Isbn13,
    Asin,
    GoogleBooks,
    OpenLibrary,
    Hardcover,
}

#[derive(Debug, Clone)]
pub struct BookIdentifier {
    pub book_id: BookId,
    pub identifier_type: IdentifierType,
    pub value: String,
}
