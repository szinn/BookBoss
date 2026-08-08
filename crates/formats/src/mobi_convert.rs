//! EPUB → MOBI6 conversion.
// Private helpers are unused until Step 2 wires the FormatService.
#![allow(dead_code)]
//!
//! Produces a minimal but reader-compatible MOBI6 (PalmDB) file from an EPUB
//! source. The output is suitable for display on Kindle devices and the Kindle
//! app, but does not aim for full spec compliance.
//!
//! # Algorithm
//!
//! 1. Open the EPUB ZIP and read the OPF to find spine item hrefs in order.
//! 2. Merge all spine HTML files into one linearised HTML document.
//! 3. Rewrite `<img src="...">` references to
//!    `kindle:embed:XXXX?mime=image/jpeg`.
//! 4. Strip non-inline `<style>` blocks and `<link rel="stylesheet">` elements.
//! 5. Collect images: cover first (if provided), then body images in ref order.
//! 6. Write a PalmDB file with PalmDoc + MOBI header + EXTH block in record 0,
//!    HTML text in records 1..N, and JPEG images in records N+1..M.

use std::{
    collections::HashMap,
    io::{Read, Write},
    path::Path,
};

use bb_core::storage::BookSidecar;
use quick_xml::{Reader, events::Event};

use crate::Error;

/// A chapter entry parsed from the EPUB NCX navigation document.
struct NavPoint {
    /// Human-readable chapter label (e.g. "Chapter 7").
    label: String,
    /// Anchor name in the merged HTML (e.g. "AlongCameASpid_chap_7"), matching
    /// the `<a name="...">` injected by `clean_html`.
    anchor: String,
}

// ── Public entry point
// ────────────────────────────────────────────────────────

