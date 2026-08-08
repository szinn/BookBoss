use std::{fmt, str::FromStr};

use chrono::{DateTime, Utc};

use crate::{
    book::BookId,
    token::{Token, TokenAlphabet, define_token_prefix},
};

define_token_prefix!(AuthorTokenPrefix, "A_");
pub type AuthorId = u64;
pub type AuthorToken = Token<AuthorTokenPrefix, AuthorId, TokenAlphabet, { i64::MAX as u128 }>;

#[derive(Debug, Clone)]
pub struct Author {
    pub id: AuthorId,
    pub version: u64,
    pub token: AuthorToken,
    pub name: String,
    pub bio: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Author {
    #[cfg(any(test, feature = "test-support"))]
    pub fn fake(id: AuthorId, name: impl Into<String>) -> Self {
        Self {
            id,
            version: 1,
            token: AuthorToken::new(id),
            name: name.into(),
            bio: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

impl BookAuthor {
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub fn fake(book_id: BookId, author_id: AuthorId, role: &str, sort_order: i32) -> Self {
        let role = role.parse::<AuthorRole>().unwrap_or(AuthorRole::Author);
        Self {
            book_id,
            author_id,
            role,
            sort_order,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NewAuthor {
    pub name: String,
    pub bio: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AuthorRole {
    Author,
    Editor,
    Translator,
    Illustrator,
}

impl AuthorRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Author => "author",
            Self::Editor => "editor",
            Self::Translator => "translator",
            Self::Illustrator => "illustrator",
        }
    }

    /// Capitalised label for display in the UI.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Author => "Author",
            Self::Editor => "Editor",
            Self::Translator => "Translator",
            Self::Illustrator => "Illustrator",
        }
    }

    /// MARC relator code for use in OPF metadata.
    pub fn marc_relator(&self) -> &'static str {
        match self {
            Self::Author => "aut",
            Self::Editor => "edt",
            Self::Translator => "trl",
            Self::Illustrator => "ill",
        }
    }
}

impl fmt::Display for AuthorRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for AuthorRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "author" => Ok(Self::Author),
            "editor" => Ok(Self::Editor),
            "translator" => Ok(Self::Translator),
            "illustrator" => Ok(Self::Illustrator),
            _ => Err(format!("unknown author role: {s}")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BookAuthor {
    pub book_id: BookId,
    pub author_id: AuthorId,
    pub role: AuthorRole,
    pub sort_order: i32,
}
