use std::collections::HashMap;

use bb_core::{
    book::{AuthorRole, BookStatus, IdentifierType, MetadataSource},
    pipeline::{ExtractedAuthor, ExtractedIdentifier, ExtractedMetadata},
    storage::{BookSidecar, SidecarAuthor, SidecarFile, SidecarIdentifier, SidecarSeries},
};
use quick_xml::{
    NsReader,
    events::Event,
    name::{Namespace, ResolveResult},
};
use serde::Deserialize;

use crate::Error;

const DC_NS: &[u8] = b"http://purl.org/dc/elements/1.1/";

// ── intermediate raw DC state
// ─────────────────────────────────────────────────

struct RawAuthor {
    name: String,
    role_code: Option<String>,
    file_as: Option<String>,
}

struct RawIdentifier {
    scheme: Option<String>,
    value: String,
}

#[derive(Default)]
struct DcFields {
    title: Option<String>,
    authors: Vec<RawAuthor>,
    description: Option<String>,
    publisher: Option<String>,
    published_date: Option<String>,
    language: Option<String>,
    identifiers: Vec<RawIdentifier>,
    bb_meta_content: Option<String>,
}

enum ParseState {
    Other,
    InTitle,
    InCreator { role: Option<String>, file_as: Option<String> },
    InDescription,
    InPublisher,
    InDate,
    InLanguage,
    InIdentifier { scheme: Option<String> },
}

// ── bookboss:metadata JSON structs
// ────────────────────────────────────────────

#[derive(Deserialize)]
struct AuthorSortOrderJson {
    name: String,
    sort_order: i32,
}

#[derive(Deserialize)]
struct BbMetaJson {
    #[serde(default)]
    series: Option<SidecarSeries>,
    #[serde(default)]
    genres: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    author_sort_orders: Vec<AuthorSortOrderJson>,
    #[serde(default)]
    rating: Option<i16>,
    status: BookStatus,
    #[serde(default)]
    metadata_source: Option<MetadataSource>,
    #[serde(default)]
    files: Vec<SidecarFile>,
}

// ── MARC / scheme helpers
// ─────────────────────────────────────────────────────

fn marc_to_author_role(code: &str) -> AuthorRole {
    match code {
        "aut" => AuthorRole::Author,
        "edt" => AuthorRole::Editor,
        "trl" => AuthorRole::Translator,
        "ill" => AuthorRole::Illustrator,
        _ => AuthorRole::Author,
    }
}

fn scheme_to_identifier_type(scheme: &str, value: &str) -> IdentifierType {
    match scheme {
        "ISBN" => {
            if value.len() == 10 {
                IdentifierType::Isbn10
            } else {
                IdentifierType::Isbn13
            }
        }
        "ASIN" => IdentifierType::Asin,
        "GoogleBooks" => IdentifierType::GoogleBooks,
        "OpenLibrary" => IdentifierType::OpenLibrary,
        "Hardcover" => IdentifierType::Hardcover,
        _ => IdentifierType::Isbn13,
    }
}

// ── core DC parser
// ────────────────────────────────────────────────────────────