/// Convert an EPUB file to MOBI6 (PalmDB) format.
///
/// * `source_epub` — path to the source `.epub` file.
/// * `dest`        — path to write the `.mobi` output.
/// * `sidecar`     — book metadata used to populate EXTH records.
/// * `cover_bytes` — optional JPEG cover image; placed as the first image
///   record.
pub fn convert_to_mobi(source_epub: &Path, dest: &Path, sidecar: &BookSidecar, cover_bytes: Option<&[u8]>) -> Result<(), Error> {
    // 1. Read EPUB and gather spine HTML + images.
    let epub_file = std::fs::File::open(source_epub)?;
    let mut archive = zip::ZipArchive::new(epub_file)?;

    // Read container.xml → OPF path.
    let opf_path = {
        let mut container = archive.by_name("META-INF/container.xml")?;
        let mut buf = Vec::new();
        container.read_to_end(&mut buf)?;
        crate::epub::find_opf_path(&buf)?
    };
    let opf_dir = match opf_path.rfind('/') {
        Some(pos) => opf_path[..pos].to_string(),
        None => String::new(),
    };

    // Read OPF bytes.
    let opf_bytes = {
        let mut f = archive.by_name(&opf_path)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        buf
    };

    // Parse manifest (id → href), spine (ordered idrefs), NCX href, and guide toc
    // href.
    let (manifest, spine_idrefs, ncx_href, guide_toc_href) = parse_opf_manifest_and_spine(&opf_bytes)?;

    // Build ordered list of HTML hrefs from the spine.
    let mut spine_hrefs: Vec<String> = Vec::new();
    for idref in &spine_idrefs {
        if let Some(href) = manifest.get(idref) {
            spine_hrefs.push(href.clone());
        }
    }

    // Collect all entry names up front (borrow checker).
    let entry_names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).map(|f| f.name().to_string()))
        .collect::<Result<_, _>>()?;

    // Build a lookup: zip path → index.
    let name_to_idx: HashMap<String, usize> = entry_names.iter().enumerate().map(|(i, n)| (n.clone(), i)).collect();

    // Build prefix map: zip_path → anchor prefix (used for id/href rewriting).
    // e.g. "OEBPS/Text/chapter2.xhtml" → "chapter2"
    let spine_prefix_map: HashMap<String, String> = spine_hrefs
        .iter()
        .map(|href| {
            let zp = resolve_zip_path(&opf_dir, href);
            let prefix = spine_prefix_for(&zp);
            (zp, prefix)
        })
        .collect();

    // Read spine HTML documents and merge them.
    let mut merged_html_parts: Vec<Vec<u8>> = Vec::new();
    // Track image hrefs in body-reference order (relative to OPF dir).
    let mut body_image_hrefs: Vec<String> = Vec::new();
    // Map from img src (as seen in HTML) to 1-based image record index.
    // Cover is always image record 1 if present; body images follow.
    let cover_offset: u32 = u32::from(cover_bytes.is_some());

    for href in &spine_hrefs {
        let zip_path = resolve_zip_path(&opf_dir, href);
        let html_bytes = if let Some(&idx) = name_to_idx.get(&zip_path) {
            let mut f = archive.by_index(idx)?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            buf
        } else {
            continue;
        };

        // Directory of this HTML file within the zip (for resolving relative hrefs).
        let html_zip_dir = match zip_path.rfind('/') {
            Some(pos) => zip_path[..pos].to_string(),
            None => String::new(),
        };
        let current_prefix = spine_prefix_map.get(&zip_path).cloned().unwrap_or_default();

        // Collect img srefs from this document (relative to its directory).
        let html_dir = match href.rfind('/') {
            Some(pos) => href[..pos].to_string(),
            None => String::new(),
        };
        let img_srcs = collect_img_srcs(&html_bytes);
        for src in &img_srcs {
            // Resolve src relative to the HTML file's directory within OPF dir.
            let abs_href = resolve_zip_path(&opf_dir, &resolve_zip_path(&html_dir, src));
            // Deduplicate.
            if !body_image_hrefs.contains(&abs_href) {
                body_image_hrefs.push(abs_href);
            }
        }

        // Build a local map: src string → kindle record index (1-based global).
        let mut src_to_record: HashMap<String, u32> = HashMap::new();
        for src in &img_srcs {
            let abs_href = resolve_zip_path(&opf_dir, &resolve_zip_path(&html_dir, src));
            // Position among body images (0-based) + cover offset + 1 = 1-based.
            if let Some(pos) = body_image_hrefs.iter().position(|h| h == &abs_href) {
                let record_idx = cover_offset + pos as u32 + 1;
                src_to_record.insert(src.clone(), record_idx);
            }
        }

        let cleaned = clean_html(&html_bytes, &src_to_record, &current_prefix, &html_zip_dir, &spine_prefix_map)?;
        merged_html_parts.push(cleaned);
    }

    // Build the merged body: wrap parts in a minimal HTML document.
    // Each part already begins with an <a name="PREFIX"> anchor injected by
    // clean_html as the very first body element, so filepos targets land on a
    // proper named anchor rather than an invisible-but-broken tag.
    let mut merged = Vec::new();
    merged.extend_from_slice(b"<html><head><meta charset=\"utf-8\"/></head><body>");
    for (i, part) in merged_html_parts.iter().enumerate() {
        if i > 0 {
            merged.extend_from_slice(b"<mbp:pagebreak/>");
        }
        merged.extend_from_slice(part);
    }
    merged.extend_from_slice(b"</body>");
    // Build <guide> element with cover and/or ToC references.
    // The ToC href uses fragment syntax so apply_filepos_links converts it to
    // filepos=NNNNNNNNNN, which is how Kindle locates the HTML ToC page.
    let toc_anchor: Option<String> = guide_toc_href.as_ref().and_then(|href| {
        let file_part = href.split('#').next().unwrap_or(href.as_str());
        let zip_path = resolve_zip_path(&opf_dir, file_part);
        spine_prefix_map.get(&zip_path).cloned()
    });
    let has_guide = cover_bytes.is_some() || toc_anchor.is_some();
    if has_guide {
        merged.extend_from_slice(b"<guide>");
        if cover_bytes.is_some() {
            merged.extend_from_slice(b"<reference type=\"cover\" title=\"Cover\" href=\"kindle:embed:0001?mime=image/jpeg\"/>");
        }
        if let Some(ref anchor) = toc_anchor {
            write!(merged, "<reference type=\"toc\" title=\"Table of Contents\" href=\"#{anchor}\"/>")?;
        }
        merged.extend_from_slice(b"</guide>");
    }
    merged.extend_from_slice(b"</html>");

    // 1b. Convert href="#anchor" → filepos=NNNNNNNNNN for Kindle MOBI6 navigation.
    // Also returns the anchor→bytepos map used to build the NCX INDX record.
    let (merged, id_to_pos) = apply_filepos_links(&merged);

    // 2. Collect images.
    let mut images: Vec<Vec<u8>> = Vec::new();
    if let Some(cb) = cover_bytes {
        images.push(cb.to_vec());
    }
    for img_href in &body_image_hrefs {
        if let Some(&idx) = name_to_idx.get(img_href) {
            let mut f = archive.by_index(idx)?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            images.push(buf);
        }
    }

    // 3. Parse NCX navigation document (if present) to build the Kindle ToC.
    let nav_points: Vec<NavPoint> = if let Some(ref nhref) = ncx_href {
        let ncx_zip_path = resolve_zip_path(&opf_dir, nhref);
        if let Some(&idx) = name_to_idx.get(&ncx_zip_path) {
            let mut f = archive.by_index(idx)?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            let ncx_dir = ncx_zip_path.rfind('/').map(|p| ncx_zip_path[..p].to_string()).unwrap_or_default();
            parse_ncx(&buf, &ncx_dir, &spine_prefix_map)
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    // 4. Write PalmDB.
    write_palmdb(dest, sidecar, &merged, &images, cover_bytes.is_some(), &nav_points, &id_to_pos)
}

// ── HTML helpers
// ──────────────────────────────────────────────────────────────

/// Walk HTML bytes and collect all `src` attribute values from `<img>`
/// elements.
fn collect_img_srcs(html: &[u8]) -> Vec<String> {
    let mut reader = Reader::from_reader(html);
    reader.config_mut().check_end_names = false;
    let mut buf = Vec::new();
    let mut srcs = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                let local = local_name_lower(e.name().as_ref());
                if local == "img" {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"src"
                            && let Ok(v) = std::str::from_utf8(attr.value.as_ref())
                        {
                            srcs.push(v.to_string());
                        }
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    srcs
}

/// Clean an HTML document for inclusion in MOBI:
/// - Extracts only `<body>` content (strips head, html wrappers).
/// - Rewrites `<img src="...">` to `kindle:embed:XXXX?mime=image/jpeg`.
/// - Strips `<style>` block elements and `<link rel="stylesheet">`.
/// - Rewrites `id` attributes by prefixing them with `current_prefix` to avoid
///   collisions when multiple spine documents are merged.
/// - Rewrites `href` links: same-file fragments become `#{prefix}_{fragment}`,
///   cross-document links become `#{target_prefix}_{fragment}` or
///   `#{target_prefix}` for bare filenames.
fn clean_html(
    html: &[u8],
    src_to_record: &HashMap<String, u32>,
    current_prefix: &str,
    html_zip_dir: &str,
    spine_prefix_map: &HashMap<String, String>,
) -> Result<Vec<u8>, Error> {
    let mut reader = Reader::from_reader(html);
    reader.config_mut().check_end_names = false;
    let mut buf = Vec::new();
    let mut out = Vec::with_capacity(html.len());

    let mut in_body = false;
    let mut anchor_emitted = false; // true once we've emitted <a name="PREFIX"> for this chapter
    let mut skip_style_depth: u32 = 0; // >0 means we are inside a <style> block to skip

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = local_name_lower(e.name().as_ref());

                if local == "body" {
                    in_body = true;
                    continue;
                }
                if !in_body {
                    continue;
                }

                // Skip <style> blocks.
                if local == "style" {
                    skip_style_depth += 1;
                    continue;
                }
                if skip_style_depth > 0 {
                    // Track nesting for any nested elements (unusual but safe).
                    skip_style_depth += 1;
                    continue;
                }

                // Skip <link rel="stylesheet">.
                if local == "link" {
                    let mut is_stylesheet = false;
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"rel"
                            && let Ok(v) = std::str::from_utf8(attr.value.as_ref())
                            && v.eq_ignore_ascii_case("stylesheet")
                        {
                            is_stylesheet = true;
                        }
                    }
                    if is_stylesheet {
                        continue;
                    }
                }

                // Rewrite <img src="...">
                if local == "img" {
                    if !anchor_emitted {
                        write!(out, "<a name=\"{current_prefix}\"></a>")?;
                        anchor_emitted = true;
                    }
                    let new_src = img_src_to_kindle_embed(e, src_to_record);
                    write!(out, "<img src=\"{new_src}\"/>")?;
                    continue;
                }

                // Rewrite <a> — preserve all attributes, rewrite href and id.
                if local == "a" {
                    if !anchor_emitted {
                        write!(out, "<a name=\"{current_prefix}\"></a>")?;
                        anchor_emitted = true;
                    }
                    out.extend_from_slice(b"<a");
                    for attr in e.attributes().flatten() {
                        out.extend_from_slice(b" ");
                        out.extend_from_slice(attr.key.as_ref());
                        out.extend_from_slice(b"=\"");
                        if attr.key.as_ref() == b"href" {
                            if let Ok(href) = std::str::from_utf8(attr.value.as_ref()) {
                                let new_href = rewrite_href(href, current_prefix, html_zip_dir, spine_prefix_map);
                                write!(out, "{new_href}")?;
                            } else {
                                out.extend_from_slice(attr.value.as_ref());
                            }
                        } else if attr.key.as_ref() == b"id" {
                            if let Ok(id_val) = std::str::from_utf8(attr.value.as_ref()) {
                                write!(out, "{current_prefix}_{id_val}")?;
                            } else {
                                out.extend_from_slice(attr.value.as_ref());
                            }
                        } else {
                            out.extend_from_slice(attr.value.as_ref());
                        }
                        out.extend_from_slice(b"\"");
                    }
                    out.extend_from_slice(b">");
                    continue;
                }

                // Passthrough — rewrite id attributes to avoid collisions.
                if !anchor_emitted {
                    write!(out, "<a name=\"{current_prefix}\"></a>")?;
                    anchor_emitted = true;
                }
                out.extend_from_slice(b"<");
                out.extend_from_slice(e.name().as_ref());
                for attr in e.attributes().flatten() {
                    out.extend_from_slice(b" ");
                    out.extend_from_slice(attr.key.as_ref());
                    out.extend_from_slice(b"=\"");
                    if attr.key.as_ref() == b"id" {
                        if let Ok(id_val) = std::str::from_utf8(attr.value.as_ref()) {
                            write!(out, "{current_prefix}_{id_val}")?;
                        } else {
                            out.extend_from_slice(attr.value.as_ref());
                        }
                    } else {
                        out.extend_from_slice(attr.value.as_ref());
                    }
                    out.extend_from_slice(b"\"");
                }
                out.extend_from_slice(b">");
            }
            Ok(Event::Empty(ref e)) => {
                let local = local_name_lower(e.name().as_ref());

                if !in_body {
                    continue;
                }
                if skip_style_depth > 0 {
                    continue;
                }

                // Skip <link rel="stylesheet"/>.
                if local == "link" {
                    let mut is_stylesheet = false;
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"rel"
                            && let Ok(v) = std::str::from_utf8(attr.value.as_ref())
                            && v.eq_ignore_ascii_case("stylesheet")
                        {
                            is_stylesheet = true;
                        }
                    }
                    if is_stylesheet {
                        continue;
                    }
                }

                if local == "img" {
                    if !anchor_emitted {
                        write!(out, "<a name=\"{current_prefix}\"></a>")?;
                        anchor_emitted = true;
                    }
                    let new_src = img_src_to_kindle_embed(e, src_to_record);
                    write!(out, "<img src=\"{new_src}\"/>")?;
                    continue;
                }

                // Self-closing <a/> (anchor target with id, no href).
                if local == "a" {
                    if !anchor_emitted {
                        write!(out, "<a name=\"{current_prefix}\"></a>")?;
                        anchor_emitted = true;
                    }
                    out.extend_from_slice(b"<a");
                    for attr in e.attributes().flatten() {
                        out.extend_from_slice(b" ");
                        out.extend_from_slice(attr.key.as_ref());
                        out.extend_from_slice(b"=\"");
                        if attr.key.as_ref() == b"id" {
                            if let Ok(id_val) = std::str::from_utf8(attr.value.as_ref()) {
                                write!(out, "{current_prefix}_{id_val}")?;
                            } else {
                                out.extend_from_slice(attr.value.as_ref());
                            }
                        } else {
                            out.extend_from_slice(attr.value.as_ref());
                        }
                        out.extend_from_slice(b"\"");
                    }
                    out.extend_from_slice(b"/>");
                    continue;
                }

                // Self-closing passthrough — rewrite id attributes.
                if !anchor_emitted {
                    write!(out, "<a name=\"{current_prefix}\"></a>")?;
                    anchor_emitted = true;
                }
                out.extend_from_slice(b"<");
                out.extend_from_slice(e.name().as_ref());
                for attr in e.attributes().flatten() {
                    out.extend_from_slice(b" ");
                    out.extend_from_slice(attr.key.as_ref());
                    out.extend_from_slice(b"=\"");
                    if attr.key.as_ref() == b"id" {
                        if let Ok(id_val) = std::str::from_utf8(attr.value.as_ref()) {
                            write!(out, "{current_prefix}_{id_val}")?;
                        } else {
                            out.extend_from_slice(attr.value.as_ref());
                        }
                    } else {
                        out.extend_from_slice(attr.value.as_ref());
                    }
                    out.extend_from_slice(b"\"");
                }
                out.extend_from_slice(b"/>");
            }
            Ok(Event::End(ref e)) => {
                let local = local_name_lower(e.name().as_ref());

                if local == "body" {
                    in_body = false;
                    continue;
                }
                if !in_body {
                    continue;
                }

                if local == "style" {
                    skip_style_depth = skip_style_depth.saturating_sub(1);
                    continue;
                }
                if skip_style_depth > 0 {
                    skip_style_depth = skip_style_depth.saturating_sub(1);
                    continue;
                }

                out.extend_from_slice(b"</");
                out.extend_from_slice(e.name().as_ref());
                out.extend_from_slice(b">");
            }
            Ok(Event::Text(ref e)) if in_body && skip_style_depth == 0 => {
                out.extend_from_slice(e.as_ref());
            }
            Ok(Event::CData(ref e)) if in_body && skip_style_depth == 0 => {
                out.extend_from_slice(e.as_ref());
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    Ok(out)
}

