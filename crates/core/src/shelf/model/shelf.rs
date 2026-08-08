use chrono::{DateTime, Utc};

use crate::{
    device::DeviceId,
    filter::BookFilter,
    library::LibraryId,
    token::{Token, TokenAlphabet, define_token_prefix},
    user::UserId,
};

define_token_prefix!(ShelfTokenPrefix, "SH_");
pub type ShelfId = u64;
pub type ShelfToken = Token<ShelfTokenPrefix, ShelfId, TokenAlphabet, { i64::MAX as u128 }>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShelfType {
    System,
    Manual,
    Smart,
}

#[derive(Debug, Clone)]
pub struct Shelf {
    pub id: ShelfId,
    pub version: u64,
    pub token: ShelfToken,
    pub owner_id: UserId,
    pub library_id: LibraryId,
    pub name: String,
    pub shelf_type: ShelfType,
    /// Device this shelf is synced to, if any.
    pub device_id: Option<DeviceId>,
    /// Filter criteria — only set for `ShelfType::Smart`.
    pub filter_criteria: Option<BookFilter>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewShelf {
    pub owner_id: UserId,
    pub library_id: LibraryId,
    pub name: String,
    pub shelf_type: ShelfType,
    pub device_id: Option<DeviceId>,
    pub filter_criteria: Option<BookFilter>,
}