fn parse_dc(xml: &[u8]) -> Result<DcFields, Error> {
    let mut reader = NsReader::from_reader(xml);
    reader.config_mut().trim_text(true);

    let mut fields = DcFields::default();
    let mut state = ParseState::Other;
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_resolved_event_into(&mut buf)? {
            (ResolveResult::Bound(ns), Event::Start(ref e)) if ns == Namespace(DC_NS) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"title" => state = ParseState::InTitle,
                    b"creator" => {
                        let mut role = None;
                        let mut file_as = None;
                        for attr in e.attributes() {
                            let attr = attr.map_err(quick_xml::Error::from)?;
                            match attr.key.as_ref() {
                                b"opf:role" => {
                                    role = Some(attr.decode_and_unescape_value(reader.decoder())?.into_owned());
                                }
                                b"opf:file-as" => {
                                    file_as = Some(attr.decode_and_unescape_value(reader.decoder())?.into_owned());
                                }
                                _ => {}
                            }
                        }
                        state = ParseState::InCreator { role, file_as };
                    }
                    b"description" => state = ParseState::InDescription,
                    b"publisher" => state = ParseState::InPublisher,
                    b"date" => state = ParseState::InDate,
                    b"language" => state = ParseState::InLanguage,
                    b"identifier" => {
                        let mut scheme = None;
                        for attr in e.attributes() {
                            let attr = attr.map_err(quick_xml::Error::from)?;
                            if attr.key.as_ref() == b"opf:scheme" {
                                scheme = Some(attr.decode_and_unescape_value(reader.decoder())?.into_owned());
                            }
                        }
                        state = ParseState::InIdentifier { scheme };
                    }
                    _ => {}
                }
            }
            (_, Event::Empty(ref e)) if e.local_name().as_ref() == b"meta" => {
                let mut is_bb = false;
                let mut content = None;
                for attr in e.attributes() {
                    let attr = attr.map_err(quick_xml::Error::from)?;
                    match attr.key.as_ref() {
                        b"name" => {
                            let val = attr.decode_and_unescape_value(reader.decoder())?;
                            if val.as_ref() == "bookboss:metadata" {
                                is_bb = true;
                            }
                        }
                        b"content" => {
                            content = Some(attr.decode_and_unescape_value(reader.decoder())?.into_owned());
                        }
                        _ => {}
                    }
                }
                if is_bb {
                    fields.bb_meta_content = content;
                }
            }
            (_, Event::Text(ref t)) => {
                let text = t.unescape()?.into_owned();
                match std::mem::replace(&mut state, ParseState::Other) {
                    ParseState::InTitle => fields.title = Some(text),
                    ParseState::InCreator { role, file_as } => {
                        fields.authors.push(RawAuthor {
                            name: text,
                            role_code: role,
                            file_as,
                        });
                    }
                    ParseState::InDescription => fields.description = Some(text),
                    ParseState::InPublisher => fields.publisher = Some(text),
                    ParseState::InDate => fields.published_date = Some(text),
                    ParseState::InLanguage => fields.language = Some(text),
                    ParseState::InIdentifier { scheme } => {
                        fields.identifiers.push(RawIdentifier { scheme, value: text });
                    }
                    ParseState::Other => {}
                }
            }
            (_, Event::End(_)) => {
                state = ParseState::Other;
            }
            (_, Event::Eof) => break,
            _ => {}
        }
    }

    Ok(fields)
}

// ── public API
// ────────────────────────────────────────────────────────────────

/// Parse a BookBoss `metadata.opf` sidecar back into a [`BookSidecar`].
pub fn parse_sidecar(xml: &[u8]) -> Result<BookSidecar, Error> {
    let fields = parse_dc(xml)?;

    let bb: BbMetaJson = fields
        .bb_meta_content
        .as_deref()
        .map(serde_json::from_str)
        .transpose()?
        .ok_or(Error::MissingField("bookboss:metadata"))?;

    // Build a name → sort_order lookup from the JSON blob.
    let sort_order_map: HashMap<&str, i32> = bb.author_sort_orders.iter().map(|a| (a.name.as_str(), a.sort_order)).collect();

    let authors: Vec<SidecarAuthor> = fields
        .authors
        .into_iter()
        .enumerate()
        .map(|(i, raw)| {
            let sort_order = sort_order_map.get(raw.name.as_str()).copied().unwrap_or(i as i32);
            let role = raw.role_code.as_deref().map(marc_to_author_role).unwrap_or(AuthorRole::Author);
            SidecarAuthor {
                name: raw.name,
                role,
                sort_order,
                file_as: raw.file_as,
            }
        })
        .collect();

    let identifiers: Vec<SidecarIdentifier> = fields
        .identifiers
        .into_iter()
        .map(|raw| {
            let scheme = raw.scheme.as_deref().unwrap_or("");
            let id_type = scheme_to_identifier_type(scheme, &raw.value);
            SidecarIdentifier {
                identifier_type: id_type,
                value: raw.value,
            }
        })
        .collect();

    Ok(BookSidecar {
        title: fields.title.ok_or(Error::MissingField("dc:title"))?,
        authors,
        description: fields.description,
        publisher: fields.publisher,
        published_date: fields.published_date.as_deref().and_then(|s| s.parse().ok()),
        language: fields.language,
        identifiers,
        series: bb.series,
        genres: bb.genres,
        tags: bb.tags,
        rating: bb.rating,
        status: bb.status,
        metadata_source: bb.metadata_source,
        files: bb.files,
    })
}

