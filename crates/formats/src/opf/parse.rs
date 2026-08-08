use std::collections::HashMap;

use bb_core::{
    book::{AuthorRole, BookStatus, IdentifierType, MetadataSource},
    pipeline::{ExtractedAuthor, ExtractedIdentifier, ExtractedMetadata},
    storage::{BookSidecar, SidecarAuthor, SidecarFile, SidecarIdentifier, SidecarSeries},
};
use bb_utils::language::normalize_language;
use quick_xml::{
    NsReader, XmlVersion,
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
    subjects: Vec<String>,
    bb_meta_content: Option<String>,
    /// OPF 3 refines data: maps element id → (`role_code`, `file_as`)
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
    InSubject,
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

// ── spinnaker:metadata JSON structs
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
    page_count: Option<i32>,
    #[serde(default)]
    author_sort_orders: Vec<AuthorSortOrderJson>,
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
        "edt" => AuthorRole::Editor,
        "trl" => AuthorRole::Translator,
        "ill" => AuthorRole::Illustrator,
        //"aut" => AuthorRole::Author,
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
    // Do NOT use trim_text(true) — it trims each text fragment
    // independently, which strips whitespace around XML entities
    // (e.g. "turned &amp; twisted" → "turned&twisted").

    let mut fields = DcFields::default();
    let mut state = ParseState::Other;
    let mut buf = Vec::new();
    let mut text_buf = String::new();

    loop {
        buf.clear();
        match reader.read_resolved_event_into(&mut buf)? {
            (ResolveResult::Bound(ns), Event::Start(ref e)) if ns == Namespace(DC_NS) => {
                text_buf.clear();
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
                                    id = Some(attr.decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())?.into_owned());
                                }
                                b"opf:role" => {
                                    role = Some(attr.decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())?.into_owned());
                                }
                                b"opf:file-as" => {
                                    file_as = Some(attr.decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())?.into_owned());
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
                    b"subject" => state = ParseState::InSubject,
                    b"identifier" => {
                        let mut id = None;
                        let mut scheme = None;
                        for attr in e.attributes() {
                            let attr = attr.map_err(quick_xml::Error::from)?;
                            match attr.key.as_ref() {
                                b"id" => {
                                    id = Some(attr.decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())?.into_owned());
                                }
                                b"opf:scheme" => {
                                    scheme = Some(attr.decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())?.into_owned());
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
                let mut meta_name = None::<String>;
                let mut content = None::<String>;
                for attr in e.attributes() {
                    let attr = attr.map_err(quick_xml::Error::from)?;
                    match attr.key.as_ref() {
                        b"name" => {
                            meta_name = Some(attr.decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())?.into_owned());
                        }
                        b"content" => {
                            content = Some(attr.decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())?.into_owned());
                        }
                        _ => {}
                    }
                }
                if meta_name.as_deref() == Some("spinnaker:metadata") {
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
                            property = Some(attr.decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())?.into_owned());
                        }
                        b"refines" => {
                            refines = Some(attr.decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())?.into_owned());
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
                text_buf.push_str(&t.decode()?);
            }
            (_, Event::GeneralRef(ref r)) => {
                // quick-xml emits XML entity references (&apos; &amp; etc.)
                // as separate GeneralRef events. Resolve and append to the
                // text accumulator so entities don't cause truncation.
                let name = r.decode()?;
                match name.as_ref() {
                    "amp" => text_buf.push('&'),
                    "lt" => text_buf.push('<'),
                    "gt" => text_buf.push('>'),
                    "apos" => text_buf.push('\''),
                    "quot" => text_buf.push('"'),
                    _ => {
                        if let Some(ch) = r.resolve_char_ref()? {
                            text_buf.push(ch);
                        }
                    }
                }
            }
            (_, Event::End(_)) => {
                // Trim outer whitespace (replaces the old trim_text(true)
                // which can't be used because it trims per-fragment).
                let text = std::mem::take(&mut text_buf).trim().to_string();
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
                    ParseState::InLanguage => fields.language = normalize_language(&text),
                    ParseState::InSubject => {
                        let s = text.trim().to_string();
                        if !s.is_empty() {
                            fields.subjects.push(s);
                        }
                    }
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
            (_, Event::Eof) => break,
            _ => {}
        }
    }

    // Apply OPF 3 refines (role, file-as) to authors that were identified by id
    // attribute.
    for author in &mut fields.authors {
        if let Some(ref id) = author.id
            && let Some((role, file_as)) = fields.meta_refines.get(id.as_str())
        {
            if author.role_code.is_none() {
                author.role_code.clone_from(role);
            }
            if author.file_as.is_none() {
                author.file_as.clone_from(file_as);
            }
        }
    }

    Ok(fields)
}