/// Build a `kindle:embed:XXXX?mime=image/jpeg` URL from an `<img>` element,
/// looking up the `src` attribute in `src_to_record`.  Falls back to a
/// placeholder if the src is not found.
fn img_src_to_kindle_embed(e: &quick_xml::events::BytesStart<'_>, src_to_record: &HashMap<String, u32>) -> String {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == b"src"
            && let Ok(src) = std::str::from_utf8(attr.value.as_ref())
        {
            let record = src_to_record.get(src).copied().unwrap_or(1);
            return format!("kindle:embed:{record:04}?mime=image/jpeg");
        }
    }
    "kindle:embed:0001?mime=image/jpeg".to_string()
}

/// Post-process the merged HTML for MOBI6 navigation.
///
/// MOBI6 / Kindle does not navigate via `href="#fragment"` — it uses
/// `filepos=NNNNNNNNNN` (10-digit zero-padded byte offset into the HTML
/// text stream).  This function performs a three-pass transform:
///
/// 1. Scan the HTML for every `id="NAME"` and record the byte offset of the
///    opening `<` of the enclosing tag.
/// 2. Rebuild the HTML, replacing each `href="#NAME"` with the placeholder
///    `filepos=0000000000` (fixed width so later positions stay stable).
/// 3. Re-scan the rebuilt HTML to find final anchor positions, then fill in the
///    actual 10-digit values.
fn apply_filepos_links(html: &[u8]) -> (Vec<u8>, HashMap<String, usize>) {
    const ID_PAT: &[u8] = b" id=\"";
    const HREF_FRAG_PAT: &[u8] = b"href=\"#";
    const FILEPOS_PLACEHOLDER: &[u8] = b"filepos=0000000000";

    // ── Pass 2: rebuild HTML, replacing href="#NAME" → filepos=0000000000.
    // Record (position_of_digits_in_out, fragment_name) for fixup later.
    let mut out: Vec<u8> = Vec::with_capacity(html.len());
    let mut fixups: Vec<(usize, String)> = Vec::new();
    let mut i = 0;
    while i < html.len() {
        if html[i..].starts_with(HREF_FRAG_PAT) {
            let frag_start = i + HREF_FRAG_PAT.len();
            if let Some(rel_end) = html[frag_start..].iter().position(|&b| b == b'"') {
                let frag = std::str::from_utf8(&html[frag_start..frag_start + rel_end]).unwrap_or("").to_string();
                let digits_pos = out.len() + b"filepos=".len();
                out.extend_from_slice(FILEPOS_PLACEHOLDER);
                fixups.push((digits_pos, frag));
                i = frag_start + rel_end + 1; // skip past closing "
                continue;
            }
        }
        out.push(html[i]);
        i += 1;
    }

    // ── Pass 3: find byte offset of every anchor in the rebuilt HTML.
    // Recognises both ` id="NAME"` (elements, mbp:pagebreak) and
    // ` name="NAME"` (<a name> first-item anchors).
    let mut id_to_pos: HashMap<String, usize> = HashMap::new();
    const NAME_PAT: &[u8] = b" name=\"";
    let mut pos = 0;
    while pos < out.len() {
        let (pat_len, val_start) = if out[pos..].starts_with(ID_PAT) {
            (ID_PAT.len(), pos + ID_PAT.len())
        } else if out[pos..].starts_with(NAME_PAT) {
            (NAME_PAT.len(), pos + NAME_PAT.len())
        } else {
            pos += 1;
            continue;
        };
        let _ = pat_len; // used above
        if let Some(rel_end) = out[val_start..].iter().position(|&b| b == b'"') {
            let anchor_val = std::str::from_utf8(&out[val_start..val_start + rel_end]).unwrap_or("").to_string();
            // Backtrack to the opening '<' of the enclosing tag.
            let tag_start = out[..pos].iter().rposition(|&b| b == b'<').unwrap_or(0);
            // id= takes precedence over name= so only insert if not already present.
            id_to_pos.entry(anchor_val).or_insert(tag_start);
            pos = val_start + rel_end + 1;
            continue;
        }
        pos += 1;
    }

    // ── Pass 4: fill placeholders with real positions.
    for (digits_pos, frag) in fixups {
        let target = id_to_pos.get(&frag).copied().unwrap_or(0);
        let digits = format!("{target:010}");
        out[digits_pos..digits_pos + 10].copy_from_slice(digits.as_bytes());
    }

    (out, id_to_pos)
}

/// Rewrite an `href` value for the merged MOBI document.
///
/// * `#X`           → `#{prefix}_X`          same-file fragment
/// * `file.xhtml#X` → `#{file_prefix}_X`     cross-document with fragment
/// * `file.xhtml`   → `#{file_prefix}`        cross-document top-of-chapter
/// * external       → unchanged
fn rewrite_href(href: &str, current_prefix: &str, html_zip_dir: &str, spine_prefix_map: &HashMap<String, String>) -> String {
    if href.is_empty() || href == "#" {
        return "#".to_string();
    }
    // External links.
    if href.starts_with("http://") || href.starts_with("https://") || href.starts_with("mailto:") {
        return href.to_string();
    }
    // Same-file fragment: #X → #{prefix}_X
    if let Some(fragment) = href.strip_prefix('#') {
        return format!("#{current_prefix}_{fragment}");
    }
    // Cross-document with fragment: file.xhtml#anchor → #{target_prefix}_anchor
    if let Some(hash_pos) = href.find('#') {
        let file_part = &href[..hash_pos];
        let fragment = &href[hash_pos + 1..];
        let zip_path = resolve_zip_path(html_zip_dir, file_part);
        if let Some(prefix) = spine_prefix_map.get(&zip_path) {
            return format!("#{prefix}_{fragment}");
        }
        return format!("#{fragment}");
    }
    // Bare filename: file.xhtml → #{target_prefix}
    let zip_path = resolve_zip_path(html_zip_dir, href);
    if let Some(prefix) = spine_prefix_map.get(&zip_path) {
        return format!("#{prefix}");
    }
    // Unresolvable — keep as-is.
    href.to_string()
}

/// Derive a stable anchor prefix from a zip path.
/// `OEBPS/Text/chapter2.xhtml` → `chapter2`
fn spine_prefix_for(zip_path: &str) -> String {
    let basename = zip_path.rsplit('/').next().unwrap_or(zip_path);
    let stem = if let Some(pos) = basename.rfind('.') { &basename[..pos] } else { basename };
    stem.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect()
}

// ── NCX / ToC helpers
// ────────────────────────────────────────────────────────────────

