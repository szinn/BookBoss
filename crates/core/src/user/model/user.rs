use std::collections::HashSet;

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use bb_utils::{define_token_prefix, token::Token};
use chrono::{DateTime, Utc};
use derive_builder::Builder;

use crate::{
    Error,
    types::{Capabilities, Capability, EmailAddress},
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

    /// Hashes a plaintext password using Argon2, returning a PHC-format string.
    ///
    /// # Errors
    ///
    /// Returns `Error::CryptoError` if hashing fails.
    pub fn encrypt_password(password: impl Into<String>) -> Result<String, Error> {
        let password = password.into();
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| Error::CryptoError(e.to_string()))?;
        Ok(hash.to_string())
    }

    /// Verifies a plaintext password against this user's stored password hash.
    pub fn check_password(&self, password: impl Into<String>) -> bool {
        let password = password.into();
        let Ok(parsed_hash) = PasswordHash::new(&self.password_hash) else {
            return false;
        };
        Argon2::default().verify_password(password.as_bytes(), &parsed_hash).is_ok()
    }

    pub fn has_capability(&self, capability: Capability) -> bool {
        self.capabilities.contains(&capability)
            || self.capabilities.contains(&Capability::SuperAdmin)
            || (capability != Capability::SuperAdmin && self.capabilities.contains(&Capability::Admin))
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
    pub fn new(username: impl Into<String>, password: impl Into<String>, email_address: impl Into<String>, capabilities: Capabilities) -> Result<Self, Error> {
        let password_hash = User::encrypt_password(password)?;

        Ok(Self {
            username: username.into(),
            password_hash,
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::types::Capability;

    // ─── User::has_capability ────────────────────────────────────────────────

    #[test]
    fn test_has_capability_present() {
        let user = User::fake(1, "alice", "hash", "alice@example.com", HashSet::from([Capability::Admin]));
        assert!(user.has_capability(Capability::Admin));
    }

    #[test]
    fn test_has_capability_absent() {
        let user = User::fake(1, "alice", "hash", "alice@example.com", HashSet::new());
        assert!(!user.has_capability(Capability::Admin));
    }

    #[test]
    fn test_has_capability_other_not_matched() {
        let user = User::fake(1, "alice", "hash", "alice@example.com", HashSet::from([Capability::EditBook]));
        assert!(!user.has_capability(Capability::Admin));
        assert!(user.has_capability(Capability::EditBook));
    }

    // ─── User::encrypt_password + check_password ─────────────────────────────

    #[test]
    fn test_encrypt_and_check_password_round_trip() {
        let hash = User::encrypt_password("correct-horse-battery-staple").unwrap();
        let user = User::fake(1, "alice", hash, "alice@example.com", HashSet::new());
        assert!(user.check_password("correct-horse-battery-staple"));
    }

    #[test]
    fn test_check_password_wrong_password_returns_false() {
        let hash = User::encrypt_password("correct").unwrap();
        let user = User::fake(1, "alice", hash, "alice@example.com", HashSet::new());
        assert!(!user.check_password("wrong"));
    }

    #[test]
    fn test_check_password_invalid_hash_returns_false() {
        let user = User::fake(1, "alice", "not-a-valid-hash", "alice@example.com", HashSet::new());
        assert!(!user.check_password("anything"));
    }

    #[test]
    fn test_check_password_empty_hash_returns_false() {
        let user = User::fake(1, "alice", "", "alice@example.com", HashSet::new());
        assert!(!user.check_password("password"));
    }

    // ─── NewUser::new ────────────────────────────────────────────────────────

    #[test]
    fn test_new_user_invalid_email_returns_error() {
        let result = NewUser::new("alice", "password", "not-an-email", HashSet::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_new_user_valid_fields_succeed() {
        let result = NewUser::new("alice", "password", "alice@example.com", HashSet::new());
        assert!(result.is_ok());
        let user = result.unwrap();
        assert_eq!(user.username, "alice");
        assert_eq!(user.email_address.as_str(), "alice@example.com");
    }

    #[test]
    fn test_new_user_password_is_hashed() {
        let result = NewUser::new("alice", "plaintext", "alice@example.com", HashSet::new());
        assert!(result.is_ok());
        assert_ne!(result.unwrap().password_hash, "plaintext");
    }

    // ─── PartialUserUpdate::is_empty ─────────────────────────────────────────

    #[test]
    fn test_partial_update_is_empty_all_none() {
        let update = PartialUserUpdate::default();
        assert!(update.is_empty());
    }

    #[test]
    fn test_partial_update_is_empty_password_some() {
        let update = PartialUserUpdate {
            password_hash: Some("hash".into()),
            ..Default::default()
        };
        assert!(!update.is_empty());
    }

    #[test]
    fn test_partial_update_is_empty_email_some() {
        let update = PartialUserUpdate {
            email_address: Some(EmailAddress::new("a@b.com").unwrap()),
            ..Default::default()
        };
        assert!(!update.is_empty());
    }

    #[test]
    fn test_partial_update_is_empty_capabilities_some() {
        let update = PartialUserUpdate {
            capabilities: Some(HashSet::new()),
            ..Default::default()
        };
        assert!(!update.is_empty());
    }

    // ─── PartialUserUpdate::new ──────────────────────────────────────────────

    #[test]
    fn test_partial_update_new_invalid_email_returns_error() {
        let result = PartialUserUpdate::new(None::<String>, Some("bad-email"), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_partial_update_new_all_none_succeeds() {
        let result = PartialUserUpdate::new(None::<String>, None::<String>, None);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_partial_update_new_valid_email_succeeds() {
        let result = PartialUserUpdate::new(None::<String>, Some("new@example.com"), None);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().email_address.unwrap().as_str(), "new@example.com");
    }

    // ─── PartialUserUpdate::apply_to ─────────────────────────────────────────

    #[test]
    fn test_apply_to_empty_is_noop() {
        let mut user = User::fake(1, "alice", "oldhash", "old@example.com", HashSet::from([Capability::Admin]));
        let update = PartialUserUpdate::default();
        update.apply_to(&mut user);
        assert_eq!(user.username, "alice");
        assert_eq!(user.password_hash, "oldhash");
        assert_eq!(user.email_address.as_str(), "old@example.com");
        assert!(user.has_capability(Capability::Admin));
    }

    #[test]
    fn test_apply_to_updates_password() {
        let mut user = User::fake(1, "alice", "oldhash", "old@example.com", HashSet::new());
        let update = PartialUserUpdate {
            password_hash: Some("newhash".into()),
            ..Default::default()
        };
        update.apply_to(&mut user);
        assert_eq!(user.password_hash, "newhash");
        assert_eq!(user.email_address.as_str(), "old@example.com");
    }

    #[test]
    fn test_apply_to_updates_email() {
        let mut user = User::fake(1, "alice", "hash", "old@example.com", HashSet::new());
        let update = PartialUserUpdate {
            email_address: Some(EmailAddress::new("new@example.com").unwrap()),
            ..Default::default()
        };
        update.apply_to(&mut user);
        assert_eq!(user.email_address.as_str(), "new@example.com");
        assert_eq!(user.password_hash, "hash");
    }

    #[test]
    fn test_apply_to_updates_capabilities() {
        let mut user = User::fake(1, "alice", "hash", "alice@example.com", HashSet::new());
        let new_caps = HashSet::from([Capability::Admin, Capability::EditBook]);
        let update = PartialUserUpdate {
            capabilities: Some(new_caps.clone()),
            ..Default::default()
        };
        update.apply_to(&mut user);
        assert_eq!(user.capabilities, new_caps);
    }

    #[test]
    fn test_apply_to_updates_all_fields() {
        let mut user = User::fake(1, "alice", "oldhash", "old@example.com", HashSet::new());
        let new_caps = HashSet::from([Capability::Admin]);
        let update = PartialUserUpdate {
            password_hash: Some("newhash".into()),
            email_address: Some(EmailAddress::new("new@example.com").unwrap()),
            capabilities: Some(new_caps.clone()),
        };
        update.apply_to(&mut user);
        assert_eq!(user.password_hash, "newhash");
        assert_eq!(user.email_address.as_str(), "new@example.com");
        assert_eq!(user.capabilities, new_caps);
    }
}
