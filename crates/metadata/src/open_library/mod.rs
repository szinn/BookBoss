mod model;

use std::collections::HashMap;

use async_trait::async_trait;
use bb_core::{
    Error,
    book::{AuthorRole, IdentifierType},
    import::ImportSource,
    pipeline::{ExtractedAuthor, ExtractedIdentifier, ExtractedMetadata, MetadataProvider, ProviderBook},
};
use model::OlBookData;
use tracing::warn;

/// Metadata provider backed by the Open Library Books API.
///
/// Performs ISBN lookup (`jscmd=data`), maps the response to [`ProviderBook`],
/// and fetches cover art bytes internally. Returns `None` when no ISBN is
/// available in the extracted metadata or when Open Library has no record.
pub struct OpenLibraryAdapter {
    client: reqwest::Client,
    base_url: String,
    covers_base_url: String,
}

impl Default for OpenLibraryAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenLibraryAdapter {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: "https://openlibrary.org".to_string(),
            covers_base_url: "https://covers.openlibrary.org".to_string(),
        }
    }

    /// Constructor with overridable base URLs — used in tests to point at a
    /// mock server.
    pub fn with_base_urls(base_url: impl Into<String>, covers_base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into(),
            covers_base_url: covers_base_url.into(),
        }
    }

    /// Returns the first ISBN-13, falling back to ISBN-10, from the extracted
    /// identifiers.
    fn find_isbn(extracted: &ExtractedMetadata) -> Option<(IdentifierType, String)> {
        let identifiers = extracted.identifiers.as_deref()?;
        identifiers
            .iter()
            .find(|id| id.identifier_type == IdentifierType::Isbn13)
            .or_else(|| identifiers.iter().find(|id| id.identifier_type == IdentifierType::Isbn10))
            .map(|id| (id.identifier_type.clone(), id.value.clone()))
    }

    /// Scans a freeform date string for the first plausible 4-digit year.
    ///
    /// Open Library `publish_date` values vary widely: "August 31, 2010",
    /// "2010", "1996-08-15".
    fn parse_year(date_str: &str) -> Option<i32> {
        let bytes = date_str.as_bytes();
        for i in 0..bytes.len().saturating_sub(3) {
            if bytes[i..i + 4].iter().all(|b| b.is_ascii_digit()) {
                if let Ok(year) = date_str[i..i + 4].parse::<i32>() {
                    if (1000..=2100).contains(&year) {
                        return Some(year);
                    }
                }
            }
        }
        None
    }

    fn map_to_extracted(data: &OlBookData, isbn_type: IdentifierType, isbn: &str) -> ExtractedMetadata {
        let authors = data.authors.as_ref().map(|authors| {
            authors
                .iter()
                .enumerate()
                .map(|(i, a)| ExtractedAuthor {
                    name: a.name.clone(),
                    role: Some(AuthorRole::Author),
                    sort_order: i as i32,
                })
                .collect()
        });

        let mut identifiers = vec![ExtractedIdentifier {
            identifier_type: isbn_type,
            value: isbn.to_string(),
        }];

        if let Some(ol_id) = data.identifiers.as_ref().and_then(|ids| ids.openlibrary.as_ref()).and_then(|ids| ids.first()) {
            identifiers.push(ExtractedIdentifier {
                identifier_type: IdentifierType::OpenLibrary,
                value: ol_id.clone(),
            });
        }

        ExtractedMetadata {
            title: data.title.clone(),
            authors,
            description: None,
            publisher: data.publishers.as_ref().and_then(|p| p.first()).map(|p| p.name.clone()),
            published_date: data.publish_date.as_deref().and_then(Self::parse_year),
            language: None,
            identifiers: Some(identifiers),
            series_name: None,
            series_number: None,
            cover_bytes: None,
        }
    }

    async fn fetch_cover(&self, data: &OlBookData, isbn: &str) -> Option<Vec<u8>> {
        let cover_url = data
            .cover
            .as_ref()
            .and_then(|c| c.large.as_ref())
            .cloned()
            .unwrap_or_else(|| format!("{}/b/isbn/{isbn}-L.jpg", self.covers_base_url));

        match self.client.get(&cover_url).send().await {
            Ok(response) if response.status().is_success() => match response.bytes().await {
                Ok(bytes) => Some(bytes.to_vec()),
                Err(e) => {
                    warn!("Failed to read Open Library cover bytes: {e}");
                    None
                }
            },
            Ok(response) => {
                warn!(status = %response.status(), url = %cover_url, "Open Library cover not available");
                None
            }
            Err(e) => {
                warn!("Failed to fetch Open Library cover: {e}");
                None
            }
        }
    }
}

