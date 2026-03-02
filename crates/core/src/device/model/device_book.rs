use chrono::{DateTime, Utc};

use crate::{
    book::{BookId, FileFormat},
    device::DeviceId,
};

#[derive(Debug, Clone)]
pub struct DeviceBook {
    pub device_id: DeviceId,
    pub book_id: BookId,
    pub format: FileFormat,
    pub synced_at: DateTime<Utc>,
    pub removed_at: Option<DateTime<Utc>>,
}
