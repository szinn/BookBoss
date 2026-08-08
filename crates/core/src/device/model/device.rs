use std::{fmt, str::FromStr};

use chrono::{DateTime, Utc};

use crate::{
    token::{Token, TokenAlphabet, define_token_prefix},
    user::UserId,
};

define_token_prefix!(DeviceTokenPrefix, "DV_");
pub type DeviceId = u64;
pub type DeviceToken = Token<DeviceTokenPrefix, DeviceId, TokenAlphabet, { i64::MAX as u128 }>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnRemovalAction {
    MarkRead,
    MarkDnf,
    Nothing,
}

impl OnRemovalAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MarkRead => "mark_read",
            Self::MarkDnf => "mark_dnf",
            Self::Nothing => "nothing",
        }
    }
}

impl fmt::Display for OnRemovalAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for OnRemovalAction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mark_read" => Ok(Self::MarkRead),
            "mark_dnf" => Ok(Self::MarkDnf),
            "nothing" => Ok(Self::Nothing),
            _ => Err(format!("unknown removal action: {s}")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Device {
    pub id: DeviceId,
    pub version: u64,
    pub token: DeviceToken,
    pub owner_id: UserId,
    pub name: String,
    pub device_type: String,
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
    pub on_removal_action: OnRemovalAction,
}
