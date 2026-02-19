use std::{fmt, hash::Hash, marker::PhantomData, str::FromStr};

use rand::RngExt as _;
use serde::{Deserialize, Serialize, de};
use thiserror::Error;

/// Base-32 alphabet excluding visually ambiguous characters (I, L, O, Q).
const ALPHABET: &[u8; 32] = b"ABCDEFGHJKMNPRSTUVWXYZ0123456789";

/// Reverse lookup table: ASCII byte → alphabet index (or `0xFF` for invalid).
/// Covers the full range `0..=b'Z'` (91 entries).
const DECODE_TABLE: [u8; 91] = {
    let mut table = [0xFF_u8; 91];
    let mut i = 0;
    while i < ALPHABET.len() {
        table[ALPHABET[i] as usize] = i as u8;
        i += 1;
    }
    table
};

/// Trait that defines the prefix for a token kind.
pub trait TokenPrefix: fmt::Debug + Clone + PartialEq + Eq {
    const PREFIX: &'static str;
}

/// Trait that abstracts encode/decode over the numeric backing type.
pub trait TokenId: Copy + PartialEq + Eq + Hash + fmt::Debug {
    /// The zero value for this type.
    const ZERO: Self;

    /// Length of the encoded identifier portion (excluding the prefix).
    const ENCODED_LEN: usize;

    /// Encode this value into a base-32 string of [`Self::ENCODED_LEN`]
    /// characters.
    fn encode(self) -> String;

    /// Encode this value directly into a byte buffer. The buffer must be
    /// exactly [`Self::ENCODED_LEN`] bytes. All bytes will be valid ASCII.
    fn encode_to_buf(self, buf: &mut [u8]);

    /// Decode a base-32 string back into this numeric type.
    fn decode(s: &str) -> Result<Self, TokenError>;

    /// Generate a random value in `1..=max` where `max` is provided as a
    /// `u128` (from the const generic on [`Token`]).
    fn random_in_range(max: u128) -> Self;
}

impl TokenId for u64 {
    const ZERO: Self = 0;
    const ENCODED_LEN: usize = 13; // 32^13 > u64::MAX

    fn random_in_range(max: u128) -> Self {
        rand::rng().random_range(1..=max as u64)
    }

    fn encode(self) -> String {
        let mut buf = [0u8; Self::ENCODED_LEN];
        self.encode_to_buf(&mut buf);
        // SAFETY: all bytes come from ALPHABET which is ASCII
        String::from_utf8(buf.to_vec()).expect("alphabet is ASCII")
    }

    fn encode_to_buf(self, buf: &mut [u8]) {
        let mut remaining = self;
        for i in (0..Self::ENCODED_LEN).rev() {
            buf[i] = ALPHABET[(remaining & 0x1F) as usize];
            remaining >>= 5;
        }
    }

    fn decode(s: &str) -> Result<Self, TokenError> {
        let mut value: u64 = 0;
        for ch in s.chars() {
            let byte = ch as usize;
            let idx = if byte < DECODE_TABLE.len() { DECODE_TABLE[byte] } else { 0xFF };
            if idx == 0xFF {
                return Err(TokenError::InvalidCharacter(ch));
            }
            value = value.checked_shl(5).and_then(|v| v.checked_add(idx as u64)).ok_or(TokenError::Overflow)?;
        }
        Ok(value)
    }
}

impl TokenId for u128 {
    const ZERO: Self = 0;
    const ENCODED_LEN: usize = 26; // 32^26 > u128::MAX

    fn random_in_range(max: u128) -> Self {
        rand::rng().random_range(1..=max)
    }

    fn encode(self) -> String {
        let mut buf = [0u8; Self::ENCODED_LEN];
        self.encode_to_buf(&mut buf);
        // SAFETY: all bytes come from ALPHABET which is ASCII
        String::from_utf8(buf.to_vec()).expect("alphabet is ASCII")
    }

    fn encode_to_buf(self, buf: &mut [u8]) {
        let mut remaining = self;
        for i in (0..Self::ENCODED_LEN).rev() {
            buf[i] = ALPHABET[(remaining & 0x1F) as usize];
            remaining >>= 5;
        }
    }

    fn decode(s: &str) -> Result<Self, TokenError> {
        let mut value: u128 = 0;
        for ch in s.chars() {
            let byte = ch as usize;
            let idx = if byte < DECODE_TABLE.len() { DECODE_TABLE[byte] } else { 0xFF };
            if idx == 0xFF {
                return Err(TokenError::InvalidCharacter(ch));
            }
            value = value.checked_shl(5).and_then(|v| v.checked_add(idx as u128)).ok_or(TokenError::Overflow)?;
        }
        Ok(value)
    }
}

/// A typed, prefixed identifier for domain entities.
///
/// Stores only the numeric ID internally. The string representation (e.g.
/// `U_ABCD1234NRST0`) is computed on demand via [`fmt::Display`].
///
/// The `MAX` const generic controls the upper bound for random ID generation
/// via [`Token::generate`]. This allows token types to cap their range (e.g.
/// to `i64::MAX` for database-safe storage) without changing the backing type.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Token<P: TokenPrefix, I: TokenId = u64, const MAX: u128 = { u64::MAX as u128 }> {
    id: I,
    _marker: PhantomData<P>,
}

