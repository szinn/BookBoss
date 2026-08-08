use bb_core::{
    Error, RepositoryError,
    book::BookId,
    device::DeviceId,
    repository::Transaction,
    shelf::{BookShelf, NewShelf, Shelf, ShelfId, ShelfRepository, ShelfToken, ShelfType},
    user::UserId,
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, ModelTrait, QueryFilter, QueryOrder, QuerySelect};

use crate::{
    entities::{book_shelves, prelude, shelves},
    error::handle_dberr,
    transaction::TransactionImpl,
};

// ── String conversions
// ────────────────────────────────────────────────────────

fn shelf_type_to_str(t: &ShelfType) -> &'static str {
    match t {
        ShelfType::System => "system",
        ShelfType::Manual => "manual",
        ShelfType::Smart => "smart",
    }
}

fn str_to_shelf_type(s: &str) -> ShelfType {
    match s {
        "system" => ShelfType::System,
        "smart" => ShelfType::Smart,
        _ => ShelfType::Manual,
    }
}

// ── From impls
// ────────────────────────────────────────────────────────────────

impl From<shelves::Model> for Shelf {
    fn from(m: shelves::Model) -> Self {
        let token = ShelfToken::new(m.id as u64);
        let filter_criteria = m.filter_criteria.and_then(|v| serde_json::from_value(v).ok());

        Self {
            id: m.id as u64,
            version: m.version as u64,
            token,
            owner_id: m.owner_id as u64,
            library_id: m.library_id as u64,
            name: m.name,
            shelf_type: str_to_shelf_type(&m.shelf_type),
            device_id: m.device_id.map(|id| id as u64),
            filter_criteria,
            created_at: m.created_at.with_timezone(&Utc),
            updated_at: m.updated_at.with_timezone(&Utc),
        }
    }
}

impl From<book_shelves::Model> for BookShelf {
    fn from(m: book_shelves::Model) -> Self {
        Self {
            book_id: m.book_id as u64,
            shelf_id: m.shelf_id as u64,
            added_at: m.added_at.with_timezone(&Utc),
            sort_order: m.sort_order,
        }
    }
}

// ── Adapter
// ───────────────────────────────────────────────────────────────────

pub(crate) struct ShelfRepositoryAdapter;

impl ShelfRepositoryAdapter {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl ShelfRepository for ShelfRepositoryAdapter {
    async fn add_shelf(&self, transaction: &dyn Transaction, book_shelf: NewShelf) -> Result<Shelf, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let token = ShelfToken::generate();
        let now = Utc::now();

        let model = shelves::ActiveModel {
            id: Set(token.id() as i64),
            token: Set(token.to_string()),
            owner_id: Set(book_shelf.owner_id as i64),
            library_id: Set(book_shelf.library_id as i64),
            name: Set(book_shelf.name),
            shelf_type: Set(shelf_type_to_str(&book_shelf.shelf_type).to_owned()),
            device_id: Set(book_shelf.device_id.map(|id| id as i64)),
            filter_criteria: Set(book_shelf.filter_criteria.and_then(|f| serde_json::to_value(f).ok())),
            version: Set(0),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        };

        let model = model.insert(transaction).await.map_err(handle_dberr)?;
        Ok(model.into())
    }

    async fn update_shelf(&self, transaction: &dyn Transaction, book_shelf: Shelf) -> Result<Shelf, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let existing = prelude::Shelves::find_by_id(book_shelf.id as i64)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .ok_or(Error::RepositoryError(RepositoryError::NotFound))?;

        if existing.version != book_shelf.version as i64 {
            return Err(Error::RepositoryError(RepositoryError::Conflict));
        }

        let mut updater: shelves::ActiveModel = existing.into();
        updater.library_id = Set(book_shelf.library_id as i64);
        updater.name = Set(book_shelf.name);
        updater.device_id = Set(book_shelf.device_id.map(|id| id as i64));
        updater.filter_criteria = Set(book_shelf.filter_criteria.and_then(|f| serde_json::to_value(f).ok()));

