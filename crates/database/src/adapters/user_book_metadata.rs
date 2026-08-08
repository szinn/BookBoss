use bb_core::{
    Error,
    book::BookId,
    reading::{ReadStatus, UserBookMetadata, UserBookMetadataRepository},
    repository::Transaction,
    user::UserId,
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use crate::{
    entities::{prelude, user_book_metadata},
    error::handle_dberr,
    transaction::TransactionImpl,
};

// ── String conversions
// ────────────────────────────────────────────────────────

fn read_status_to_str(s: ReadStatus) -> &'static str {
    match s {
        ReadStatus::Unread => "unread",
        ReadStatus::Reading => "reading",
        ReadStatus::Paused => "paused",
        ReadStatus::Rereading => "rereading",
        ReadStatus::Read => "read",
        ReadStatus::Abandoned => "abandoned",
    }
}

fn str_to_read_status(s: &str) -> ReadStatus {
    match s {
        "reading" => ReadStatus::Reading,
        "paused" => ReadStatus::Paused,
        "rereading" => ReadStatus::Rereading,
        "read" => ReadStatus::Read,
        "abandoned" => ReadStatus::Abandoned,
        _ => ReadStatus::Unread,
    }
}

// ── From impl
// ─────────────────────────────────────────────────────────────────

impl From<user_book_metadata::Model> for UserBookMetadata {
    fn from(m: user_book_metadata::Model) -> Self {
        Self {
            user_id: m.user_id as u64,
            book_id: m.book_id as u64,
            read_status: str_to_read_status(&m.read_status),
            progress_percentage: m.progress_percentage.map(|v| v as u16),
            position_type: m.position_type,
            position_token: m.position_token,
            last_progress_at: m.last_progress_at.map(|t| t.with_timezone(&Utc)),
            spent_reading_minutes: m.spent_reading_minutes,
            remaining_time_minutes: m.remaining_time_minutes,
            personal_rating: m.personal_rating.map(|v| v as u8),
            times_read: m.times_read as u32,
            date_started: m.date_started.map(|t| t.with_timezone(&Utc)),
            date_finished: m.date_finished.map(|t| t.with_timezone(&Utc)),
            last_opened_at: m.last_opened_at.map(|t| t.with_timezone(&Utc)),
            notes: m.notes,
        }
    }
}

// ── Adapter
// ───────────────────────────────────────────────────────────────────

pub(crate) struct UserBookMetadataRepositoryAdapter;

impl UserBookMetadataRepositoryAdapter {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl UserBookMetadataRepository for UserBookMetadataRepositoryAdapter {
    async fn upsert(&self, transaction: &dyn Transaction, metadata: UserBookMetadata) -> Result<UserBookMetadata, Error> {
        let tx = TransactionImpl::get_db_transaction(transaction)?;

        let existing = prelude::UserBookMetadata::find_by_id((metadata.user_id as i64, metadata.book_id as i64))
            .one(tx)
            .await
            .map_err(handle_dberr)?;

        let model = if let Some(existing) = existing {
            let mut active: user_book_metadata::ActiveModel = existing.into();
            active.read_status = Set(read_status_to_str(metadata.read_status).to_owned());
            active.progress_percentage = Set(metadata.progress_percentage.map(|v| v as i16));
            active.position_type = Set(metadata.position_type);
            active.position_token = Set(metadata.position_token);
            active.last_progress_at = Set(metadata.last_progress_at.map(Into::into));
            active.spent_reading_minutes = Set(metadata.spent_reading_minutes);
            active.remaining_time_minutes = Set(metadata.remaining_time_minutes);
            active.personal_rating = Set(metadata.personal_rating.map(i16::from));
            active.times_read = Set(metadata.times_read as i32);
            active.date_started = Set(metadata.date_started.map(Into::into));
            active.date_finished = Set(metadata.date_finished.map(Into::into));
            active.last_opened_at = Set(metadata.last_opened_at.map(Into::into));
            active.notes = Set(metadata.notes);
            active.update(tx).await.map_err(handle_dberr)?
        } else {
            let active = user_book_metadata::ActiveModel {
                user_id: Set(metadata.user_id as i64),
                book_id: Set(metadata.book_id as i64),
                read_status: Set(read_status_to_str(metadata.read_status).to_owned()),
                progress_percentage: Set(metadata.progress_percentage.map(|v| v as i16)),
                position_type: Set(metadata.position_type),
                position_token: Set(metadata.position_token),
                last_progress_at: Set(metadata.last_progress_at.map(Into::into)),
                spent_reading_minutes: Set(metadata.spent_reading_minutes),
                remaining_time_minutes: Set(metadata.remaining_time_minutes),
                personal_rating: Set(metadata.personal_rating.map(i16::from)),
                times_read: Set(metadata.times_read as i32),
                date_started: Set(metadata.date_started.map(Into::into)),
                date_finished: Set(metadata.date_finished.map(Into::into)),
                last_opened_at: Set(metadata.last_opened_at.map(Into::into)),
                notes: Set(metadata.notes),
            };
            active.insert(tx).await.map_err(handle_dberr)?
        };

        Ok(model.into())
    }