// ── public API
// ────────────────────────────────────────────────────────────────

/// Parse a `BookBoss` `metadata.opf` sidecar back into a [`BookSidecar`].
pub fn parse_sidecar(xml: &[u8]) -> Result<BookSidecar, Error> {
    let fields = parse_dc(xml)?;

    let bb: BbMetaJson = fields
        .bb_meta_content
        .as_deref()
        .map(serde_json::from_str)
        .transpose()?
        .ok_or(Error::MissingField("spinnaker:metadata"))?;

    // Build a name → sort_order lookup from the JSON blob.
    let sort_order_map: HashMap<&str, i32> = bb.author_sort_orders.iter().map(|a| (a.name.as_str(), a.sort_order)).collect();

    let authors: Vec<SidecarAuthor> = fields
        .authors
        .into_iter()
        .enumerate()
        .map(|(i, raw)| {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_possible_wrap,
                reason = "author list index; books have far fewer authors than i32::MAX"
            )]
            let sort_order = sort_order_map.get(raw.name.as_str()).copied().unwrap_or(i as i32);
            let role = raw.role_code.as_deref().map_or(AuthorRole::Author, marc_to_author_role);
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
        genres: if bb.genres.is_empty() { fields.subjects } else { bb.genres },
        tags: bb.tags,
        page_count: bb.page_count,
        status: bb.status,
        metadata_source: bb.metadata_source,
        files: bb.files,
    })
}

/// Extract metadata from an OPF document (e.g. embedded in an EPUB).
///
/// Only reads Dublin Core fields; ignores the `spinnaker:metadata` extension.
pub fn extract_metadata(xml: &[u8]) -> Result<ExtractedMetadata, Error> {
    let fields = parse_dc(xml)?;

    let authors: Vec<ExtractedAuthor> = fields
        .authors
        .into_iter()
        .enumerate()
        .map(|(i, raw)| {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_possible_wrap,
                reason = "author list index; books have far fewer authors than i32::MAX"
            )]
            let sort_order = i as i32;
            ExtractedAuthor {
                name: raw.name,
                role: raw.role_code.as_deref().map(marc_to_author_role),
                sort_order,
            }
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

    // Pull spinnaker:metadata fields when the blob is present.
    let (genres, tags, page_count, series_name, series_number) = if let Some(json) = &fields.bb_meta_content {
        if let Ok(bb) = serde_json::from_str::<BbMetaJson>(json) {
            let genres = if bb.genres.is_empty() { fields.subjects.clone() } else { bb.genres };
            let (sname, snumber) = bb.series.map_or((None, None), |s| (Some(s.name), s.number));
            (genres, bb.tags, bb.page_count, sname, snumber)
        } else {
            (fields.subjects.clone(), vec![], None, None, None)
        }
    } else {
        (fields.subjects.clone(), vec![], None, None, None)
    };

    Ok(ExtractedMetadata {
        title: fields.title,
        authors: if authors.is_empty() { None } else { Some(authors) },
        description: fields.description,
        publisher: fields.publisher,
        published_date: fields.published_date.as_deref().and_then(parse_year),
        language: fields.language,
        identifiers: if identifiers.is_empty() { None } else { Some(identifiers) },
        series_name,
        series_number,
        genres,
        tags,
        page_count,
        has_spinnaker_metadata: fields.bb_meta_content.is_some(),
        cover_bytes: None,
    })
}

/// Information about a cover image found in an OPF manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverInfo {
    /// The `href` attribute of the manifest item (relative to the OPF file).
    pub href: String,
    /// The `id` attribute of the manifest item.
    pub id: String,
    /// Whether the manifest item already has `properties="cover-image"` (EPUB
    /// 3).
    pub has_cover_image_property: bool,
}

