use bb_core::{
    Error, RepositoryError,
    book::{
        AuthorRole, Book, BookAuthor, BookFile, BookFilter, BookId, BookIdentifier,
        BookRepository, BookStatus, BookToken, FileFormat, IdentifierType, MetadataSource, NewBook,
    },
    repository::Transaction,
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use sea_orm::sea_query::Query;

use crate::{
    entities::{book_authors, book_files, book_genres, book_identifiers, book_tags, books, prelude},
    error::handle_dberr,
    transaction::TransactionImpl,
};

// ─── Enum conversions ────────────────────────────────────────────────────────

fn book_status_to_str(status: &BookStatus) -> &'static str {
    match status {
        BookStatus::Incoming => "incoming",
        BookStatus::Available => "available",
        BookStatus::Archived => "archived",
    }
}

fn str_to_book_status(s: &str) -> Result<BookStatus, Error> {
    match s {
        "incoming" => Ok(BookStatus::Incoming),
        "available" => Ok(BookStatus::Available),
        "archived" => Ok(BookStatus::Archived),
        _ => Err(Error::RepositoryError(RepositoryError::Database(format!("unknown book status: {s}")))),
    }
}

fn metadata_source_to_str(ms: &MetadataSource) -> &'static str {
    match ms {
        MetadataSource::Hardcover => "hardcover",
        MetadataSource::OpenLibrary => "open_library",
        MetadataSource::Manual => "manual",
    }
}

fn str_to_metadata_source(s: &str) -> Result<MetadataSource, Error> {
    match s {
        "hardcover" => Ok(MetadataSource::Hardcover),
        "open_library" => Ok(MetadataSource::OpenLibrary),
        "manual" => Ok(MetadataSource::Manual),
        _ => Err(Error::RepositoryError(RepositoryError::Database(format!("unknown metadata source: {s}")))),
    }
}

fn str_to_author_role(s: &str) -> Result<AuthorRole, Error> {
    match s {
        "author" => Ok(AuthorRole::Author),
        "editor" => Ok(AuthorRole::Editor),
        "translator" => Ok(AuthorRole::Translator),
        "illustrator" => Ok(AuthorRole::Illustrator),
        _ => Err(Error::RepositoryError(RepositoryError::Database(format!("unknown author role: {s}")))),
    }
}

fn str_to_file_format(s: &str) -> Result<FileFormat, Error> {
    match s {
        "epub" => Ok(FileFormat::Epub),
        "mobi" => Ok(FileFormat::Mobi),
        "azw3" => Ok(FileFormat::Azw3),
        "pdf" => Ok(FileFormat::Pdf),
        "cbz" => Ok(FileFormat::Cbz),
        _ => Err(Error::RepositoryError(RepositoryError::Database(format!("unknown file format: {s}")))),
    }
}

fn str_to_identifier_type(s: &str) -> Result<IdentifierType, Error> {
    match s {
        "isbn10" => Ok(IdentifierType::Isbn10),
        "isbn13" => Ok(IdentifierType::Isbn13),
        "asin" => Ok(IdentifierType::Asin),
        "google_books" => Ok(IdentifierType::GoogleBooks),
        "open_library" => Ok(IdentifierType::OpenLibrary),
        "hardcover" => Ok(IdentifierType::Hardcover),
        _ => Err(Error::RepositoryError(RepositoryError::Database(format!("unknown identifier type: {s}")))),
    }
}

// ─── From impls ──────────────────────────────────────────────────────────────

impl From<books::Model> for Book {
    fn from(model: books::Model) -> Self {
        let token = BookToken::new(model.id as u64);
        Self {
            id: model.id as u64,
            version: model.version as u64,
            token,
            title: model.title,
            status: str_to_book_status(&model.status).expect("DB has unknown book status"),
            description: model.description,
            published_date: model.published_date,
            language: model.language,
            series_id: model.series_id.map(|id| id as u64),
            series_number: model.series_number,
            publisher_id: model.publisher_id.map(|id| id as u64),
            page_count: model.page_count,
            rating: model.rating,
            metadata_source: model
                .metadata_source
                .as_deref()
                .map(|s| str_to_metadata_source(s).expect("DB has unknown metadata source")),
            cover_path: model.cover_path,
            created_at: model.created_at.with_timezone(&Utc),
            updated_at: model.updated_at.with_timezone(&Utc),
        }
    }
}