impl<P: TokenPrefix, I: TokenId, const MAX: u128> fmt::Debug for Token<P, I, MAX> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Token({self})")
    }
}

/// Errors that can occur when parsing a token from a string.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TokenError {
    #[error("invalid prefix: expected \"{expected}\", found \"{found}\"")]
    InvalidPrefix { expected: &'static str, found: String },

    #[error("invalid length: expected {expected}, found {found}")]
    InvalidLength { expected: usize, found: usize },

    #[error("invalid character: '{0}'")]
    InvalidCharacter(char),

    #[error("encoded value overflow")]
    Overflow,
}

impl<P: TokenPrefix, I: TokenId, const MAX: u128> Token<P, I, MAX> {
    /// Create a token from a numeric ID.
    pub fn new(id: I) -> Self {
        Self { id, _marker: PhantomData }
    }

    /// Generate a new token with a random ID in `1..=MAX`.
    pub fn generate() -> Self {
        Self::new(I::random_in_range(MAX))
    }

    /// Parse a token from its string representation (e.g. `"U_ABCD1234NRST0"`).
    pub fn parse(s: &str) -> Result<Self, TokenError> {
        let prefix = P::PREFIX;
        if !s.starts_with(prefix) {
            let found_len = s.len().min(prefix.len());
            return Err(TokenError::InvalidPrefix {
                expected: prefix,
                found: s[..found_len].to_string(),
            });
        }

        let encoded = &s[prefix.len()..];
        if encoded.len() != I::ENCODED_LEN {
            return Err(TokenError::InvalidLength {
                expected: prefix.len() + I::ENCODED_LEN,
                found: s.len(),
            });
        }

        let id = I::decode(encoded)?;
        Ok(Self::new(id))
    }

    /// Get the underlying numeric ID.
    pub fn id(&self) -> I {
        self.id
    }

    /// Check if a string is a well-formed token of this type.
    pub fn is_valid(s: &str) -> bool {
        Self::parse(s).is_ok()
    }
}

impl<P: TokenPrefix, I: TokenId, const MAX: u128> fmt::Display for Token<P, I, MAX> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(P::PREFIX)?;
        let mut buf = [0u8; 26]; // max encoded length (u128)
        let buf = &mut buf[..I::ENCODED_LEN];
        self.id.encode_to_buf(buf);
        // SAFETY: all bytes come from ALPHABET which is ASCII
        f.write_str(std::str::from_utf8(buf).expect("alphabet is ASCII"))
    }
}

impl<P: TokenPrefix, I: TokenId, const MAX: u128> FromStr for Token<P, I, MAX> {
    type Err = TokenError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl<P: TokenPrefix, I: TokenId, const MAX: u128> Serialize for Token<P, I, MAX> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de, P: TokenPrefix, I: TokenId, const MAX: u128> Deserialize<'de> for Token<P, I, MAX> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).map_err(de::Error::custom)
    }
}

