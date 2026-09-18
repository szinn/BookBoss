mod model;

use async_trait::async_trait;
use bb_core::{
    Error,
    book::{AuthorRole, IdentifierType},
    import::ImportSource,
    metadata::MetadataProvider,
    pipeline::{ExtractedAuthor, ExtractedIdentifier, ExtractedMetadata, ProviderBook},
};
use bb_utils::{
    date::parse_year,
    language::normalize_language,
    similarity::{author_similarity, combined_score, title_similarity},
};
use model::{Volume, VolumeInfo, VolumeList};
use tracing::warn;

/// Minimum combined similarity score for an ISBN search result to be accepted
/// without falling back to a title search.
const CANDIDATE_THRESHOLD: f32 = 0.5;

/// Metadata provider backed by the Google Books Volumes API.
///
/// Tries ISBN lookup first. If the result's title/author score falls below
/// [`CANDIDATE_THRESHOLD`], or no ISBN is available, falls back to a title
/// search returning up to 10 candidates which are scored and the best is
/// returned. Returns `None` when neither strategy finds a result.
pub struct GoogleBooksAdapter {
    client: reqwest::Client,
    api_token: String,
    base_url: String,
}

impl GoogleBooksAdapter {
    pub fn new(api_token: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(crate::REQUEST_TIMEOUT)
                .build()
                .expect("failed to build reqwest client"),
            api_token: api_token.into(),
            base_url: "https://www.googleapis.com/books/v1".to_string(),
        }
    }

    /// Constructor with overridable base URL — used in tests to point at a
    /// mock server.
    pub fn with_base_url(api_token: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(crate::REQUEST_TIMEOUT)
                .build()
                .expect("failed to build reqwest client"),
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

    fn map_to_extracted(volume_id: &str, info: &VolumeInfo) -> ExtractedMetadata {
        let authors = info.authors.as_ref().map(|names| {
            names
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    #[expect(
                        clippy::cast_possible_truncation,
                        clippy::cast_possible_wrap,
                        reason = "author list index; books have far fewer authors than i32::MAX"
                    )]
                    let sort_order = i as i32;
                    ExtractedAuthor {
                        name: name.clone(),
                        role: Some(AuthorRole::Author),
                        sort_order,
                    }
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
            published_date: info.published_date.as_deref().and_then(parse_year),
            language: info.language.as_deref().and_then(normalize_language),
            identifiers: Some(identifiers),
            series_name: None,
            series_number: None,
            genres: info.categories.clone().unwrap_or_default(),
            tags: vec![],
            page_count: None,
            has_spinnaker_metadata: false,
            cover_bytes: None,
        }
    }

    async fn fetch_cover(&self, info: &VolumeInfo) -> Option<Vec<u8>> {
        // Prefer thumbnail; strip zoom parameter to get a slightly larger
        // image.
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

    /// Searches Google Books by ISBN; returns the first matching volume or
    /// `None`.
    async fn search_isbn(&self, isbn: &str) -> Result<Option<Volume>, Error> {
        let url = format!("{}/volumes?q=isbn:{isbn}&key={}", self.base_url, self.api_token);
        let start = std::time::Instant::now();
        let response: VolumeList = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(format!("Google Books request failed: {e}")))?
            .json()
            .await
            .map_err(|e| Error::Infrastructure(format!("Google Books response parse failed: {e}")))?;
        tracing::debug!(isbn, elapsed_ms = start.elapsed().as_millis(), "Google Books ISBN search completed");
        Ok(response.items.and_then(|mut items| items.drain(..).next()))
    }

    /// Searches Google Books by title; returns up to 10 candidate volumes.
    ///
    /// Uses [`reqwest::Url`] to properly percent-encode the title.
    async fn search_title(&self, title: &str) -> Result<Vec<Volume>, Error> {
        let mut url = reqwest::Url::parse(&format!("{}/volumes", self.base_url))
            .map_err(|e| Error::Infrastructure(format!("Google Books URL construction failed: {e}")))?;
        url.query_pairs_mut()
            .append_pair("q", &format!("intitle:{title}"))
            .append_pair("key", &self.api_token)
            .append_pair("maxResults", "10");
        let start = std::time::Instant::now();
        let response: VolumeList = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(format!("Google Books request failed: {e}")))?
            .json()
            .await
            .map_err(|e| Error::Infrastructure(format!("Google Books response parse failed: {e}")))?;
        tracing::debug!(title, elapsed_ms = start.elapsed().as_millis(), "Google Books title search completed");
        Ok(response.items.unwrap_or_default())
    }

    /// Scores a candidate volume against an extracted title and primary author.
    /// Uses `0.5` as a neutral author score when either side is missing.
    fn score_candidate(volume: &Volume, title: Option<&str>, author: Option<&str>) -> f32 {
        let t_score = match (title, volume.volume_info.title.as_deref()) {
            (Some(a), Some(b)) => title_similarity(a, b),
            _ => 0.0,
        };
        let a_score = match (author, volume.volume_info.authors.as_deref().and_then(|a| a.first())) {
            (Some(a), Some(b)) => author_similarity(a, b),
            _ => 0.5,
        };
        combined_score(t_score, a_score)
    }
}

