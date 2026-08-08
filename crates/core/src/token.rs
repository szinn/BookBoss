use alderkit_token::define_alphabet;
pub use alderkit_token::{
    define_token_prefix,
    token::{Token, TokenError},
};

// Byte order must exactly match the retired bb-utils alphabet — every token
// already persisted in the database was encoded against this exact ordering.
define_alphabet!(TokenAlphabet, b"Y4XK0N8AR3G6JM2VT9BS5WC1DPH7EUZF");

#[cfg(test)]
mod tests {
    use alderkit_token::token::Alphabet;

    use super::*;

    define_token_prefix!(TestPrefix, "T_");
    type TestToken = Token<TestPrefix, u64, TokenAlphabet>;

    define_token_prefix!(BigPrefix, "B_");
    type BigToken = Token<BigPrefix, u128, TokenAlphabet>;

    #[test]
    fn core_alphabet_matches_retired_bb_utils_alphabet() {
        assert_eq!(TokenAlphabet::ALPHABET, b"Y4XK0N8AR3G6JM2VT9BS5WC1DPH7EUZF");
    }

    #[test]
    fn zero_encodes_to_all_first_char() {
        assert_eq!(TestToken::new(0).to_string(), "T_YYYYYYYYYYYYY");
    }

    #[test]
    fn known_value_encoding() {
        assert_eq!(TestToken::new(1).to_string(), "T_YYYYYYYYYYYY4");
    }

    #[test]
    fn u64_max_round_trips() {
        let token = TestToken::new(u64::MAX);
        let parsed = TestToken::parse(&token.to_string()).unwrap();
        assert_eq!(parsed.id(), u64::MAX);
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
    fn u128_zero_encodes_to_26_ys() {
        assert_eq!(BigToken::new(0).to_string(), "B_YYYYYYYYYYYYYYYYYYYYYYYYYY");
    }

    #[test]
    fn u128_known_value_encoding() {
        assert_eq!(BigToken::new(1).to_string(), "B_YYYYYYYYYYYYYYYYYYYYYYYYY4");
    }

    #[test]
    fn u128_max_round_trips() {
        let token = BigToken::new(u128::MAX);
        let parsed = BigToken::parse(&token.to_string()).unwrap();
        assert_eq!(parsed.id(), u128::MAX);
    }
}
