mod model;

use async_trait::async_trait;
use bb_core::{
    Error,
    book::{AuthorRole, IdentifierType},
    import::ImportSource,
    metadata::MetadataProvider,
    pipeline::{ExtractedAuthor, ExtractedIdentifier, ExtractedMetadata, ProviderBook},
};
use bb_utils::similarity::{author_similarity, combined_score, title_similarity};
use model::{GraphQlResponse, HcBookDocument};
use rust_decimal::Decimal;
use tracing::warn;

/// Minimum combined similarity score for an ISBN search result to be accepted
/// without falling back to a title search.
const CANDIDATE_THRESHOLD: f32 = 0.5;

/// Metadata provider backed by the Hardcover GraphQL API.
///
/// Tries ISBN lookup first. If the result's title/author score falls below
/// [`CANDIDATE_THRESHOLD`], or no ISBN is available, falls back to a title
/// search returning up to 10 candidates which are scored and the best
/// is returned. Returns `None` when neither strategy finds a result.
pub struct HardcoverAdapter {
    client: reqwest::Client,
    api_token: String,
    base_url: String,
}

impl HardcoverAdapter {
    pub fn new(api_token: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(crate::REQUEST_TIMEOUT)
                .build()
                .expect("failed to build reqwest client"),
            api_token: api_token.into(),
            base_url: "https://api.hardcover.app/v1/graphql".to_string(),
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

    fn map_contribution_type(s: &str) -> AuthorRole {
        match s {
            "Editor" => AuthorRole::Editor,
            "Translator" => AuthorRole::Translator,
            "Illustrator" => AuthorRole::Illustrator,
            _ => AuthorRole::Author,
        }
    }

    fn map_to_extracted(doc: &HcBookDocument) -> ExtractedMetadata {
        // Authors: zip contributions with contribution_types (parallel arrays).
        let authors = doc.contributions.as_ref().map(|contribs| {
            let types = doc.contribution_types.as_deref().unwrap_or(&[]);
            contribs
                .iter()
                .enumerate()
                .filter_map(|(i, c)| {
                    let name = c.author.as_ref()?.name.clone();
                    let role = types.get(i).map_or(AuthorRole::Author, |t| Self::map_contribution_type(t));
                    #[expect(
                        clippy::cast_possible_truncation,
                        clippy::cast_possible_wrap,
                        reason = "author list index; books have far fewer authors than i32::MAX"
                    )]
                    let sort_order = i as i32;
                    Some(ExtractedAuthor {
                        name,
                        role: Some(role),
                        sort_order,
                    })
                })
                .collect()
        });

        // ISBNs: classify by string length.
        let identifiers = {
            let mut ids: Vec<ExtractedIdentifier> = doc
                .isbns
                .as_deref()
                .unwrap_or(&[])
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
                .collect();

            // Hardcover's own ID as an identifier.
            ids.push(ExtractedIdentifier {
                identifier_type: IdentifierType::Hardcover,
                value: doc.id.clone(),
            });
            Some(ids)
        };

        // Series name: prefer featured_series.series.name, fall back to
        // series_names[0].
        let series_name = doc
            .featured_series
            .as_ref()
            .and_then(|fs| fs.series.as_ref())
            .map(|s| s.name.clone())
            .or_else(|| doc.series_names.as_ref().and_then(|names| names.first()).cloned());

        // Series number: featured_series.details is a string like "1" or "1.5".
        let series_number = doc
            .featured_series
            .as_ref()
            .and_then(|fs| fs.details.as_deref())
            .and_then(|d| d.parse::<Decimal>().ok());

