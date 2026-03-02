use bb_utils::{define_token_prefix, token::Token};

define_token_prefix!(ShelfTokenPrefix, "SH_");
pub type ShelfId = u64;
pub type ShelfToken = Token<ShelfTokenPrefix, ShelfId, { i64::MAX as u128 }>;