/// Extract metadata from an OPF document (e.g. embedded in an EPUB).
///
/// Only reads Dublin Core fields; ignores the `bookboss:metadata` extension.
pub fn extract_metadata(xml: &[u8]) -> Result<ExtractedMetadata, Error> {
    let fields = parse_dc(xml)?;

    let authors: Vec<ExtractedAuthor> = fields
        .authors
        .into_iter()
        .enumerate()
        .map(|(i, raw)| ExtractedAuthor {
            name: raw.name,
            role: raw.role_code.as_deref().map(marc_to_author_role),
            sort_order: i as i32,
        })
        .collect();

    let identifiers: Vec<ExtractedIdentifier> = fields
        .identifiers
        .into_iter()
        .map(|raw| {
            let scheme = raw.scheme.as_deref().unwrap_or("");
            let id_type = scheme_to_identifier_type(scheme, &raw.value);
            ExtractedIdentifier {
                identifier_type: id_type,
                value: raw.value,
            }
        })
        .collect();

    Ok(ExtractedMetadata {
        title: fields.title,
        authors: if authors.is_empty() { None } else { Some(authors) },
        description: fields.description,
        publisher: fields.publisher,
        published_date: fields.published_date.as_deref().and_then(|s| s.parse().ok()),
        language: fields.language,
        identifiers: if identifiers.is_empty() { None } else { Some(identifiers) },
        series_name: None,
        series_number: None,
    })
}

#[cfg(test)]
mod tests {
    use bb_core::book::{AuthorRole, BookStatus, IdentifierType};

    use super::{extract_metadata, parse_sidecar};
    use crate::opf::write::tests::full_test_sidecar;

    #[test]
    fn extract_metadata_from_opf_snippet() {
        let opf = br#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"
            xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>Dune</dc:title>
    <dc:creator opf:role="aut" opf:file-as="Herbert, Frank">Frank Herbert</dc:creator>
    <dc:description>A science fiction epic.</dc:description>
    <dc:publisher>Chilton Books</dc:publisher>
    <dc:date>1965</dc:date>
    <dc:language>en</dc:language>
    <dc:identifier opf:scheme="ISBN">9780441013593</dc:identifier>
  </metadata>
  <manifest/>
  <spine/>
</package>"#;

        let meta = extract_metadata(opf).expect("parse failed");

        assert_eq!(meta.title.as_deref(), Some("Dune"));
        let authors = meta.authors.as_ref().unwrap();
        assert_eq!(authors.len(), 1);
        assert_eq!(authors[0].name, "Frank Herbert");
        assert_eq!(authors[0].role, Some(AuthorRole::Author));
        assert_eq!(meta.description.as_deref(), Some("A science fiction epic."));
        assert_eq!(meta.publisher.as_deref(), Some("Chilton Books"));
        assert_eq!(meta.published_date, Some(1965));
        assert_eq!(meta.language.as_deref(), Some("en"));
        let ids = meta.identifiers.as_ref().unwrap();
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0].identifier_type, IdentifierType::Isbn13);
        assert_eq!(ids[0].value, "9780441013593");
    }

    #[test]
    fn isbn10_vs_isbn13() {
        let opf_isbn10 = br#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"
            xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>Test</dc:title>
    <dc:identifier opf:scheme="ISBN">0441013597</dc:identifier>
  </metadata>
  <manifest/>
  <spine/>
</package>"#;

        let opf_isbn13 = br#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"
            xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>Test</dc:title>
    <dc:identifier opf:scheme="ISBN">9780441013593</dc:identifier>
  </metadata>
  <manifest/>
  <spine/>
</package>"#;

        let meta10 = extract_metadata(opf_isbn10).expect("parse failed");
        let id10 = &meta10.identifiers.as_ref().unwrap()[0];
        assert_eq!(id10.identifier_type, IdentifierType::Isbn10);

        let meta13 = extract_metadata(opf_isbn13).expect("parse failed");
        let id13 = &meta13.identifiers.as_ref().unwrap()[0];
        assert_eq!(id13.identifier_type, IdentifierType::Isbn13);
    }

    #[test]
    fn parse_sidecar_roundtrip() {
        use crate::opf::write_sidecar;
        let original = full_test_sidecar();
        let bytes = write_sidecar(&original).expect("write failed");
        let parsed = parse_sidecar(&bytes).expect("parse failed");
        assert_eq!(parsed.title, original.title);
        assert_eq!(parsed.status, BookStatus::Available);
    }
}
