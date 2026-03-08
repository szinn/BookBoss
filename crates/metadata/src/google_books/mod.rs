mod model;

use async_trait::async_trait;
use bb_core::{
    Error,
    book::{AuthorRole, IdentifierType},
    import::ImportSource,
    pipeline::{ExtractedAuthor, ExtractedIdentifier, ExtractedMetadata, MetadataProvider, ProviderBook},
};
use model::{VolumeInfo, VolumeList};
use tracing::warn;

/// Metadata provider backed by the Google Books Volumes API.
///
/// Performs ISBN lookup via `q=isbn:{isbn}`, maps the first result to
/// [`ProviderBook`], and fetches cover art bytes internally. Returns `None`
/// when no ISBN is available or Google Books has no matching record.
pub struct GoogleBooksAdapter {
    client: reqwest::Client,
    api_token: String,
    base_url: String,
}

impl GoogleBooksAdapter {
    pub fn new(api_token: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_token: api_token.into(),
            base_url: "https://www.googleapis.com/books/v1".to_string(),
        }
    }

    /// Constructor with overridable base URL — used in tests to point at a
    /// mock server.
    pub fn with_base_url(api_token: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_token: api_token.into(),
            base_url: base_url.into(),
        }
    }

    /// Returns the first ISBN-13, falling back to ISBN-10, from the extracted
    /// identifiers.
    fn find_isbn(extracted: &ExtractedMetadata) -> Option<String> {
        let identifiers = extracted.identifiers.as_deref()?;
        identifiers
            .iter()
            .find(|id| id.identifier_type == IdentifierType::Isbn13)
            .or_else(|| identifiers.iter().find(|id| id.identifier_type == IdentifierType::Isbn10))
            .map(|id| id.value.clone())
    }

    /// Scans a freeform date string for the first plausible 4-digit year.
    ///
    /// Google Books `publishedDate` values vary: "2010-11-17", "2010", "Nov
    /// 2010".
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

    fn map_to_extracted(volume_id: &str, info: &VolumeInfo) -> ExtractedMetadata {
        let authors = info.authors.as_ref().map(|names| {
            names
                .iter()
                .enumerate()
                .map(|(i, name)| ExtractedAuthor {
                    name: name.clone(),
                    role: Some(AuthorRole::Author),
                    sort_order: i as i32,
                })
                .collect()
        });

        let mut identifiers: Vec<ExtractedIdentifier> = info
            .industry_identifiers
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter_map(|ii| {
                let id_type = match ii.id_type.as_str() {
                    "ISBN_13" => IdentifierType::Isbn13,
                    "ISBN_10" => IdentifierType::Isbn10,
                    _ => return None,
                };
                Some(ExtractedIdentifier {
                    identifier_type: id_type,
                    value: ii.identifier.clone(),
                })
            })
            .collect();

        identifiers.push(ExtractedIdentifier {
            identifier_type: IdentifierType::GoogleBooks,
            value: volume_id.to_string(),
        });

        ExtractedMetadata {
            title: info.title.clone(),
            authors,
            description: info.description.clone(),
            publisher: info.publisher.clone(),
            published_date: info.published_date.as_deref().and_then(Self::parse_year),
            language: info.language.clone(),
            identifiers: Some(identifiers),
            series_name: None,
            series_number: None,
            cover_bytes: None,
        }
    }

    async fn fetch_cover(&self, info: &VolumeInfo) -> Option<Vec<u8>> {
        // Prefer thumbnail; strip zoom parameter to get a slightly larger image.
        let url = info
            .image_links
            .as_ref()
            .and_then(|il| il.thumbnail.as_ref().or(il.small_thumbnail.as_ref()))?
            .replace("&zoom=1", "&zoom=0")
            .replace("zoom=1&", "zoom=0&");

        match self.client.get(&url).send().await {
            Ok(response) if response.status().is_success() => match response.bytes().await {
                Ok(bytes) => Some(bytes.to_vec()),
                Err(e) => {
                    warn!("Failed to read Google Books cover bytes: {e}");
                    None
                }
            },
            Ok(response) => {
                warn!(status = %response.status(), url = %url, "Google Books cover not available");
                None
            }
            Err(e) => {
                warn!("Failed to fetch Google Books cover: {e}");
                None
            }
        }
    }
}

