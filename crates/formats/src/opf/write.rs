use std::io::Cursor;

use bb_core::{
    book::{BookStatus, MetadataSource},
    storage::{BookSidecar, SidecarAuthor, SidecarFile, SidecarSeries},
};
use quick_xml::{
    Writer,
    events::{BytesEnd, BytesStart, BytesText, Event},
};
use serde::Serialize;

use crate::Error;

#[derive(Serialize)]
struct AuthorSortOrder {
    name: String,
    sort_order: i32,
}

#[derive(Serialize)]
struct BbMeta<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    series: Option<&'a SidecarSeries>,
    genres: &'a Vec<String>,
    tags: &'a Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_count: Option<i32>,
    author_sort_orders: Vec<AuthorSortOrder>,
    status: &'a BookStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata_source: Option<&'a MetadataSource>,
    files: &'a Vec<SidecarFile>,
}

/// Serialises only the `<metadata>…</metadata>` block for the given sidecar.
/// Used both by `write_sidecar` (for standalone `.opf` files) and by
/// `enrich_epub` (to splice updated metadata into an existing EPUB OPF).
pub(crate) fn write_metadata_xml(sidecar: &BookSidecar) -> Result<Vec<u8>, Error> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    let mut meta_elem = BytesStart::new("metadata");
    meta_elem.push_attribute(("xmlns:dc", "http://purl.org/dc/elements/1.1/"));
    meta_elem.push_attribute(("xmlns:opf", "http://www.idpf.org/2007/opf"));
    writer.write_event(Event::Start(meta_elem))?;

    // dc:title
    writer.write_event(Event::Start(BytesStart::new("dc:title")))?;
    writer.write_event(Event::Text(BytesText::new(&sidecar.title)))?;
    writer.write_event(Event::End(BytesEnd::new("dc:title")))?;

    // Authors sorted by sort_order
    let mut sorted_authors: Vec<&SidecarAuthor> = sidecar.authors.iter().collect();
    sorted_authors.sort_by_key(|a| a.sort_order);
    for author in &sorted_authors {
        let mut creator = BytesStart::new("dc:creator");
        creator.push_attribute(("opf:role", author.role.marc_relator()));
        if let Some(file_as) = &author.file_as {
            creator.push_attribute(("opf:file-as", file_as.as_str()));
        }
        writer.write_event(Event::Start(creator))?;
        writer.write_event(Event::Text(BytesText::new(&author.name)))?;
        writer.write_event(Event::End(BytesEnd::new("dc:creator")))?;
    }

    if let Some(desc) = &sidecar.description {
        writer.write_event(Event::Start(BytesStart::new("dc:description")))?;
        writer.write_event(Event::Text(BytesText::new(desc)))?;
        writer.write_event(Event::End(BytesEnd::new("dc:description")))?;
    }

    if let Some(publisher) = &sidecar.publisher {
        writer.write_event(Event::Start(BytesStart::new("dc:publisher")))?;
        writer.write_event(Event::Text(BytesText::new(publisher)))?;
        writer.write_event(Event::End(BytesEnd::new("dc:publisher")))?;
    }

    if let Some(date) = sidecar.published_date {
        writer.write_event(Event::Start(BytesStart::new("dc:date")))?;
        writer.write_event(Event::Text(BytesText::new(&date.to_string())))?;
        writer.write_event(Event::End(BytesEnd::new("dc:date")))?;
    }

    if let Some(lang) = &sidecar.language {
        writer.write_event(Event::Start(BytesStart::new("dc:language")))?;
        writer.write_event(Event::Text(BytesText::new(lang)))?;
        writer.write_event(Event::End(BytesEnd::new("dc:language")))?;
    }

    for genre in &sidecar.genres {
        writer.write_event(Event::Start(BytesStart::new("dc:subject")))?;
        writer.write_event(Event::Text(BytesText::new(genre)))?;
        writer.write_event(Event::End(BytesEnd::new("dc:subject")))?;
    }

    for identifier in &sidecar.identifiers {
        let mut id_elem = BytesStart::new("dc:identifier");
        id_elem.push_attribute(("opf:scheme", identifier.identifier_type.opf_scheme()));
        writer.write_event(Event::Start(id_elem))?;
        writer.write_event(Event::Text(BytesText::new(&identifier.value)))?;
        writer.write_event(Event::End(BytesEnd::new("dc:identifier")))?;
    }

    if let Some(series) = &sidecar.series {
        let mut meta_series = BytesStart::new("meta");
        meta_series.push_attribute(("name", "calibre:series"));
        meta_series.push_attribute(("content", series.name.as_str()));
        writer.write_event(Event::Empty(meta_series))?;

        if let Some(number) = series.number {
            let mut meta_index = BytesStart::new("meta");
            meta_index.push_attribute(("name", "calibre:series_index"));
            meta_index.push_attribute(("content", number.to_string().as_str()));
            writer.write_event(Event::Empty(meta_index))?;
        }
    }

    // spinnaker:metadata JSON blob
    let bb_meta = BbMeta {
        series: sidecar.series.as_ref(),
        genres: &sidecar.genres,
        tags: &sidecar.tags,
        page_count: sidecar.page_count,
        author_sort_orders: sorted_authors
            .iter()
            .map(|a| AuthorSortOrder {
                name: a.name.clone(),
                sort_order: a.sort_order,
            })
            .collect(),
        status: &sidecar.status,
        metadata_source: sidecar.metadata_source.as_ref(),
        files: &sidecar.files,
    };
    let json = serde_json::to_string(&bb_meta)?;
    let mut meta_bb = BytesStart::new("meta");
    meta_bb.push_attribute(("name", "spinnaker:metadata"));
    meta_bb.push_attribute(("content", json.as_str()));
    writer.write_event(Event::Empty(meta_bb))?;

    writer.write_event(Event::End(BytesEnd::new("metadata")))?;

    Ok(writer.into_inner().into_inner())
}