// ─── Adapter ─────────────────────────────────────────────────────────────────

pub(crate) struct BookRepositoryAdapter;

impl BookRepositoryAdapter {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl BookRepository for BookRepositoryAdapter {
    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn add_book(&self, transaction: &dyn Transaction, book: NewBook) -> Result<Book, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let token = BookToken::generate();
        let now = Utc::now();

        let model = books::ActiveModel {
            id: Set(token.id() as i64),
            token: Set(token.to_string()),
            title: Set(book.title),
            status: Set(book_status_to_str(&book.status).to_string()),
            description: Set(book.description),
            published_date: Set(book.published_date),
            language: Set(book.language),
            series_id: Set(book.series_id.map(|id| id as i64)),
            series_number: Set(book.series_number),
            publisher_id: Set(book.publisher_id.map(|id| id as i64)),
            page_count: Set(book.page_count),
            rating: Set(book.rating),
            metadata_source: Set(book.metadata_source.as_ref().map(|ms| metadata_source_to_str(ms).to_string())),
            cover_path: Set(book.cover_path),
            version: Set(0),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };

        let model = model.insert(transaction).await.map_err(handle_dberr)?;

        Ok(model.into())
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn update_book(&self, transaction: &dyn Transaction, book: Book) -> Result<Book, Error> {
        if book.id == 0 {
            return Err(Error::InvalidId(book.id));
        }
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let existing = prelude::Books::find_by_id(book.id as i64)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

        if existing.version != book.version as i64 {
            return Err(Error::RepositoryError(RepositoryError::Conflict));
        }

        let mut updater: books::ActiveModel = existing.into();
        updater.title = Set(book.title);
        updater.status = Set(book_status_to_str(&book.status).to_string());
        updater.description = Set(book.description);
        updater.published_date = Set(book.published_date);
        updater.language = Set(book.language);
        updater.series_id = Set(book.series_id.map(|id| id as i64));
        updater.series_number = Set(book.series_number);
        updater.publisher_id = Set(book.publisher_id.map(|id| id as i64));
        updater.page_count = Set(book.page_count);
        updater.rating = Set(book.rating);
        updater.metadata_source = Set(book.metadata_source.as_ref().map(|ms| metadata_source_to_str(ms).to_string()));
        updater.cover_path = Set(book.cover_path);

        let updated = updater.update(transaction).await.map_err(handle_dberr)?;

        Ok(updated.into())
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn find_by_id(&self, transaction: &dyn Transaction, id: BookId) -> Result<Option<Book>, Error> {
        if id == 0 {
            return Err(Error::InvalidId(id));
        }
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::Books::find_by_id(id as i64)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .map(Into::into))
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn find_by_token(&self, transaction: &dyn Transaction, token: &BookToken) -> Result<Option<Book>, Error> {
        self.find_by_id(transaction, token.id()).await
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn list_books(
        &self,
        transaction: &dyn Transaction,
        filter: &BookFilter,
        start_id: Option<BookId>,
        page_size: Option<u64>,
    ) -> Result<Vec<Book>, Error> {
        const DEFAULT_PAGE_SIZE: u64 = 50;
        const MAX_PAGE_SIZE: u64 = 50;

        if let Some(page_size) = page_size {
            if page_size < 1 {
                return Err(Error::InvalidPageSize(page_size));
            }
        }

        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let mut query = prelude::Books::find().order_by_asc(books::Column::Id);

        if let Some(start_id) = start_id {
            query = query.filter(books::Column::Id.gte(start_id as i64));
        }

        if let Some(status) = &filter.status {
            query = query.filter(books::Column::Status.eq(book_status_to_str(status)));
        }

        if let Some(series_id) = filter.series_id {
            query = query.filter(books::Column::SeriesId.eq(series_id as i64));
        }

        if let Some(author_id) = filter.author_id {
            let mut subq = Query::select();
            subq.column(book_authors::Column::BookId)
                .from(book_authors::Entity)
                .and_where(book_authors::Column::AuthorId.eq(author_id as i64));
            query = query.filter(books::Column::Id.in_subquery(subq));
        }

        if let Some(genre_id) = filter.genre_id {
            let mut subq = Query::select();
            subq.column(book_genres::Column::BookId)
                .from(book_genres::Entity)
                .and_where(book_genres::Column::GenreId.eq(genre_id as i64));
            query = query.filter(books::Column::Id.in_subquery(subq));
        }

        if let Some(tag_id) = filter.tag_id {
            let mut subq = Query::select();
            subq.column(book_tags::Column::BookId)
                .from(book_tags::Entity)
                .and_where(book_tags::Column::TagId.eq(tag_id as i64));
            query = query.filter(books::Column::Id.in_subquery(subq));
        }

        let page_size = page_size.unwrap_or(DEFAULT_PAGE_SIZE).min(MAX_PAGE_SIZE);
        query = query.limit(page_size);

        let rows = query.all(transaction).await.map_err(handle_dberr)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn authors_for_book(&self, transaction: &dyn Transaction, book_id: BookId) -> Result<Vec<BookAuthor>, Error> {
        if book_id == 0 {
            return Err(Error::InvalidId(book_id));
        }
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let rows = prelude::BookAuthors::find()
            .filter(book_authors::Column::BookId.eq(book_id as i64))
            .order_by_asc(book_authors::Column::SortOrder)
            .all(transaction)
            .await
            .map_err(handle_dberr)?;

        rows.into_iter()
            .map(|m| {
                Ok(BookAuthor {
                    book_id: m.book_id as u64,
                    author_id: m.author_id as u64,
                    role: str_to_author_role(&m.role)?,
                    sort_order: m.sort_order,
                })
            })
            .collect()
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn files_for_book(&self, transaction: &dyn Transaction, book_id: BookId) -> Result<Vec<BookFile>, Error> {
        if book_id == 0 {
            return Err(Error::InvalidId(book_id));
        }
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let rows = prelude::BookFiles::find()
            .filter(book_files::Column::BookId.eq(book_id as i64))
            .all(transaction)
            .await
            .map_err(handle_dberr)?;

        rows.into_iter()
            .map(|m| {
                Ok(BookFile {
                    book_id: m.book_id as u64,
                    format: str_to_file_format(&m.format)?,
                    file_path: m.file_path,
                    file_size: m.file_size,
                    file_hash: m.file_hash,
                })
            })
            .collect()
    }

    #[tracing::instrument(level = "trace", skip(self, transaction))]
    async fn identifiers_for_book(&self, transaction: &dyn Transaction, book_id: BookId) -> Result<Vec<BookIdentifier>, Error> {
        if book_id == 0 {
            return Err(Error::InvalidId(book_id));
        }
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let rows = prelude::BookIdentifiers::find()
            .filter(book_identifiers::Column::BookId.eq(book_id as i64))
            .all(transaction)
            .await
            .map_err(handle_dberr)?;

        rows.into_iter()
            .map(|m| {
                Ok(BookIdentifier {
                    book_id: m.book_id as u64,
                    identifier_type: str_to_identifier_type(&m.identifier_type)?,
                    value: m.value,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bb_core::{
        Error, RepositoryError,
        book::{
            AuthorRole, Book, BookFilter, BookRepository, BookStatus, BookToken, FileFormat,
            IdentifierType, MetadataSource, NewAuthor, NewBook, NewGenre, NewSeries, NewTag,
        },
        repository::RepositoryService,
    };
    use sea_orm::{ActiveModelTrait, ActiveValue::Set, Database};

    use crate::{
        create_repository_service,
        entities::{book_authors, book_files, book_genres, book_identifiers, book_tags},
        transaction::TransactionImpl,
    };

    async fn setup() -> Arc<RepositoryService> {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        create_repository_service(db).await.unwrap()
    }

    fn new_book(title: &str) -> NewBook {
        NewBook {
            title: title.to_owned(),
            status: BookStatus::Available,
            description: None,
            published_date: None,
            language: None,
            series_id: None,
            series_number: None,
            publisher_id: None,
            page_count: None,
            rating: None,
            metadata_source: None,
            cover_path: None,
        }
    }

    // ─── add_book ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_add_book_success() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc.book_repository().add_book(&*tx, new_book("Dune")).await;

        assert!(result.is_ok());
        let b = result.unwrap();
        assert_ne!(b.id, 0);
        assert_eq!(b.title, "Dune");
        assert_eq!(b.status, BookStatus::Available);
        assert_eq!(b.token.id(), b.id);
    }

    #[tokio::test]
    async fn test_add_book_all_optional_fields() {
        use rust_decimal::Decimal;

        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let book = NewBook {
            title: "Foundation".to_owned(),
            status: BookStatus::Incoming,
            description: Some("A classic sci-fi novel".to_owned()),
            published_date: Some(1951),
            language: Some("en".to_owned()),
            series_id: None,
            series_number: Some(Decimal::new(1, 0)),
            publisher_id: None,
            page_count: Some(244),
            rating: Some(5),
            metadata_source: Some(MetadataSource::Manual),
            cover_path: Some("/covers/foundation.jpg".to_owned()),
        };

        let b = svc.book_repository().add_book(&*tx, book).await.unwrap();

        assert_eq!(b.status, BookStatus::Incoming);
        assert_eq!(b.description.as_deref(), Some("A classic sci-fi novel"));
        assert_eq!(b.published_date, Some(1951));
        assert_eq!(b.language.as_deref(), Some("en"));
        assert_eq!(b.page_count, Some(244));
        assert_eq!(b.rating, Some(5));
        assert_eq!(b.metadata_source, Some(MetadataSource::Manual));
        assert_eq!(b.cover_path.as_deref(), Some("/covers/foundation.jpg"));
    }

    // ─── find_by_id ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_id_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let inserted = svc.book_repository().add_book(&*tx, new_book("Dune")).await.unwrap();
        let result = svc.book_repository().find_by_id(&*tx, inserted.id).await;

        assert_eq!(result.unwrap().unwrap().title, "Dune");
    }

    #[tokio::test]
    async fn test_find_by_id_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(svc.book_repository().find_by_id(&*tx, 999).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_find_by_id_zero_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(matches!(svc.book_repository().find_by_id(&*tx, 0).await, Err(Error::InvalidId(0))));
    }

    // ─── find_by_token ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_token_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let inserted = svc.book_repository().add_book(&*tx, new_book("Dune")).await.unwrap();
        let result = svc.book_repository().find_by_token(&*tx, &inserted.token).await;

        assert_eq!(result.unwrap().unwrap().id, inserted.id);
    }

    #[tokio::test]
    async fn test_find_by_token_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(svc.book_repository().find_by_token(&*tx, &BookToken::new(999)).await.unwrap().is_none());
    }

    // ─── update_book ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_update_book_success() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut b = svc.book_repository().add_book(&*tx, new_book("Old Title")).await.unwrap();
        b.title = "New Title".to_owned();
        let updated = svc.book_repository().update_book(&*tx, b).await.unwrap();

        assert_eq!(updated.title, "New Title");
    }

    #[tokio::test]
    async fn test_update_book_increments_version() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut b = svc.book_repository().add_book(&*tx, new_book("Book")).await.unwrap();
        let version_before = b.version;
        b.title = "Updated".to_owned();
        let updated = svc.book_repository().update_book(&*tx, b).await.unwrap();

        assert_eq!(updated.version, version_before + 1);
    }

    #[tokio::test]
    async fn test_update_book_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let b = Book {
            id: 999,
            version: 0,
            token: BookToken::new(999),
            title: "Ghost".to_owned(),
            status: BookStatus::Available,
            description: None,
            published_date: None,
            language: None,
            series_id: None,
            series_number: None,
            publisher_id: None,
            page_count: None,
            rating: None,
            metadata_source: None,
            cover_path: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        assert!(matches!(
            svc.book_repository().update_book(&*tx, b).await,
            Err(Error::RepositoryError(RepositoryError::NotFound))
        ));
    }

    #[tokio::test]
    async fn test_update_book_version_conflict() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let mut b = svc.book_repository().add_book(&*tx, new_book("Book")).await.unwrap();
        b.version = 99;
        b.title = "Updated".to_owned();

        assert!(matches!(
            svc.book_repository().update_book(&*tx, b).await,
            Err(Error::RepositoryError(RepositoryError::Conflict))
        ));
    }

    #[tokio::test]
    async fn test_update_book_zero_id_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let b = Book {
            id: 0,
            version: 0,
            token: BookToken::new(1),
            title: "Invalid".to_owned(),
            status: BookStatus::Available,
            description: None,
            published_date: None,
            language: None,
            series_id: None,
            series_number: None,
            publisher_id: None,
            page_count: None,
            rating: None,
            metadata_source: None,
            cover_path: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        assert!(matches!(svc.book_repository().update_book(&*tx, b).await, Err(Error::InvalidId(0))));
    }

    // ─── list_books ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_books_empty() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(svc.book_repository().list_books(&*tx, &BookFilter::default(), None, None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_books_returns_all() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.book_repository().add_book(&*tx, new_book("Book A")).await.unwrap();
        svc.book_repository().add_book(&*tx, new_book("Book B")).await.unwrap();

        assert_eq!(svc.book_repository().list_books(&*tx, &BookFilter::default(), None, None).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_list_books_filter_by_status() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.book_repository().add_book(&*tx, new_book("Available")).await.unwrap();
        svc.book_repository()
            .add_book(&*tx, NewBook { status: BookStatus::Incoming, ..new_book("Incoming") })
            .await
            .unwrap();

        let filter = BookFilter { status: Some(BookStatus::Available), ..Default::default() };
        let results = svc.book_repository().list_books(&*tx, &filter, None, None).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Available");
    }

    #[tokio::test]
    async fn test_list_books_filter_by_series_id() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let series = svc
            .series_repository()
            .add_series(&*tx, NewSeries { name: "Dune".into(), description: None })
            .await
            .unwrap();
        let b1 = svc
            .book_repository()
            .add_book(&*tx, NewBook { series_id: Some(series.id), ..new_book("Dune 1") })
            .await
            .unwrap();
        svc.book_repository().add_book(&*tx, new_book("Other")).await.unwrap();

        let filter = BookFilter { series_id: Some(series.id), ..Default::default() };
        let results = svc.book_repository().list_books(&*tx, &filter, None, None).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, b1.id);
    }

