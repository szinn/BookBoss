use crate::book::BookId;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    pub file_size: i64,
    pub file_hash: String,
}

impl BookFile {
    #[cfg(any(test, feature = "test-support"))]
    pub fn fake(book_id: BookId, format: &str) -> Self {
        let format = match format {
            "mobi" => FileFormat::Mobi,
            "azw3" => FileFormat::Azw3,
            "pdf" => FileFormat::Pdf,
            "cbz" => FileFormat::Cbz,
            _ => FileFormat::Epub,
        };
        Self {
            book_id,
            format,
            file_size: 0,
            file_hash: String::new(),
        }
    }
}