/// Parse an EPUB2 NCX file and return navPoints in playOrder.
///
/// Each navPoint's `content src` is resolved to the corresponding anchor name
/// in the merged MOBI HTML using the spine prefix map.
fn parse_ncx(ncx: &[u8], ncx_dir: &str, spine_prefix_map: &HashMap<String, String>) -> Vec<NavPoint> {
    let mut reader = Reader::from_reader(ncx);
    reader.config_mut().trim_text(true);
    reader.config_mut().check_end_names = false;
    let mut buf = Vec::new();

    struct Pending {
        play_order: u32,
        label: String,
        src: String,
    }
    let mut stack: Vec<Pending> = Vec::new();
    let mut result: Vec<(u32, String, String)> = Vec::new(); // (play_order, label, src)
    let mut in_nav_label = false;
    let mut in_text = false;

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = local_name_lower(e.name().as_ref());
                match local.as_str() {
                    "navpoint" => {
                        let mut play_order = 0u32;
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref().eq_ignore_ascii_case(b"playorder")
                                && let Ok(v) = std::str::from_utf8(attr.value.as_ref())
                            {
                                play_order = v.trim().parse().unwrap_or(0);
                            }
                        }
                        stack.push(Pending {
                            play_order,
                            label: String::new(),
                            src: String::new(),
                        });
                    }
                    "navlabel" => in_nav_label = true,
                    "text" if in_nav_label => in_text = true,
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = local_name_lower(e.name().as_ref());
                if local == "content"
                    && let Some(top) = stack.last_mut()
                {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"src"
                            && let Ok(v) = std::str::from_utf8(attr.value.as_ref())
                        {
                            top.src = v.to_string();
                        }
                    }
                }
            }
            Ok(Event::Text(ref e)) if in_text => {
                if let Some(top) = stack.last_mut()
                    && let Ok(text) = std::str::from_utf8(e.as_ref())
                {
                    top.label.push_str(text);
                }
            }
            Ok(Event::End(ref e)) => {
                let local = local_name_lower(e.name().as_ref());
                match local.as_str() {
                    "navpoint" => {
                        if let Some(p) = stack.pop() {
                            let label = p.label.trim().to_string();
                            if !label.is_empty() && !p.src.is_empty() {
                                result.push((p.play_order, label, p.src));
                            }
                        }
                    }
                    "navlabel" => in_nav_label = false,
                    "text" => in_text = false,
                    _ => {}
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    result.sort_by_key(|(order, _, _)| *order);
    result
        .into_iter()
        .map(|(_, label, src)| {
            let anchor = resolve_ncx_src(&src, ncx_dir, spine_prefix_map);
            NavPoint { label, anchor }
        })
        .collect()
}

/// Resolve a navPoint `content src` to an anchor name in the merged MOBI HTML.
///
/// * `chapter.html`          → prefix of that spine item (bare chapter anchor)
/// * `chapter.html#section`  → `{prefix}_{section}`
fn resolve_ncx_src(src: &str, ncx_dir: &str, spine_prefix_map: &HashMap<String, String>) -> String {
    let (file_part, fragment) = if let Some(pos) = src.find('#') {
        (&src[..pos], Some(&src[pos + 1..]))
    } else {
        (src, None)
    };
    let zip_path = resolve_zip_path(ncx_dir, file_part);
    if let Some(prefix) = spine_prefix_map.get(&zip_path) {
        match fragment {
            Some(frag) => format!("{prefix}_{frag}"),
            None => prefix.clone(),
        }
    } else {
        match fragment {
            Some(frag) => frag.to_string(),
            None => spine_prefix_for(file_part),
        }
    }
}

/// Encode an integer as a hex string with a length prefix byte, matching
/// Calibre's `encode_number_as_hex`.  The hex string is always an even number
/// of ASCII hex digits (upper-case).  Used as INDX entry keys.
///
/// Examples: 0 → [2, '0', '0'], 1 → [2, '0', '1'], 101 → [2, '6', '5'],
/// 256 → [4, '0', '1', '0', '0'].
fn encode_number_as_hex(num: usize) -> Vec<u8> {
    let hex = format!("{num:X}");
    let hex = if hex.len() % 2 != 0 { format!("0{hex}") } else { hex };
    let mut out = Vec::with_capacity(1 + hex.len());
    out.push(hex.len() as u8);
    out.extend_from_slice(hex.as_bytes());
    out
}

/// Encode a value as a MOBI variable-length integer (big-endian, MSB=1 means
/// more bytes follow).
fn write_varlen(buf: &mut Vec<u8>, mut value: usize) {
    let mut bytes = [0u8; 5];
    let mut n = 0;
    loop {
        bytes[n] = (value & 0x7F) as u8;
        n += 1;
        value >>= 7;
        if value == 0 {
            break;
        }
    }
    // MOBI VarLen: write bytes MSB-first; the LAST byte carries the stop bit
    // (0x80), all preceding bytes have MSB=0 (continuation).  This is the
    // reverse of standard protobuf VarInt and matches what Kindle/Calibre uses.
    for i in (0..n).rev() {
        let b = bytes[i];
        buf.push(if i == 0 { b | 0x80 } else { b });
    }
}

/// Build the CNCX record: a packed sequence of VarLen(length) + UTF-8 label
/// bytes for each navPoint.  Returns the raw record bytes and the byte offset
/// of each label within that record (needed for INDX tag-3 values).
fn build_cncx(nav_points: &[NavPoint]) -> (Vec<u8>, Vec<usize>) {
    let mut bytes = Vec::new();
    let mut offsets = Vec::with_capacity(nav_points.len());
    for np in nav_points {
        offsets.push(bytes.len());
        let label = np.label.as_bytes();
        write_varlen(&mut bytes, label.len());
        bytes.extend_from_slice(label);
    }
    (bytes, offsets)
}

/// Build the NCX INDX records for the Kindle Table of Contents.
///
/// Returns `(header_indx, leaf_indx)`.  Kindle expects a two-level B-tree:
///   ncxRecord+0 = header INDX (indexType=0): TAGX + routing entry + IDXT.
///   ncxRecord+1 = leaf  INDX (indexType=1): actual navpoint entries + IDXT.
///   ncxRecord+2 = CNCX: packed label strings (located by Kindle at ncxRecord+2
///                 by convention; cncxRecord=0 in both INDX headers).
///
/// Tags (matching Calibre):
///   1 = pos   (filepos byte offset in the uncompressed text)
///   2 = size  (byte length of this navpoint's section)
///   3 = noff  (byte offset of this entry's label in the CNCX record)
///   4 = depth (always 1 for a flat ToC)
///
/// `total_text_len` is used to compute the section size of the last navpoint.
fn build_indx(nav_points: &[NavPoint], cncx_offsets: &[usize], id_to_pos: &HashMap<String, usize>, total_text_len: usize) -> (Vec<u8>, Vec<u8>) {
    // TAGX — lives in the header INDX; the leaf INDX references it implicitly.
    const TAGX: &[u8] = &[
        b'T', b'A', b'G', b'X', 0, 0, 0, 32, // tagx_len = 32 bytes total
        0, 0, 0, 1, // control_byte_count = 1
        1, 1, 0x01, 0, // tag 1 = pos,   bitmask bit 0
        2, 1, 0x02, 0, // tag 2 = size,  bitmask bit 1
        3, 1, 0x04, 0, // tag 3 = noff,  bitmask bit 2
        4, 1, 0x08, 0, // tag 4 = depth, bitmask bit 3
        0, 0, 0x00, 1, // terminator
    ];

    // ── Filepos list ─────────────────────────────────────────────────────────
    let filepos_list: Vec<usize> = nav_points.iter().map(|np| id_to_pos.get(&np.anchor).copied().unwrap_or(0)).collect();

    // ── Build leaf entries ───────────────────────────────────────────────────
    // Each entry key uses encode_number_as_hex (matching Calibre and all
    // working Kindle MOBIs): a length-prefixed even-digit hex string.
    // e.g. 0→[2,'0','0'], 101→[2,'6','5'], 256→[4,'0','1','0','0'].
    let leaf_entries_start: usize = 192;
    let mut leaf_entries: Vec<Vec<u8>> = Vec::new();
    for (i, (_np, &cncx_off)) in nav_points.iter().zip(cncx_offsets.iter()).enumerate() {
        let key = encode_number_as_hex(i);
        let filepos = filepos_list[i];
        let next_pos = filepos_list.get(i + 1).copied().unwrap_or(total_text_len);
        let section_size = next_pos.saturating_sub(filepos);

        let mut entry = key;
        entry.push(0x0F); // control byte: all 4 tags present
        write_varlen(&mut entry, filepos); // tag 1 (pos)
        write_varlen(&mut entry, section_size); // tag 2 (size)
        write_varlen(&mut entry, cncx_off); // tag 3 (noff)
        write_varlen(&mut entry, 0); // tag 4 (depth = 0, top-level)
        leaf_entries.push(entry);
    }

    // Leaf IDXT: entry offsets from start of leaf record.
    let mut leaf_entry_offsets: Vec<u16> = Vec::new();
    let mut leaf_pos = leaf_entries_start;
    for entry in &leaf_entries {
        leaf_entry_offsets.push(leaf_pos as u16);
        leaf_pos += entry.len();
    }
    let leaf_idxt_offset = leaf_pos as u32;

    // ── Header INDX routing entry ────────────────────────────────────────────
    // Calibre format (verified against working Kindle MOBIs):
    //   encode_number_as_hex(last_index) + u16(entry_count) + padding to 4-byte
    // boundary. The entry area always starts at offset 192+32=224.  Padding
    // aligns the IDXT to a 4-byte boundary; for up to 65535 entries this is
    // always at 232.
    let last_index = nav_points.len().saturating_sub(1);
    let last_key = encode_number_as_hex(last_index);
    let count_bytes = (nav_points.len() as u16).to_be_bytes();
    let entry_payload: Vec<u8> = last_key.iter().copied().chain(count_bytes).collect();
    let entry_area_start: u32 = 192 + TAGX.len() as u32; // = 224
    let raw_end = entry_area_start as usize + entry_payload.len();
    let pad = (4usize.wrapping_sub(raw_end % 4)) % 4;
    let hdr_idxt_offset: u32 = raw_end as u32 + pad as u32;
    let hdr_entry_offset: u16 = entry_area_start as u16; // IDXT points here (start of routing entry)

    // ── Assemble header INDX ─────────────────────────────────────────────────
    let mut hdr_header = vec![0u8; 192];
    hdr_header[0x00..0x04].copy_from_slice(b"INDX");
    hdr_header[0x04..0x08].copy_from_slice(&192u32.to_be_bytes()); // headerLength
    // indexType at 0x10 = 2 (inflection), matching Calibre header INDX
    hdr_header[0x10..0x14].copy_from_slice(&2u32.to_be_bytes());
    hdr_header[0x14..0x18].copy_from_slice(&hdr_idxt_offset.to_be_bytes()); // idxtOffset
    hdr_header[0x18..0x1C].copy_from_slice(&1u32.to_be_bytes()); // numRecords = 1 leaf
    hdr_header[0x1C..0x20].copy_from_slice(&65001u32.to_be_bytes()); // encoding UTF-8
    hdr_header[0x20..0x24].copy_from_slice(&0xFFFF_FFFFu32.to_be_bytes()); // index_language sentinel
    hdr_header[0x24..0x28].copy_from_slice(&(nav_points.len() as u32).to_be_bytes()); // totalEntries
    hdr_header[0x34..0x38].copy_from_slice(&1u32.to_be_bytes()); // cncxCount = 1 CNCX record
    // tagxOffset at 0xB4: TAGX starts right after the 192-byte header.
    hdr_header[0xB4..0xB8].copy_from_slice(&192u32.to_be_bytes());

    let mut hdr_idxt = Vec::new();
    hdr_idxt.extend_from_slice(b"IDXT");
    hdr_idxt.extend_from_slice(&hdr_entry_offset.to_be_bytes());
    hdr_idxt.extend_from_slice(&[0u8]); // 1 byte padding (Calibre convention)

    let mut header_indx = hdr_header;
    header_indx.extend_from_slice(TAGX);
    header_indx.extend_from_slice(&entry_payload);
    header_indx.extend_from_slice(&vec![0u8; pad]); // alignment padding
    header_indx.extend_from_slice(&hdr_idxt);

    // ── Assemble leaf INDX ───────────────────────────────────────────────────
    let mut leaf_header = vec![0u8; 192];
    leaf_header[0x00..0x04].copy_from_slice(b"INDX");
    leaf_header[0x04..0x08].copy_from_slice(&192u32.to_be_bytes()); // headerLength
    leaf_header[0x0C..0x10].copy_from_slice(&1u32.to_be_bytes()); // indexType = 1 (leaf node)
    leaf_header[0x14..0x18].copy_from_slice(&leaf_idxt_offset.to_be_bytes()); // idxtOffset
    leaf_header[0x18..0x1C].copy_from_slice(&(nav_points.len() as u32).to_be_bytes()); // numEntries
    // Leaf INDX: encoding and language fields are sentinels (Calibre convention)
    leaf_header[0x1C..0x20].copy_from_slice(&0xFFFF_FFFFu32.to_be_bytes()); // encoding (sentinel)
    leaf_header[0x20..0x24].copy_from_slice(&0xFFFF_FFFFu32.to_be_bytes()); // language (sentinel)
    // tagxOffset = 0: leaf uses the header INDX's TAGX
    // cncxRecord = 0: Kindle locates CNCX at ncxRecord+2 by convention

    let mut leaf_idxt = Vec::new();
    leaf_idxt.extend_from_slice(b"IDXT");
    for off in &leaf_entry_offsets {
        leaf_idxt.extend_from_slice(&off.to_be_bytes());
    }
    // Pad to 4-byte boundary (align_block convention from Calibre).
    let idxt_pad = (4usize.wrapping_sub(leaf_idxt.len() % 4)) % 4;
    leaf_idxt.extend_from_slice(&vec![0u8; idxt_pad]);

    let mut leaf_indx = leaf_header;
    for entry in &leaf_entries {
        leaf_indx.extend_from_slice(entry);
    }
    leaf_indx.extend_from_slice(&leaf_idxt);

    (header_indx, leaf_indx)
}

// ── OPF parsing
// ───────────────────────────────────────────────────────────────

/// Parse an OPF document and return:
/// - manifest: id → href map
/// - spine: ordered idref list
/// - ncx_href: optional href of the NCX navigation document
/// - guide_toc_href: optional href of the `<reference type="toc">` guide entry
#[allow(clippy::type_complexity)]
fn parse_opf_manifest_and_spine(opf: &[u8]) -> Result<(HashMap<String, String>, Vec<String>, Option<String>, Option<String>), Error> {
    let mut reader = Reader::from_reader(opf);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut manifest: HashMap<String, String> = HashMap::new();
    let mut spine: Vec<String> = Vec::new();
    let mut in_manifest = false;
    let mut in_spine = false;
    let mut in_guide = false;
    // href of the NCX item (set when we see media-type="application/x-dtbncx+xml").
    let mut ncx_href: Option<String> = None;
    // Fallback: spine toc="..." attribute lets us look up the NCX id in manifest.
    let mut spine_toc_id: Option<String> = None;
    // href of the OPF guide's type="toc" reference (HTML ToC page).
    let mut guide_toc_href: Option<String> = None;

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = local_name_lower(e.name().as_ref());
                match local.as_str() {
                    "manifest" => in_manifest = true,
                    "guide" => in_guide = true,
                    "spine" => {
                        in_spine = true;
                        // <spine toc="ncx-id"> — fallback if media-type lookup fails.
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"toc" {
                                spine_toc_id = std::str::from_utf8(attr.value.as_ref()).ok().map(String::from);
                            }
                        }
                    }
                    "item" if in_manifest => {
                        let mut id = None;
                        let mut href = None;
                        let mut media_type = None;
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"id" => id = std::str::from_utf8(attr.value.as_ref()).ok().map(String::from),
                                b"href" => href = std::str::from_utf8(attr.value.as_ref()).ok().map(String::from),
                                b"media-type" => media_type = std::str::from_utf8(attr.value.as_ref()).ok().map(String::from),
                                _ => {}
                            }
                        }
                        if let (Some(id), Some(href)) = (id, href) {
                            if media_type.as_deref() == Some("application/x-dtbncx+xml") {
                                ncx_href = Some(href.clone());
                            }
                            manifest.insert(id, href);
                        }
                    }
                    "itemref" if in_spine => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"idref"
                                && let Ok(v) = std::str::from_utf8(attr.value.as_ref())
                            {
                                spine.push(v.to_string());
                            }
                        }
                    }
                    "reference" if in_guide => {
                        let mut ref_type = None;
                        let mut ref_href = None;
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"type" => ref_type = std::str::from_utf8(attr.value.as_ref()).ok().map(String::from),
                                b"href" => ref_href = std::str::from_utf8(attr.value.as_ref()).ok().map(String::from),
                                _ => {}
                            }
                        }
                        if ref_type.as_deref() == Some("toc") {
                            guide_toc_href = ref_href;
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = local_name_lower(e.name().as_ref());
                match local.as_str() {
                    "item" if in_manifest => {
                        let mut id = None;
                        let mut href = None;
                        let mut media_type = None;
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"id" => id = std::str::from_utf8(attr.value.as_ref()).ok().map(String::from),
                                b"href" => href = std::str::from_utf8(attr.value.as_ref()).ok().map(String::from),
                                b"media-type" => media_type = std::str::from_utf8(attr.value.as_ref()).ok().map(String::from),
                                _ => {}
                            }
                        }
                        if let (Some(id), Some(href)) = (id, href) {
                            if media_type.as_deref() == Some("application/x-dtbncx+xml") {
                                ncx_href = Some(href.clone());
                            }
                            manifest.insert(id, href);
                        }
                    }
                    "spine" => {
                        // Self-closing <spine toc="..."/> (unusual but possible).
                        in_spine = true;
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"toc" {
                                spine_toc_id = std::str::from_utf8(attr.value.as_ref()).ok().map(String::from);
                            }
                        }
                    }
                    "itemref" if in_spine => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"idref"
                                && let Ok(v) = std::str::from_utf8(attr.value.as_ref())
                            {
                                spine.push(v.to_string());
                            }
                        }
                    }
                    "reference" if in_guide => {
                        let mut ref_type = None;
                        let mut ref_href = None;
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"type" => ref_type = std::str::from_utf8(attr.value.as_ref()).ok().map(String::from),
                                b"href" => ref_href = std::str::from_utf8(attr.value.as_ref()).ok().map(String::from),
                                _ => {}
                            }
                        }
                        if ref_type.as_deref() == Some("toc") {
                            guide_toc_href = ref_href;
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let local = local_name_lower(e.name().as_ref());
                match local.as_str() {
                    "manifest" => in_manifest = false,
                    "spine" => in_spine = false,
                    "guide" => in_guide = false,
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(e.into()),
            _ => {}
        }
    }

    // Fallback: resolve NCX href via spine toc= attribute if media-type lookup
    // missed it.
    if ncx_href.is_none()
        && let Some(ref toc_id) = spine_toc_id
    {
        ncx_href = manifest.get(toc_id).cloned();
    }

    Ok((manifest, spine, ncx_href, guide_toc_href))
}

