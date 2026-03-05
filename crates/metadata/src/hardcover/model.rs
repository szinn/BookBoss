use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(super) struct GraphQlResponse {
    pub data: GraphQlData,
}

#[derive(Debug, Deserialize)]
pub(super) struct GraphQlData {
    pub search: SearchResult,
}

#[derive(Debug, Deserialize)]
pub(super) struct SearchResult {
    pub results: SearchResults,
}

#[derive(Debug, Deserialize)]
pub(super) struct SearchResults {
    pub found: u32,
    pub hits: Option<Vec<Hit>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Hit {
    pub document: HcBookDocument,
}

#[derive(Debug, Deserialize)]
pub(super) struct HcBookDocument {
    pub id: String,
    pub title: Option<String>,
    pub description: Option<String>,
    /// Parallel to `contributions` — index i gives the role for
    /// contributions[i].
    pub contribution_types: Option<Vec<String>>,
    pub contributions: Option<Vec<HcContribution>>,
    pub image: Option<HcImage>,
    pub isbns: Option<Vec<String>>,
    pub release_year: Option<i32>,
    pub series_names: Option<Vec<String>>,
    /// Present when the book belongs to a series.
    pub featured_series: Option<HcFeaturedSeries>,
}

#[derive(Debug, Deserialize)]
pub(super) struct HcFeaturedSeries {
    /// Series position (book number). Stored as a string like `"1"` or `"1.5"`.
    pub details: Option<String>,
    pub series: Option<HcSeries>,
}

#[derive(Debug, Deserialize)]
pub(super) struct HcSeries {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct HcContribution {
    pub author: Option<HcAuthor>,
}

#[derive(Debug, Deserialize)]
pub(super) struct HcAuthor {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct HcImage {
    pub url: Option<String>,
}
