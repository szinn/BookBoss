mod parse;
mod write;

pub use parse::{extract_cover_href, extract_metadata, parse_sidecar};
pub use write::write_sidecar;

#[cfg(test)]
mod regression_tests;