/// Find cover image information within an EPUB OPF document.
///
/// Detection priority:
/// 1. **EPUB 3**: `<item properties="cover-image"/>` — returns immediately.
/// 2. **EPUB 2**: `<meta name="cover" content="item-id"/>` + matching manifest
///    item.
/// 3. **Heuristic**: manifest item with an `id` containing "cover"
///    (case-insensitive) and an image `media-type`. Prefers exact `id="cover"`
///    or `id="cover-image"` over substring matches.
#[must_use]
pub fn extract_cover_info(opf_xml: &[u8]) -> Option<CoverInfo> {
    use quick_xml::Reader;

    let mut reader = Reader::from_reader(opf_xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut cover_meta_id: Option<String> = None;
    // id → (href, media_type)
    let mut manifest_items: HashMap<String, (String, Option<String>)> = HashMap::new();

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
                                b"name" if attr.decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder()).ok().as_deref() == Some("cover") => {
                                    is_cover = true;
                                }
                                b"content" => {
                                    content = attr
                                        .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
                                        .ok()
                                        .map(std::borrow::Cow::into_owned);
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
                        let mut media_type = None;
                        let mut is_cover_image = false;
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"id" => {
                                    id = attr
                                        .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
                                        .ok()
                                        .map(std::borrow::Cow::into_owned);
                                }
                                b"href" => {
                                    href = attr
                                        .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
                                        .ok()
                                        .map(std::borrow::Cow::into_owned);
                                }
                                b"media-type" => {
                                    media_type = attr
                                        .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
                                        .ok()
                                        .map(std::borrow::Cow::into_owned);
                                }
                                b"properties" => {
                                    if let Ok(v) = attr.decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
                                        && v.split_whitespace().any(|p| p == "cover-image")
                                    {
                                        is_cover_image = true;
                                    }
                                }
                                _ => {}
                            }
                        }
                        if let (Some(id), Some(href)) = (id, href) {
                            if is_cover_image {
                                return Some(CoverInfo {
                                    href,
                                    id,
                                    has_cover_image_property: true,
                                });
                            }
                            manifest_items.insert(id, (href, media_type));
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
    if let Some(id) = cover_meta_id
        && let Some((href, _)) = manifest_items.remove(&id)
    {
        return Some(CoverInfo {
            href,
            id,
            has_cover_image_property: false,
        });
    }

    // Heuristic: look for a manifest item whose id contains "cover" with an
    // image media-type. Prefer exact id "cover" > "cover-image" > substring.
    let mut candidates: Vec<(String, String)> = manifest_items
        .into_iter()
        .filter(|(id, (_, mt))| id.to_ascii_lowercase().contains("cover") && mt.as_deref().is_some_and(|m| m.starts_with("image/")))
        .map(|(id, (href, _))| (id, href))
        .collect();

    if candidates.is_empty() {
        return None;
    }

    // Sort: exact "cover" first, then "cover-image", then the rest
    candidates.sort_by_key(|(id, _)| {
        let lower = id.to_ascii_lowercase();
        if lower == "cover" {
            0
        } else if lower == "cover-image" {
            1
        } else {
            2
        }
    });

    let (id, href) = candidates.remove(0);
    Some(CoverInfo {
        href,
        id,
        has_cover_image_property: false,
    })
}

/// Find the cover image href within an EPUB OPF document.
///
/// Convenience wrapper around [`extract_cover_info`] that returns only the
/// href. See that function for detection priority and heuristic details.
#[must_use]
pub fn extract_cover_href(opf_xml: &[u8]) -> Option<String> {
    extract_cover_info(opf_xml).map(|ci| ci.href)
}

#[cfg(test)]
mod cover_info_tests {
    use super::*;

    #[test]
    fn epub3_cover_info() {
        let opf = br#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>T</dc:title></metadata>
  <manifest>
    <item id="ci" href="cover.jpg" media-type="image/jpeg" properties="cover-image"/>
  </manifest>
</package>"#;
        let info = extract_cover_info(opf).expect("should find cover");
        assert_eq!(info.href, "cover.jpg");
        assert_eq!(info.id, "ci");
        assert!(info.has_cover_image_property);
    }

