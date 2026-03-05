mod model;

use async_trait::async_trait;
use bb_core::{
    Error,
    book::{AuthorRole, IdentifierType},
    import::ImportSource,
    pipeline::{ExtractedAuthor, ExtractedIdentifier, ExtractedMetadata, MetadataProvider, ProviderBook},
};
use model::{GraphQlResponse, HcBookDocument};
use rust_decimal::Decimal;
use tracing::warn;

/// Metadata provider backed by the Hardcover GraphQL API.
///
/// Performs ISBN lookup via the `search` query with `query_type: "ISBN"`,
/// maps the first result to [`ProviderBook`], and fetches cover art bytes
/// internally. Returns `None` when no ISBN is available or Hardcover has
/// no matching record.
pub struct HardcoverAdapter {
    client: reqwest::Client,
    api_token: String,
    base_url: String,
}

impl HardcoverAdapter {
    pub fn new(api_token: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_token: api_token.into(),
            base_url: "https://api.hardcover.app/v1/graphql".to_string(),
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
                    let role = types.get(i).map(|t| Self::map_contribution_type(t)).unwrap_or(AuthorRole::Author);
                    Some(ExtractedAuthor {
                        name,
                        role: Some(role),
                        sort_order: i as i32,
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
}

#[async_trait]
impl MetadataProvider for HardcoverAdapter {
    fn name(&self) -> &'static str {
        "Hardcover"
    }

    async fn enrich(&self, extracted: &ExtractedMetadata) -> Result<Option<ProviderBook>, Error> {
        let Some(isbn) = Self::find_isbn(extracted) else {
            return Ok(None);
        };

        let query = format!(r#"query ISBNLookup {{ search(query: "{isbn}", query_type: "ISBN", per_page: 1, page: 1) {{ results }} }}"#);

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

        let Some(hit) = response.data.search.results.hits.and_then(|mut h| h.drain(..).next()) else {
            return Ok(None);
        };

        let doc = &hit.document;
        let metadata = Self::map_to_extracted(doc);
        let cover_bytes = self.fetch_cover(doc).await;

        Ok(Some(ProviderBook {
            metadata,
            cover_bytes,
            source: ImportSource::Hardcover,
        }))
    }
}

#[cfg(test)]
mod tests {
    use bb_core::pipeline::ExtractedIdentifier;
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
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {"search": {"results": {"found": 0, "hits": []}}}
            })))
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

    #[test]
    fn contribution_type_mapping() {
        assert_eq!(HardcoverAdapter::map_contribution_type("Author"), AuthorRole::Author);
        assert_eq!(HardcoverAdapter::map_contribution_type("Editor"), AuthorRole::Editor);
        assert_eq!(HardcoverAdapter::map_contribution_type("Translator"), AuthorRole::Translator);
        assert_eq!(HardcoverAdapter::map_contribution_type("Illustrator"), AuthorRole::Illustrator);
        assert_eq!(HardcoverAdapter::map_contribution_type("Narrator"), AuthorRole::Author);
    }
}
