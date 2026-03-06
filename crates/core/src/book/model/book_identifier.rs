use crate::book::BookId;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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

impl BookIdentifier {
    #[cfg(any(test, feature = "test-support"))]
    pub fn fake(book_id: BookId, identifier_type: &str, value: impl Into<String>) -> Self {
        let identifier_type = match identifier_type {
            "isbn10" => IdentifierType::Isbn10,
            "asin" => IdentifierType::Asin,
            "google_books" => IdentifierType::GoogleBooks,
            "open_library" => IdentifierType::OpenLibrary,
            "hardcover" => IdentifierType::Hardcover,
            _ => IdentifierType::Isbn13,
        };
        Self {
            book_id,
            identifier_type,
            value: value.into(),
        }
    }
}
