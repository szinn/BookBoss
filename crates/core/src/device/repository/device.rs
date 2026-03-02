use crate::{
    Error,
    book::BookId,
    device::{Device, DeviceBook, DeviceId, DeviceSyncLog, DeviceToken, NewDevice, NewDeviceSyncLog},
    repository::Transaction,
    user::UserId,
};

#[async_trait::async_trait]
pub trait DeviceRepository: Send + Sync {
    // Device CRUD
    async fn add_device(&self, transaction: &dyn Transaction, device: NewDevice) -> Result<Device, Error>;
    async fn update_device(&self, transaction: &dyn Transaction, device: Device) -> Result<Device, Error>;
    async fn find_by_id(&self, transaction: &dyn Transaction, id: DeviceId) -> Result<Option<Device>, Error>;
    async fn find_by_token(&self, transaction: &dyn Transaction, token: &DeviceToken) -> Result<Option<Device>, Error>;
    async fn list_for_user(&self, transaction: &dyn Transaction, owner_id: UserId) -> Result<Vec<Device>, Error>;

    // Device books
    async fn add_device_book(&self, transaction: &dyn Transaction, book: DeviceBook) -> Result<DeviceBook, Error>;
    async fn remove_device_book(&self, transaction: &dyn Transaction, device_id: DeviceId, book_id: BookId) -> Result<(), Error>;
    async fn books_for_device(&self, transaction: &dyn Transaction, device_id: DeviceId) -> Result<Vec<DeviceBook>, Error>;

    // Sync log
    async fn add_sync_log(&self, transaction: &dyn Transaction, log: NewDeviceSyncLog) -> Result<DeviceSyncLog, Error>;
    async fn list_sync_logs_for_device(&self, transaction: &dyn Transaction, device_id: DeviceId, page_size: Option<u64>) -> Result<Vec<DeviceSyncLog>, Error>;
}
