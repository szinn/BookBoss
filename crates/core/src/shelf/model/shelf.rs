use bb_utils::{define_token_prefix, token::Token};
use chrono::{DateTime, Utc};

use crate::{device::DeviceId, shelf::ShelfFilter, user::UserId};

define_token_prefix!(ShelfTokenPrefix, "SH_");
pub type ShelfId = u64;
pub type ShelfToken = Token<ShelfTokenPrefix, ShelfId, { i64::MAX as u128 }>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShelfType {
    System,
    Manual,
    Smart,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShelfVisibility {
    Private,
    Public,
}

#[derive(Debug, Clone)]
pub struct Shelf {
    pub id: ShelfId,
    pub version: u64,
    pub token: ShelfToken,
    pub owner_id: UserId,
    pub name: String,
    pub shelf_type: ShelfType,
    pub visibility: ShelfVisibility,
    /// Device this shelf is synced to, if any.
    pub device_id: Option<DeviceId>,
    /// Filter criteria — only set for `ShelfType::Smart`.
    pub filter_criteria: Option<ShelfFilter>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewShelf {
    pub owner_id: UserId,
    pub name: String,
    pub shelf_type: ShelfType,
    pub visibility: ShelfVisibility,
    pub device_id: Option<DeviceId>,
    pub filter_criteria: Option<ShelfFilter>,
}
