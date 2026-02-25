use std::collections::HashSet;

use bb_utils::{define_token_prefix, token::Token};
use chrono::{DateTime, Utc};
use derive_builder::Builder;

use crate::{
    Error,
    types::{Capabilities, EmailAddress},
};

define_token_prefix!(UserTokenPrefix, "U_");
pub type UserId = u64;
pub type UserToken = Token<UserTokenPrefix, UserId, { i64::MAX as u128 }>;

#[derive(Debug, Clone, Builder)]
pub struct User {
    pub id: UserId,
    pub version: u64,
    pub token: UserToken,
    pub username: String,
    pub password_hash: String,
    pub email_address: EmailAddress,
    pub capabilities: Capabilities,
    #[builder(default = "Utc::now()")]
    pub created_at: DateTime<Utc>,
    #[builder(default = "Utc::now()")]
    pub updated_at: DateTime<Utc>,
}

impl Default for User {
    fn default() -> Self {
        let token = UserToken::generate();

        Self {
            id: token.id(),
            version: 0,
            token,
            username: String::new(),
            password_hash: String::new(),
            email_address: EmailAddress::new("default@example.com").expect("default email is valid"),
            capabilities: HashSet::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

impl User {
    /// Creates a fake user with default timestamps and a generated token.
    /// Only available in test builds.
    #[cfg(any(test, feature = "test-support"))]
    pub fn fake(id: UserId, name: impl Into<String>, password_hash: impl Into<String>, email_address: impl Into<String>, capabilities: Capabilities) -> Self {
        UserBuilder::default()
            .id(id)
            .version(0)
            .token(UserToken::new(id))
            .username(name.into())
            .password_hash(password_hash.into())
            .email_address(EmailAddress::new(email_address).expect("test email should be valid"))
            .capabilities(capabilities)
            .build()
            .expect("test user should build successfully")
    }
}

#[derive(Debug, Clone)]
pub struct NewUser {
    pub username: String,
    pub password_hash: String,
    pub email_address: EmailAddress,
    pub capabilities: Capabilities,
}

impl NewUser {
    /// Creates a new user with password hash, validated email and capabilities.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if email is invalid.
    pub fn new(
        username: impl Into<String>,
        password_hash: impl Into<String>,
        email_address: impl Into<String>,
        capabilities: Capabilities,
    ) -> Result<Self, Error> {
        Ok(Self {
            username: username.into(),
            password_hash: password_hash.into(),
            email_address: EmailAddress::new(email_address)?,
            capabilities,
        })
    }
}

impl Default for NewUser {
    fn default() -> Self {
        Self {
            username: String::new(),
            password_hash: String::new(),
            email_address: EmailAddress::new("default@example.com").expect("default email is valid"),
            capabilities: HashSet::new(),
        }
    }
}

/// Represents a partial update to a User.
///
/// Used to consolidate update logic between HTTP and gRPC handlers.
/// All fields are optional - only provided fields will be updated.
#[derive(Debug, Default, Clone)]
pub struct PartialUserUpdate {
    pub password_hash: Option<String>,
    pub email_address: Option<EmailAddress>,
    pub capabilities: Option<Capabilities>,
}

impl PartialUserUpdate {
    /// Creates a new partial update with validated email if provided.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if email or age is invalid.
    pub fn new(password_hash: Option<impl Into<String>>, email_address: Option<impl Into<String>>, capabilities: Option<Capabilities>) -> Result<Self, Error> {
        Ok(Self {
            password_hash: password_hash.map(Into::into),
            email_address: email_address.map(EmailAddress::new).transpose()?,
            capabilities,
        })
    }

    /// Apply this partial update to an existing user, consuming self.
    ///
    /// Only modifies fields that have `Some` values.
    pub fn apply_to(self, user: &mut User) {
        if let Some(password_hash) = self.password_hash {
            user.password_hash = password_hash;
        }
        if let Some(email_address) = self.email_address {
            user.email_address = email_address;
        }
        if let Some(capabilities) = self.capabilities {
            user.capabilities = capabilities;
        }
    }

    /// Returns true if all fields are None.
    pub fn is_empty(&self) -> bool {
        self.password_hash.is_none() && self.email_address.is_none() && self.capabilities.is_none()
    }
}