    async fn find_by_user_and_book(&self, transaction: &dyn Transaction, user_id: UserId, book_id: BookId) -> Result<Option<UserBookMetadata>, Error> {
        let tx = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::UserBookMetadata::find_by_id((user_id as i64, book_id as i64))
            .one(tx)
            .await
            .map_err(handle_dberr)?
            .map(Into::into))
    }

    async fn list_for_user(
        &self,
        transaction: &dyn Transaction,
        user_id: UserId,
        status: Option<ReadStatus>,
        start_book_id: Option<BookId>,
        page_size: Option<u64>,
    ) -> Result<Vec<UserBookMetadata>, Error> {
        if let Some(page_size) = page_size
            && page_size < 1
        {
            return Err(Error::InvalidPageSize(page_size));
        }

        let tx = TransactionImpl::get_db_transaction(transaction)?;

        let mut query = prelude::UserBookMetadata::find()
            .filter(user_book_metadata::Column::UserId.eq(user_id as i64))
            .order_by_asc(user_book_metadata::Column::BookId);

        if let Some(status) = status {
            query = query.filter(user_book_metadata::Column::ReadStatus.eq(read_status_to_str(status)));
        }

        if let Some(start_id) = start_book_id {
            query = query.filter(user_book_metadata::Column::BookId.gte(start_id as i64));
        }

        if let Some(limit) = page_size {
            query = query.limit(limit);
        }
        let rows = query.all(tx).await.map_err(handle_dberr)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn list_for_user_and_books(&self, transaction: &dyn Transaction, user_id: UserId, book_ids: &[BookId]) -> Result<Vec<UserBookMetadata>, Error> {
        if book_ids.is_empty() {
            return Ok(Vec::new());
        }

        let tx = TransactionImpl::get_db_transaction(transaction)?;

        let ids: Vec<i64> = book_ids.iter().map(|&id| id as i64).collect();
        let rows = prelude::UserBookMetadata::find()
            .filter(user_book_metadata::Column::UserId.eq(user_id as i64))
            .filter(user_book_metadata::Column::BookId.is_in(ids))
            .all(tx)
            .await
            .map_err(handle_dberr)?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn book_ids_for_user(&self, transaction: &dyn Transaction, user_id: UserId) -> Result<Vec<BookId>, Error> {
        let tx = TransactionImpl::get_db_transaction(transaction)?;

        let rows = prelude::UserBookMetadata::find()
            .filter(user_book_metadata::Column::UserId.eq(user_id as i64))
            .select_only()
            .column(user_book_metadata::Column::BookId)
            .into_tuple::<i64>()
            .all(tx)
            .await
            .map_err(handle_dberr)?;

        Ok(rows.into_iter().map(|id| id as BookId).collect())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bb_core::{
        book::{BookStatus, NewBook},
        reading::ReadStatus,
        repository::RepositoryService,
        types::Capabilities,
        user::NewUser,
    };
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

    fn base_metadata(user_id: u64, book_id: u64) -> bb_core::reading::UserBookMetadata {
        bb_core::reading::UserBookMetadata {
            user_id,
            book_id,
            read_status: ReadStatus::Unread,
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
        }
    }

    // ─── upsert ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_upsert_insert_new_row() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let book_id = new_book(&svc, "Dune").await;
        let tx = svc.repository().begin().await.unwrap();

        let meta = bb_core::reading::UserBookMetadata {
            read_status: ReadStatus::Reading,
            ..base_metadata(user_id, book_id)
        };

        let result = svc.user_book_metadata_repository().upsert(&*tx, meta).await;

        assert!(result.is_ok());
        let saved = result.unwrap();
        assert_eq!(saved.user_id, user_id);
        assert_eq!(saved.book_id, book_id);
        assert_eq!(saved.read_status, ReadStatus::Reading);
    }

    #[tokio::test]
    async fn test_upsert_updates_existing_row() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let book_id = new_book(&svc, "Dune").await;
        let tx = svc.repository().begin().await.unwrap();

        svc.user_book_metadata_repository().upsert(&*tx, base_metadata(user_id, book_id)).await.unwrap();

        let updated = bb_core::reading::UserBookMetadata {
            read_status: ReadStatus::Read,
            times_read: 1,
            personal_rating: Some(5),
            notes: Some("Excellent".to_owned()),
            ..base_metadata(user_id, book_id)
        };

        let result = svc.user_book_metadata_repository().upsert(&*tx, updated).await;

        assert!(result.is_ok());
        let saved = result.unwrap();
        assert_eq!(saved.read_status, ReadStatus::Read);
        assert_eq!(saved.times_read, 1);
        assert_eq!(saved.personal_rating, Some(5));
        assert_eq!(saved.notes.as_deref(), Some("Excellent"));
    }

    // ─── find_by_user_and_book ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_find_by_user_and_book_found() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let book_id = new_book(&svc, "Dune").await;
        let tx = svc.repository().begin().await.unwrap();

        svc.user_book_metadata_repository().upsert(&*tx, base_metadata(user_id, book_id)).await.unwrap();

        let found = svc.user_book_metadata_repository().find_by_user_and_book(&*tx, user_id, book_id).await;

        assert!(found.is_ok());
        assert!(found.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_find_by_user_and_book_not_found() {
        let svc = setup().await;
        let tx = svc.repository().begin().await.unwrap();

        let found = svc.user_book_metadata_repository().find_by_user_and_book(&*tx, 999, 999).await;

        assert!(found.is_ok());
        assert!(found.unwrap().is_none());
    }

    // ─── list_for_user ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_list_for_user_returns_only_own_rows() {
        let svc = setup().await;
        let alice = new_user(&svc, "alice").await;
        let bob = new_user(&svc, "bob").await;
        let book_id = new_book(&svc, "Dune").await;
        let tx = svc.repository().begin().await.unwrap();

        svc.user_book_metadata_repository().upsert(&*tx, base_metadata(alice, book_id)).await.unwrap();
        svc.user_book_metadata_repository().upsert(&*tx, base_metadata(bob, book_id)).await.unwrap();

        let alice_rows = svc.user_book_metadata_repository().list_for_user(&*tx, alice, None, None, None).await.unwrap();

        assert_eq!(alice_rows.len(), 1);
        assert_eq!(alice_rows[0].user_id, alice);
    }

    #[tokio::test]
    async fn test_list_for_user_filter_by_status() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let book1 = new_book(&svc, "Dune").await;
        let book2 = new_book(&svc, "Foundation").await;
        let tx = svc.repository().begin().await.unwrap();

        svc.user_book_metadata_repository()
            .upsert(
                &*tx,
                bb_core::reading::UserBookMetadata {
                    read_status: ReadStatus::Reading,
                    ..base_metadata(user_id, book1)
                },
            )
            .await
            .unwrap();
        svc.user_book_metadata_repository()
            .upsert(
                &*tx,
                bb_core::reading::UserBookMetadata {
                    read_status: ReadStatus::Read,
                    ..base_metadata(user_id, book2)
                },
            )
            .await
            .unwrap();

        let reading = svc
            .user_book_metadata_repository()
            .list_for_user(&*tx, user_id, Some(ReadStatus::Reading), None, None)
            .await
            .unwrap();

        assert_eq!(reading.len(), 1);
        assert_eq!(reading[0].read_status, ReadStatus::Reading);
    }

    #[tokio::test]
    async fn test_list_for_user_pagination() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let book1 = new_book(&svc, "A").await;
        let book2 = new_book(&svc, "B").await;
        let book3 = new_book(&svc, "C").await;
        let tx = svc.repository().begin().await.unwrap();

        for book_id in [book1, book2, book3] {
            svc.user_book_metadata_repository().upsert(&*tx, base_metadata(user_id, book_id)).await.unwrap();
        }

        let page1 = svc
            .user_book_metadata_repository()
            .list_for_user(&*tx, user_id, None, None, Some(2))
            .await
            .unwrap();
        assert_eq!(page1.len(), 2);

        let last_id = page1.last().unwrap().book_id;
        let page2 = svc
            .user_book_metadata_repository()
            .list_for_user(&*tx, user_id, None, Some(last_id + 1), Some(2))
            .await
            .unwrap();
        assert_eq!(page2.len(), 1);
    }

    #[tokio::test]
    async fn test_list_for_user_page_size_zero_returns_error() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let tx = svc.repository().begin().await.unwrap();

        assert!(matches!(
            svc.user_book_metadata_repository().list_for_user(&*tx, user_id, None, None, Some(0)).await,
            Err(bb_core::Error::InvalidPageSize(0))
        ));
    }
}