/// Define a token prefix type and its associated `TokenPrefix` implementation.
///
/// # Example
///
/// ```
/// use hex_play_utils::{define_token_prefix, token::Token};
///
/// define_token_prefix!(UserPrefix, "U_");
/// type UserId = u64;
/// type UserToken = Token<UserPrefix>;          // u64 (default), MAX = u64::MAX
///
/// define_token_prefix!(SessionPrefix, "S_");
/// type SessionId = u128;
/// type SessionToken = Token<SessionPrefix, SessionId>;
/// ```
#[macro_export]
macro_rules! define_token_prefix {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name;

        impl $crate::token::TokenPrefix for $name {
            const PREFIX: &'static str = $prefix;
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    define_token_prefix!(TestPrefix, "T_");
    type TestToken = Token<TestPrefix>;

    define_token_prefix!(UserPrefix, "U_");
    type UserId = u64;
    type UserToken = Token<UserPrefix, UserId>;

    // --- u64 tests (unchanged, use default type parameter) ---

    #[test]
    fn round_trip() {
        for id in [0, 1, 42, 1000, 123_456_789, u64::MAX] {
            let token = TestToken::new(id);
            let s = token.to_string();
            let parsed = TestToken::parse(&s).unwrap();
            assert_eq!(parsed.id(), id);
        }
    }

    #[test]
    fn zero_encodes_to_all_first_char() {
        let token = TestToken::new(0);
        assert_eq!(token.to_string(), "T_AAAAAAAAAAAAA");
    }

    #[test]
    fn u64_max_round_trips() {
        let token = TestToken::new(u64::MAX);
        let s = token.to_string();
        let parsed = TestToken::parse(&s).unwrap();
        assert_eq!(parsed.id(), u64::MAX);
    }

    #[test]
    fn known_value_encoding() {
        let token = TestToken::new(1);
        let s = token.to_string();
        assert_eq!(s, "T_AAAAAAAAAAAAB");
    }

    #[test]
    fn wrong_prefix_error() {
        let err = UserToken::parse("T_AAAAAAAAAAAAA").unwrap_err();
        assert_eq!(
            err,
            TokenError::InvalidPrefix {
                expected: "U_",
                found: "T_".to_string(),
            }
        );
    }

    #[test]
    fn invalid_character_error() {
        // 'I' is not in the alphabet
        let err = TestToken::parse("T_AAAAAAAAAAAIA").unwrap_err();
        assert_eq!(err, TokenError::InvalidCharacter('I'));
    }

    #[test]
    fn excluded_characters_rejected() {
        for ch in ['I', 'L', 'O', 'Q'] {
            let s = format!("T_AAAAAAAAAAAA{ch}");
            let err = TestToken::parse(&s).unwrap_err();
            assert_eq!(err, TokenError::InvalidCharacter(ch));
        }
    }

    #[test]
    fn wrong_length_error() {
        let err = TestToken::parse("T_AAAA").unwrap_err();
        assert_eq!(err, TokenError::InvalidLength { expected: 15, found: 6 });
    }

    #[test]
    fn is_valid_returns_true_for_valid() {
        let s = TestToken::new(42).to_string();
        assert!(TestToken::is_valid(&s));
    }

    #[test]
    fn is_valid_returns_false_for_invalid() {
        assert!(!TestToken::is_valid("INVALID"));
        assert!(!TestToken::is_valid("T_SHORT"));
        assert!(!TestToken::is_valid("X_AAAAAAAAAAAAA"));
    }

    #[test]
    fn from_str_works() {
        let s = TestToken::new(99).to_string();
        let parsed: TestToken = s.parse().unwrap();
        assert_eq!(parsed.id(), 99);
    }

    #[test]
    fn different_prefix_types_are_distinct() {
        let test_s = TestToken::new(42).to_string();
        let user_s = UserToken::new(42).to_string();
        assert_ne!(test_s, user_s);
        assert!(test_s.starts_with("T_"));
        assert!(user_s.starts_with("U_"));
    }

    #[test]
    fn serde_round_trip() {
        let token = TestToken::new(123_456);
        let json = serde_json::to_string(&token).unwrap();
        let parsed: TestToken = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id(), 123_456);
    }

    #[test]
    fn serde_serializes_as_string() {
        let token = TestToken::new(0);
        let json = serde_json::to_string(&token).unwrap();
        assert_eq!(json, r#""T_AAAAAAAAAAAAA""#);
    }

    #[test]
    fn serde_rejects_invalid_token() {
        let result = serde_json::from_str::<TestToken>(r#""INVALID""#);
        assert!(result.is_err());
    }

    #[test]
    fn debug_format() {
        let token = TestToken::new(0);
        let debug = format!("{token:?}");
        assert_eq!(debug, "Token(T_AAAAAAAAAAAAA)");
    }

    // --- u128 tests ---

    define_token_prefix!(BigPrefix, "B_");
    type BigToken = Token<BigPrefix, u128>;

    #[test]
    fn u128_round_trip() {
        for id in [0u128, 1, u64::MAX as u128, u128::MAX] {
            let token = BigToken::new(id);
            let s = token.to_string();
            let parsed = BigToken::parse(&s).unwrap();
            assert_eq!(parsed.id(), id);
        }
    }

    #[test]
    fn u128_zero_encodes_to_26_as() {
        let token = BigToken::new(0);
        assert_eq!(token.to_string(), "B_AAAAAAAAAAAAAAAAAAAAAAAAAA");
    }

    #[test]
    fn u128_max_round_trips() {
        let token = BigToken::new(u128::MAX);
        let s = token.to_string();
        let parsed = BigToken::parse(&s).unwrap();
        assert_eq!(parsed.id(), u128::MAX);
    }

    #[test]
    fn u128_known_value_encoding() {
        let token = BigToken::new(1);
        let s = token.to_string();
        // 25 A's + B
        assert_eq!(s, "B_AAAAAAAAAAAAAAAAAAAAAAAAAB");
    }

    #[test]
    fn u128_wrong_length_error() {
        // prefix (2) + encoded (26) = 28
        let err = BigToken::parse("B_AAAA").unwrap_err();
        assert_eq!(err, TokenError::InvalidLength { expected: 28, found: 6 });
    }

    #[test]
    fn u128_serde_round_trip() {
        let token = BigToken::new(123_456);
        let json = serde_json::to_string(&token).unwrap();
        let parsed: BigToken = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id(), 123_456);
    }

    #[test]
    fn u128_debug_format() {
        let token = BigToken::new(0);
        let debug = format!("{token:?}");
        assert_eq!(debug, "Token(B_AAAAAAAAAAAAAAAAAAAAAAAAAA)");
    }

    // --- custom MAX tests ---

    define_token_prefix!(CappedPrefix, "C_");
    type CappedToken = Token<CappedPrefix, u64, { i64::MAX as u128 }>;

    #[test]
    fn capped_generate_respects_max() {
        for _ in 0..1000 {
            let token = CappedToken::generate();
            assert!(token.id() >= 1);
            assert!(token.id() <= i64::MAX as u64);
        }
    }
}