#[async_trait]
impl MetadataProvider for GoogleBooksAdapter {
    fn name(&self) -> &'static str {
        "Google Books"
    }

    async fn enrich(&self, extracted: &ExtractedMetadata) -> Result<Option<ProviderBook>, Error> {
        let isbn = Self::find_isbn(extracted);
        let title = extracted.title.as_deref();
        let author = extracted
            .authors
            .as_deref()
            .and_then(|a| a.iter().min_by_key(|a| a.sort_order))
            .map(|a| a.name.as_str());

        let mut title_search_performed = false;

        // ── Step 1: ISBN search
        // ───────────────────────────────────────────────
        let isbn_volume = if let Some(ref isbn_str) = isbn {
            self.search_isbn(isbn_str).await?
        } else {
            None
        };

        // ── Step 2: Validate ISBN result against embedded title/author
        // ────────
        //
        // If no title is available there is nothing to validate against, so the
        // ISBN result is accepted as-is (the pipeline will score it 0.0 and
        // reject it anyway). If validation fails, fall through to title search.
        let best_volume = match (isbn_volume, title) {
            (Some(vol), Some(t)) => {
                let score = Self::score_candidate(&vol, Some(t), author);
                if score >= CANDIDATE_THRESHOLD {
                    Some(vol)
                } else {
                    tracing::debug!(score, "Google Books ISBN result score too low; falling back to title search");
                    None
                }
            }
            (Some(vol), None) => Some(vol),
            (None, _) => None,
        };

        // ── Step 3: Title search fallback
        // ─────────────────────────────────────
        let best_volume = if best_volume.is_none() {
            if let Some(title_str) = title {
                title_search_performed = true;
                let candidates = self.search_title(title_str).await?;
                tracing::debug!(candidates = candidates.len(), "Google Books title search returned candidates");
                candidates.into_iter().max_by(|a, b| {
                    let sa = Self::score_candidate(a, title, author);
                    let sb = Self::score_candidate(b, title, author);
                    sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
                })
            } else {
                None
            }
        } else {
            best_volume
        };

        // ── Log summary and return
        // ─────────────────────────────────────────────
        let final_score = best_volume.as_ref().map(|v| Self::score_candidate(v, title, author));
        tracing::debug!(
            isbn = ?isbn,
            title = ?title,
            title_search_performed,
            score = ?final_score,
            "Google Books enrichment completed"
        );

        match best_volume {
            Some(volume) => {
                let metadata = Self::map_to_extracted(&volume.id, &volume.volume_info);
                let cover_bytes = self.fetch_cover(&volume.volume_info).await;
                Ok(Some(ProviderBook {
                    metadata,
                    cover_bytes,
                    source: ImportSource::GoogleBooks,
                }))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
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

    fn extracted_with_isbn_and_title(isbn: &str, title: &str, author: &str) -> ExtractedMetadata {
        ExtractedMetadata {
            title: Some(title.to_string()),
            authors: Some(vec![ExtractedAuthor {
                name: author.to_string(),
                role: Some(AuthorRole::Author),
                sort_order: 0,
            }]),
            identifiers: Some(vec![ExtractedIdentifier {
                identifier_type: IdentifierType::Isbn13,
                value: isbn.to_string(),
            }]),
            ..Default::default()
        }
    }

    fn volume_item(id: &str, title: &str, author: &str, isbn: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "volumeInfo": {
                "title": title,
                "authors": [author],
                "industryIdentifiers": [{"type": "ISBN_13", "identifier": isbn}],
                "imageLinks": null
            }
        })
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

    fn empty_response() -> serde_json::Value {
        serde_json::json!({"totalItems": 0})
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
            .respond_with(ResponseTemplate::new(200).set_body_json(empty_response()))
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

        // No title in extracted — ISBN result accepted without validation.
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

    #[tokio::test]
    async fn enrich_accepts_isbn_result_when_title_matches() {
        let server = MockServer::start().await;
        let isbn = "9780765326355";
        let cover_url = format!("{}/cover?id=abc123", server.uri());

        Mock::given(method("GET"))
            .and(path("/volumes"))
            .and(query_param("q", format!("isbn:{isbn}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(volume_response(isbn, &cover_url)))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/cover"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"fake-cover".to_vec()))
            .mount(&server)
            .await;

        let extracted = extracted_with_isbn_and_title(isbn, "The Way of Kings", "Brandon Sanderson");
        let adapter = GoogleBooksAdapter::with_base_url("token", server.uri());
        let result = adapter.enrich(&extracted).await.unwrap();
        assert_eq!(result.expect("expected ProviderBook").metadata.title.as_deref(), Some("The Way of Kings"));
        // Only one volumes request (no title search fallback).
        assert_eq!(
            server.received_requests().await.unwrap().iter().filter(|r| r.url.path() == "/volumes").count(),
            1
        );
    }

    #[tokio::test]
    async fn enrich_falls_back_to_title_search_when_isbn_is_wrong_book() {
        let server = MockServer::start().await;
        let isbn = "9781250281470";

        // ISBN search returns the wrong book.
        Mock::given(method("GET"))
            .and(path("/volumes"))
            .and(query_param("q", format!("isbn:{isbn}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "totalItems": 1,
                "items": [volume_item("bad1", "Some Completely Different Book", "Wrong Author", isbn)]
            })))
            .mount(&server)
            .await;

        // Title search returns the correct book.
        Mock::given(method("GET"))
            .and(path("/volumes"))
            .and(query_param("q", "intitle:The Correct Book Title"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "totalItems": 1,
                "items": [volume_item("good1", "The Correct Book Title", "Right Author", "9780000000001")]
            })))
            .mount(&server)
            .await;

        let extracted = extracted_with_isbn_and_title(isbn, "The Correct Book Title", "Right Author");
        let adapter = GoogleBooksAdapter::with_base_url("token", server.uri());
        let result = adapter.enrich(&extracted).await.unwrap();
        let book = result.expect("expected ProviderBook");

        assert_eq!(book.metadata.title.as_deref(), Some("The Correct Book Title"));
        assert_eq!(
            server.received_requests().await.unwrap().iter().filter(|r| r.url.path() == "/volumes").count(),
            2
        );
    }

    #[tokio::test]
    async fn enrich_picks_best_candidate_from_title_search() {
        let server = MockServer::start().await;

        // No ISBN — goes straight to title search; returns two candidates.
        Mock::given(method("GET"))
            .and(path("/volumes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "totalItems": 2,
                "items": [
                    volume_item("1", "The Hobbit", "Wrong Author", "9780000000001"),
                    volume_item("2", "The Hobbit", "J.R.R. Tolkien", "9780000000002")
                ]
            })))
            .mount(&server)
            .await;

        let extracted = ExtractedMetadata {
            title: Some("The Hobbit".to_string()),
            authors: Some(vec![ExtractedAuthor {
                name: "J.R.R. Tolkien".to_string(),
                role: Some(AuthorRole::Author),
                sort_order: 0,
            }]),
            ..Default::default()
        };

        let adapter = GoogleBooksAdapter::with_base_url("token", server.uri());
        let result = adapter.enrich(&extracted).await.unwrap();
        let book = result.expect("expected ProviderBook");

        let identifiers = book.metadata.identifiers.as_ref().expect("expected identifiers");
        assert!(
            identifiers
                .iter()
                .any(|id| id.identifier_type == IdentifierType::GoogleBooks && id.value == "2")
        );
    }

    #[tokio::test]
    async fn enrich_returns_err_on_http_5xx() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/volumes"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let adapter = GoogleBooksAdapter::with_base_url("token", server.uri());
        let result = adapter.enrich(&extracted_with_isbn13("9780765326355")).await;
        assert!(result.is_err(), "expected Err on 5xx response, got {result:?}");
    }

    #[tokio::test]
    async fn enrich_returns_err_on_malformed_json() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/volumes"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;

        let adapter = GoogleBooksAdapter::with_base_url("token", server.uri());
        let result = adapter.enrich(&extracted_with_isbn13("9780765326355")).await;
        assert!(result.is_err(), "expected Err on malformed JSON, got {result:?}");
    }

    #[test]
    fn parse_year_delegates_to_bb_utils() {
        assert_eq!(parse_year("2010-08-31"), Some(2010));
        assert_eq!(parse_year("no year"), None);
    }
}