// ── PalmDB writer
// ─────────────────────────────────────────────────────────────

/// Write a minimal MOBI6 / PalmDB file to `dest`.
fn write_palmdb(
    dest: &Path,
    sidecar: &BookSidecar,
    html: &[u8],
    images: &[Vec<u8>],
    has_cover: bool,
    nav_points: &[NavPoint],
    id_to_pos: &HashMap<String, usize>,
) -> Result<(), Error> {
    // Chunk HTML into records of at most 4096 bytes.
    const MAX_TEXT_RECORD: usize = 4096;
    let text_chunks: Vec<&[u8]> = html.chunks(MAX_TEXT_RECORD).collect();
    let text_record_count = text_chunks.len();
    let total_text_len = html.len() as u32;

    let image_record_start = text_record_count + 1; // record 0 is header
    let first_image_index = image_record_start as u32;
    let cover_record_index = if has_cover { image_record_start as u32 } else { 0xFFFF_FFFF };

    // Build NCX INDX (header + leaf) + CNCX records (only when nav_points are
    // available). Record layout: ncxRecord+0 = header INDX, ncxRecord+1 = leaf
    // INDX, ncxRecord+2 = CNCX.
    let (header_indx_record, leaf_indx_record, cncx_record, ncx_record_index) = if nav_points.is_empty() {
        (None, None, None, 0xFFFF_FFFFu32)
    } else {
        let ncx_idx = (1 + text_record_count + images.len()) as u32;
        let (cncx_bytes, cncx_offsets) = build_cncx(nav_points);
        let (header_indx_bytes, leaf_indx_bytes) = build_indx(nav_points, &cncx_offsets, id_to_pos, total_text_len as usize);
        (Some(header_indx_bytes), Some(leaf_indx_bytes), Some(cncx_bytes), ncx_idx)
    };

    let ncx_extra = if header_indx_record.is_some() { 3 } else { 0 }; // header INDX + leaf INDX + CNCX
    let total_records = 1 + text_record_count + images.len() + ncx_extra;

    // Build EXTH block first (we need its size for record 0 layout).
    let exth_bytes = build_exth(sidecar, cover_record_index, has_cover);

    // Title bytes (UTF-8).
    let title_bytes = sidecar.title.as_bytes();

    // Build record 0: PalmDoc header + MOBI header + title + EXTH.
    let record0 = build_record0(
        total_text_len,
        text_record_count as u16,
        first_image_index,
        title_bytes,
        &exth_bytes,
        text_record_count as u32,
        image_record_start as u32,
        ncx_record_index,
    );

    // Compute record offsets.
    // PalmDB file layout:
    //   [0..78)      PalmDB header (name 32 + attrs/ver 4 + dates 16 +
    //                appInfo/sortInfo 8 + type/creator 8 + seed/nextList 8 +
    //                numRecords 2 = 78 bytes)
    //   [78..)       record list: numRecords × 8 bytes
    //   [78 + numRecords*8 ..) record data (must be even-aligned by 2 bytes)
    let palmdb_header_size: u32 = 78;
    let record_list_size: u32 = total_records as u32 * 8;
    let data_start: u32 = palmdb_header_size + record_list_size;
    // PalmDB spec requires data start at an even offset; add padding if needed.
    let data_start = if data_start.is_multiple_of(2) { data_start } else { data_start + 1 };

    let mut offsets: Vec<u32> = Vec::with_capacity(total_records);
    let mut current = data_start;
    offsets.push(current);
    current += record0.len() as u32;

    for chunk in &text_chunks {
        offsets.push(current);
        current += chunk.len() as u32;
    }
    for img in images {
        offsets.push(current);
        current += img.len() as u32;
    }
    if let Some(ref indx) = header_indx_record {
        offsets.push(current);
        current += indx.len() as u32;
    }
    if let Some(ref indx) = leaf_indx_record {
        offsets.push(current);
        current += indx.len() as u32;
    }
    if let Some(ref cncx) = cncx_record {
        offsets.push(current);
        current += cncx.len() as u32;
    }
    let _ = current; // final value not needed

    // Now write the file.
    let mut f = std::fs::File::create(dest)?;

    // ── PalmDB header (78 bytes) ─────────────────────────────────────────
    // name: 32 bytes, null-padded, truncated at 31.
    let mut name_buf = [0u8; 32];
    let name_bytes = sidecar.title.as_bytes();
    let copy_len = name_bytes.len().min(31);
    name_buf[..copy_len].copy_from_slice(&name_bytes[..copy_len]);
    f.write_all(&name_buf)?;

    f.write_all(&0u16.to_be_bytes())?; // attributes
    f.write_all(&0u16.to_be_bytes())?; // version
    f.write_all(&0u32.to_be_bytes())?; // creationDate
    f.write_all(&0u32.to_be_bytes())?; // modificationDate
    f.write_all(&0u32.to_be_bytes())?; // lastBackupDate
    f.write_all(&0u32.to_be_bytes())?; // modificationNumber
    f.write_all(&0u32.to_be_bytes())?; // appInfoID
    f.write_all(&0u32.to_be_bytes())?; // sortInfoID
    f.write_all(b"BOOK")?; // type
    f.write_all(b"MOBI")?; // creator
    f.write_all(&0u32.to_be_bytes())?; // uniqueIDSeed
    f.write_all(&0u32.to_be_bytes())?; // nextRecordListID
    f.write_all(&(total_records as u16).to_be_bytes())?; // numRecords

    // ── Record list ──────────────────────────────────────────────────────
    for (i, &offset) in offsets.iter().enumerate() {
        f.write_all(&offset.to_be_bytes())?;
        // attributes (1 byte) + uniqueID (3 bytes) packed as u32 = 0.
        // Give each record a unique ID = its index for good measure.
        let uid: u32 = i as u32;
        f.write_all(&uid.to_be_bytes())?;
    }

    // Padding byte if data_start was bumped to even.
    let raw_data_start = palmdb_header_size + record_list_size;
    if !raw_data_start.is_multiple_of(2) {
        f.write_all(&[0u8])?;
    }

    // ── Record data ──────────────────────────────────────────────────────
    f.write_all(&record0)?;
    for chunk in &text_chunks {
        f.write_all(chunk)?;
    }
    for img in images {
        f.write_all(img)?;
    }
    if let Some(ref indx) = header_indx_record {
        f.write_all(indx)?;
    }
    if let Some(ref indx) = leaf_indx_record {
        f.write_all(indx)?;
    }
    if let Some(ref cncx) = cncx_record {
        f.write_all(cncx)?;
    }

    f.flush()?;
    Ok(())
}

