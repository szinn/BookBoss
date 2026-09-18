mod model;

use std::collections::HashMap;

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
    similarity::{author_similarity, combined_score, title_similarity},
};
use model::{OlBookData, OlSearchDoc, OlSearchResponse};
use tracing::warn;

/// Minimum combined similarity score for an ISBN search result to be accepted
/// without falling back to a title search.
const CANDIDATE_THRESHOLD: f32 = 0.5;

/// Metadata provider backed by the Open Library Books API.
///
/// Tries ISBN lookup first. If the result's title/author score falls below
/// [`CANDIDATE_THRESHOLD`], or no ISBN is available, falls back to a title
/// search (`/search.json`) returning up to 10 candidates which are scored
/// and the best is returned. Returns `None` when neither strategy finds a
/// result.
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
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(crate::REQUEST_TIMEOUT)
                .build()
                .expect("failed to build reqwest client"),
            base_url: "https://openlibrary.org".to_string(),
            covers_base_url: "https://covers.openlibrary.org".to_string(),
        }
    }

    /// Constructor with overridable base URLs — used in tests to point at a
    /// mock server.
    pub fn with_base_urls(base_url: impl Into<String>, covers_base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(crate::REQUEST_TIMEOUT)
                .build()
                .expect("failed to build reqwest client"),
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

    /// Builds a `Vec<ExtractedAuthor>` from an iterator of name strings.
    /// All authors are assigned role `Author` with sequential `sort_order`.
    fn authors_from_names<'a>(names: impl Iterator<Item = &'a str>) -> Vec<ExtractedAuthor> {
        names
            .enumerate()
            .map(|(i, name)| {
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_possible_wrap,
                    reason = "author list index; books have far fewer authors than i32::MAX"
                )]
                let sort_order = i as i32;
                ExtractedAuthor {
                    name: name.to_string(),
                    role: Some(AuthorRole::Author),
                    sort_order,
                }
            })
            .collect()
    }

    fn map_to_extracted(data: &OlBookData, isbn_type: IdentifierType, isbn: &str) -> ExtractedMetadata {
        let authors = data.authors.as_ref().map(|a| Self::authors_from_names(a.iter().map(|a| a.name.as_str())));

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
            published_date: data.publish_date.as_deref().and_then(parse_year),
            language: None,
            identifiers: Some(identifiers),
            series_name: None,
            series_number: None,
            genres: data.subjects.as_deref().unwrap_or(&[]).iter().map(|s| s.name.clone()).collect(),
            tags: vec![],
            page_count: None,
            has_spinnaker_metadata: false,
            cover_bytes: None,
        }
    }

    /// Maps an `OlSearchDoc` (from `/search.json`) to `ExtractedMetadata`.
    fn map_search_doc_to_extracted(doc: &OlSearchDoc) -> ExtractedMetadata {
        let authors = doc.author_name.as_ref().map(|names| Self::authors_from_names(names.iter().map(String::as_str)));

        // Classify ISBNs by length (search results mix ISBN-10 and ISBN-13).
        let identifiers = doc.isbn.as_ref().map(|isbns| {
            isbns
                .iter()
                .filter_map(|isbn| {
                    let id_type = match isbn.len() {
                        10 => IdentifierType::Isbn10,
                        13 => IdentifierType::Isbn13,
                        _ => return None,
                    };
                    Some(ExtractedIdentifier {
                        identifier_type: id_type,
                        value: isbn.clone(),
                    })
                })
                .collect()
        });

        ExtractedMetadata {
            title: doc.title.clone(),
            authors,
            description: None,
            publisher: doc.publisher.as_ref().and_then(|p| p.first()).cloned(),
            published_date: doc.first_publish_year,
            language: None,
            identifiers,
            series_name: None,
            series_number: None,
            genres: vec![],
            tags: vec![],
            page_count: None,
            has_spinnaker_metadata: false,
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

        self.fetch_cover_url(&cover_url).await
    }

    async fn fetch_cover_for_doc(&self, doc: &OlSearchDoc) -> Option<Vec<u8>> {
        let cover_url = if let Some(cover_id) = doc.cover_i {
            format!("{}/b/id/{cover_id}-L.jpg", self.covers_base_url)
        } else {
            let isbn = doc.isbn.as_ref().and_then(|isbns| isbns.first())?;
            format!("{}/b/isbn/{isbn}-L.jpg", self.covers_base_url)
        };

        self.fetch_cover_url(&cover_url).await
    }

    async fn fetch_cover_url(&self, url: &str) -> Option<Vec<u8>> {
        match self.client.get(url).send().await {
            Ok(response) if response.status().is_success() => match response.bytes().await {
                Ok(bytes) => Some(bytes.to_vec()),
                Err(e) => {
                    warn!("Failed to read Open Library cover bytes: {e}");
                    None
                }
            },
            Ok(response) => {
                warn!(status = %response.status(), url = %url, "Open Library cover not available");
                None
            }
            Err(e) => {
                warn!("Failed to fetch Open Library cover: {e}");
                None
            }
        }
    }

    /// Looks up a book by ISBN using the Books API (`jscmd=data`).
    async fn search_isbn(&self, isbn_type: IdentifierType, isbn: &str) -> Result<Option<(IdentifierType, String, OlBookData)>, Error> {
        let url = format!("{}/api/books?bibkeys=ISBN:{isbn}&format=json&jscmd=data", self.base_url);
        let start = std::time::Instant::now();
        let response: HashMap<String, OlBookData> = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(format!("Open Library request failed: {e}")))?
            .json()
            .await
            .map_err(|e| Error::Infrastructure(format!("Open Library response parse failed: {e}")))?;
        tracing::debug!(isbn, elapsed_ms = start.elapsed().as_millis(), "Open Library ISBN search completed");
        let key = format!("ISBN:{isbn}");
        Ok(response
            .into_iter()
            .find(|(k, _)| k == &key)
            .map(|(_, data)| (isbn_type, isbn.to_string(), data)))
    }

    /// Searches Open Library by title via `/search.json`; returns up to 10
    /// candidate documents.
    async fn search_title(&self, title: &str) -> Result<Vec<OlSearchDoc>, Error> {
        let mut url = reqwest::Url::parse(&format!("{}/search.json", self.base_url))
            .map_err(|e| Error::Infrastructure(format!("Open Library URL construction failed: {e}")))?;
        url.query_pairs_mut().append_pair("title", title).append_pair("limit", "10");
        let start = std::time::Instant::now();
        let response: OlSearchResponse = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| Error::Infrastructure(format!("Open Library request failed: {e}")))?
            .json()
            .await
            .map_err(|e| Error::Infrastructure(format!("Open Library response parse failed: {e}")))?;
        tracing::debug!(title, elapsed_ms = start.elapsed().as_millis(), "Open Library title search completed");
        Ok(response.docs)
    }

    /// Scores an `OlSearchDoc` candidate against an extracted title and primary
    /// author. Uses `0.5` as a neutral author score when either side is
    /// missing.
    fn score_search_doc(doc: &OlSearchDoc, title: Option<&str>, author: Option<&str>) -> f32 {
        let t_score = match (title, doc.title.as_deref()) {
            (Some(a), Some(b)) => title_similarity(a, b),
            _ => 0.0,
        };
        let a_score = match (author, doc.author_name.as_ref().and_then(|a| a.first())) {
            (Some(a), Some(b)) => author_similarity(a, b),
            _ => 0.5,
        };
        combined_score(t_score, a_score)
    }

    /// Scores an `OlBookData` (ISBN lookup result) against an extracted title
    /// and primary author.
    fn score_book_data(data: &OlBookData, title: Option<&str>, author: Option<&str>) -> f32 {
        let t_score = match (title, data.title.as_deref()) {
            (Some(a), Some(b)) => title_similarity(a, b),
            _ => 0.0,
        };
        let a_score = match (author, data.authors.as_ref().and_then(|a| a.first())) {
            (Some(a), Some(b)) => author_similarity(a, &b.name),
            _ => 0.5,
        };
        combined_score(t_score, a_score)
    }
}

