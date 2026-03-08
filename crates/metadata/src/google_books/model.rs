use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct VolumeList {
    pub items: Option<Vec<Volume>>,
}

#[derive(Debug, Deserialize)]
pub struct Volume {
    pub id: String,
    #[serde(rename = "volumeInfo")]
    pub volume_info: VolumeInfo,
}

#[derive(Debug, Deserialize)]
pub struct VolumeInfo {
    pub title: Option<String>,
    pub authors: Option<Vec<String>>,
    pub publisher: Option<String>,
    #[serde(rename = "publishedDate")]
    pub published_date: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "industryIdentifiers")]
    pub industry_identifiers: Option<Vec<IndustryIdentifier>>,
    pub language: Option<String>,
    #[serde(rename = "imageLinks")]
    pub image_links: Option<ImageLinks>,
}

#[derive(Debug, Deserialize)]
pub struct IndustryIdentifier {
    #[serde(rename = "type")]
    pub id_type: String,
    pub identifier: String,
}

#[derive(Debug, Deserialize)]
pub struct ImageLinks {
    pub thumbnail: Option<String>,
    #[serde(rename = "smallThumbnail")]
    pub small_thumbnail: Option<String>,
}
