use bb_core::{
    Error,
    book::BookId,
    device::{Device, DeviceBook, DeviceId, DeviceRepository, DeviceSyncLog, DeviceToken, NewDevice, NewDeviceSyncLog},
    repository::Transaction,
    user::UserId,
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, ModelTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};

use crate::{
    entities::{device_books, device_sync_log, devices, prelude},
    error::handle_dberr,
    transaction::TransactionImpl,
};

// ── From impls
// ────────────────────────────────────────────────────────────────

impl From<devices::Model> for Device {
    fn from(m: devices::Model) -> Self {
        let token = DeviceToken::new(m.id as u64);
        Self {
            id: m.id as u64,
            version: m.version as u64,
            token,
            owner_id: m.owner_id as u64,
            name: m.name,
            device_type: m.device_type,
            on_removal_action: m.on_removal_action.parse().expect("DB has unknown removal action"),
            last_synced_at: m.last_synced_at.map(|t| t.with_timezone(&Utc)),
            created_at: m.created_at.with_timezone(&Utc),
            updated_at: m.updated_at.with_timezone(&Utc),
        }
    }
}

impl From<device_books::Model> for DeviceBook {
    fn from(m: device_books::Model) -> Self {
        Self {
            device_id: m.device_id as u64,
            book_id: m.book_id as u64,
            format: m.format.parse().expect("DB has unknown file format"),
            file_role: m.file_role.parse().expect("DB has unknown file role"),
            synced_at: m.synced_at.with_timezone(&Utc),
        }
    }
}

impl From<device_sync_log::Model> for DeviceSyncLog {
    fn from(m: device_sync_log::Model) -> Self {
        Self {
            id: m.id,
            device_id: m.device_id as u64,
            status: m.status.parse().expect("DB has unknown sync status"),
            books_added: m.books_added,
            books_removed: m.books_removed,
            started_at: m.started_at.with_timezone(&Utc),
            completed_at: m.completed_at.map(|t| t.with_timezone(&Utc)),
        }
    }
}

// ── Adapter
// ───────────────────────────────────────────────────────────────────

pub(crate) struct DeviceRepositoryAdapter;

impl DeviceRepositoryAdapter {
    pub(crate) fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl DeviceRepository for DeviceRepositoryAdapter {
    async fn add_device(&self, transaction: &dyn Transaction, device: NewDevice) -> Result<Device, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let token = DeviceToken::generate();
        let now = Utc::now();

        let model = devices::ActiveModel {
            id: Set(token.id() as i64),
            token: Set(token.to_string()),
            owner_id: Set(device.owner_id as i64),
            name: Set(device.name),
            device_type: Set(device.device_type),
            on_removal_action: Set(device.on_removal_action.to_string()),
            last_synced_at: Set(None),
            version: Set(0),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(transaction)
        .await
        .map_err(handle_dberr)?;

        Ok(model.into())
    }

    async fn update_device(&self, transaction: &dyn Transaction, device: Device) -> Result<Device, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let existing = prelude::Devices::find_by_id(device.id as i64)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .ok_or(bb_core::RepositoryError::NotFound)?;

        let mut updater: devices::ActiveModel = existing.into();
        updater.name = Set(device.name);
        updater.device_type = Set(device.device_type);
        updater.on_removal_action = Set(device.on_removal_action.to_string());
        updater.last_synced_at = Set(device.last_synced_at.map(Into::into));

        let result = updater.update(transaction).await.map_err(handle_dberr)?;
        Ok(result.into())
    }

    async fn delete_device(&self, transaction: &dyn Transaction, device: Device) -> Result<(), Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        if let Some(existing) = prelude::Devices::find_by_id(device.id as i64).one(transaction).await.map_err(handle_dberr)? {
            existing.delete(transaction).await.map_err(handle_dberr)?;
        }