pub fn write_sidecar(sidecar: &BookSidecar) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    out.extend_from_slice(
        b"<?xml version=\"1.0\" encoding=\"utf-8\"?>\
          <package xmlns=\"http://www.idpf.org/2007/opf\" version=\"2.0\">",
    );
    out.extend(write_metadata_xml(sidecar)?);
    out.extend_from_slice(b"<manifest/><spine/></package>");
    Ok(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use bb_core::{
        book::{AuthorRole, BookStatus, FileFormat, IdentifierType, MetadataSource},
        storage::{BookSidecar, SidecarAuthor, SidecarFile, SidecarIdentifier, SidecarSeries},
    };
    use rust_decimal::Decimal;

    use super::write_sidecar;
    use crate::opf::parse_sidecar;

    pub fn full_test_sidecar() -> BookSidecar {
        BookSidecar {
            title: "The Way of Kings".to_string(),
            authors: vec![
                SidecarAuthor {
                    name: "Brandon Sanderson".to_string(),
                    role: AuthorRole::Author,
                    sort_order: 0,
                    file_as: Some("Sanderson, Brandon".to_string()),
                },
                SidecarAuthor {
                    name: "Jane Editor".to_string(),
                    role: AuthorRole::Editor,
                    sort_order: 1,
                    file_as: None,
                },
            ],
            description: Some("An epic fantasy novel.".to_string()),
            publisher: Some("Tor Books".to_string()),
            published_date: Some(2010),
            language: Some("en".to_string()),
            identifiers: vec![
                SidecarIdentifier {
                    identifier_type: IdentifierType::Isbn13,
                    value: "9780765326355".to_string(),
                },
                SidecarIdentifier {
                    identifier_type: IdentifierType::Asin,
                    value: "B003P2WO5E".to_string(),
                },
            ],
            series: Some(SidecarSeries {
                name: "The Stormlight Archive".to_string(),
                number: Some(Decimal::from(1)),
            }),
            genres: vec!["Fantasy".to_string(), "Epic Fantasy".to_string()],
            tags: vec!["magic-system".to_string()],
            page_count: Some(1007),
            status: BookStatus::Available,
            metadata_source: Some(MetadataSource::Hardcover),
            files: vec![SidecarFile {
                format: FileFormat::Epub,
                hash: "abc123".to_string(),
            }],
        }
    }

    /// Verify that every field present in the sidecar survives a write → parse
    /// roundtrip. This catches regressions where a field is stored only in a
    /// secondary location (e.g. JSON blob) but silently lost on one of the two
    /// paths.
    #[test]
    fn all_fields_roundtrip() {
        let original = full_test_sidecar();
        let bytes = write_sidecar(&original).expect("write failed");
        let parsed = parse_sidecar(&bytes).expect("parse failed");

        assert_eq!(parsed.title, original.title);
        assert_eq!(parsed.authors.len(), original.authors.len());
        assert_eq!(parsed.description, original.description);
        assert_eq!(parsed.publisher, original.publisher);
        assert_eq!(parsed.published_date, original.published_date);
        assert_eq!(parsed.language, original.language);
        assert_eq!(parsed.identifiers.len(), original.identifiers.len());
        assert_eq!(parsed.series.as_ref().map(|s| &s.name), original.series.as_ref().map(|s| &s.name));
        assert_eq!(parsed.series.as_ref().and_then(|s| s.number), original.series.as_ref().and_then(|s| s.number));
        assert_eq!(parsed.genres, original.genres, "genres must survive write → parse");
        assert_eq!(parsed.tags, original.tags, "tags must survive write → parse");
        assert_eq!(parsed.status, original.status);
        assert_eq!(parsed.metadata_source, original.metadata_source);
        assert_eq!(parsed.files.len(), original.files.len());
    }

    /// Verify that genres are emitted as `dc:subject` elements so e-readers
    /// (e.g. Kobo) can display them. Genres stored only in the private
    /// `spinnaker:metadata` blob would not be caught by a roundtrip test alone.
    #[test]
    fn genres_written_as_dc_subject() {
        let sidecar = full_test_sidecar();
        let bytes = write_sidecar(&sidecar).expect("write failed");
        let xml = std::str::from_utf8(&bytes).expect("utf8");

        for genre in &sidecar.genres {
            let expected = format!("<dc:subject>{genre}</dc:subject>");
            assert!(xml.contains(&expected), "expected {expected:?} in OPF output");
        }
    }

    #[test]
    fn roundtrip_full() {
        let original = full_test_sidecar();
        let bytes = write_sidecar(&original).expect("write failed");
        let parsed = parse_sidecar(&bytes).expect("parse failed");

        assert_eq!(parsed.title, original.title);
        assert_eq!(parsed.authors.len(), original.authors.len());
        assert_eq!(parsed.authors[0].name, original.authors[0].name);
        assert_eq!(parsed.authors[0].role, original.authors[0].role);
        assert_eq!(parsed.authors[0].sort_order, original.authors[0].sort_order);
        assert_eq!(parsed.authors[0].file_as, original.authors[0].file_as);
        assert_eq!(parsed.authors[1].name, original.authors[1].name);
        assert_eq!(parsed.authors[1].sort_order, original.authors[1].sort_order);
        assert_eq!(parsed.description, original.description);
        assert_eq!(parsed.publisher, original.publisher);
        assert_eq!(parsed.published_date, original.published_date);
        assert_eq!(parsed.language, original.language);
        assert_eq!(parsed.identifiers.len(), original.identifiers.len());
        assert_eq!(parsed.identifiers[0].identifier_type, original.identifiers[0].identifier_type);
        assert_eq!(parsed.identifiers[0].value, original.identifiers[0].value);
        assert_eq!(parsed.series.as_ref().map(|s| &s.name), original.series.as_ref().map(|s| &s.name));
        assert_eq!(parsed.series.as_ref().and_then(|s| s.number), original.series.as_ref().and_then(|s| s.number));
        assert_eq!(parsed.genres, original.genres);
        assert_eq!(parsed.tags, original.tags);
        assert_eq!(parsed.status, original.status);
        assert_eq!(parsed.metadata_source, original.metadata_source);
        assert_eq!(parsed.files.len(), original.files.len());
        assert_eq!(parsed.files[0].format, original.files[0].format);
        assert_eq!(parsed.files[0].hash, original.files[0].hash);
    }

    #[test]
    fn roundtrip_minimal() {
        let original = BookSidecar {
            title: "Minimal Book".to_string(),
            authors: vec![],
            description: None,
            publisher: None,
            published_date: None,
            language: None,
            identifiers: vec![],
            series: None,
            genres: vec![],
            tags: vec![],
            page_count: None,
            status: BookStatus::Incoming,
            metadata_source: None,
            files: vec![],
        };
        let bytes = write_sidecar(&original).expect("write failed");
        let parsed = parse_sidecar(&bytes).expect("parse failed");

        assert_eq!(parsed.title, original.title);
        assert!(parsed.authors.is_empty());
        assert_eq!(parsed.description, None);
        assert_eq!(parsed.publisher, None);
        assert_eq!(parsed.published_date, None);
        assert_eq!(parsed.language, None);
        assert!(parsed.identifiers.is_empty());
        assert!(parsed.series.is_none());
        assert!(parsed.genres.is_empty());
        assert!(parsed.tags.is_empty());
        assert_eq!(parsed.status, BookStatus::Incoming);
        assert_eq!(parsed.metadata_source, None);
        assert!(parsed.files.is_empty());
    }

    #[test]
    fn author_sort_order_preserved() {
        let original = BookSidecar {
            title: "Multi Author".to_string(),
            authors: vec![
                SidecarAuthor {
                    name: "Third Author".to_string(),
                    role: AuthorRole::Author,
                    sort_order: 2,
                    file_as: None,
                },
                SidecarAuthor {
                    name: "First Author".to_string(),
                    role: AuthorRole::Author,
                    sort_order: 0,
                    file_as: None,
                },
                SidecarAuthor {
                    name: "Second Author".to_string(),
                    role: AuthorRole::Translator,
                    sort_order: 1,
                    file_as: None,
                },
            ],
            description: None,
            publisher: None,
            published_date: None,
            language: None,
            identifiers: vec![],
            series: None,
            genres: vec![],
            tags: vec![],
            page_count: None,
            status: BookStatus::Available,
            metadata_source: None,
            files: vec![],
        };
        let bytes = write_sidecar(&original).expect("write failed");
        let parsed = parse_sidecar(&bytes).expect("parse failed");

        assert_eq!(parsed.authors.len(), 3);
        // After roundtrip, authors should be reconstructed with correct
        // sort_orders. Find by name and check sort_order.
        let first = parsed.authors.iter().find(|a| a.name == "First Author").unwrap();
        let second = parsed.authors.iter().find(|a| a.name == "Second Author").unwrap();
        let third = parsed.authors.iter().find(|a| a.name == "Third Author").unwrap();
        assert_eq!(first.sort_order, 0);
        assert_eq!(second.sort_order, 1);
        assert_eq!(third.sort_order, 2);
        assert_eq!(second.role, AuthorRole::Translator);
    }

    #[test]
    fn write_empty_authors_list() {
        use bb_core::{book::BookStatus, storage::BookSidecar};
        let sidecar = BookSidecar {
            title: "Untitled".to_string(),
            authors: vec![],
            description: None,
            publisher: None,
            published_date: None,
            language: None,
            identifiers: vec![],
            series: None,
            genres: vec![],
            tags: vec![],
            page_count: None,
            status: BookStatus::Available,
            metadata_source: None,
            files: vec![],
        };
        let bytes = write_sidecar(&sidecar).expect("write failed");
        let xml = std::str::from_utf8(&bytes).expect("utf8");
        assert!(!xml.contains("dc:creator"));
        assert!(xml.contains("<dc:title>Untitled</dc:title>"));
    }

    #[test]
    fn write_multiple_identifiers_same_type() {
        use bb_core::{
            book::{BookStatus, IdentifierType},
            storage::{BookSidecar, SidecarIdentifier},
        };
        let sidecar = BookSidecar {
            title: "Test".to_string(),
            authors: vec![],
            description: None,
            publisher: None,
            published_date: None,
            language: None,
            identifiers: vec![
                SidecarIdentifier {
                    identifier_type: IdentifierType::Isbn13,
                    value: "9780765326355".to_string(),
                },
                SidecarIdentifier {
                    identifier_type: IdentifierType::Isbn13,
                    value: "9780765326362".to_string(),
                },
            ],
            series: None,
            genres: vec![],
            tags: vec![],
            page_count: None,
            status: BookStatus::Available,
            metadata_source: None,
            files: vec![],
        };
        let bytes = write_sidecar(&sidecar).expect("write failed");
        let xml = std::str::from_utf8(&bytes).expect("utf8");
        assert!(xml.contains("9780765326355"));
        assert!(xml.contains("9780765326362"));
    }
}