#[async_trait]
impl MetadataProvider for OpenLibraryAdapter {
    fn name(&self) -> &'static str {
        "Open Library"
    }

    async fn enrich(&self, extracted: &ExtractedMetadata) -> Result<Option<ProviderBook>, Error> {
        let Some((isbn_type, isbn)) = Self::find_isbn(extracted) else {
            return Ok(None);
        };

        let url = format!("{}/api/books?bibkeys=ISBN:{isbn}&format=json&jscmd=data", self.base_url);

        let response: HashMap<String, OlBookData> = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(format!("Open Library request failed: {e}")))?
            .json()
            .await
            .map_err(|e| Error::Infrastructure(format!("Open Library response parse failed: {e}")))?;

        let key = format!("ISBN:{isbn}");
        let Some(book_data) = response.get(&key) else {
            return Ok(None);
        };

        let metadata = Self::map_to_extracted(book_data, isbn_type, &isbn);
        let cover_bytes = self.fetch_cover(book_data, &isbn).await;

        Ok(Some(ProviderBook {
            metadata,
            cover_bytes,
            source: ImportSource::OpenLibrary,
        }))
    }
}

#[cfg(test)]
mod tests {
    use bb_core::pipeline::ExtractedIdentifier;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path, query_param},
    };

    use super::*;

    fn extracted_with_isbn13(isbn: &str) -> ExtractedMetadata {
        ExtractedMetadata {
            identifiers: Some(vec![ExtractedIdentifier {
                identifier_type: IdentifierType::Isbn13,
                value: isbn.to_string(),
            }]),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn enrich_returns_none_when_no_isbn() {
        let adapter = OpenLibraryAdapter::new();
        let extracted = ExtractedMetadata::default();
        let result = adapter.enrich(&extracted).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn enrich_returns_none_when_book_not_found() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/books"))
            .and(query_param("bibkeys", "ISBN:9780765326355"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        let adapter = OpenLibraryAdapter::with_base_urls(server.uri(), server.uri());
        let result = adapter.enrich(&extracted_with_isbn13("9780765326355")).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn enrich_maps_book_and_fetches_cover() {
        let server = MockServer::start().await;
        let isbn = "9780765326355";

        Mock::given(method("GET"))
            .and(path("/api/books"))
            .and(query_param("bibkeys", format!("ISBN:{isbn}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                format!("ISBN:{isbn}"): {
                    "title": "The Way of Kings",
                    "authors": [{"name": "Brandon Sanderson"}],
                    "publishers": [{"name": "Tor Books"}],
                    "publish_date": "August 31, 2010",
                    "identifiers": {
                        "isbn_13": [isbn],
                        "openlibrary": ["OL7353617M"]
                    }
                }
            })))
            .mount(&server)
            .await;

        // Cover fallback URL: {covers_base_url}/b/isbn/{isbn}-L.jpg
        Mock::given(method("GET"))
            .and(path(format!("/b/isbn/{isbn}-L.jpg")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"fake-cover-bytes".to_vec()))
            .mount(&server)
            .await;

        let adapter = OpenLibraryAdapter::with_base_urls(server.uri(), server.uri());
        let result = adapter.enrich(&extracted_with_isbn13(isbn)).await.unwrap();
        let book = result.expect("expected ProviderBook");

        assert_eq!(book.metadata.title.as_deref(), Some("The Way of Kings"));
        assert_eq!(book.metadata.published_date, Some(2010));
        assert_eq!(book.metadata.publisher.as_deref(), Some("Tor Books"));

        let authors = book.metadata.authors.as_ref().expect("expected authors");
        assert_eq!(authors.len(), 1);
        assert_eq!(authors[0].name, "Brandon Sanderson");

        let identifiers = book.metadata.identifiers.as_ref().expect("expected identifiers");
        assert!(identifiers.iter().any(|id| id.identifier_type == IdentifierType::Isbn13));
        assert!(identifiers.iter().any(|id| id.identifier_type == IdentifierType::OpenLibrary));

        assert_eq!(book.cover_bytes.as_deref(), Some(b"fake-cover-bytes".as_slice()));
    }

    #[test]
    fn parse_year_handles_freeform_dates() {
        assert_eq!(OpenLibraryAdapter::parse_year("August 31, 2010"), Some(2010));
        assert_eq!(OpenLibraryAdapter::parse_year("2010"), Some(2010));
        assert_eq!(OpenLibraryAdapter::parse_year("1996-08-15"), Some(1996));
        assert_eq!(OpenLibraryAdapter::parse_year("no year here"), None);
        assert_eq!(OpenLibraryAdapter::parse_year(""), None);
    }
}
