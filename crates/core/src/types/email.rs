use std::fmt;

use serde::{Deserialize, Serialize, de};

use crate::Error;

/// A validated email address that must contain '@'.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EmailAddress(String);

impl EmailAddress {
    /// Creates a new Email if the value contains '@'.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the email doesn't contain '@'.
    pub fn new(email: impl Into<String>) -> Result<Self, Error> {
        let email = email.into();
        if !email.contains('@') {
            return Err(Error::Validation(format!("Invalid email format: {}", email)));
        }
        Ok(Self(email))
    }

    /// Returns the email as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes self and returns the inner String.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for EmailAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for EmailAddress {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Serialize for EmailAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for EmailAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        EmailAddress::new(s).map_err(|e| de::Error::custom(e.to_string()))
    }
}