        Ok(())
    }

    async fn find_by_id(&self, transaction: &dyn Transaction, id: DeviceId) -> Result<Option<Device>, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::Devices::find_by_id(id as i64)
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .map(Into::into))
    }

    async fn find_by_token(&self, transaction: &dyn Transaction, token: DeviceToken) -> Result<Option<Device>, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::Devices::find()
            .filter(devices::Column::Token.eq(token.to_string()))
            .one(transaction)
            .await
            .map_err(handle_dberr)?
            .map(Into::into))
    }

    async fn list_for_user(&self, transaction: &dyn Transaction, owner_id: UserId) -> Result<Vec<Device>, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::Devices::find()
            .filter(devices::Column::OwnerId.eq(owner_id as i64))
            .order_by_asc(devices::Column::CreatedAt)
            .all(transaction)
            .await
            .map_err(handle_dberr)?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    async fn count_with_name_prefix(&self, transaction: &dyn Transaction, owner_id: UserId, prefix: &str) -> Result<u64, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::Devices::find()
            .filter(devices::Column::OwnerId.eq(owner_id as i64))
            .filter(devices::Column::Name.like(format!("{prefix}%")))
            .count(transaction)
            .await
            .map_err(handle_dberr)?)
    }

    async fn add_device_book(&self, transaction: &dyn Transaction, book: DeviceBook) -> Result<DeviceBook, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let now = Utc::now();
        let model = device_books::ActiveModel {
            device_id: Set(book.device_id as i64),
            book_id: Set(book.book_id as i64),
            format: Set(book.format.to_string()),
            file_role: Set(book.file_role.to_string()),
            synced_at: Set(now.into()),
        }
        .insert(transaction)
        .await
        .map_err(handle_dberr)?;

        Ok(model.into())
    }

    async fn update_device_book(&self, transaction: &dyn Transaction, book: DeviceBook) -> Result<DeviceBook, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let now = Utc::now();
        prelude::DeviceBooks::update_many()
            .set(device_books::ActiveModel {
                format: Set(book.format.to_string()),
                file_role: Set(book.file_role.to_string()),
                synced_at: Set(now.into()),
                ..Default::default()
            })
            .filter(device_books::Column::DeviceId.eq(book.device_id as i64))
            .filter(device_books::Column::BookId.eq(book.book_id as i64))
            .exec(transaction)
            .await
            .map_err(handle_dberr)?;

        Ok(DeviceBook { synced_at: now, ..book })
    }

    async fn remove_device_book(&self, transaction: &dyn Transaction, device_id: DeviceId, book_id: BookId) -> Result<(), Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        prelude::DeviceBooks::delete_many()
            .filter(device_books::Column::DeviceId.eq(device_id as i64))
            .filter(device_books::Column::BookId.eq(book_id as i64))
            .exec(transaction)
            .await
            .map_err(handle_dberr)?;

        Ok(())
    }

    async fn clear_device_books(&self, transaction: &dyn Transaction, device_id: DeviceId) -> Result<(), Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        prelude::DeviceBooks::delete_many()
            .filter(device_books::Column::DeviceId.eq(device_id as i64))
            .exec(transaction)
            .await
            .map_err(handle_dberr)?;

        Ok(())
    }

    async fn books_for_device(&self, transaction: &dyn Transaction, device_id: DeviceId) -> Result<Vec<DeviceBook>, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        Ok(prelude::DeviceBooks::find()
            .filter(device_books::Column::DeviceId.eq(device_id as i64))
            .all(transaction)
            .await
            .map_err(handle_dberr)?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    async fn add_sync_log(&self, transaction: &dyn Transaction, log: NewDeviceSyncLog) -> Result<DeviceSyncLog, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let model = device_sync_log::ActiveModel {
            device_id: Set(log.device_id as i64),
            status: Set(log.status.to_string()),
            books_added: Set(log.books_added),
            books_removed: Set(log.books_removed),
            started_at: Set(log.started_at.into()),
            completed_at: Set(log.completed_at.map(Into::into)),
            ..Default::default()
        }
        .insert(transaction)
        .await
        .map_err(handle_dberr)?;

        Ok(model.into())
    }

    async fn list_sync_logs_for_device(&self, transaction: &dyn Transaction, device_id: DeviceId, page_size: Option<u64>) -> Result<Vec<DeviceSyncLog>, Error> {
        let transaction = TransactionImpl::get_db_transaction(transaction)?;

        let mut query = prelude::DeviceSyncLogs::find()
            .filter(device_sync_log::Column::DeviceId.eq(device_id as i64))
            .order_by_desc(device_sync_log::Column::StartedAt);

        if let Some(limit) = page_size {
            query = query.limit(limit);
        }

        Ok(query.all(transaction).await.map_err(handle_dberr)?.into_iter().map(Into::into).collect())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bb_core::{
        book::{BookStatus, FileFormat, FileRole, NewBook},
        device::{DeviceBook, NewDevice, NewDeviceSyncLog, OnRemovalAction, SyncStatus},
        repository::RepositoryService,
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

    fn new_device(owner_id: u64, name: &str) -> NewDevice {
        NewDevice {
            owner_id,
            name: name.to_owned(),
            device_type: "kobo".to_owned(),
            on_removal_action: OnRemovalAction::Nothing,
        }
    }

    // ── add_device / find
    // ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn add_device_and_find_by_token() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let tx = svc.repository().begin().await.unwrap();

        let device = svc.device_repository().add_device(&*tx, new_device(user_id, "My Kobo")).await.unwrap();
        tx.commit().await.unwrap();

        let tx = svc.repository().begin_read_only().await.unwrap();
        let found = svc.device_repository().find_by_token(&*tx, device.token).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "My Kobo");
    }

    #[tokio::test]
    async fn add_device_and_find_by_id() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let tx = svc.repository().begin().await.unwrap();

        let device = svc.device_repository().add_device(&*tx, new_device(user_id, "Kindle")).await.unwrap();
        tx.commit().await.unwrap();

        let tx = svc.repository().begin_read_only().await.unwrap();
        let found = svc.device_repository().find_by_id(&*tx, device.id).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().device_type, "kobo");
    }

    #[tokio::test]
    async fn find_by_token_returns_none_for_unknown() {
        let svc = setup().await;
        let tx = svc.repository().begin_read_only().await.unwrap();
        let ghost = bb_core::device::DeviceToken::new(999_999);

        let found = svc.device_repository().find_by_token(&*tx, ghost).await.unwrap();

        assert!(found.is_none());
    }

    // ── list_for_user
    // ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn list_for_user_empty_initially() {
        let svc = setup().await;
        let user_id = new_user(&svc, "bob").await;
        let tx = svc.repository().begin_read_only().await.unwrap();

        let devices = svc.device_repository().list_for_user(&*tx, user_id).await.unwrap();

        assert!(devices.is_empty());
    }

    #[tokio::test]
    async fn list_for_user_returns_own_devices_only() {
        let svc = setup().await;
        let alice = new_user(&svc, "alice").await;
        let bob = new_user(&svc, "bob").await;
        let tx = svc.repository().begin().await.unwrap();
        svc.device_repository().add_device(&*tx, new_device(alice, "Alice's Kobo")).await.unwrap();
        svc.device_repository().add_device(&*tx, new_device(bob, "Bob's Kindle")).await.unwrap();
        tx.commit().await.unwrap();

        let tx = svc.repository().begin_read_only().await.unwrap();
        let alice_devices = svc.device_repository().list_for_user(&*tx, alice).await.unwrap();

        assert_eq!(alice_devices.len(), 1);
        assert_eq!(alice_devices[0].name, "Alice's Kobo");
    }

    // ── update_device
    // ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn update_device_changes_name_and_removal_action() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let tx = svc.repository().begin().await.unwrap();
        let mut device = svc.device_repository().add_device(&*tx, new_device(user_id, "Old Name")).await.unwrap();
        tx.commit().await.unwrap();

        device.name = "New Name".to_string();
        device.on_removal_action = OnRemovalAction::MarkRead;
        let tx = svc.repository().begin().await.unwrap();
        let updated = svc.device_repository().update_device(&*tx, device).await.unwrap();
        tx.commit().await.unwrap();

        assert_eq!(updated.name, "New Name");
        assert_eq!(updated.on_removal_action, OnRemovalAction::MarkRead);
    }

    // ── delete_device
    // ─────────────────────────────────────────────────────────

    #[tokio::test]
    async fn delete_device_removes_record() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let tx = svc.repository().begin().await.unwrap();
        let device = svc.device_repository().add_device(&*tx, new_device(user_id, "To Delete")).await.unwrap();
        tx.commit().await.unwrap();

        let tx = svc.repository().begin().await.unwrap();
        svc.device_repository().delete_device(&*tx, device.clone()).await.unwrap();
        tx.commit().await.unwrap();

        let tx = svc.repository().begin_read_only().await.unwrap();
        let found = svc.device_repository().find_by_id(&*tx, device.id).await.unwrap();
        assert!(found.is_none());
    }

    // ── count_with_name_prefix
    // ────────────────────────────────────────────────

    #[tokio::test]
    async fn count_with_name_prefix_matches_prefix() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let tx = svc.repository().begin().await.unwrap();
        svc.device_repository().add_device(&*tx, new_device(user_id, "Alice's Device")).await.unwrap();
        svc.device_repository()
            .add_device(&*tx, new_device(user_id, "Alice's Device (2)"))
            .await
            .unwrap();
        svc.device_repository().add_device(&*tx, new_device(user_id, "Other")).await.unwrap();
        tx.commit().await.unwrap();

        let tx = svc.repository().begin_read_only().await.unwrap();
        let count = svc.device_repository().count_with_name_prefix(&*tx, user_id, "Alice's Device").await.unwrap();

        assert_eq!(count, 2);
    }

    // ── add_device_book / books_for_device / remove_device_book ──────────────

    #[tokio::test]
    async fn add_device_book_and_retrieve() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let book_id = new_book(&svc, "Dune").await;
        let tx = svc.repository().begin().await.unwrap();
        let device = svc.device_repository().add_device(&*tx, new_device(user_id, "Kobo")).await.unwrap();
        tx.commit().await.unwrap();

        let tx = svc.repository().begin().await.unwrap();
        svc.device_repository()
            .add_device_book(
                &*tx,
                DeviceBook {
                    device_id: device.id,
                    book_id,
                    format: FileFormat::Epub,
                    file_role: FileRole::Original,
                    synced_at: Utc::now(),
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let tx = svc.repository().begin_read_only().await.unwrap();
        let books = svc.device_repository().books_for_device(&*tx, device.id).await.unwrap();
        assert_eq!(books.len(), 1);
        assert_eq!(books[0].book_id, book_id);
    }

    #[tokio::test]
    async fn remove_device_book_removes_entry() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let book_id = new_book(&svc, "Foundation").await;
        let tx = svc.repository().begin().await.unwrap();
        let device = svc.device_repository().add_device(&*tx, new_device(user_id, "Kobo")).await.unwrap();
        svc.device_repository()
            .add_device_book(
                &*tx,
                DeviceBook {
                    device_id: device.id,
                    book_id,
                    format: FileFormat::Epub,
                    file_role: FileRole::Original,
                    synced_at: Utc::now(),
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let tx = svc.repository().begin().await.unwrap();
        svc.device_repository().remove_device_book(&*tx, device.id, book_id).await.unwrap();
        tx.commit().await.unwrap();

        let tx = svc.repository().begin_read_only().await.unwrap();
        let books = svc.device_repository().books_for_device(&*tx, device.id).await.unwrap();
        assert!(books.is_empty());
    }

    // ── sync log
    // ──────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn add_sync_log_and_list() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let tx = svc.repository().begin().await.unwrap();
        let device = svc.device_repository().add_device(&*tx, new_device(user_id, "Kobo")).await.unwrap();
        tx.commit().await.unwrap();

        let tx = svc.repository().begin().await.unwrap();
        svc.device_repository()
            .add_sync_log(
                &*tx,
                NewDeviceSyncLog {
                    device_id: device.id,
                    status: SyncStatus::Completed,
                    books_added: 3,
                    books_removed: 1,
                    started_at: Utc::now(),
                    completed_at: Some(Utc::now()),
                },
            )
            .await
            .unwrap();
        tx.commit().await.unwrap();

        let tx = svc.repository().begin_read_only().await.unwrap();
        let logs = svc.device_repository().list_sync_logs_for_device(&*tx, device.id, None).await.unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].status, SyncStatus::Completed);
        assert_eq!(logs[0].books_added, 3);
        assert_eq!(logs[0].books_removed, 1);
    }

    #[tokio::test]
    async fn list_sync_logs_respects_page_size() {
        let svc = setup().await;
        let user_id = new_user(&svc, "alice").await;
        let tx = svc.repository().begin().await.unwrap();
        let device = svc.device_repository().add_device(&*tx, new_device(user_id, "Kobo")).await.unwrap();
        for _ in 0..5 {
            svc.device_repository()
                .add_sync_log(
                    &*tx,
                    NewDeviceSyncLog {
                        device_id: device.id,
                        status: SyncStatus::Completed,
                        books_added: 0,
                        books_removed: 0,
                        started_at: Utc::now(),
                        completed_at: None,
                    },
                )
                .await
                .unwrap();
        }
        tx.commit().await.unwrap();

        let tx = svc.repository().begin_read_only().await.unwrap();
        let limited = svc.device_repository().list_sync_logs_for_device(&*tx, device.id, Some(3)).await.unwrap();
        assert_eq!(limited.len(), 3);
    }
}
