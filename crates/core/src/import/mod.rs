use bb_utils::{define_token_prefix, token::Token};

define_token_prefix!(ImportJobTokenPrefix, "IJ_");
pub type ImportJobId = u64;
pub type ImportJobToken = Token<ImportJobTokenPrefix, ImportJobId, { i64::MAX as u128 }>;
