use chrono::{DateTime, Utc};

use crate::token::{Token, TokenAlphabet, define_token_prefix};

define_token_prefix!(LibraryTokenPrefix, "LB_");
pub type LibraryId = u64;
pub type LibraryToken = Token<LibraryTokenPrefix, LibraryId, TokenAlphabet, { i64::MAX as u128 }>;

/// The hard-coded ID of the "All Books" system library.
/// Must match the value seeded in migration 33.
pub const ALL_BOOKS_LIBRARY_ID: LibraryId = 1;

/// The stable token for "All Books" — `LibraryToken::new(1_u64).to_string()`.
pub const ALL_BOOKS_LIBRARY_TOKEN: &str = "LB_YYYYYYYYYYYY4";

#[derive(Debug, Clone)]
pub struct Library {
    pub id: LibraryId,
    pub version: u64,
    pub token: LibraryToken,
    pub name: String,
    pub is_system: bool,
    /// The user who owns this library, if it is a personal library.
    /// `None` for system libraries and admin-created shared libraries.
    pub owner_id: Option<crate::user::UserId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewLibrary {
    pub name: String,
    /// Set to `Some(user_id)` for personal libraries, `None` for shared ones.
    pub owner_id: Option<crate::user::UserId>,
}

/// Returns the token for the "All Books" system library (id=1).
pub fn all_books_library_token() -> LibraryToken {
    LibraryToken::new(ALL_BOOKS_LIBRARY_ID)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_books_token_matches_token_encoding() {
        assert_eq!(all_books_library_token().to_string(), ALL_BOOKS_LIBRARY_TOKEN);
    }
}