    #[tokio::test]
    async fn test_list_books_filter_by_author_id() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let author = svc.author_repository().add_author(&*tx, NewAuthor { name: "Herbert".into(), bio: None }).await.unwrap();
        let book = svc.book_repository().add_book(&*tx, new_book("Dune")).await.unwrap();
        svc.book_repository().add_book(&*tx, new_book("Other")).await.unwrap();

        let db_tx = TransactionImpl::get_db_transaction(&*tx).unwrap();
        book_authors::ActiveModel {
            book_id: Set(book.id as i64),
            author_id: Set(author.id as i64),
            role: Set("author".to_owned()),
            sort_order: Set(0),
        }
        .insert(db_tx)
        .await
        .unwrap();

        let filter = BookFilter { author_id: Some(author.id), ..Default::default() };
        let results = svc.book_repository().list_books(&*tx, &filter, None, None).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, book.id);
    }

    #[tokio::test]
    async fn test_list_books_filter_by_genre_id() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let genre = svc.genre_repository().add_genre(&*tx, NewGenre { name: "Sci-Fi".into() }).await.unwrap();
        let book = svc.book_repository().add_book(&*tx, new_book("Dune")).await.unwrap();
        svc.book_repository().add_book(&*tx, new_book("Other")).await.unwrap();

        let db_tx = TransactionImpl::get_db_transaction(&*tx).unwrap();
        book_genres::ActiveModel {
            book_id: Set(book.id as i64),
            genre_id: Set(genre.id as i64),
        }
        .insert(db_tx)
        .await
        .unwrap();

        let filter = BookFilter { genre_id: Some(genre.id), ..Default::default() };
        let results = svc.book_repository().list_books(&*tx, &filter, None, None).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, book.id);
    }

    #[tokio::test]
    async fn test_list_books_filter_by_tag_id() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let tag = svc.tag_repository().add_tag(&*tx, NewTag { name: "classic".into() }).await.unwrap();
        let book = svc.book_repository().add_book(&*tx, new_book("Dune")).await.unwrap();
        svc.book_repository().add_book(&*tx, new_book("Other")).await.unwrap();

        let db_tx = TransactionImpl::get_db_transaction(&*tx).unwrap();
        book_tags::ActiveModel {
            book_id: Set(book.id as i64),
            tag_id: Set(tag.id as i64),
        }
        .insert(db_tx)
        .await
        .unwrap();

        let filter = BookFilter { tag_id: Some(tag.id), ..Default::default() };
        let results = svc.book_repository().list_books(&*tx, &filter, None, None).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, book.id);
    }

    #[tokio::test]
    async fn test_list_books_start_id_filters() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        svc.book_repository().add_book(&*tx, new_book("Book A")).await.unwrap();
        svc.book_repository().add_book(&*tx, new_book("Book B")).await.unwrap();

        let all = svc.book_repository().list_books(&*tx, &BookFilter::default(), None, None).await.unwrap();
        let result = svc.book_repository().list_books(&*tx, &BookFilter::default(), Some(all[1].id), None).await.unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, all[1].id);
    }

    #[tokio::test]
    async fn test_list_books_page_size_zero_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(matches!(
            svc.book_repository().list_books(&*tx, &BookFilter::default(), None, Some(0)).await,
            Err(Error::InvalidPageSize(0))
        ));
    }

    // ─── authors_for_book ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_authors_for_book_empty() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let book = svc.book_repository().add_book(&*tx, new_book("Dune")).await.unwrap();

        assert!(svc.book_repository().authors_for_book(&*tx, book.id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_authors_for_book_ordered_by_sort_order() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let book = svc.book_repository().add_book(&*tx, new_book("Dune")).await.unwrap();
        let a1 = svc.author_repository().add_author(&*tx, NewAuthor { name: "Author A".into(), bio: None }).await.unwrap();
        let a2 = svc.author_repository().add_author(&*tx, NewAuthor { name: "Author B".into(), bio: None }).await.unwrap();

        let db_tx = TransactionImpl::get_db_transaction(&*tx).unwrap();
        book_authors::ActiveModel { book_id: Set(book.id as i64), author_id: Set(a1.id as i64), role: Set("author".to_owned()), sort_order: Set(2) }
            .insert(db_tx)
            .await
            .unwrap();
        book_authors::ActiveModel { book_id: Set(book.id as i64), author_id: Set(a2.id as i64), role: Set("editor".to_owned()), sort_order: Set(1) }
            .insert(db_tx)
            .await
            .unwrap();

        let results = svc.book_repository().authors_for_book(&*tx, book.id).await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].author_id, a2.id);
        assert_eq!(results[0].role, AuthorRole::Editor);
        assert_eq!(results[1].author_id, a1.id);
        assert_eq!(results[1].role, AuthorRole::Author);
    }

    #[tokio::test]
    async fn test_authors_for_book_zero_id_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(matches!(svc.book_repository().authors_for_book(&*tx, 0).await, Err(Error::InvalidId(0))));
    }

    // ─── files_for_book ──────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_files_for_book_empty() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let book = svc.book_repository().add_book(&*tx, new_book("Dune")).await.unwrap();

        assert!(svc.book_repository().files_for_book(&*tx, book.id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_files_for_book_returns_files() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let book = svc.book_repository().add_book(&*tx, new_book("Dune")).await.unwrap();

        let db_tx = TransactionImpl::get_db_transaction(&*tx).unwrap();
        book_files::ActiveModel {
            book_id: Set(book.id as i64),
            format: Set("epub".to_owned()),
            file_path: Set("/books/dune.epub".to_owned()),
            file_size: Set(1_024_000),
            file_hash: Set("abc123".to_owned()),
        }
        .insert(db_tx)
        .await
        .unwrap();

        let results = svc.book_repository().files_for_book(&*tx, book.id).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].format, FileFormat::Epub);
        assert_eq!(results[0].file_path, "/books/dune.epub");
        assert_eq!(results[0].file_size, 1_024_000);
    }

    #[tokio::test]
    async fn test_files_for_book_zero_id_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(matches!(svc.book_repository().files_for_book(&*tx, 0).await, Err(Error::InvalidId(0))));
    }

    // ─── identifiers_for_book ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_identifiers_for_book_empty() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let book = svc.book_repository().add_book(&*tx, new_book("Dune")).await.unwrap();

        assert!(svc.book_repository().identifiers_for_book(&*tx, book.id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_identifiers_for_book_returns_identifiers() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let book = svc.book_repository().add_book(&*tx, new_book("Dune")).await.unwrap();

        let db_tx = TransactionImpl::get_db_transaction(&*tx).unwrap();
        book_identifiers::ActiveModel {
            book_id: Set(book.id as i64),
            identifier_type: Set("isbn13".to_owned()),
            value: Set("9780441172719".to_owned()),
        }
        .insert(db_tx)
        .await
        .unwrap();

        let results = svc.book_repository().identifiers_for_book(&*tx, book.id).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].identifier_type, IdentifierType::Isbn13);
        assert_eq!(results[0].value, "9780441172719");
    }

    #[tokio::test]
    async fn test_identifiers_for_book_zero_id_returns_error() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(matches!(svc.book_repository().identifiers_for_book(&*tx, 0).await, Err(Error::InvalidId(0))));
    }
}