        let result = updater.update(transaction).await.map_err(handle_dberr)?;
        Ok(result.into())
    }

    async fn delete_shelf(&self, transaction: &dyn Transaction, book_shelf: Shelf) -> Result<(), Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        if let Some(existing) = prelude::Shelves::find_by_id(book_shelf.id as i64)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
        {
            existing.delete(transaction).await.map_err(handle_dberr)?;
        }

        Ok(())
    }

    async fn delete_shelves_for_user(&self, transaction: &dyn Transaction, owner_id: UserId) -> Result<(), Error> {
        let db = TransactionImpl::get_db_transaction(transaction)?;

        prelude::Shelves::delete_many()
            .filter(shelves::Column::OwnerId.eq(owner_id as i64))
            .exec(db)
            .await
            .map_err(handle_dberr)?;

        Ok(())
    }

    async fn find_by_id(&self, transaction: &dyn Transaction, id: ShelfId) -> Result<Option<Shelf>, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::Shelves::find_by_id(id as i64)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .map(Into::into))
    }

    async fn find_by_token(&self, transaction: &dyn Transaction, token: ShelfToken) -> Result<Option<Shelf>, Error> {
        self.find_by_id(transaction, token.id()).await
    }

    async fn list_for_user(&self, transaction: &dyn Transaction, owner_id: UserId) -> Result<Vec<Shelf>, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let rows = prelude::Shelves::find()
            .filter(shelves::Column::OwnerId.eq(owner_id as i64))
            .order_by_asc(shelves::Column::CreatedAt)
            .all(transaction)
            .await
            .map_err(handle_dberr)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn add_book_to_shelf(&self, transaction: &dyn Transaction, book_shelf: BookShelf) -> Result<BookShelf, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        // Idempotent: return existing row if book is already on this shelf.
        if let Some(existing) = prelude::BookShelves::find_by_id((book_shelf.book_id as i64, book_shelf.shelf_id as i64))
            .one(transaction)
            .await
            .map_err(handle_dberr)?
        {
            return Ok(existing.into());
        }

        // sort_order = MAX(sort_order) + 1 for this shelf, or 0 if empty.
        let max_sort_order = prelude::BookShelves::find()
            .filter(book_shelves::Column::ShelfId.eq(book_shelf.shelf_id as i64))
            .order_by_desc(book_shelves::Column::SortOrder)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .map_or(-1, |m| m.sort_order);

        let model = book_shelves::ActiveModel {
            book_id: Set(book_shelf.book_id as i64),
            shelf_id: Set(book_shelf.shelf_id as i64),
            added_at: Set(Utc::now().into()),
            sort_order: Set(max_sort_order + 1),
        };

        let model = model.insert(transaction).await.map_err(handle_dberr)?;
        Ok(model.into())
    }

    async fn remove_book_from_shelf(&self, transaction: &dyn Transaction, shelf_id: ShelfId, book_id: BookId) -> Result<(), Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        if let Some(existing) = prelude::BookShelves::find_by_id((book_id as i64, shelf_id as i64))
            .one(transaction)
            .await
            .map_err(handle_dberr)?
        {
            existing.delete(transaction).await.map_err(handle_dberr)?;
        }

        Ok(())
    }

    async fn books_for_shelf(
        &self,
        transaction: &dyn Transaction,
        shelf_id: ShelfId,
        offset: Option<u64>,
        page_size: Option<u64>,
    ) -> Result<Vec<BookShelf>, Error> {
        if let Some(page_size) = page_size
            && page_size < 1
        {
            return Err(Error::InvalidPageSize(page_size));
        }

        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let mut query = prelude::BookShelves::find()
            .filter(book_shelves::Column::ShelfId.eq(shelf_id as i64))
            .order_by_asc(book_shelves::Column::BookId);

        if let Some(offset) = offset {
            query = query.offset(offset);
        }

        if let Some(page_size) = page_size {
            query = query.limit(page_size);
        }

        let rows = query.all(transaction).await.map_err(handle_dberr)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn book_ids_for_user(&self, transaction: &dyn Transaction, user_id: UserId) -> Result<Vec<BookId>, Error> {
        let tx = TransactionImpl::get_db_transaction(transaction)?;

        // Step 1: shelf IDs owned by this user.
        let shelf_ids: Vec<i64> = prelude::Shelves::find()
            .filter(shelves::Column::OwnerId.eq(user_id as i64))
            .select_only()
            .column(shelves::Column::Id)
            .into_tuple::<i64>()
            .all(tx)
            .await
            .map_err(handle_dberr)?;

        if shelf_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Step 2: distinct book IDs across all those shelves.
        let book_ids: Vec<i64> = prelude::BookShelves::find()
            .filter(book_shelves::Column::ShelfId.is_in(shelf_ids))
            .select_only()
            .column(book_shelves::Column::BookId)
            .distinct()
            .into_tuple::<i64>()
            .all(tx)
            .await
            .map_err(handle_dberr)?;

        Ok(book_ids.into_iter().map(|id| id as BookId).collect())
    }

    async fn find_by_device_id(&self, transaction: &dyn Transaction, device_id: DeviceId) -> Result<Option<Shelf>, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::Shelves::find()
            .filter(shelves::Column::DeviceId.eq(device_id as i64))
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .map(Into::into))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bb_core::{
        book::{AuthorRole, BookStatus, NewAuthor, NewBook, NewGenre, NewSeries, NewTag},
        filter::{BookFilter, EntityRef, FilterCondition, FilterGroup, FilterReadStatus, FilterRule, NumericOp, SetOp, TextOp},
        reading::{ReadStatus, UserBookMetadata},
        repository::RepositoryService,
        shelf::{BookShelf, NewShelf, ShelfType},
        types::Capabilities,
        user::NewUser,
    };
    use chrono::Utc;
    use sea_orm::Database;

    use crate::create_repository_service;

    async fn setup() -> Arc<RepositoryService> {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        create_repository_service(db).await.unwrap()
    }

    async fn new_user(svc: &RepositoryService, username: &str) -> u64 {
        let tx = svc.repository().begin().await.unwrap();
        let user = svc
            .user_repository()
            .add_user(
                &*tx,
                NewUser::new(
                    username,
                    "password",
                    format!("{username}@example.com"),
                    Capabilities::default(),
                    "Test User",
                    false,
                )
                .unwrap(),
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
        user.id
    }

    async fn new_book(svc: &RepositoryService, title: &str) -> u64 {
        let tx = svc.repository().begin().await.unwrap();
        let book = svc
            .book_repository()
            .add_book(
                &*tx,
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
                    has_cover: false,
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
        book.id
    }

    fn manual_shelf(owner_id: u64, name: &str) -> NewShelf {
        NewShelf {
            owner_id,
            library_id: bb_core::library::ALL_BOOKS_LIBRARY_ID,
            name: name.to_owned(),
            shelf_type: ShelfType::Manual,
            device_id: None,
            filter_criteria: None,
        }
    }

    fn book_shelf_entry(book_id: u64, shelf_id: u64) -> BookShelf {
        BookShelf {
            book_id,
            shelf_id,
            added_at: Utc::now(),
            sort_order: 0,
        }
    }

    async fn new_book_with_rating(svc: &RepositoryService, title: &str, rating: i16) -> u64 {
        let tx = svc.repository().begin().await.unwrap();
        let book = svc
            .book_repository()
            .add_book(
                &*tx,
                NewBook {
                    title: title.to_owned(),
                    status: BookStatus::Available,
                    rating: Some(rating),
                    description: None,
                    published_date: None,
                    language: None,
                    series_id: None,
                    series_number: None,
                    publisher_id: None,
                    page_count: None,
                    metadata_source: None,
                    has_cover: false,
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
        book.id
    }

    async fn new_author(svc: &RepositoryService, name: &str) -> u64 {
        let tx = svc.repository().begin().await.unwrap();
        let author = svc
            .author_repository()
            .add_author(
                &*tx,
                NewAuthor {
                    name: name.to_owned(),
                    bio: None,
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
        author.id
    }

    async fn link_author(svc: &RepositoryService, book_id: u64, author_id: u64) {
        let tx = svc.repository().begin().await.unwrap();
        svc.book_repository()
            .add_book_author(&*tx, book_id, author_id, AuthorRole::Author, 0)
            .await
            .unwrap();
        tx.commit().await.unwrap();
    }

    async fn new_genre(svc: &RepositoryService, name: &str) -> u64 {
        let tx = svc.repository().begin().await.unwrap();
        let genre = svc.genre_repository().add_genre(&*tx, NewGenre { name: name.to_owned() }).await.unwrap();
        tx.commit().await.unwrap();
        genre.id
    }

    async fn link_genre(svc: &RepositoryService, book_id: u64, genre_id: u64) {
        let tx = svc.repository().begin().await.unwrap();
        svc.book_repository().add_book_genre(&*tx, book_id, genre_id).await.unwrap();
        tx.commit().await.unwrap();
    }

    async fn new_tag(svc: &RepositoryService, name: &str) -> u64 {
        let tx = svc.repository().begin().await.unwrap();
        let tag = svc.tag_repository().add_tag(&*tx, NewTag { name: name.to_owned() }).await.unwrap();
        tx.commit().await.unwrap();
        tag.id
    }

    async fn link_tag(svc: &RepositoryService, book_id: u64, tag_id: u64) {
        let tx = svc.repository().begin().await.unwrap();
        svc.book_repository().add_book_tag(&*tx, book_id, tag_id).await.unwrap();
        tx.commit().await.unwrap();
    }

    async fn set_read_status(svc: &RepositoryService, user_id: u64, book_id: u64, status: ReadStatus) {
        let tx = svc.repository().begin().await.unwrap();
        svc.user_book_metadata_repository()
            .upsert(
                &*tx,
                UserBookMetadata {
                    user_id,
                    book_id,
                    read_status: status,
                    progress_percentage: None,
                    position_type: None,
                    position_token: None,
                    last_progress_at: None,
                    spent_reading_minutes: None,
                    remaining_time_minutes: None,
                    personal_rating: None,
                    times_read: 0,
                    date_started: None,
                    date_finished: None,
                    last_opened_at: None,
                    notes: None,
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();
    }

    fn empty_and_filter() -> BookFilter {
        BookFilter::Group(FilterGroup {
            condition: FilterCondition::And,
            items: vec![],
        })
    }

    // ─── add_shelf ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_add_shelf_success() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let tx = svc.repository().begin().await.unwrap();

        let result = svc.shelf_repository().add_shelf(&*tx, manual_shelf(user_id, "Favourites")).await;

        assert!(result.is_ok());
        let shelf = result.unwrap();
        assert_ne!(shelf.id, 0);
        assert_eq!(shelf.name, "Favourites");
        assert_eq!(shelf.owner_id, user_id);
        assert_eq!(shelf.shelf_type, ShelfType::Manual);
        assert_eq!(shelf.token.id(), shelf.id);
    }

    // ─── find_by_id ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_id_found() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let tx = svc.repository().begin().await.unwrap();

        let shelf = svc.shelf_repository().add_shelf(&*tx, manual_shelf(user_id, "To Read")).await.unwrap();
        let found = svc.shelf_repository().find_by_id(&*tx, shelf.id).await.unwrap();

        assert!(found.is_some());
        assert_eq!(found.unwrap().id, shelf.id);
    }

    #[tokio::test]
    async fn test_find_by_id_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(svc.shelf_repository().find_by_id(&*tx, 999_999).await.unwrap().is_none());
    }

    // ─── find_by_token ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_token_found() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let tx = svc.repository().begin().await.unwrap();

        let shelf = svc.shelf_repository().add_shelf(&*tx, manual_shelf(user_id, "To Read")).await.unwrap();
        let found = svc.shelf_repository().find_by_token(&*tx, shelf.token).await.unwrap();

        assert!(found.is_some());
        assert_eq!(found.unwrap().id, shelf.id);
    }

    // ─── list_for_user ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_for_user_returns_own_shelves() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let tx = svc.repository().begin().await.unwrap();

        svc.shelf_repository().add_shelf(&*tx, manual_shelf(user_id, "A")).await.unwrap();
        svc.shelf_repository().add_shelf(&*tx, manual_shelf(user_id, "B")).await.unwrap();

        let shelves = svc.shelf_repository().list_for_user(&*tx, user_id).await.unwrap();
        assert_eq!(shelves.len(), 2);
    }

    #[tokio::test]
    async fn test_list_for_user_excludes_other_users() {
        let svc = setup().await;
        let alice = new_user(&svc, "alice").await;
        let bob = new_user(&svc, "bob").await;
        let tx = svc.repository().begin().await.unwrap();

        svc.shelf_repository().add_shelf(&*tx, manual_shelf(alice, "Alice's Shelf")).await.unwrap();
        svc.shelf_repository().add_shelf(&*tx, manual_shelf(bob, "Bob's Shelf")).await.unwrap();

        let alice_shelves = svc.shelf_repository().list_for_user(&*tx, alice).await.unwrap();
        assert_eq!(alice_shelves.len(), 1);
        assert_eq!(alice_shelves[0].owner_id, alice);
    }

    // ─── update_shelf ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_update_shelf_success() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let tx = svc.repository().begin().await.unwrap();

        let shelf = svc.shelf_repository().add_shelf(&*tx, manual_shelf(user_id, "Old Name")).await.unwrap();
        let version_before = shelf.version;

        let mut updated = shelf;
        updated.name = "New Name".to_owned();

        let result = svc.shelf_repository().update_shelf(&*tx, updated).await;
        assert!(result.is_ok());
        let saved = result.unwrap();
        assert_eq!(saved.name, "New Name");
        assert_eq!(saved.version, version_before + 1);
    }

    #[tokio::test]
    async fn test_update_shelf_version_conflict() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let tx = svc.repository().begin().await.unwrap();

        let mut shelf = svc.shelf_repository().add_shelf(&*tx, manual_shelf(user_id, "Shelf")).await.unwrap();
        shelf.version = 99;

        assert!(matches!(
            svc.shelf_repository().update_shelf(&*tx, shelf).await,
            Err(bb_core::Error::RepositoryError(bb_core::RepositoryError::Conflict))
        ));
    }

    #[tokio::test]
    async fn test_update_shelf_not_found() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let tx = svc.repository().begin().await.unwrap();

        let mut shelf = svc.shelf_repository().add_shelf(&*tx, manual_shelf(user_id, "Shelf")).await.unwrap();
        shelf.id = 999_999;

        assert!(matches!(
            svc.shelf_repository().update_shelf(&*tx, shelf).await,
            Err(bb_core::Error::RepositoryError(bb_core::RepositoryError::NotFound))
        ));
    }

    // ─── delete_shelf ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_delete_shelf_success() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let tx = svc.repository().begin().await.unwrap();

        let shelf = svc.shelf_repository().add_shelf(&*tx, manual_shelf(user_id, "Temp")).await.unwrap();
        svc.shelf_repository().delete_shelf(&*tx, shelf.clone()).await.unwrap();

        assert!(svc.shelf_repository().find_by_id(&*tx, shelf.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_delete_shelf_cascades_book_shelves() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let book_id = new_book(&svc, "Dune").await;
        let tx = svc.repository().begin().await.unwrap();

        let shelf = svc.shelf_repository().add_shelf(&*tx, manual_shelf(user_id, "Temp")).await.unwrap();
        let shelf_id = shelf.id;
        svc.shelf_repository()
            .add_book_to_shelf(&*tx, book_shelf_entry(book_id, shelf_id))
            .await
            .unwrap();
        svc.shelf_repository().delete_shelf(&*tx, shelf).await.unwrap();

        let remaining = svc.shelf_repository().books_for_shelf(&*tx, shelf_id, None, None).await.unwrap();
        assert!(remaining.is_empty());
    }

    // ─── add_book_to_shelf ───────────────────────────────────────────────────

    #[tokio::test]
    async fn test_add_book_to_shelf_success() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let book_id = new_book(&svc, "Dune").await;
        let tx = svc.repository().begin().await.unwrap();

        let shelf = svc.shelf_repository().add_shelf(&*tx, manual_shelf(user_id, "Favs")).await.unwrap();
        let result = svc.shelf_repository().add_book_to_shelf(&*tx, book_shelf_entry(book_id, shelf.id)).await;

        assert!(result.is_ok());
        let bs = result.unwrap();
        assert_eq!(bs.book_id, book_id);
        assert_eq!(bs.shelf_id, shelf.id);
        assert_eq!(bs.sort_order, 0);
    }

    #[tokio::test]
    async fn test_add_book_to_shelf_idempotent() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let book_id = new_book(&svc, "Dune").await;
        let tx = svc.repository().begin().await.unwrap();

        let shelf = svc.shelf_repository().add_shelf(&*tx, manual_shelf(user_id, "Favs")).await.unwrap();

        svc.shelf_repository()
            .add_book_to_shelf(&*tx, book_shelf_entry(book_id, shelf.id))
            .await
            .unwrap();
        let result = svc.shelf_repository().add_book_to_shelf(&*tx, book_shelf_entry(book_id, shelf.id)).await;

        result.unwrap();
        let books = svc.shelf_repository().books_for_shelf(&*tx, shelf.id, None, None).await.unwrap();
        assert_eq!(books.len(), 1);
    }

    #[tokio::test]
    async fn test_add_book_to_shelf_increments_sort_order() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let book1 = new_book(&svc, "Dune").await;
        let book2 = new_book(&svc, "Foundation").await;
        let tx = svc.repository().begin().await.unwrap();

        let shelf = svc.shelf_repository().add_shelf(&*tx, manual_shelf(user_id, "Favs")).await.unwrap();
        let bs1 = svc.shelf_repository().add_book_to_shelf(&*tx, book_shelf_entry(book1, shelf.id)).await.unwrap();
        let bs2 = svc.shelf_repository().add_book_to_shelf(&*tx, book_shelf_entry(book2, shelf.id)).await.unwrap();

        assert_eq!(bs1.sort_order, 0);
        assert_eq!(bs2.sort_order, 1);
    }

    // ─── remove_book_from_shelf ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_remove_book_from_shelf_success() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let book_id = new_book(&svc, "Dune").await;
        let tx = svc.repository().begin().await.unwrap();

        let shelf = svc.shelf_repository().add_shelf(&*tx, manual_shelf(user_id, "Favs")).await.unwrap();
        svc.shelf_repository()
            .add_book_to_shelf(&*tx, book_shelf_entry(book_id, shelf.id))
            .await
            .unwrap();
        svc.shelf_repository().remove_book_from_shelf(&*tx, shelf.id, book_id).await.unwrap();

        let books = svc.shelf_repository().books_for_shelf(&*tx, shelf.id, None, None).await.unwrap();
        assert!(books.is_empty());
    }

    #[tokio::test]
    async fn test_remove_book_from_shelf_nonexistent_is_ok() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let tx = svc.repository().begin().await.unwrap();

        let shelf = svc.shelf_repository().add_shelf(&*tx, manual_shelf(user_id, "Favs")).await.unwrap();
        let result = svc.shelf_repository().remove_book_from_shelf(&*tx, shelf.id, 999_999).await;

        result.unwrap();
    }

    // ─── books_for_shelf ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_books_for_shelf_sorted_by_sort_order() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let book1 = new_book(&svc, "Dune").await;
        let book2 = new_book(&svc, "Foundation").await;
        let book3 = new_book(&svc, "Hyperion").await;
        let tx = svc.repository().begin().await.unwrap();

        let shelf = svc.shelf_repository().add_shelf(&*tx, manual_shelf(user_id, "Favs")).await.unwrap();
        for book_id in [book1, book2, book3] {
            svc.shelf_repository()
                .add_book_to_shelf(&*tx, book_shelf_entry(book_id, shelf.id))
                .await
                .unwrap();
        }

        let all_books = svc.shelf_repository().books_for_shelf(&*tx, shelf.id, None, None).await.unwrap();
        assert_eq!(all_books.len(), 3);
    }

    #[tokio::test]
    async fn test_books_for_shelf_pagination() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let book1 = new_book(&svc, "A").await;
        let book2 = new_book(&svc, "B").await;
        let book3 = new_book(&svc, "C").await;
        let tx = svc.repository().begin().await.unwrap();

        let shelf = svc.shelf_repository().add_shelf(&*tx, manual_shelf(user_id, "Shelf")).await.unwrap();
        for book_id in [book1, book2, book3] {
            svc.shelf_repository()
                .add_book_to_shelf(&*tx, book_shelf_entry(book_id, shelf.id))
                .await
                .unwrap();
        }

        let page1 = svc.shelf_repository().books_for_shelf(&*tx, shelf.id, None, Some(2)).await.unwrap();
        assert_eq!(page1.len(), 2);

        let page2 = svc.shelf_repository().books_for_shelf(&*tx, shelf.id, Some(2), Some(2)).await.unwrap();
        assert_eq!(page2.len(), 1);
    }

    #[tokio::test]
    async fn test_books_for_shelf_none_page_size_returns_all_rows() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let mut book_ids = Vec::with_capacity(51);
        for i in 0..51 {
            book_ids.push(new_book(&svc, &format!("Book {i}")).await);
        }
        let tx = svc.repository().begin().await.unwrap();

        let shelf = svc.shelf_repository().add_shelf(&*tx, manual_shelf(user_id, "Big shelf")).await.unwrap();
        for book_id in book_ids {
            svc.shelf_repository()
                .add_book_to_shelf(&*tx, book_shelf_entry(book_id, shelf.id))
                .await
                .unwrap();
        }

        let entries = svc.shelf_repository().books_for_shelf(&*tx, shelf.id, None, None).await.unwrap();

        assert_eq!(entries.len(), 51, "page_size=None must return every shelf entry");
    }

    // ─── books_for_filter ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_books_for_filter_empty_and_returns_all_available() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        new_book(&svc, "Dune").await;
        new_book(&svc, "Foundation").await;

        let tx = svc.repository().begin().await.unwrap();
        let books = svc
            .collection_repository()
            .books_for_filter(&*tx, &empty_and_filter(), user_id, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(books.len(), 2);
    }

    #[tokio::test]
    async fn test_books_for_filter_page_size_zero_returns_error() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(matches!(
            svc.collection_repository()
                .books_for_filter(&*tx, &empty_and_filter(), user_id, None, None, Some(0), None)
                .await,
            Err(bb_core::Error::InvalidPageSize(0))
        ));
    }

    // ─── TitleText filter ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_books_for_filter_title_contains() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let dune = new_book(&svc, "Dune").await;
        new_book(&svc, "Foundation").await;

        let filter = BookFilter::Rule(FilterRule::TitleText {
            op: TextOp::Contains,
            value: "dun".to_owned(),
        });
        let tx = svc.repository().begin().await.unwrap();
        let books = svc
            .collection_repository()
            .books_for_filter(&*tx, &filter, user_id, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(books.len(), 1);
        assert_eq!(books[0].id, dune);
    }

    #[tokio::test]
    async fn test_books_for_filter_title_contains_case_insensitive() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let dune = new_book(&svc, "Dune Messiah").await;
        new_book(&svc, "Foundation").await;

        // Search in uppercase — should still match
        let filter = BookFilter::Rule(FilterRule::TitleText {
            op: TextOp::Contains,
            value: "DUNE".to_owned(),
        });
        let tx = svc.repository().begin().await.unwrap();
        let books = svc
            .collection_repository()
            .books_for_filter(&*tx, &filter, user_id, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(books.len(), 1);
        assert_eq!(books[0].id, dune);
    }

    #[tokio::test]
    async fn test_books_for_filter_title_starts_with() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let dune = new_book(&svc, "Dune Messiah").await;
        new_book(&svc, "Children of Dune").await;

        let filter = BookFilter::Rule(FilterRule::TitleText {
            op: TextOp::StartsWith,
            value: "Dune".to_owned(),
        });
        let tx = svc.repository().begin().await.unwrap();
        let books = svc
            .collection_repository()
            .books_for_filter(&*tx, &filter, user_id, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(books.len(), 1);
        assert_eq!(books[0].id, dune);
    }

    // ─── Series filter ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_books_for_filter_series_includes_any() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;

        let tx = svc.repository().begin().await.unwrap();
        let series = svc
            .series_repository()
            .add_series(
                &*tx,
                NewSeries {
                    name: "Dune Saga".to_owned(),
                    description: None,
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let tx = svc.repository().begin().await.unwrap();
        let dune = svc
            .book_repository()
            .add_book(
                &*tx,
                NewBook {
                    title: "Dune".to_owned(),
                    status: BookStatus::Available,
                    series_id: Some(series.id),
                    description: None,
                    published_date: None,
                    language: None,
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

        new_book(&svc, "Foundation").await;

        let filter = BookFilter::Rule(FilterRule::Series {
            op: SetOp::IncludesAny,
            values: vec![EntityRef {
                id: series.id as i64,
                label: "Dune Saga".to_owned(),
            }],
        });
        let tx = svc.repository().begin().await.unwrap();
        let books = svc
            .collection_repository()
            .books_for_filter(&*tx, &filter, user_id, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(books.len(), 1);
        assert_eq!(books[0].id, dune.id);
    }

    #[tokio::test]
    async fn test_books_for_filter_series_is_empty() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;

        let tx = svc.repository().begin().await.unwrap();
        let series = svc
            .series_repository()
            .add_series(
                &*tx,
                NewSeries {
                    name: "Dune Saga".to_owned(),
                    description: None,
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let tx = svc.repository().begin().await.unwrap();
        svc.book_repository()
            .add_book(
                &*tx,
                NewBook {
                    title: "Dune".to_owned(),
                    status: BookStatus::Available,
                    series_id: Some(series.id),
                    description: None,
                    published_date: None,
                    language: None,
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

        let standalone = new_book(&svc, "Foundation").await;

        let filter = BookFilter::Rule(FilterRule::Series {
            op: SetOp::IsEmpty,
            values: vec![],
        });
        let tx = svc.repository().begin().await.unwrap();
        let books = svc
            .collection_repository()
            .books_for_filter(&*tx, &filter, user_id, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(books.len(), 1);
        assert_eq!(books[0].id, standalone);
    }

    // ─── Author filter ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_books_for_filter_author_includes_any() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let dune = new_book(&svc, "Dune").await;
        let foundation = new_book(&svc, "Foundation").await;
        let herbert = new_author(&svc, "Frank Herbert").await;
        let asimov = new_author(&svc, "Isaac Asimov").await;
        link_author(&svc, dune, herbert).await;
        link_author(&svc, foundation, asimov).await;

        let filter = BookFilter::Rule(FilterRule::Author {
            op: SetOp::IncludesAny,
            values: vec![EntityRef {
                id: herbert as i64,
                label: "Frank Herbert".to_owned(),
            }],
        });
        let tx = svc.repository().begin().await.unwrap();
        let books = svc
            .collection_repository()
            .books_for_filter(&*tx, &filter, user_id, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(books.len(), 1);
        assert_eq!(books[0].id, dune);
    }

    #[tokio::test]
    async fn test_books_for_filter_author_includes_all() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let co_authored = new_book(&svc, "Co-Authored Book").await;
        let solo = new_book(&svc, "Solo Book").await;
        let author_a = new_author(&svc, "Author A").await;
        let author_b = new_author(&svc, "Author B").await;
        link_author(&svc, co_authored, author_a).await;
        link_author(&svc, co_authored, author_b).await;
        link_author(&svc, solo, author_a).await;

        // IncludesAll([A, B]) — only the co-authored book qualifies
        let filter = BookFilter::Rule(FilterRule::Author {
            op: SetOp::IncludesAll,
            values: vec![
                EntityRef {
                    id: author_a as i64,
                    label: "Author A".to_owned(),
                },
                EntityRef {
                    id: author_b as i64,
                    label: "Author B".to_owned(),
                },
            ],
        });
        let tx = svc.repository().begin().await.unwrap();
        let books = svc
            .collection_repository()
            .books_for_filter(&*tx, &filter, user_id, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(books.len(), 1);
        assert_eq!(books[0].id, co_authored);
    }

    #[tokio::test]
    async fn test_books_for_filter_author_excludes_all() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let dune = new_book(&svc, "Dune").await;
        let foundation = new_book(&svc, "Foundation").await;
        let herbert = new_author(&svc, "Frank Herbert").await;
        link_author(&svc, dune, herbert).await;
        // "Foundation" has no Herbert

        let filter = BookFilter::Rule(FilterRule::Author {
            op: SetOp::ExcludesAll,
            values: vec![EntityRef {
                id: herbert as i64,
                label: "Frank Herbert".to_owned(),
            }],
        });
        let tx = svc.repository().begin().await.unwrap();
        let books = svc
            .collection_repository()
            .books_for_filter(&*tx, &filter, user_id, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(books.len(), 1);
        assert_eq!(books[0].id, foundation);
    }

    #[tokio::test]
    async fn test_books_for_filter_author_text_contains() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let dune = new_book(&svc, "Dune").await;
        new_book(&svc, "Foundation").await;
        let herbert = new_author(&svc, "Frank Herbert").await;
        link_author(&svc, dune, herbert).await;

        let filter = BookFilter::Rule(FilterRule::AuthorText {
            op: TextOp::Contains,
            value: "herbert".to_owned(),
        });
        let tx = svc.repository().begin().await.unwrap();
        let books = svc
            .collection_repository()
            .books_for_filter(&*tx, &filter, user_id, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(books.len(), 1);
        assert_eq!(books[0].id, dune);
    }

    // ─── Genre filter ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_books_for_filter_genre_includes_any() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let dune = new_book(&svc, "Dune").await;
        new_book(&svc, "Foundation").await;
        let scifi = new_genre(&svc, "Science Fiction").await;
        link_genre(&svc, dune, scifi).await;

        let filter = BookFilter::Rule(FilterRule::Genre {
            op: SetOp::IncludesAny,
            values: vec![EntityRef {
                id: scifi as i64,
                label: "Science Fiction".to_owned(),
            }],
        });
        let tx = svc.repository().begin().await.unwrap();
        let books = svc
            .collection_repository()
            .books_for_filter(&*tx, &filter, user_id, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(books.len(), 1);
        assert_eq!(books[0].id, dune);
    }

    // ─── Tag filter ───────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_books_for_filter_tag_includes_any() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let dune = new_book(&svc, "Dune").await;
        new_book(&svc, "Foundation").await;
        let fav = new_tag(&svc, "favourite").await;
        link_tag(&svc, dune, fav).await;

        let filter = BookFilter::Rule(FilterRule::Tag {
            op: SetOp::IncludesAny,
            values: vec![EntityRef {
                id: fav as i64,
                label: "favourite".to_owned(),
            }],
        });
        let tx = svc.repository().begin().await.unwrap();
        let books = svc
            .collection_repository()
            .books_for_filter(&*tx, &filter, user_id, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(books.len(), 1);
        assert_eq!(books[0].id, dune);
    }

    // ─── ReadStatus filter ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_books_for_filter_read_status_includes_any_explicit() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let reading_book = new_book(&svc, "Currently Reading").await;
        new_book(&svc, "Unread Book").await;
        set_read_status(&svc, user_id, reading_book, ReadStatus::Reading).await;

        let filter = BookFilter::Rule(FilterRule::ReadStatus {
            op: SetOp::IncludesAny,
            values: vec![FilterReadStatus::Reading],
        });
        let tx = svc.repository().begin().await.unwrap();
        let books = svc
            .collection_repository()
            .books_for_filter(&*tx, &filter, user_id, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(books.len(), 1);
        assert_eq!(books[0].id, reading_book);
    }

    #[tokio::test]
    async fn test_books_for_filter_read_status_unread_includes_implicit() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let unread_book = new_book(&svc, "No UBM Row").await;
        let reading_book = new_book(&svc, "Currently Reading").await;
        set_read_status(&svc, user_id, reading_book, ReadStatus::Reading).await;

        // Unread includes books with no UBM row (implicit unread)
        let filter = BookFilter::Rule(FilterRule::ReadStatus {
            op: SetOp::IncludesAny,
            values: vec![FilterReadStatus::Unread],
        });
        let tx = svc.repository().begin().await.unwrap();
        let books = svc
            .collection_repository()
            .books_for_filter(&*tx, &filter, user_id, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(books.len(), 1);
        assert_eq!(books[0].id, unread_book);
    }

    #[tokio::test]
    async fn test_books_for_filter_read_status_active_expands() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let unread = new_book(&svc, "Unread (no row)").await;
        let reading = new_book(&svc, "Reading").await;
        let done = new_book(&svc, "Done").await;
        set_read_status(&svc, user_id, reading, ReadStatus::Reading).await;
        set_read_status(&svc, user_id, done, ReadStatus::Read).await;

        // Active expands to Unread + Reading + Rereading
        let filter = BookFilter::Rule(FilterRule::ReadStatus {
            op: SetOp::IncludesAny,
            values: vec![FilterReadStatus::Active],
        });
        let tx = svc.repository().begin().await.unwrap();
        let mut books = svc
            .collection_repository()
            .books_for_filter(&*tx, &filter, user_id, None, None, None, None)
            .await
            .unwrap();
        books.sort_by_key(|b| b.id);

        let ids: Vec<u64> = books.iter().map(|b| b.id).collect();
        assert!(ids.contains(&unread), "implicit unread should be included");
        assert!(ids.contains(&reading), "reading should be included");
        assert!(!ids.contains(&done), "read should be excluded");
    }

    #[tokio::test]
    async fn test_books_for_filter_read_status_excludes_all() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let unread = new_book(&svc, "Unread (no row)").await;
        let reading = new_book(&svc, "Reading").await;
        set_read_status(&svc, user_id, reading, ReadStatus::Reading).await;

        // ExcludesAll([Reading]) — only the implicit-unread book passes
        let filter = BookFilter::Rule(FilterRule::ReadStatus {
            op: SetOp::ExcludesAll,
            values: vec![FilterReadStatus::Reading],
        });
        let tx = svc.repository().begin().await.unwrap();
        let books = svc
            .collection_repository()
            .books_for_filter(&*tx, &filter, user_id, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(books.len(), 1);
        assert_eq!(books[0].id, unread);
    }

    // ─── Rating filter ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_books_for_filter_rating_gte() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let high = new_book_with_rating(&svc, "Five Star", 5).await;
        new_book_with_rating(&svc, "Two Star", 2).await;
        new_book(&svc, "Unrated").await;

        let filter = BookFilter::Rule(FilterRule::Rating { op: NumericOp::Gte, value: 4 });
        let tx = svc.repository().begin().await.unwrap();
        let books = svc
            .collection_repository()
            .books_for_filter(&*tx, &filter, user_id, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(books.len(), 1);
        assert_eq!(books[0].id, high);
    }

    // ─── Composite filters ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_books_for_filter_or_group() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let dune = new_book(&svc, "Dune").await;
        let foundation = new_book(&svc, "Foundation").await;
        new_book(&svc, "Hyperion").await;

        // OR(title contains "Dune", title contains "Foundation")
        let filter = BookFilter::Group(FilterGroup {
            condition: FilterCondition::Or,
            items: vec![
                BookFilter::Rule(FilterRule::TitleText {
                    op: TextOp::Contains,
                    value: "Dune".to_owned(),
                }),
                BookFilter::Rule(FilterRule::TitleText {
                    op: TextOp::Contains,
                    value: "Foundation".to_owned(),
                }),
            ],
        });
        let tx = svc.repository().begin().await.unwrap();
        let mut books = svc
            .collection_repository()
            .books_for_filter(&*tx, &filter, user_id, None, None, None, None)
            .await
            .unwrap();
        books.sort_by_key(|b| b.id);

        let ids: Vec<u64> = books.iter().map(|b| b.id).collect();
        assert_eq!(books.len(), 2);
        assert!(ids.contains(&dune));
        assert!(ids.contains(&foundation));
    }

    #[tokio::test]
    async fn test_books_for_filter_and_narrows_results() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let dune = new_book(&svc, "Dune").await;
        new_book(&svc, "Foundation").await;
        let herbert = new_author(&svc, "Frank Herbert").await;
        link_author(&svc, dune, herbert).await;

        // AND(title contains "Dune", author includes Frank Herbert) — exact match
        let filter = BookFilter::Rule(FilterRule::TitleText {
            op: TextOp::Contains,
            value: "Dune".to_owned(),
        })
        .and(BookFilter::Rule(FilterRule::Author {
            op: SetOp::IncludesAny,
            values: vec![EntityRef {
                id: herbert as i64,
                label: "Frank Herbert".to_owned(),
            }],
        }));
        let tx = svc.repository().begin().await.unwrap();
        let books = svc
            .collection_repository()
            .books_for_filter(&*tx, &filter, user_id, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(books.len(), 1);
        assert_eq!(books[0].id, dune);
    }

    #[tokio::test]
    async fn test_books_for_filter_empty_or_group_returns_nothing() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        new_book(&svc, "Dune").await;

        let filter = BookFilter::Group(FilterGroup {
            condition: FilterCondition::Or,
            items: vec![],
        });
        let tx = svc.repository().begin().await.unwrap();
        let books = svc
            .collection_repository()
            .books_for_filter(&*tx, &filter, user_id, None, None, None, None)
            .await
            .unwrap();

        assert!(books.is_empty());
    }

    // ─── count_for_filter ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_count_for_filter_matches_books_for_filter() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        new_book(&svc, "Dune").await;
        new_book(&svc, "Foundation").await;
        new_book(&svc, "Hyperion").await;

        let filter = empty_and_filter();
        let tx = svc.repository().begin().await.unwrap();
        let books = svc
            .collection_repository()
            .books_for_filter(&*tx, &filter, user_id, None, None, None, None)
            .await
            .unwrap();
        let count = svc.collection_repository().count_for_filter(&*tx, &filter, user_id, None).await.unwrap();

        assert_eq!(count, books.len() as u64);
        assert_eq!(count, 3);
    }
}
