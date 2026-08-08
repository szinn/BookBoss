/// Scans a freeform date string for the first plausible 4-digit year.
///
/// Handles formats like `"August 31, 2010"`, `"2010"`, `"1996-08-15"`,
/// `"2010-11-17"`. Returns `None` if no year in the range 1000–2100 is found.
///
/// # Examples
///
/// ```
/// use bb_utils::date::parse_year;
/// assert_eq!(parse_year("August 31, 2010"), Some(2010));
/// assert_eq!(parse_year("no year here"), None);
/// ```
pub fn parse_year(date_str: &str) -> Option<i32> {
    let bytes = date_str.as_bytes();
    for i in 0..bytes.len().saturating_sub(3) {
        if bytes[i..i + 4].iter().all(u8::is_ascii_digit)
            && let Ok(year) = date_str[i..i + 4].parse::<i32>()
            && (1000..=2100).contains(&year)
        {
            return Some(year);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::parse_year;

    #[test]
    fn parse_year_handles_freeform_dates() {
        assert_eq!(parse_year("August 31, 2010"), Some(2010));
        assert_eq!(parse_year("2010"), Some(2010));
        assert_eq!(parse_year("1996-08-15"), Some(1996));
        assert_eq!(parse_year("2010-08-31"), Some(2010));
        assert_eq!(parse_year("no year here"), None);
        assert_eq!(parse_year(""), None);
    }

    #[test]
    fn parse_year_out_of_range_returns_none() {
        assert_eq!(parse_year("3456"), None); // > 2100
        assert_eq!(parse_year("0999"), None); // < 1000
        assert_eq!(parse_year("999"), None); // only 3 digits
        assert_eq!(parse_year("2100"), Some(2100)); // upper boundary
        assert_eq!(parse_year("1000"), Some(1000)); // lower boundary
    }
}