#[async_trait]
impl MetadataProvider for GoogleBooksAdapter {
    fn name(&self) -> &'static str {
        "Google Books"
    }

    async fn enrich(&self, extracted: &ExtractedMetadata) -> Result<Option<ProviderBook>, Error> {
        let Some(isbn) = Self::find_isbn(extracted) else {
            return Ok(None);
        };

        let url = format!("{}/volumes?q=isbn:{isbn}&key={}", self.base_url, self.api_token);

        let response: VolumeList = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(format!("Google Books request failed: {e}")))?
            .json()
            .await
            .map_err(|e| Error::Infrastructure(format!("Google Books response parse failed: {e}")))?;

        let Some(volume) = response.items.and_then(|mut items| items.drain(..).next()) else {
            return Ok(None);
        };

        let metadata = Self::map_to_extracted(&volume.id, &volume.volume_info);
        let cover_bytes = self.fetch_cover(&volume.volume_info).await;

        Ok(Some(ProviderBook {
            metadata,
            cover_bytes,
            source: ImportSource::GoogleBooks,
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

    fn volume_response(isbn: &str, cover_url: &str) -> serde_json::Value {
        serde_json::json!({
            "totalItems": 1,
            "items": [{
                "id": "abc123",
                "volumeInfo": {
                    "title": "The Way of Kings",
                    "authors": ["Brandon Sanderson"],
                    "publisher": "Tor Books",
                    "publishedDate": "2010-08-31",
                    "description": "A great book.",
                    "industryIdentifiers": [
                        {"type": "ISBN_13", "identifier": isbn},
                        {"type": "ISBN_10", "identifier": "0765326353"}
                    ],
                    "language": "en",
                    "imageLinks": {
                        "thumbnail": cover_url
                    }
                }
            }]
        })
    }

    #[tokio::test]
    async fn enrich_returns_none_when_no_isbn() {
        let adapter = GoogleBooksAdapter::new("token");
        let result = adapter.enrich(&ExtractedMetadata::default()).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn enrich_returns_none_when_not_found() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/volumes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "totalItems": 0
            })))
            .mount(&server)
            .await;

        let adapter = GoogleBooksAdapter::with_base_url("token", server.uri());
        let result = adapter.enrich(&extracted_with_isbn13("9780765326355")).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn enrich_maps_book_fields() {
        let server = MockServer::start().await;
        let isbn = "9780765326355";
        // Thumbnail URL points at mock server; zoom=1 so fetch_cover replaces it with zoom=0.
        let cover_url = format!("{}/cover?id=abc123&zoom=1", server.uri());

        Mock::given(method("GET"))
            .and(path("/volumes"))
            .and(query_param("q", format!("isbn:{isbn}")))
            .and(query_param("key", "token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(volume_response(isbn, &cover_url)))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/cover"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"fake-cover".to_vec()))
            .mount(&server)
            .await;

        let adapter = GoogleBooksAdapter::with_base_url("token", server.uri());
        let result = adapter.enrich(&extracted_with_isbn13(isbn)).await.unwrap();
        let book = result.expect("expected ProviderBook");

        assert_eq!(book.metadata.title.as_deref(), Some("The Way of Kings"));
        assert_eq!(book.metadata.published_date, Some(2010));
        assert_eq!(book.metadata.publisher.as_deref(), Some("Tor Books"));
        assert_eq!(book.metadata.language.as_deref(), Some("en"));
        assert_eq!(book.source, ImportSource::GoogleBooks);

        let authors = book.metadata.authors.as_ref().expect("expected authors");
        assert_eq!(authors.len(), 1);
        assert_eq!(authors[0].name, "Brandon Sanderson");
        assert_eq!(authors[0].role, Some(AuthorRole::Author));

        let identifiers = book.metadata.identifiers.as_ref().expect("expected identifiers");
        assert!(identifiers.iter().any(|id| id.identifier_type == IdentifierType::Isbn13 && id.value == isbn));
        assert!(identifiers.iter().any(|id| id.identifier_type == IdentifierType::Isbn10));
        assert!(
            identifiers
                .iter()
                .any(|id| id.identifier_type == IdentifierType::GoogleBooks && id.value == "abc123")
        );

        assert_eq!(book.cover_bytes.as_deref(), Some(b"fake-cover".as_slice()));
    }

    #[test]
    fn parse_year_handles_freeform_dates() {
        assert_eq!(GoogleBooksAdapter::parse_year("2010-08-31"), Some(2010));
        assert_eq!(GoogleBooksAdapter::parse_year("2010"), Some(2010));
        assert_eq!(GoogleBooksAdapter::parse_year("no year"), None);
        assert_eq!(GoogleBooksAdapter::parse_year(""), None);
    }
}
