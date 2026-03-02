use bb_utils::{define_token_prefix, token::Token};
use chrono::{DateTime, Utc};

use crate::{book::FileFormat, user::UserId};

define_token_prefix!(DeviceTokenPrefix, "DV_");
pub type DeviceId = u64;
pub type DeviceToken = Token<DeviceTokenPrefix, DeviceId, { i64::MAX as u128 }>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnRemovalAction {
    MarkRead,
    MarkDnf,
    Nothing,
}

#[derive(Debug, Clone)]
pub struct Device {
    pub id: DeviceId,
    pub version: u64,
    pub token: DeviceToken,
    pub owner_id: UserId,
    pub name: String,
    pub device_type: String,
    pub preferred_format: Option<FileFormat>,
    pub on_removal_action: OnRemovalAction,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewDevice {
    pub owner_id: UserId,
    pub name: String,
    pub device_type: String,
    pub preferred_format: Option<FileFormat>,
    pub on_removal_action: OnRemovalAction,
}