/// Build record 0: PalmDoc header + MOBI header + EXTH block + title bytes.
fn build_record0(
    total_text_len: u32,
    text_record_count: u16,
    first_image_index: u32,
    title_bytes: &[u8],
    exth_bytes: &[u8],
    last_text_record: u32, // = text_record_count (1-based last index)
    _image_record_start: u32,
    ncx_record: u32, // record index of INDX record, or 0xFFFFFFFF if no NCX
) -> Vec<u8> {
    let mut rec = Vec::new();

    // ── PalmDoc header (16 bytes) ────────────────────────────────────────
    rec.extend_from_slice(&1u16.to_be_bytes()); // compression: no compression
    rec.extend_from_slice(&0u16.to_be_bytes()); // unused
    rec.extend_from_slice(&total_text_len.to_be_bytes()); // textLength
    rec.extend_from_slice(&text_record_count.to_be_bytes()); // recordCount
    rec.extend_from_slice(&4096u16.to_be_bytes()); // recordSize
    rec.extend_from_slice(&0u16.to_be_bytes()); // encryptionType
    rec.extend_from_slice(&0u16.to_be_bytes()); // unknown

    // ── MOBI header (232 bytes) ──────────────────────────────────────────
    // Record 0 layout: PalmDoc (16) + MOBI (232) + EXTH + title.
    // fullNameOffset must point past the EXTH block.
    let full_name_offset: u32 = 16 + 232 + exth_bytes.len() as u32;
    let full_name_length: u32 = title_bytes.len() as u32;
    let first_non_book_index: u32 = last_text_record + 1;
    let exth_flags: u32 = if exth_bytes.is_empty() { 0 } else { 0x50 };

    let mobi_start = rec.len();
    rec.extend_from_slice(b"MOBI"); //  0 identifier
    rec.extend_from_slice(&232u32.to_be_bytes()); //  4 headerLength
    rec.extend_from_slice(&2u32.to_be_bytes()); //  8 mobiType (book)
    rec.extend_from_slice(&65001u32.to_be_bytes()); // 12 textEncoding (UTF-8)
    rec.extend_from_slice(&0u32.to_be_bytes()); // 16 uniqueID
    rec.extend_from_slice(&6u32.to_be_bytes()); // 20 fileVersion
    // 24-64: 10 × 0xFFFFFFFF sentinel values for absent index records
    for _ in 0..10 {
        rec.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes());
    }
    rec.extend_from_slice(&first_non_book_index.to_be_bytes()); // 64 firstNonBookIndex
    rec.extend_from_slice(&full_name_offset.to_be_bytes()); // 68 fullNameOffset
    rec.extend_from_slice(&full_name_length.to_be_bytes()); // 72 fullNameLength
    rec.extend_from_slice(&0x0409u32.to_be_bytes()); // 76 locale (en-US)
    rec.extend_from_slice(&0u32.to_be_bytes()); // 80 inputLanguage
    rec.extend_from_slice(&0u32.to_be_bytes()); // 84 outputLanguage
    rec.extend_from_slice(&6u32.to_be_bytes()); // 88 minVersion
    rec.extend_from_slice(&first_image_index.to_be_bytes()); // 92 firstImageIndex
    rec.extend_from_slice(&0u32.to_be_bytes()); // 96 huffmanRecordOffset
    rec.extend_from_slice(&0u32.to_be_bytes()); // 100 huffmanRecordCount
    rec.extend_from_slice(&0u32.to_be_bytes()); // 104 huffmanTableOffset
    rec.extend_from_slice(&0u32.to_be_bytes()); // 108 huffmanTableLength
    rec.extend_from_slice(&exth_flags.to_be_bytes()); // 112 exthFlags
    rec.extend_from_slice(&[0u8; 32]); // 116 reserved2
    rec.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes()); // 148 drmOffset
    rec.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes()); // 152 drmCount
    rec.extend_from_slice(&0u32.to_be_bytes()); // 156 drmSize
    rec.extend_from_slice(&0u32.to_be_bytes()); // 160 drmFlags
    rec.extend_from_slice(&[0u8; 12]); // 164 reserved3
    rec.extend_from_slice(&1u16.to_be_bytes()); // 176 firstContentRecordNumber
    rec.extend_from_slice(&(last_text_record as u16).to_be_bytes()); // 178 lastContentRecordNumber
    rec.extend_from_slice(&1u32.to_be_bytes()); // 180 unknown constant
    rec.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes()); // 184 fcisRecord (absent)
    rec.extend_from_slice(&0u32.to_be_bytes()); // 188 fcisCount
    rec.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes()); // 192 flisRecord (absent)
    rec.extend_from_slice(&0u32.to_be_bytes()); // 196 flisCount
    rec.extend_from_slice(&[0u8; 8]); // 200 unknown3
    rec.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes()); // 208 unknown sentinel
    rec.extend_from_slice(&0u32.to_be_bytes()); // 212 unknown
    rec.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes()); // 216 unknown sentinel
    rec.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes()); // 220 unknown sentinel
    rec.extend_from_slice(&0u32.to_be_bytes()); // 224 extraDataFlags (u32)
    rec.extend_from_slice(&ncx_record.to_be_bytes()); // 228 primary_index_record_idx (NCX)

    // Pad MOBI header to exactly 232 bytes from mobi_start.
    let mobi_written = rec.len() - mobi_start;
    if mobi_written < 232 {
        let pad = 232 - mobi_written;
        rec.resize(rec.len() + pad, 0u8);
    }

    // EXTH block (must come before title string).
    rec.extend_from_slice(exth_bytes);

    // Title string (fullNameOffset points here).
    rec.extend_from_slice(title_bytes);

    rec
}

