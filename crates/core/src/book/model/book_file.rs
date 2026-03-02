use crate::book::BookId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileFormat {
    Epub,
    Mobi,
    Azw3,
    Pdf,
    Cbz,
}

#[derive(Debug, Clone)]
pub struct BookFile {
    pub book_id: BookId,
    pub format: FileFormat,
    pub file_path: String,
    pub file_size: i64,
    pub file_hash: String,
}
