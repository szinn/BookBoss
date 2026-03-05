use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(super) struct OlAuthor {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct OlPublisher {
    pub name: String,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct OlCover {
    pub large: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct OlIdentifiers {
    pub openlibrary: Option<Vec<String>>,
}

/// Subset of the Open Library Books API response (`jscmd=data`) used by the
/// adapter.
///
/// All fields are optional — OL record completeness varies widely.
#[derive(Debug, Deserialize)]
pub(super) struct OlBookData {
    pub title: Option<String>,
    pub authors: Option<Vec<OlAuthor>>,
    pub publishers: Option<Vec<OlPublisher>>,
    pub publish_date: Option<String>,
    pub cover: Option<OlCover>,
    pub identifiers: Option<OlIdentifiers>,
}