/// Build the EXTH block bytes (identifier + headerLength + recordCount +
/// records).
fn build_exth(sidecar: &BookSidecar, cover_record_index: u32, has_cover: bool) -> Vec<u8> {
    let mut records: Vec<(u32, Vec<u8>)> = Vec::new();

    // 100 = author (first author).
    if let Some(author) = sidecar.authors.first() {
        records.push((100, author.name.as_bytes().to_vec()));
    }

    // 101 = publisher.
    if let Some(pub_name) = &sidecar.publisher {
        records.push((101, pub_name.as_bytes().to_vec()));
    }

    // 503 = title (updated title).
    records.push((503, sidecar.title.as_bytes().to_vec()));

    // 506 = cover record index (0-based from start of file records, but
    // the EXTH 506 value is the 0-based index of the cover record in the
    // MOBI record numbering — i.e. the record number itself).
    if has_cover {
        records.push((506, cover_record_index.to_be_bytes().to_vec()));
        // 537 = thumbnail record index (same as cover for simplicity).
        records.push((537, cover_record_index.to_be_bytes().to_vec()));
    }

    // 517 = series name, 518 = series index.
    if let Some(series) = &sidecar.series {
        records.push((517, series.name.as_bytes().to_vec()));
        if let Some(number) = &series.number {
            records.push((518, number.to_string().as_bytes().to_vec()));
        }
    }

    // 105 = subject/genre (one record per genre).
    for genre in &sidecar.genres {
        records.push((105, genre.as_bytes().to_vec()));
    }

    // 106 = published date (year as 4-digit ASCII string).
    if let Some(year) = sidecar.published_date {
        records.push((106, year.to_string().as_bytes().to_vec()));
    }

    if records.is_empty() {
        return Vec::new();
    }

    // Compute total length.
    // Each record: 4 (type) + 4 (length) + data.len() bytes.
    let records_data_len: usize = records.iter().map(|(_, d)| 8 + d.len()).sum();
    // EXTH block: "EXTH" (4) + headerLength (4) + recordCount (4) + records.
    let total_len = 4 + 4 + 4 + records_data_len;

    let mut out = Vec::with_capacity(total_len);
    out.extend_from_slice(b"EXTH");
    out.extend_from_slice(&(total_len as u32).to_be_bytes());
    out.extend_from_slice(&(records.len() as u32).to_be_bytes());

    for (rec_type, data) in &records {
        let rec_len = 8 + data.len();
        out.extend_from_slice(&rec_type.to_be_bytes());
        out.extend_from_slice(&(rec_len as u32).to_be_bytes());
        out.extend_from_slice(data);
    }

    // Pad EXTH block to 4-byte boundary and update the stored headerLength.
    let remainder = out.len() % 4;
    if remainder != 0 {
        let pad = 4 - remainder;
        out.resize(out.len() + pad, 0u8);
        // Update the headerLength field at bytes 4..8.
        let new_len = out.len() as u32;
        out[4..8].copy_from_slice(&new_len.to_be_bytes());
    }

    out
}

// ── Shared helpers
// ────────────────────────────────────────────────────────────

fn normalize_zip_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

fn resolve_zip_path(dir: &str, href: &str) -> String {
    let combined = if dir.is_empty() { href.to_string() } else { format!("{dir}/{href}") };
    normalize_zip_path(&combined)
}

fn local_name_lower(qualified: &[u8]) -> String {
    let s = std::str::from_utf8(qualified).unwrap_or("");
    let local = s.split(':').next_back().unwrap_or(s);
    local.to_ascii_lowercase()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use bb_core::{
        book::{AuthorRole, BookStatus, FileFormat},
        storage::{BookSidecar, SidecarAuthor, SidecarFile, SidecarSeries},
    };
    use rust_decimal::Decimal;
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;

    use super::convert_to_mobi;

    // ── Minimal synthetic EPUB builder ───────────────────────────────────

    const CONTAINER_XML: &[u8] = br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;

    const CONTENT_OPF: &[u8] = br#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Test Book</dc:title>
    <dc:creator>Test Author</dc:creator>
  </metadata>
  <manifest>
    <item id="ch1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>"#;

    const CHAPTER1_XHTML: &[u8] = b"<html><body><p>Hello world</p></body></html>";

    fn build_test_epub() -> Vec<u8> {
        let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let cursor = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);

        zip.start_file("mimetype", stored).unwrap();
        zip.write_all(b"application/epub+zip").unwrap();

        zip.start_file("META-INF/container.xml", stored).unwrap();
        zip.write_all(CONTAINER_XML).unwrap();

        zip.start_file("OEBPS/content.opf", stored).unwrap();
        zip.write_all(CONTENT_OPF).unwrap();

        zip.start_file("OEBPS/chapter1.xhtml", stored).unwrap();
        zip.write_all(CHAPTER1_XHTML).unwrap();

        zip.finish().unwrap().into_inner()
    }

    fn make_sidecar(title: &str, author: &str) -> BookSidecar {
        BookSidecar {
            title: title.to_string(),
            authors: vec![SidecarAuthor {
                name: author.to_string(),
                role: AuthorRole::Author,
                sort_order: 0,
                file_as: None,
            }],
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
            files: vec![SidecarFile {
                format: FileFormat::Epub,
                hash: "abc".to_string(),
            }],
        }
    }

    // ── Test 1: output is a PalmDB file (not a ZIP) ───────────────────────

    #[test]
    fn test_convert_produces_palmdb_signature() {
        let dir = tempdir().unwrap();
        let epub_path = dir.path().join("test.epub");
        let mobi_path = dir.path().join("test.mobi");

        std::fs::write(&epub_path, build_test_epub()).unwrap();
        let sidecar = make_sidecar("Test Book", "Test Author");

        convert_to_mobi(&epub_path, &mobi_path, &sidecar, None).expect("convert_to_mobi failed");

        let out = std::fs::read(&mobi_path).unwrap();
        // A MOBI/PalmDB file must NOT start with PK (ZIP magic).
        assert!(!out.starts_with(b"PK"), "output should not be a ZIP file (PK magic found)");
        // PalmDB type field at offset 60 must be "BOOK".
        assert_eq!(&out[60..64], b"BOOK", "PalmDB type field should be BOOK");
        // PalmDB creator field at offset 64 must be "MOBI".
        assert_eq!(&out[64..68], b"MOBI", "PalmDB creator field should be MOBI");
    }

    // ── Test 2: EXTH block contains the author name ───────────────────────

    #[test]
    fn test_convert_includes_exth_author() {
        let dir = tempdir().unwrap();
        let epub_path = dir.path().join("test.epub");
        let mobi_path = dir.path().join("test.mobi");

        std::fs::write(&epub_path, build_test_epub()).unwrap();
        let sidecar = make_sidecar("My Book", "Jane Austen");

        convert_to_mobi(&epub_path, &mobi_path, &sidecar, None).expect("convert_to_mobi failed");

        let out = std::fs::read(&mobi_path).unwrap();
        // The author string must appear somewhere in the output bytes.
        let author_bytes = b"Jane Austen";
        let found = out.windows(author_bytes.len()).any(|w| w == author_bytes);
        assert!(found, "author name 'Jane Austen' should appear in MOBI output");
    }

    // ── Test 3: EXTH record 506 present when cover is provided ───────────

    #[test]
    fn test_convert_cover_record_index_matches_exth_506() {
        let dir = tempdir().unwrap();
        let epub_path = dir.path().join("test.epub");
        let mobi_path = dir.path().join("test.mobi");

        std::fs::write(&epub_path, build_test_epub()).unwrap();
        let sidecar = make_sidecar("Covered Book", "Cover Author");

        // Minimal fake JPEG.
        let fake_cover: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46];

        convert_to_mobi(&epub_path, &mobi_path, &sidecar, Some(fake_cover)).expect("convert_to_mobi with cover failed");

        let out = std::fs::read(&mobi_path).unwrap();
        // EXTH record type 506 is present: the bytes 0x00 0x00 0x01 0xFA (506 in
        // big-endian u32).
        let exth_506_type = 506u32.to_be_bytes();
        let found = out.windows(4).any(|w| w == exth_506_type);
        assert!(found, "EXTH record type 506 (cover record index) should be present in output");
    }

    // ── Test 4: EXTH block contains the series name ───────────────────────

    #[test]
    fn test_convert_includes_exth_series() {
        let dir = tempdir().unwrap();
        let epub_path = dir.path().join("test.epub");
        let mobi_path = dir.path().join("test.mobi");

        std::fs::write(&epub_path, build_test_epub()).unwrap();
        let mut sidecar = make_sidecar("Foundation", "Isaac Asimov");
        sidecar.series = Some(SidecarSeries {
            name: "Foundation Series".to_string(),
            number: Some(Decimal::new(1, 0)),
        });

        convert_to_mobi(&epub_path, &mobi_path, &sidecar, None).expect("convert_to_mobi failed");

        let out = std::fs::read(&mobi_path).unwrap();
        let series_bytes = b"Foundation Series";
        let found = out.windows(series_bytes.len()).any(|w| w == series_bytes);
        assert!(found, "series name should appear in MOBI output");
    }

    // ── Test 5: NCX-equipped EPUB produces INDX + CNCX records ───────────

    const CONTENT_OPF_WITH_NCX: &[u8] = br#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>NCX Test Book</dc:title>
    <dc:creator>NCX Author</dc:creator>
  </metadata>
  <manifest>
    <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
    <item id="ch1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
    <item id="ch2" href="chapter2.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine toc="ncx">
    <itemref idref="ch1"/>
    <itemref idref="ch2"/>
  </spine>
