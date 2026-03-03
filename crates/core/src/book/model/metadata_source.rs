/// Canonical source for a book's metadata, used to decide whether
/// and where to re-fetch.
///
/// Distinct from `import::ImportSource`, which records which provider
/// was used during the import pipeline.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MetadataSource {
    Hardcover,
    OpenLibrary,
    /// Metadata was hand-entered or edited by an admin. Do not auto-re-fetch.
    Manual,
}