        ExtractedMetadata {
            title: doc.title.clone(),
            authors,
            description: doc.description.clone(),
            publisher: None, // not available in search results
            published_date: doc.release_year,
            language: None,
            identifiers,
            series_name,
            series_number,
            genres: vec![],
            tags: vec![],
            page_count: None,
            has_spinnaker_metadata: false,
            cover_bytes: None,
        }
    }

    async fn fetch_cover(&self, doc: &HcBookDocument) -> Option<Vec<u8>> {
        let url = doc.image.as_ref().and_then(|img| img.url.as_ref())?;

        match self.client.get(url).send().await {
            Ok(response) if response.status().is_success() => match response.bytes().await {
                Ok(bytes) => Some(bytes.to_vec()),
                Err(e) => {
                    warn!("Failed to read Hardcover cover bytes: {e}");
                    None
                }
            },
            Ok(response) => {
                warn!(status = %response.status(), url = %url, "Hardcover cover not available");
                None
            }
            Err(e) => {
                warn!("Failed to fetch Hardcover cover: {e}");
                None
            }
        }
    }

    /// Searches Hardcover by ISBN; returns the first matching document or
    /// `None`.
    async fn search_isbn(&self, isbn: &str) -> Result<Option<HcBookDocument>, Error> {
        let query = format!(r#"query ISBNLookup {{ search(query: "{isbn}", query_type: "ISBN", per_page: 1, page: 1) {{ results }} }}"#);
        let start = std::time::Instant::now();
        let response: GraphQlResponse = self
            .client
            .post(&self.base_url)
            .bearer_auth(&self.api_token)
            .json(&serde_json::json!({ "query": query }))
            .send()
            .await
            .map_err(|e| Error::Infrastructure(format!("Hardcover request failed: {e}")))?
            .json()
            .await
            .map_err(|e| Error::Infrastructure(format!("Hardcover response parse failed: {e}")))?;
        tracing::debug!(isbn, elapsed_ms = start.elapsed().as_millis(), "Hardcover ISBN search completed");
        Ok(response.data.search.results.hits.and_then(|mut h| h.drain(..).next()).map(|h| h.document))
    }

    /// Searches Hardcover by title; returns up to 10 candidate documents.
    async fn search_title(&self, title: &str) -> Result<Vec<HcBookDocument>, Error> {
        let escaped = title.replace('\\', "\\\\").replace('"', "\\\"");
        let query = format!(r#"query TitleLookup {{ search(query: "{escaped}", query_type: "DEFAULT", per_page: 10, page: 1) {{ results }} }}"#);
        let start = std::time::Instant::now();
        let response: GraphQlResponse = self
            .client
            .post(&self.base_url)
            .bearer_auth(&self.api_token)
            .json(&serde_json::json!({ "query": query }))
            .send()
            .await
            .map_err(|e| Error::Infrastructure(format!("Hardcover request failed: {e}")))?
            .json()
            .await
            .map_err(|e| Error::Infrastructure(format!("Hardcover response parse failed: {e}")))?;
        tracing::debug!(title, elapsed_ms = start.elapsed().as_millis(), "Hardcover title search completed");
        Ok(response.data.search.results.hits.unwrap_or_default().into_iter().map(|h| h.document).collect())
    }

    /// Scores a candidate document against an extracted title and primary
    /// author. Uses `0.5` as a neutral author score when either side is
    /// missing.
    fn score_candidate(doc: &HcBookDocument, title: Option<&str>, author: Option<&str>) -> f32 {
        let t_score = match (title, doc.title.as_deref()) {
            (Some(a), Some(b)) => title_similarity(a, b),
            _ => 0.0,
        };
        let a_score = match (author, doc.contributions.as_ref().and_then(|c| c.first()).and_then(|c| c.author.as_ref())) {
            (Some(a), Some(b)) => author_similarity(a, &b.name),
            _ => 0.5,
        };
        combined_score(t_score, a_score)
    }
}

#[async_trait]
impl MetadataProvider for HardcoverAdapter {
    fn name(&self) -> &'static str {
        "Hardcover"
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
        let isbn_doc = if let Some(ref isbn_str) = isbn {
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
        let best_doc = match (isbn_doc, title) {
            (Some(doc), Some(t)) => {
                let score = Self::score_candidate(&doc, Some(t), author);
                if score >= CANDIDATE_THRESHOLD {
                    Some(doc)
                } else {
                    tracing::debug!(score, "Hardcover ISBN result score too low; falling back to title search");
                    None
                }
            }
            (Some(doc), None) => Some(doc),
            (None, _) => None,
        };

        // ── Step 3: Title search fallback
        // ─────────────────────────────────────
        let best_doc = if best_doc.is_none() {
            if let Some(title_str) = title {
                title_search_performed = true;
                let candidates = self.search_title(title_str).await?;
                tracing::debug!(candidates = candidates.len(), "Hardcover title search returned candidates");
                candidates.into_iter().max_by(|a, b| {
                    let sa = Self::score_candidate(a, title, author);
                    let sb = Self::score_candidate(b, title, author);
                    sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
                })
            } else {
                None
            }
        } else {
            best_doc
        };

        // ── Log summary and return
        // ─────────────────────────────────────────────
        let final_score = best_doc.as_ref().map(|doc| Self::score_candidate(doc, title, author));
        tracing::debug!(
            isbn = ?isbn,
            title = ?title,
            title_search_performed,
            score = ?final_score,
            "Hardcover enrichment completed"
        );

        match best_doc {
            Some(doc) => {
                let metadata = Self::map_to_extracted(&doc);
                let cover_bytes = self.fetch_cover(&doc).await;
                Ok(Some(ProviderBook {
                    metadata,
                    cover_bytes,
                    source: ImportSource::Hardcover,
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
        matchers::{header, method, path},
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

    fn book_document(id: &str, title: &str, author: &str, isbn: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "title": title,
            "description": "A great book",
            "author_names": [author],
            "contribution_types": ["Author"],
            "contributions": [{"author": {"name": author}}],
            "image": {"url": null},
            "isbns": [isbn],
            "release_year": 2010,
            "series_names": [],
            "featured_series": null
        })
    }

    fn search_response_for(doc: &serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "data": {
                "search": {
                    "results": {
                        "found": 1,
                        "hits": [{"document": doc}]
                    }
                }
            }
        })
    }

    fn search_response(isbn: &str) -> serde_json::Value {
        serde_json::json!({
            "data": {
                "search": {
                    "results": {
                        "found": 1,
                        "hits": [{
                            "document": {
                                "id": "12345",
                                "title": "The Way of Kings",
                                "description": "A great book",
                                "author_names": ["Brandon Sanderson"],
                                "contribution_types": ["Author"],
                                "contributions": [{"author": {"name": "Brandon Sanderson"}}],
                                "image": {"url": null},
                                "isbns": [isbn],
                                "release_year": 2010,
                                "series_names": ["The Stormlight Archive"],
                                "featured_series": {
                                    "details": "1",
                                    "position": 1,
                                    "series": {"name": "The Stormlight Archive"}
                                }
                            }
                        }]
                    }
                }
            }
        })
    }

    fn empty_search_response() -> serde_json::Value {
        serde_json::json!({
            "data": {"search": {"results": {"found": 0, "hits": []}}}
        })
    }

    #[tokio::test]
    async fn enrich_returns_none_when_no_isbn() {
        let adapter = HardcoverAdapter::new("token");
        let result = adapter.enrich(&ExtractedMetadata::default()).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn enrich_returns_none_when_not_found() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(empty_search_response()))
            .mount(&server)
            .await;

        let adapter = HardcoverAdapter::with_base_url("token", server.uri());
        let result = adapter.enrich(&extracted_with_isbn13("9780765326355")).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn enrich_maps_book_with_series() {
        let server = MockServer::start().await;
        let isbn = "9780765326355";

        Mock::given(method("POST"))
            .and(path("/"))
            .and(header("authorization", "Bearer token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(search_response(isbn)))
            .mount(&server)
            .await;

        // No title in extracted — ISBN result accepted without validation.
        let adapter = HardcoverAdapter::with_base_url("token", server.uri());
        let result = adapter.enrich(&extracted_with_isbn13(isbn)).await.unwrap();
        let book = result.expect("expected ProviderBook");

        assert_eq!(book.metadata.title.as_deref(), Some("The Way of Kings"));
        assert_eq!(book.metadata.published_date, Some(2010));
        assert_eq!(book.metadata.series_name.as_deref(), Some("The Stormlight Archive"));
        assert_eq!(book.metadata.series_number, Some(rust_decimal::Decimal::ONE));
        assert_eq!(book.source, ImportSource::Hardcover);

        let authors = book.metadata.authors.as_ref().expect("expected authors");
        assert_eq!(authors.len(), 1);
        assert_eq!(authors[0].name, "Brandon Sanderson");
        assert_eq!(authors[0].role, Some(AuthorRole::Author));

        let identifiers = book.metadata.identifiers.as_ref().expect("expected identifiers");
        assert!(identifiers.iter().any(|id| id.identifier_type == IdentifierType::Isbn13 && id.value == isbn));
        assert!(
            identifiers
                .iter()
                .any(|id| id.identifier_type == IdentifierType::Hardcover && id.value == "12345")
        );
    }

    #[tokio::test]
    async fn enrich_accepts_isbn_result_when_title_matches() {
        let server = MockServer::start().await;
        let isbn = "9780765326355";

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(search_response(isbn)))
            .mount(&server)
            .await;

        let extracted = extracted_with_isbn_and_title(isbn, "The Way of Kings", "Brandon Sanderson");
        let adapter = HardcoverAdapter::with_base_url("token", server.uri());
        let result = adapter.enrich(&extracted).await.unwrap();
        let book = result.expect("expected ProviderBook");

        assert_eq!(book.metadata.title.as_deref(), Some("The Way of Kings"));
        // Only one request should have been made (no title search fallback).
        assert_eq!(server.received_requests().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn enrich_falls_back_to_title_search_when_isbn_is_wrong_book() {
        let server = MockServer::start().await;
        let isbn = "9781250281470";

        // ISBN search returns the wrong book.
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(search_response_for(&book_document(
                "99999",
                "Some Completely Different Book",
                "Wrong Author",
                isbn,
            ))))
            .up_to_n_times(1)
            .mount(&server)
            .await;

        // Title search returns the correct book.
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(search_response_for(&book_document(
                "11111",
                "The Correct Book Title",
                "Right Author",
                "9780000000001",
            ))))
            .mount(&server)
            .await;

        let extracted = extracted_with_isbn_and_title(isbn, "The Correct Book Title", "Right Author");
        let adapter = HardcoverAdapter::with_base_url("token", server.uri());
        let result = adapter.enrich(&extracted).await.unwrap();
        let book = result.expect("expected ProviderBook");

        assert_eq!(book.metadata.title.as_deref(), Some("The Correct Book Title"));
        // Two requests should have been made: ISBN search + title search
        // fallback.
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn enrich_picks_best_candidate_from_title_search() {
        let server = MockServer::start().await;

        // No ISBN — goes straight to title search; returns two candidates.
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "search": {
                        "results": {
                            "found": 2,
                            "hits": [
                                {"document": book_document("1", "The Hobbit", "Wrong Author", "9780000000001")},
                                {"document": book_document("2", "The Hobbit", "J.R.R. Tolkien", "9780000000002")}
                            ]
                        }
                    }
                }
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

        let adapter = HardcoverAdapter::with_base_url("token", server.uri());
        let result = adapter.enrich(&extracted).await.unwrap();
        let book = result.expect("expected ProviderBook");

        // The candidate with the matching author should win.
        let identifiers = book.metadata.identifiers.as_ref().expect("expected identifiers");
        assert!(identifiers.iter().any(|id| id.identifier_type == IdentifierType::Hardcover && id.value == "2"));
    }

    #[tokio::test]
    async fn enrich_returns_err_on_http_5xx() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let adapter = HardcoverAdapter::with_base_url("token", server.uri());
        let result = adapter.enrich(&extracted_with_isbn13("9780765326355")).await;
        assert!(result.is_err(), "expected Err on 5xx response, got {result:?}");
    }

    #[tokio::test]
    async fn enrich_returns_err_on_malformed_json() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;

        let adapter = HardcoverAdapter::with_base_url("token", server.uri());
        let result = adapter.enrich(&extracted_with_isbn13("9780765326355")).await;
        assert!(result.is_err(), "expected Err on malformed JSON, got {result:?}");
    }

    #[test]
    fn contribution_type_mapping() {
        assert_eq!(HardcoverAdapter::map_contribution_type("Author"), AuthorRole::Author);
        assert_eq!(HardcoverAdapter::map_contribution_type("Editor"), AuthorRole::Editor);
        assert_eq!(HardcoverAdapter::map_contribution_type("Translator"), AuthorRole::Translator);
        assert_eq!(HardcoverAdapter::map_contribution_type("Illustrator"), AuthorRole::Illustrator);
        assert_eq!(HardcoverAdapter::map_contribution_type("Narrator"), AuthorRole::Author);
    }
}