</package>"#;

    const TOC_NCX: &[u8] = br#"<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE ncx PUBLIC "-//NISO//DTD ncx 2005-1//EN" "http://www.daisy.org/z3986/2005/ncx-2005-1.dtd">
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
  <navMap>
    <navPoint id="np1" playOrder="1">
      <navLabel><text>Chapter One</text></navLabel>
      <content src="chapter1.xhtml"/>
    </navPoint>
    <navPoint id="np2" playOrder="2">
      <navLabel><text>Chapter Two</text></navLabel>
      <content src="chapter2.xhtml"/>
    </navPoint>
  </navMap>
</ncx>"#;

    fn build_test_epub_with_ncx() -> Vec<u8> {
        let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let cursor = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);

        zip.start_file("mimetype", stored).unwrap();
        zip.write_all(b"application/epub+zip").unwrap();

        zip.start_file("META-INF/container.xml", stored).unwrap();
        zip.write_all(CONTAINER_XML).unwrap();

        zip.start_file("OEBPS/content.opf", stored).unwrap();
        zip.write_all(CONTENT_OPF_WITH_NCX).unwrap();

        zip.start_file("OEBPS/toc.ncx", stored).unwrap();
        zip.write_all(TOC_NCX).unwrap();

        zip.start_file("OEBPS/chapter1.xhtml", stored).unwrap();
        zip.write_all(b"<html><body><p>Chapter one body.</p></body></html>").unwrap();

        zip.start_file("OEBPS/chapter2.xhtml", stored).unwrap();
        zip.write_all(b"<html><body><p>Chapter two body.</p></body></html>").unwrap();

        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn test_ncx_epub_produces_indx_and_cncx_records() {
        let dir = tempdir().unwrap();
        let epub_path = dir.path().join("ncx_test.epub");
        let mobi_path = dir.path().join("ncx_test.mobi");

        std::fs::write(&epub_path, build_test_epub_with_ncx()).unwrap();
        let sidecar = make_sidecar("NCX Test Book", "NCX Author");

        convert_to_mobi(&epub_path, &mobi_path, &sidecar, None).expect("convert_to_mobi with NCX failed");

        let out = std::fs::read(&mobi_path).unwrap();

        // INDX magic must appear somewhere after the text records.
        let indx_magic = b"INDX";
        assert!(out.windows(4).any(|w| w == indx_magic), "INDX record should be present in NCX MOBI output");

        // TAGX magic must appear (it's embedded in the INDX record).
        let tagx_magic = b"TAGX";
        assert!(out.windows(4).any(|w| w == tagx_magic), "TAGX section should be present in INDX record");

        // Chapter label "Chapter One" must appear in the CNCX record.
        let label = b"Chapter One";
        assert!(
            out.windows(label.len()).any(|w| w == label),
            "chapter label 'Chapter One' should appear in CNCX record"
        );

        // The MOBI header ncxRecord field (at byte offset 16+228 = 244 from record 0
        // start) must not be 0xFFFFFFFF — it should point to the INDX record.
        // Record 0 starts at offset data_start in the file.  For a 2-record text body +
        // no images, ncxRecord = 3 (records 1,2 = text; record 3 = INDX).
        // We verify only that the field is not the "no NCX" sentinel.
        let num_records = u16::from_be_bytes([out[76], out[77]]) as usize;
        let record_list_start = 78usize;
        let record0_offset = u32::from_be_bytes([
            out[record_list_start],
            out[record_list_start + 1],
            out[record_list_start + 2],
            out[record_list_start + 3],
        ]) as usize;
        let _ = num_records;
        // ncxRecord is at MOBI-header offset 228 (last 4 bytes), MOBI header starts at
        // record0+16.
        let ncx_record_field_pos = record0_offset + 16 + 228;
        let ncx_record_val = u32::from_be_bytes([
            out[ncx_record_field_pos],
            out[ncx_record_field_pos + 1],
            out[ncx_record_field_pos + 2],
            out[ncx_record_field_pos + 3],
        ]);
        assert_ne!(ncx_record_val, 0xFFFF_FFFF, "ncxRecord field should not be the 'no NCX' sentinel");

        // Locate the header INDX record (at ncxRecord) and verify its indexType=0.
        let indx_record_num = ncx_record_val as usize;
        let hdr_indx_offset = u32::from_be_bytes([
            out[record_list_start + indx_record_num * 8],
            out[record_list_start + indx_record_num * 8 + 1],
            out[record_list_start + indx_record_num * 8 + 2],
            out[record_list_start + indx_record_num * 8 + 3],
        ]) as usize;
        assert_eq!(&out[hdr_indx_offset..hdr_indx_offset + 4], b"INDX", "header INDX magic");
        let hdr_index_type = u32::from_be_bytes([
            out[hdr_indx_offset + 0x0C],
            out[hdr_indx_offset + 0x0D],
            out[hdr_indx_offset + 0x0E],
            out[hdr_indx_offset + 0x0F],
        ]);
        assert_eq!(hdr_index_type, 0, "header INDX indexType should be 0 (interior/root)");

        // Leaf INDX is at ncxRecord+1 with indexType=1.
        let leaf_record_num = indx_record_num + 1;
        let leaf_indx_offset = u32::from_be_bytes([
            out[record_list_start + leaf_record_num * 8],
            out[record_list_start + leaf_record_num * 8 + 1],
            out[record_list_start + leaf_record_num * 8 + 2],
            out[record_list_start + leaf_record_num * 8 + 3],
        ]) as usize;
        assert_eq!(&out[leaf_indx_offset..leaf_indx_offset + 4], b"INDX", "leaf INDX magic");
        let leaf_index_type = u32::from_be_bytes([
            out[leaf_indx_offset + 0x0C],
            out[leaf_indx_offset + 0x0D],
            out[leaf_indx_offset + 0x0E],
            out[leaf_indx_offset + 0x0F],
        ]);
        assert_eq!(leaf_index_type, 1, "leaf INDX indexType should be 1 (leaf)");

        // CNCX is at ncxRecord+2. Verify chapter label is present in it.
        let cncx_record_num = indx_record_num + 2;
        let cncx_record_offset = u32::from_be_bytes([
            out[record_list_start + cncx_record_num * 8],
            out[record_list_start + cncx_record_num * 8 + 1],
            out[record_list_start + cncx_record_num * 8 + 2],
            out[record_list_start + cncx_record_num * 8 + 3],
        ]) as usize;
        let cncx_next_offset = if cncx_record_num + 1 < num_records {
            u32::from_be_bytes([
                out[record_list_start + (cncx_record_num + 1) * 8],
                out[record_list_start + (cncx_record_num + 1) * 8 + 1],
                out[record_list_start + (cncx_record_num + 1) * 8 + 2],
                out[record_list_start + (cncx_record_num + 1) * 8 + 3],
            ]) as usize
        } else {
            out.len()
        };
        let cncx_data = &out[cncx_record_offset..cncx_next_offset];
        assert!(
            cncx_data.windows(label.len()).any(|w| w == label),
            "CNCX record at ncxRecord+2 should contain chapter label"
        );
    }

    #[test]
    fn test_no_ncx_epub_sets_ncx_record_sentinel() {
        let dir = tempdir().unwrap();
        let epub_path = dir.path().join("no_ncx.epub");
        let mobi_path = dir.path().join("no_ncx.mobi");

        std::fs::write(&epub_path, build_test_epub()).unwrap();
        let sidecar = make_sidecar("No NCX Book", "Plain Author");

        convert_to_mobi(&epub_path, &mobi_path, &sidecar, None).expect("convert_to_mobi failed");

        let out = std::fs::read(&mobi_path).unwrap();

        let record_list_start = 78usize;
        let record0_offset = u32::from_be_bytes([
            out[record_list_start],
            out[record_list_start + 1],
            out[record_list_start + 2],
            out[record_list_start + 3],
        ]) as usize;
        let ncx_record_field_pos = record0_offset + 16 + 228;
        let ncx_record_val = u32::from_be_bytes([
            out[ncx_record_field_pos],
            out[ncx_record_field_pos + 1],
            out[ncx_record_field_pos + 2],
            out[ncx_record_field_pos + 3],
        ]);
        assert_eq!(ncx_record_val, 0xFFFF_FFFF, "ncxRecord field should be 0xFFFFFFFF when no NCX");
    }
}
