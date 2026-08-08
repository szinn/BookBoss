use bb_core::{
    Error,
    book::{Book, BookSortOrder},
    collection::CollectionRepository,
    filter::BookFilter,
    library::LibraryId,
    repository::Transaction,
    user::UserId,
};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect, sea_query::Query};

use crate::{
    entities::{books, library_books, prelude},
    error::handle_dberr,
    filter::build_condition,
    transaction::TransactionImpl,
};

pub struct CollectionRepositoryAdapter;

impl CollectionRepositoryAdapter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl CollectionRepository for CollectionRepositoryAdapter {
    async fn count_available_books(&self, transaction: &dyn Transaction) -> Result<u64, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::Books::find()
            .filter(books::Column::Status.eq("available"))
            .count(transaction)
            .await
            .map_err(handle_dberr)?)
    }

    async fn count_authors(&self, transaction: &dyn Transaction) -> Result<u64, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::Authors::find().count(transaction).await.map_err(handle_dberr)?)
    }

    async fn books_for_filter(
        &self,
        transaction: &dyn Transaction,
        filter: &BookFilter,
        user_id: UserId,
        library_id: Option<LibraryId>,
        offset: Option<u64>,
        page_size: Option<u64>,
        sort: Option<BookSortOrder>,
    ) -> Result<Vec<Book>, Error> {
        if let Some(page_size) = page_size
            && page_size < 1
        {
            return Err(Error::InvalidPageSize(page_size));
        }

        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let mut query = prelude::Books::find()
            .filter(books::Column::Status.eq("available"))
            .filter(build_condition(filter, user_id).map_err(bb_core::Error::RepositoryError)?);

        if let Some(lib_id) = library_id
            && !filter.contains_library_rule()
        {
            let mut subq = Query::select();
            subq.column(library_books::Column::BookId)
                .from(library_books::Entity)
                .and_where(library_books::Column::LibraryId.eq(lib_id as i64));
            query = query.filter(books::Column::Id.in_subquery(subq));
        }

        let query = crate::sort::apply_book_sort(query, sort);

        let mut query = if let Some(offset) = offset { query.offset(offset) } else { query };

        if let Some(page_size) = page_size {
            query = query.limit(page_size);
        }

        let rows = query.all(transaction).await.map_err(handle_dberr)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn count_for_filter(&self, transaction: &dyn Transaction, filter: &BookFilter, user_id: UserId, library_id: Option<LibraryId>) -> Result<u64, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let mut query = prelude::Books::find()
            .filter(books::Column::Status.eq("available"))
            .filter(build_condition(filter, user_id).map_err(bb_core::Error::RepositoryError)?);

        if let Some(lib_id) = library_id
            && !filter.contains_library_rule()
        {
            let mut subq = Query::select();
            subq.column(library_books::Column::BookId)
                .from(library_books::Entity)
                .and_where(library_books::Column::LibraryId.eq(lib_id as i64));
            query = query.filter(books::Column::Id.in_subquery(subq));
        }

        Ok(query.count(transaction).await.map_err(handle_dberr)?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bb_core::{
        book::{BookStatus, NewBook},
        filter::{BookFilter, FilterCondition, FilterGroup},
        repository::RepositoryService,
    };
    use sea_orm::Database;

    use crate::create_repository_service;

    async fn setup() -> Arc<RepositoryService> {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        create_repository_service(db).await.unwrap()
    }

    async fn add_book(svc: &RepositoryService, title: &str, status: BookStatus) -> u64 {
        let tx = svc.repository().begin().await.unwrap();
        let book = svc
            .book_repository()
            .add_book(
                &*tx,
                NewBook {
                    title: title.to_owned(),
                    status,
                    description: None,
                    published_date: None,
                    language: None,
                    series_id: None,
                    series_number: None,
                    publisher_id: None,
                    page_count: None,
                    rating: None,
                    metadata_source: None,
                    has_cover: false,
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
        book.id
    }

    fn empty_filter() -> BookFilter {
        BookFilter::Group(FilterGroup {
            condition: FilterCondition::And,
            items: vec![],
        })
    }

    // ─── count_available_books ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_count_available_books_empty() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();
        assert_eq!(svc.collection_repository().count_available_books(&*tx).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_count_available_books_filters_by_status() {
        let svc = setup().await;
        add_book(&svc, "Available Book", BookStatus::Available).await;
        add_book(&svc, "Incoming Book", BookStatus::Incoming).await;

        let tx = svc.repository().begin().await.unwrap();
        assert_eq!(svc.collection_repository().count_available_books(&*tx).await.unwrap(), 1);
    }

    // ─── count_authors ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_count_authors_empty() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();
        assert_eq!(svc.collection_repository().count_authors(&*tx).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_count_authors() {
        use bb_core::book::NewAuthor;

        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();
        svc.author_repository()
            .add_author(
                &*tx,
                NewAuthor {
                    name: "Author A".into(),
                    bio: None,
                },
            )
            .await
            .unwrap();
        svc.author_repository()
            .add_author(
                &*tx,
                NewAuthor {
                    name: "Author B".into(),
                    bio: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(svc.collection_repository().count_authors(&*tx).await.unwrap(), 2);
    }

    // ─── books_for_filter ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_books_for_filter_returns_available_books() {
        let svc = setup().await;
        add_book(&svc, "Available Book", BookStatus::Available).await;
        add_book(&svc, "Incoming Book", BookStatus::Incoming).await;

        let tx = svc.repository().begin().await.unwrap();
        let results = svc
            .collection_repository()
            .books_for_filter(&*tx, &empty_filter(), 0, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Available Book");
    }

    #[tokio::test]
    async fn test_books_for_filter_page_size_zero_returns_error() {
        use bb_core::Error;

        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();
        assert!(matches!(
            svc.collection_repository()
                .books_for_filter(&*tx, &empty_filter(), 0, None, None, Some(0), None)
                .await,
            Err(Error::InvalidPageSize(0))
        ));
    }

    #[tokio::test]
    async fn test_books_for_filter_none_page_size_returns_all_rows() {
        let svc = setup().await;
        for i in 0..51 {
            add_book(&svc, &format!("Book {i}"), BookStatus::Available).await;
        }
        let tx = svc.repository().begin().await.unwrap();

        let books = svc
            .collection_repository()
            .books_for_filter(&*tx, &empty_filter(), 0, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(books.len(), 51, "page_size=None must return every matching row");
    }

    // ─── count_for_filter ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_count_for_filter_matches_list_length() {
        let svc = setup().await;
        add_book(&svc, "Book One", BookStatus::Available).await;
        add_book(&svc, "Book Two", BookStatus::Available).await;
        add_book(&svc, "Incoming", BookStatus::Incoming).await;

        let tx = svc.repository().begin().await.unwrap();
        let count = svc.collection_repository().count_for_filter(&*tx, &empty_filter(), 0, None).await.unwrap();
        let list = svc
            .collection_repository()
            .books_for_filter(&*tx, &empty_filter(), 0, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(count, list.len() as u64);
        assert_eq!(count, 2);
    }
}
