use bb_utils::{define_token_prefix, token::Token};

define_token_prefix!(DeviceTokenPrefix, "DV_");
pub type DeviceId = u64;
pub type DeviceToken = Token<DeviceTokenPrefix, DeviceId, { i64::MAX as u128 }>;