#[async_trait]
impl MetadataProvider for OpenLibraryAdapter {
    fn name(&self) -> &'static str {
        "Open Library"
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
        let isbn_result = if let Some((isbn_type, isbn_val)) = isbn.clone() {
            self.search_isbn(isbn_type, &isbn_val).await?
        } else {
            None
        };

        // ── Step 2: Validate ISBN result against embedded title/author
        // ────────
        let isbn_accepted = match &isbn_result {
            Some((_, _, data)) => {
                if title.is_some() {
                    let score = Self::score_book_data(data, title, author);
                    if score < CANDIDATE_THRESHOLD {
                        tracing::debug!(score, "Open Library ISBN result score too low; falling back to title search");
                        false
                    } else {
                        true
                    }
                } else {
                    true // no title to validate against
                }
            }
            None => false,
        };

        // ── Step 3: Title search fallback
        // ─────────────────────────────────────
        enum Best {
            FromIsbn(IdentifierType, String, OlBookData),
            FromSearch(OlSearchDoc),
        }

        let best = if isbn_accepted {
            let (isbn_type, isbn_val, data) = isbn_result.unwrap();
            Some(Best::FromIsbn(isbn_type, isbn_val, data))
        } else if let Some(title_str) = title {
            title_search_performed = true;
            let candidates = self.search_title(title_str).await?;
            tracing::debug!(candidates = candidates.len(), "Open Library title search returned candidates");
            candidates
                .into_iter()
                .max_by(|a, b| {
                    let sa = Self::score_search_doc(a, title, author);
                    let sb = Self::score_search_doc(b, title, author);
                    sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(Best::FromSearch)
        } else {
            None
        };

        // ── Log summary and return
        // ─────────────────────────────────────────────
        let isbn_str = isbn.as_ref().map(|(_, v)| v.as_str());
        let final_score = best.as_ref().map(|b| match b {
            Best::FromIsbn(_, _, data) => Self::score_book_data(data, title, author),
            Best::FromSearch(doc) => Self::score_search_doc(doc, title, author),
        });
        tracing::debug!(
            isbn = ?isbn_str,
            title = ?title,
            title_search_performed,
            score = ?final_score,
            "Open Library enrichment completed"
        );

        match best {
            Some(Best::FromIsbn(isbn_type, isbn_val, data)) => {
                let metadata = Self::map_to_extracted(&data, isbn_type, &isbn_val);
                let cover_bytes = self.fetch_cover(&data, &isbn_val).await;
                Ok(Some(ProviderBook {
                    metadata,
                    cover_bytes,
                    source: ImportSource::OpenLibrary,
                }))
            }
            Some(Best::FromSearch(doc)) => {
                let metadata = Self::map_search_doc_to_extracted(&doc);
                let cover_bytes = self.fetch_cover_for_doc(&doc).await;
                Ok(Some(ProviderBook {
                    metadata,
                    cover_bytes,
                    source: ImportSource::OpenLibrary,
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

    fn isbn_response(isbn: &str) -> serde_json::Value {
        serde_json::json!({
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
        })
    }

    fn search_doc(title: &str, author: &str, cover_i: Option<i64>) -> serde_json::Value {
        serde_json::json!({
            "title": title,
            "author_name": [author],
            "isbn": ["9780000000001"],
            "first_publish_year": 2010,
            "cover_i": cover_i
        })
    }

    #[tokio::test]
    async fn enrich_returns_none_when_no_isbn() {
        let adapter = OpenLibraryAdapter::new();
        let result = adapter.enrich(&ExtractedMetadata::default()).await.unwrap();
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
            .respond_with(ResponseTemplate::new(200).set_body_json(isbn_response(isbn)))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path(format!("/b/isbn/{isbn}-L.jpg")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"fake-cover-bytes".to_vec()))
            .mount(&server)
            .await;

        // No title in extracted — ISBN result accepted without validation.
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

    #[tokio::test]
    async fn enrich_accepts_isbn_result_when_title_matches() {
        let server = MockServer::start().await;
        let isbn = "9780765326355";

        Mock::given(method("GET"))
            .and(path("/api/books"))
            .respond_with(ResponseTemplate::new(200).set_body_json(isbn_response(isbn)))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path(format!("/b/isbn/{isbn}-L.jpg")))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"cover".to_vec()))
            .mount(&server)
            .await;

        let extracted = extracted_with_isbn_and_title(isbn, "The Way of Kings", "Brandon Sanderson");
        let adapter = OpenLibraryAdapter::with_base_urls(server.uri(), server.uri());
        let result = adapter.enrich(&extracted).await.unwrap();
        assert_eq!(result.expect("expected ProviderBook").metadata.title.as_deref(), Some("The Way of Kings"));
        // Only one books API request (no title search fallback).
        assert_eq!(
            server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .filter(|r| r.url.path() == "/api/books")
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn enrich_falls_back_to_title_search_when_isbn_is_wrong_book() {
        let server = MockServer::start().await;
        let isbn = "9781250281470";

        // ISBN search returns the wrong book.
        Mock::given(method("GET"))
            .and(path("/api/books"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                format!("ISBN:{isbn}"): {
                    "title": "Some Completely Different Book",
                    "authors": [{"name": "Wrong Author"}],
                    "publishers": [{"name": "Some Publisher"}],
                    "publish_date": "2020",
                    "identifiers": {"isbn_13": [isbn], "openlibrary": ["OL999M"]}
                }
            })))
            .mount(&server)
            .await;

        // Title search returns the correct book.
        Mock::given(method("GET"))
            .and(path("/search.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "numFound": 1,
                "docs": [search_doc("The Correct Book Title", "Right Author", Some(12345_i64))]
            })))
            .mount(&server)
            .await;

        // Cover fetch for the search doc.
        Mock::given(method("GET"))
            .and(path("/b/id/12345-L.jpg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"cover".to_vec()))
            .mount(&server)
            .await;

        let extracted = extracted_with_isbn_and_title(isbn, "The Correct Book Title", "Right Author");
        let adapter = OpenLibraryAdapter::with_base_urls(server.uri(), server.uri());
        let result = adapter.enrich(&extracted).await.unwrap();
        let book = result.expect("expected ProviderBook");

        assert_eq!(book.metadata.title.as_deref(), Some("The Correct Book Title"));
        assert_eq!(
            server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .filter(|r| r.url.path() == "/api/books")
                .count(),
            1
        );
        assert_eq!(
            server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .filter(|r| r.url.path() == "/search.json")
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn enrich_picks_best_candidate_from_title_search() {
        let server = MockServer::start().await;

        // No ISBN — goes straight to title search; returns two candidates.
        Mock::given(method("GET"))
            .and(path("/search.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "numFound": 2,
                "docs": [
                    search_doc("The Hobbit", "Wrong Author", None),
                    search_doc("The Hobbit", "J.R.R. Tolkien", Some(99999_i64))
                ]
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/b/id/99999-L.jpg"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"cover".to_vec()))
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

        let adapter = OpenLibraryAdapter::with_base_urls(server.uri(), server.uri());
        let result = adapter.enrich(&extracted).await.unwrap();
        let book = result.expect("expected ProviderBook");

        // The Tolkien candidate should win.
        let authors = book.metadata.authors.as_ref().expect("authors");
        assert_eq!(authors[0].name, "J.R.R. Tolkien");
    }

    #[tokio::test]
    async fn enrich_returns_err_on_http_5xx() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/books"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let adapter = OpenLibraryAdapter::with_base_urls(server.uri(), server.uri());
        let result = adapter.enrich(&extracted_with_isbn13("9780765326355")).await;
        assert!(result.is_err(), "expected Err on 5xx response, got {result:?}");
    }

    #[tokio::test]
    async fn enrich_returns_err_on_malformed_json() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/books"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;

        let adapter = OpenLibraryAdapter::with_base_urls(server.uri(), server.uri());
        let result = adapter.enrich(&extracted_with_isbn13("9780765326355")).await;
        assert!(result.is_err(), "expected Err on malformed JSON, got {result:?}");
    }

    #[test]
    fn parse_year_delegates_to_bb_utils() {
        // Full coverage lives in bb_utils::date::tests.
        assert_eq!(parse_year("August 31, 2010"), Some(2010));
        assert_eq!(parse_year("no year"), None);
    }
}
