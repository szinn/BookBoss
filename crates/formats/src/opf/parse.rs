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
    id: Option<String>,
    name: String,
    role_code: Option<String>,
    file_as: Option<String>,
}

struct RawIdentifier {
    id: Option<String>,
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
    /// OPF 3 refines data: maps element id → (role_code, file_as)
    meta_refines: HashMap<String, (Option<String>, Option<String>)>,
}

enum ParseState {
    Other,
    InTitle,
    InCreator {
        id: Option<String>,
        role: Option<String>,
        file_as: Option<String>,
    },
    InDescription,
    InPublisher,
    InDate,
    InLanguage,
    InIdentifier {
        id: Option<String>,
        scheme: Option<String>,
    },
    /// OPF 3: collecting text for a `<meta refines="#id">` element.
    /// `is_role`: true = collecting role code, false = collecting file-as.
    InMetaRefine {
        is_role: bool,
        refines_id: String,
    },
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

/// Classify an identifier, returning `None` for unknown/unrecognised schemes.
///
/// Handles both explicit `opf:scheme` attributes (OPF 2) and the
/// Calibre-style value-prefix format `"scheme:value"` (OPF 3, no attribute).
/// Classify an identifier, returning `(type, effective_value)` or `None`.
///
/// The returned value may differ from `value` when the ISBN is encoded in the
/// `id` attribute (e.g. `id="isbn9781529061819"` with a UUID value).
fn classify_identifier(scheme: Option<&str>, value: &str, id_hint: Option<&str>) -> Option<(IdentifierType, String)> {
    let (effective_scheme, bare_value) = match scheme.filter(|s| !s.is_empty()) {
        Some(s) => (s.to_uppercase(), value),
        None => {
            if let Some(pos) = value.find(':') {
                // "scheme:value" prefix (e.g. Calibre's "calibre:20139").
                (value[..pos].to_uppercase(), &value[pos + 1..])
            } else {
                // No scheme and no prefix — try heuristic ISBN detection on the value.
                if let Some(id_type) = isbn_from_bare_value(value) {
                    return Some((id_type, value.to_string()));
                }
                // Last resort: check if the id attribute encodes the ISBN,
                // e.g. id="isbn9781529061819" with a UUID as the element value.
                return isbn_from_id_attr(id_hint);
            }
        }
    };
    let result = match effective_scheme.as_str() {
        "ISBN" => Some((isbn_type(bare_value), bare_value.to_string())),
        "ASIN" => Some((IdentifierType::Asin, value.to_string())),
        "GOOGLEBOOKS" => Some((IdentifierType::GoogleBooks, value.to_string())),
        "OPENLIBRARY" => Some((IdentifierType::OpenLibrary, value.to_string())),
        "HARDCOVER" => Some((IdentifierType::Hardcover, value.to_string())),
        _ => None,
    };
    // If the value-based classification failed, try the id attribute as a
    // fallback (e.g. id="isbn9781529061819" with a UUID element value).
    result.or_else(|| isbn_from_id_attr(id_hint))
}

/// Extract an ISBN from an OPF `id` attribute of the form
/// `"isbn9781529061819"`.
fn isbn_from_id_attr(id: Option<&str>) -> Option<(IdentifierType, String)> {
    let id = id?;
    let rest = id.strip_prefix("isbn").or_else(|| id.strip_prefix("ISBN"))?;
    let id_type = isbn_from_bare_value(rest)?;
    Some((id_type, rest.to_string()))
}

/// Detect an ISBN-10 or ISBN-13 from a value that contains only digits (and
/// optionally a trailing `X` for ISBN-10).  Returns `None` for anything else.
fn isbn_from_bare_value(value: &str) -> Option<IdentifierType> {
    let v = value.trim();
    let all_digits = v.chars().all(|c| c.is_ascii_digit());
    match v.len() {
        13 if all_digits => Some(IdentifierType::Isbn13),
        10 if v[..9].chars().all(|c| c.is_ascii_digit()) && (v.ends_with(|c: char| c.is_ascii_digit()) || v.ends_with('X')) => Some(IdentifierType::Isbn10),
        _ => None,
    }
}

fn isbn_type(value: &str) -> IdentifierType {
    if value.len() == 10 { IdentifierType::Isbn10 } else { IdentifierType::Isbn13 }
}

/// Extract a publication year from a dc:date value.
///
/// Handles:
/// - plain year: `"1965"` → 1965
/// - ISO date:   `"2022-10-11"` → 2022
/// - ISO datetime: `"2022-08-19T11:29:46Z"` → 2022
fn parse_year(s: &str) -> Option<i32> {
    // Fast path: plain integer year.
    if let Ok(y) = s.parse() {
        return Some(y);
    }
    // Take the first hyphen-delimited segment and try that as a year.
    s.split('-').next()?.parse().ok()
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
                        let mut id = None;
                        let mut role = None;
                        let mut file_as = None;
                        for attr in e.attributes() {
                            let attr = attr.map_err(quick_xml::Error::from)?;
                            match attr.key.as_ref() {
                                b"id" => {
                                    id = Some(attr.decode_and_unescape_value(reader.decoder())?.into_owned());
                                }
                                b"opf:role" => {
                                    role = Some(attr.decode_and_unescape_value(reader.decoder())?.into_owned());
                                }
                                b"opf:file-as" => {
                                    file_as = Some(attr.decode_and_unescape_value(reader.decoder())?.into_owned());
                                }
                                _ => {}
                            }
                        }
                        state = ParseState::InCreator { id, role, file_as };
                    }
                    b"description" => state = ParseState::InDescription,
                    b"publisher" => state = ParseState::InPublisher,
                    b"date" => state = ParseState::InDate,
                    b"language" => state = ParseState::InLanguage,
                    b"identifier" => {
                        let mut id = None;
                        let mut scheme = None;
                        for attr in e.attributes() {
                            let attr = attr.map_err(quick_xml::Error::from)?;
                            match attr.key.as_ref() {
                                b"id" => {
                                    id = Some(attr.decode_and_unescape_value(reader.decoder())?.into_owned());
                                }
                                b"opf:scheme" => {
                                    scheme = Some(attr.decode_and_unescape_value(reader.decoder())?.into_owned());
                                }
                                _ => {}
                            }
                        }
                        state = ParseState::InIdentifier { id, scheme };
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
            // OPF 3: <meta property="role|file-as" refines="#id">text</meta>
            (_, Event::Start(ref e)) if e.local_name().as_ref() == b"meta" => {
                let mut property = None::<String>;
                let mut refines = None::<String>;
                for attr in e.attributes() {
                    let attr = attr.map_err(quick_xml::Error::from)?;
                    match attr.key.as_ref() {
                        b"property" => {
                            property = Some(attr.decode_and_unescape_value(reader.decoder())?.into_owned());
                        }
                        b"refines" => {
                            refines = Some(attr.decode_and_unescape_value(reader.decoder())?.into_owned());
                        }
                        _ => {}
                    }
                }
                if let (Some(prop), Some(ref_id)) = (property, refines) {
                    let refines_id = ref_id.trim_start_matches('#').to_string();
                    match prop.as_str() {
                        "role" => state = ParseState::InMetaRefine { is_role: true, refines_id },
                        "file-as" => state = ParseState::InMetaRefine { is_role: false, refines_id },
                        _ => {}
                    }
                }
            }
            (_, Event::Text(ref t)) => {
                let text = t.decode()?.into_owned();
                match std::mem::replace(&mut state, ParseState::Other) {
                    ParseState::InTitle => fields.title = Some(text),
                    ParseState::InCreator { id, role, file_as } => {
                        fields.authors.push(RawAuthor {
                            id,
                            name: text,
                            role_code: role,
                            file_as,
                        });
                    }
                    ParseState::InDescription => fields.description = Some(text),
                    ParseState::InPublisher => fields.publisher = Some(text),
                    ParseState::InDate => fields.published_date = Some(text),
                    ParseState::InLanguage => fields.language = Some(text),
                    ParseState::InIdentifier { id, scheme } => {
                        fields.identifiers.push(RawIdentifier { id, scheme, value: text });
                    }
                    ParseState::InMetaRefine { is_role, refines_id } => {
                        let entry = fields.meta_refines.entry(refines_id).or_default();
                        if is_role {
                            entry.0 = Some(text);
                        } else {
                            entry.1 = Some(text);
                        }
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

    // Apply OPF 3 refines (role, file-as) to authors that were identified by id
    // attribute.
    for author in &mut fields.authors {
        if let Some(ref id) = author.id {
            if let Some((role, file_as)) = fields.meta_refines.get(id.as_str()) {
                if author.role_code.is_none() {
                    author.role_code.clone_from(role);
                }
                if author.file_as.is_none() {
                    author.file_as.clone_from(file_as);
                }
            }
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
        .filter_map(|raw| {
            let (id_type, id_value) = classify_identifier(raw.scheme.as_deref(), &raw.value, raw.id.as_deref())?;
            Some(SidecarIdentifier {
                identifier_type: id_type,
                value: id_value,
            })
        })
        .collect();

    Ok(BookSidecar {
        title: fields.title.ok_or(Error::MissingField("dc:title"))?,
        authors,
        description: fields.description,
        publisher: fields.publisher,
        published_date: fields.published_date.as_deref().and_then(parse_year),
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
        .filter_map(|raw| {
            let (id_type, id_value) = classify_identifier(raw.scheme.as_deref(), &raw.value, raw.id.as_deref())?;
            Some(ExtractedIdentifier {
                identifier_type: id_type,
                value: id_value,
            })
        })
        .collect();

    Ok(ExtractedMetadata {
        title: fields.title,
        authors: if authors.is_empty() { None } else { Some(authors) },
        description: fields.description,
        publisher: fields.publisher,
        published_date: fields.published_date.as_deref().and_then(parse_year),
        language: fields.language,
        identifiers: if identifiers.is_empty() { None } else { Some(identifiers) },
        series_name: None,
        series_number: None,
        cover_bytes: None,
    })
}

/// Find the cover image href within an EPUB OPF document.
///
/// Handles both EPUB 2 (`<meta name="cover" content="item-id"/>` + manifest
/// lookup) and EPUB 3 (`<item properties="cover-image"/>` in the manifest).
/// Returns the href exactly as written in the manifest (caller must resolve it
/// relative to the OPF file's directory within the ZIP archive).
pub fn extract_cover_href(opf_xml: &[u8]) -> Option<String> {
    use std::collections::HashMap;

    use quick_xml::{Reader, events::Event};

    let mut reader = Reader::from_reader(opf_xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut cover_meta_id: Option<String> = None;
    let mut manifest_items: HashMap<String, String> = HashMap::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) => {
                match e.local_name().as_ref() {
                    b"meta" => {
                        // EPUB 2: <meta name="cover" content="item-id"/>
                        let mut is_cover = false;
                        let mut content = None;
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"name" => {
                                    if attr.decode_and_unescape_value(reader.decoder()).ok().as_deref() == Some("cover") {
                                        is_cover = true;
                                    }
                                }
                                b"content" => {
                                    content = attr.decode_and_unescape_value(reader.decoder()).ok().map(|v| v.into_owned());
                                }
                                _ => {}
                            }
                        }
                        if is_cover {
                            cover_meta_id = content;
                        }
                    }
                    b"item" => {
                        let mut id = None;
                        let mut href = None;
                        let mut is_cover_image = false;
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"id" => {
                                    id = attr.decode_and_unescape_value(reader.decoder()).ok().map(|v| v.into_owned());
                                }
                                b"href" => {
                                    href = attr.decode_and_unescape_value(reader.decoder()).ok().map(|v| v.into_owned());
                                }
                                b"properties" => {
                                    // EPUB 3: properties may be a space-separated list
                                    if let Ok(v) = attr.decode_and_unescape_value(reader.decoder()) {
                                        if v.split_whitespace().any(|p| p == "cover-image") {
                                            is_cover_image = true;
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        if is_cover_image {
                            return href; // EPUB 3: direct match
                        }
                        if let (Some(id), Some(href)) = (id, href) {
                            manifest_items.insert(id, href);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    // EPUB 2: resolve cover id against collected manifest items
    cover_meta_id.and_then(|id| manifest_items.remove(&id))
}
