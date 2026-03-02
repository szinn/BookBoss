use chrono::{DateTime, Utc};

use crate::device::DeviceId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct NewDeviceSyncLog {
    pub device_id: DeviceId,
    pub status: SyncStatus,
    pub books_added: i32,
    pub books_removed: i32,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct DeviceSyncLog {
    pub id: i64,
    pub device_id: DeviceId,
    pub status: SyncStatus,
    pub books_added: i32,
    pub books_removed: i32,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
