use std::io::Cursor;

use bb_core::{
    book::{AuthorRole, BookStatus, IdentifierType, MetadataSource},
    storage::{BookSidecar, SidecarAuthor, SidecarFile, SidecarSeries},
};
use quick_xml::{
    Writer,
    events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event},
};
use serde::Serialize;

use crate::Error;

fn author_role_to_marc(role: &AuthorRole) -> &'static str {
    match role {
        AuthorRole::Author => "aut",
        AuthorRole::Editor => "edt",
        AuthorRole::Translator => "trl",
        AuthorRole::Illustrator => "ill",
    }
}

fn identifier_type_to_scheme(id_type: &IdentifierType) -> &'static str {
    match id_type {
        IdentifierType::Isbn10 | IdentifierType::Isbn13 => "ISBN",
        IdentifierType::Asin => "ASIN",
        IdentifierType::GoogleBooks => "GoogleBooks",
        IdentifierType::OpenLibrary => "OpenLibrary",
        IdentifierType::Hardcover => "Hardcover",
    }
}

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
    author_sort_orders: Vec<AuthorSortOrder>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rating: Option<i16>,
    status: &'a BookStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata_source: Option<&'a MetadataSource>,
    files: &'a Vec<SidecarFile>,
}

pub fn write_sidecar(sidecar: &BookSidecar) -> Result<Vec<u8>, Error> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;

    let mut pkg = BytesStart::new("package");
    pkg.push_attribute(("xmlns", "http://www.idpf.org/2007/opf"));
    pkg.push_attribute(("version", "2.0"));
    writer.write_event(Event::Start(pkg))?;

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
        creator.push_attribute(("opf:role", author_role_to_marc(&author.role)));
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

    for identifier in &sidecar.identifiers {
        let mut id_elem = BytesStart::new("dc:identifier");
        id_elem.push_attribute(("opf:scheme", identifier_type_to_scheme(&identifier.identifier_type)));
        writer.write_event(Event::Start(id_elem))?;
        writer.write_event(Event::Text(BytesText::new(&identifier.value)))?;
        writer.write_event(Event::End(BytesEnd::new("dc:identifier")))?;
    }

    // bookboss:metadata JSON blob
    let bb_meta = BbMeta {
        series: sidecar.series.as_ref(),
        genres: &sidecar.genres,
        tags: &sidecar.tags,
        author_sort_orders: sorted_authors
            .iter()
            .map(|a| AuthorSortOrder {
                name: a.name.clone(),
                sort_order: a.sort_order,
            })
            .collect(),
        rating: sidecar.rating,
        status: &sidecar.status,
        metadata_source: sidecar.metadata_source.as_ref(),
        files: &sidecar.files,
    };
    let json = serde_json::to_string(&bb_meta)?;
    let mut meta_bb = BytesStart::new("meta");
    meta_bb.push_attribute(("name", "bookboss:metadata"));
    meta_bb.push_attribute(("content", json.as_str()));
    writer.write_event(Event::Empty(meta_bb))?;

    writer.write_event(Event::End(BytesEnd::new("metadata")))?;
    writer.write_event(Event::Empty(BytesStart::new("manifest")))?;
    writer.write_event(Event::Empty(BytesStart::new("spine")))?;
    writer.write_event(Event::End(BytesEnd::new("package")))?;

    Ok(writer.into_inner().into_inner())
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
            rating: Some(5),
            status: BookStatus::Available,
            metadata_source: Some(MetadataSource::Hardcover),
            files: vec![SidecarFile {
                format: FileFormat::Epub,
                hash: "abc123".to_string(),
            }],
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
        assert_eq!(parsed.rating, original.rating);
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
            rating: None,
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
        assert_eq!(parsed.rating, None);
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
            rating: None,
            status: BookStatus::Available,
            metadata_source: None,
            files: vec![],
        };
        let bytes = write_sidecar(&original).expect("write failed");
        let parsed = parse_sidecar(&bytes).expect("parse failed");

        assert_eq!(parsed.authors.len(), 3);
        // After roundtrip, authors should be reconstructed with correct sort_orders.
        // Find by name and check sort_order.
        let first = parsed.authors.iter().find(|a| a.name == "First Author").unwrap();
        let second = parsed.authors.iter().find(|a| a.name == "Second Author").unwrap();
        let third = parsed.authors.iter().find(|a| a.name == "Third Author").unwrap();
        assert_eq!(first.sort_order, 0);
        assert_eq!(second.sort_order, 1);
        assert_eq!(third.sort_order, 2);
        assert_eq!(second.role, AuthorRole::Translator);
    }
}