    #[test]
    fn epub2_cover_info() {
        let opf = br#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>T</dc:title>
    <meta name="cover" content="cover-img"/>
  </metadata>
  <manifest>
    <item id="cover-img" href="images/cover.jpg" media-type="image/jpeg"/>
  </manifest>
</package>"#;
        let info = extract_cover_info(opf).expect("should find cover");
        assert_eq!(info.href, "images/cover.jpg");
        assert_eq!(info.id, "cover-img");
        assert!(!info.has_cover_image_property);
    }

    #[test]
    fn heuristic_cover_by_id() {
        let opf = br#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>T</dc:title></metadata>
  <manifest>
    <item id="cover" href="cover.jpeg" media-type="image/jpeg"/>
    <item id="ch1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
</package>"#;
        let info = extract_cover_info(opf).expect("should find cover via heuristic");
        assert_eq!(info.href, "cover.jpeg");
        assert_eq!(info.id, "cover");
        assert!(!info.has_cover_image_property);
    }

    #[test]
    fn heuristic_ignores_non_image() {
        let opf = br#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>T</dc:title></metadata>
  <manifest>
    <item id="cover" href="cover.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
</package>"#;
        assert!(extract_cover_info(opf).is_none());
    }

    #[test]
    fn heuristic_prefers_exact_cover_id() {
        let opf = br#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>T</dc:title></metadata>
  <manifest>
    <item id="cover-page" href="coverpage.jpg" media-type="image/jpeg"/>
    <item id="cover" href="cover.jpg" media-type="image/jpeg"/>
  </manifest>
</package>"#;
        let info = extract_cover_info(opf).expect("should find cover");
        assert_eq!(info.id, "cover");
        assert_eq!(info.href, "cover.jpg");
    }

    #[test]
    fn no_cover_returns_none() {
        let opf = br#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>T</dc:title></metadata>
  <manifest>
    <item id="ch1" href="chapter.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
</package>"#;
        assert!(extract_cover_info(opf).is_none());
    }

    #[test]
    fn extract_cover_href_wrapper() {
        let opf = br#"<?xml version="1.0"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>T</dc:title></metadata>
  <manifest>
    <item id="ci" href="cover.jpg" media-type="image/jpeg" properties="cover-image"/>
  </manifest>
</package>"#;
        assert_eq!(extract_cover_href(opf), Some("cover.jpg".to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_year_unparseable_returns_none() {
        assert_eq!(parse_year("circa 1850"), None);
        assert_eq!(parse_year("unknown"), None);
        assert_eq!(parse_year(""), None);
    }

    #[test]
    fn classify_identifier_calibre_isbn_prefix() {
        use bb_core::book::IdentifierType;
        let result = classify_identifier(None, "isbn:9780140449136", None);
        assert_eq!(result, Some((IdentifierType::Isbn13, "9780140449136".to_string())));
    }

    #[test]
    fn extract_metadata_no_authors() {
        let opf = br#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Untitled</dc:title>
  </metadata>
  <manifest/>
  <spine/>
</package>"#;
        let meta = extract_metadata(opf).expect("parse failed");
        assert_eq!(meta.title.as_deref(), Some("Untitled"));
        assert!(meta.authors.is_none());
    }

    #[test]
    fn duplicate_identifiers_same_scheme() {
        use bb_core::book::IdentifierType;
        let opf = br#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"
            xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>Test</dc:title>
    <dc:identifier opf:scheme="ISBN">9780140449136</dc:identifier>
    <dc:identifier opf:scheme="ISBN">014044913X</dc:identifier>
  </metadata>
  <manifest/>
  <spine/>
</package>"#;
        let meta = extract_metadata(opf).expect("parse failed");
        let ids = meta.identifiers.as_ref().expect("expected identifiers");
        assert_eq!(ids.len(), 2);
        assert!(ids.iter().any(|i| i.identifier_type == IdentifierType::Isbn13));
        assert!(ids.iter().any(|i| i.identifier_type == IdentifierType::Isbn10));
    }
}
