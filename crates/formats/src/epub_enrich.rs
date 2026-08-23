use std::{
    io::{Read, Write},
    path::Path,
};

use bb_core::storage::BookSidecar;
use quick_xml::{
    Reader, Writer, XmlVersion,
    events::{BytesEnd, BytesStart, Event},
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::{
    Error,
    epub::{find_opf_path, resolve_zip_path},
    opf::{extract_cover_info, write_metadata_xml},
};

/// Produces an enriched copy of an EPUB file at `dest`.
///
/// - Rewrites the embedded OPF metadata from `sidecar`.
/// - If `cover` is `Some`: replaces the existing cover image, or adds a new
///   `cover.jpg` entry (with EPUB3 manifest declaration) if none was present.
/// - All other entries are copied verbatim, preserving their compression.
/// - `mimetype` is always written first as an uncompressed entry, as required
///   by the EPUB spec.
pub fn enrich_epub(source: &Path, dest: &Path, sidecar: &BookSidecar, cover: Option<&[u8]>) -> Result<(), Error> {
    let src_file = std::fs::File::open(source)?;
    let mut src = ZipArchive::new(src_file)?;

    // ── 1. Locate OPF and existing cover within the archive ─────────────────

    let opf_path = {
        let mut c = src.by_name("META-INF/container.xml")?;
        let mut buf = Vec::new();
        c.read_to_end(&mut buf)?;
        find_opf_path(&buf)?
    };

    let opf_dir = match opf_path.rfind('/') {
        Some(pos) => opf_path[..pos].to_string(),
        None => String::new(),
    };

    // Detect existing cover image in the source OPF.
    let cover_info = {
        let mut f = src.by_name(&opf_path)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        extract_cover_info(&buf)
    };

    let existing_cover_zip_path: Option<String> = cover_info.as_ref().map(|ci| resolve_zip_path(&opf_dir, &ci.href));

    // ── 2. Build updated OPF (splice new metadata; ensure cover declaration) ─

    let new_opf: String = {
        let mut f = src.by_name(&opf_path)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        let opf_str = std::str::from_utf8(&buf)?;
        let mut updated = replace_opf_metadata(opf_str, sidecar)?;

        if cover.is_some() {
            match &cover_info {
                Some(ci) if !ci.has_cover_image_property => {
                    // Existing cover found but missing EPUB 3 property — fix it
                    updated = ensure_cover_image_property(&updated, &ci.id)?;
                }
                Some(_) => {
                    // Already has properties="cover-image" — nothing to do
                }
                None => {
                    // No existing cover — inject a new manifest entry
                    updated = inject_cover_manifest_entry(&updated, "cover.jpg")?;
                }
            }
        }
        updated
    };

    // If we're adding a cover that didn't exist before, this is its ZIP path.
    let new_cover_zip_path: Option<String> = if cover.is_some() && existing_cover_zip_path.is_none() {
        Some(resolve_zip_path(&opf_dir, "cover.jpg"))
    } else {
        None
    };

    // ── 3. Enumerate source entries ──────────────────────────────────────────

    let entry_count = src.len();
    let mut entries: Vec<(usize, String)> = Vec::with_capacity(entry_count);
    for i in 0..entry_count {
        let f = src.by_index(i)?;
        entries.push((i, f.name().to_string()));
    }

    // ── 4. Write destination archive ─────────────────────────────────────────

    let dest_file = std::fs::File::create(dest)?;
    let mut dest_zip = ZipWriter::new(std::io::BufWriter::new(dest_file));

    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // `mimetype` must be the very first entry and must be uncompressed.
    if let Some(&(idx, _)) = entries.iter().find(|(_, n)| n == "mimetype") {
        let mut f = src.by_index(idx)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        dest_zip.start_file("mimetype", stored)?;
        dest_zip.write_all(&buf)?;
    }

    for (i, name) in &entries {
        if name == "mimetype" {
            continue; // already written first
        }

        if name == &opf_path {
            dest_zip.start_file(name, deflated)?;
            dest_zip.write_all(new_opf.as_bytes())?;
        } else if Some(name) == existing_cover_zip_path.as_ref() {
            // Replace or preserve existing cover entry.
            if let Some(cover_bytes) = cover {
                dest_zip.start_file(name, stored)?;
                dest_zip.write_all(cover_bytes)?;
            } else {
                copy_entry(&mut src, &mut dest_zip, *i)?;
            }
        } else {
            copy_entry(&mut src, &mut dest_zip, *i)?;
        }
    }

    // If we're adding a cover to an EPUB that had none, write the new entry.
    if let (Some(new_path), Some(cover_bytes)) = (&new_cover_zip_path, cover) {
        dest_zip.start_file(new_path, stored)?;
        dest_zip.write_all(cover_bytes)?;
    }

    let mut buf_writer = dest_zip.finish()?;
    buf_writer.flush()?;

    Ok(())
}

/// Copy a single ZIP entry (by index) from `src` to `dst`, preserving the
/// original compression method.
fn copy_entry<R, W>(src: &mut ZipArchive<R>, dst: &mut ZipWriter<W>, index: usize) -> Result<(), Error>
where
    R: Read + std::io::Seek,
    W: Write + std::io::Seek,
{
    let mut f = src.by_index(index)?;
    let method = f.compression();
    let name = f.name().to_string();
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    let opts = SimpleFileOptions::default().compression_method(method);
    dst.start_file(name, opts)?;
    dst.write_all(&buf)?;
    Ok(())
}

/// Replace the `<metadata>…</metadata>` block in `opf_xml` with freshly
/// serialised metadata from `sidecar`. Manifest, spine, and all other
/// elements are left untouched.
fn replace_opf_metadata(opf_xml: &str, sidecar: &BookSidecar) -> Result<String, Error> {
    let new_meta = write_metadata_xml(sidecar)?;
    let mut reader = Reader::from_str(opf_xml);
    reader.config_mut().trim_text(false);
    let mut writer = Writer::new(Vec::new());
    let mut buf = Vec::new();
    let mut skipping = false;
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf)? {
            Event::Start(ref e) if e.local_name().as_ref() == "metadata" => {
                writer.get_mut().extend_from_slice(&new_meta);
                skipping = true;
            }
            Event::End(ref e) if skipping && e.local_name().as_ref() == "metadata" => {
                skipping = false;
            }
            Event::Eof => break,
            event => {
                if !skipping {
                    writer.write_event(event)?;
                }
            }
        }
    }
    String::from_utf8(writer.into_inner()).map_err(|e| Error::InvalidValue(e.to_string()))
}

/// Inject a cover image manifest entry into an OPF's `<manifest>` block.
///
/// Handles both `<manifest>…</manifest>` (inserts before the closing tag) and
/// `<manifest/>` (self-closing empty manifest — expands it).
fn inject_cover_manifest_entry(opf_xml: &str, cover_href: &str) -> Result<String, Error> {
    let mut reader = Reader::from_str(opf_xml);
    reader.config_mut().trim_text(false);
    let mut writer = Writer::new(Vec::new());
    let mut buf = Vec::new();
    let cover_href = cover_href.to_string();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf)? {
            Event::End(ref e) if e.local_name().as_ref() == "manifest" => {
                writer.write_event(Event::Empty(build_cover_item(&cover_href)))?;
                writer.write_event(Event::End(BytesEnd::new("manifest")))?;
            }
            Event::Empty(ref e) if e.local_name().as_ref() == "manifest" => {
                writer.write_event(Event::Start(BytesStart::new("manifest")))?;
                writer.write_event(Event::Empty(build_cover_item(&cover_href)))?;
                writer.write_event(Event::End(BytesEnd::new("manifest")))?;
            }
            Event::Eof => break,
            event => writer.write_event(event)?,
        }
    }
    String::from_utf8(writer.into_inner()).map_err(|e| Error::InvalidValue(e.to_string()))
}

fn build_cover_item(cover_href: &str) -> BytesStart<'static> {
    let mut item = BytesStart::new("item");
    item.push_attribute(("id", "cover-image"));
    item.push_attribute(("href", cover_href));
    item.push_attribute(("media-type", "image/jpeg"));
    item.push_attribute(("properties", "cover-image"));
    item
}

/// Ensure a manifest `<item>` with the given `id` has
/// `properties="cover-image"`. Adds the attribute if missing, appends
/// `cover-image` to an existing `properties` value, or no-ops if already
/// present.
fn ensure_cover_image_property(opf_xml: &str, item_id: &str) -> Result<String, Error> {
    let mut reader = Reader::from_str(opf_xml);
    reader.config_mut().trim_text(false);
    let mut writer = Writer::new(Vec::new());
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf)? {
            Event::Empty(ref e) if e.local_name().as_ref() == "item" => {
                let is_target = e
                    .attributes()
                    .flatten()
                    .any(|a| a.key.as_ref() == "id" && a.normalized_value(XmlVersion::Implicit1_0).ok().as_deref() == Some(item_id));
                if is_target {
                    let patched = patch_cover_image_property(e)?;
                    writer.write_event(Event::Empty(patched))?;
                } else {
                    writer.write_event(Event::Empty(e.borrow()))?;
                }
            }
            Event::Eof => break,
            event => writer.write_event(event)?,
        }
    }
    String::from_utf8(writer.into_inner()).map_err(|e| Error::InvalidValue(e.to_string()))
}

fn patch_cover_image_property(elem: &quick_xml::events::BytesStart<'_>) -> Result<BytesStart<'static>, Error> {
    let mut new_elem = BytesStart::new("item");
    let mut found_properties = false;
    for attr in elem.attributes().flatten() {
        if attr.key.as_ref() == "properties" {
            let val = attr.normalized_value(XmlVersion::Implicit1_0)?;
            if val.split_whitespace().any(|p| p == "cover-image") {
                new_elem.push_attribute(("properties", val.as_ref()));
            } else {
                let patched = format!("{val} cover-image");
                new_elem.push_attribute(("properties", patched.as_str()));
            }
            found_properties = true;
        } else {
            let key = attr.key.as_ref();
            let val = attr.normalized_value(XmlVersion::Implicit1_0)?;
            new_elem.push_attribute((key, val.as_ref()));
        }
    }
    if !found_properties {
        new_elem.push_attribute(("properties", "cover-image"));
    }
    Ok(new_elem)
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    use tempfile::tempdir;

    use super::enrich_epub;
    use crate::{
        opf::parse_sidecar,
        test_support::{CONTAINER_XML, make_sidecar},
    };

    const FAKE_JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F'];

    fn build_epub_no_cover(title: &str) -> Vec<u8> {
        let opf = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>{title}</dc:title>
  </metadata>
  <manifest/>
  <spine/>
</package>"#
        );
        build_epub_zip(&opf, None)
    }

    fn build_epub2_with_cover(title: &str) -> Vec<u8> {
        let opf = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>{title}</dc:title>
    <meta name="cover" content="cover-img"/>
  </metadata>
  <manifest>
    <item id="cover-img" href="cover.jpg" media-type="image/jpeg"/>
  </manifest>
  <spine/>
</package>"#
        );
        build_epub_zip(&opf, Some(("cover.jpg", FAKE_JPEG)))
    }

    fn build_epub3_with_cover(title: &str) -> Vec<u8> {
        let opf = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>{title}</dc:title>
  </metadata>
  <manifest>
    <item id="cover" href="images/cover.jpg" media-type="image/jpeg" properties="cover-image"/>
  </manifest>
  <spine/>
</package>"#
        );
        build_epub_zip(&opf, Some(("images/cover.jpg", FAKE_JPEG)))
    }

    fn build_epub_zip(opf: &str, cover: Option<(&str, &[u8])>) -> Vec<u8> {
        let cursor = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        zip.start_file("mimetype", stored).unwrap();
        zip.write_all(b"application/epub+zip").unwrap();

        zip.start_file("META-INF/container.xml", stored).unwrap();
        zip.write_all(CONTAINER_XML).unwrap();

        zip.start_file("content.opf", stored).unwrap();
        zip.write_all(opf.as_bytes()).unwrap();

        if let Some((path, bytes)) = cover {
            zip.start_file(path, stored).unwrap();
            zip.write_all(bytes).unwrap();
        }

        zip.finish().unwrap().into_inner()
    }

    const NEW_COVER: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'N', b'E', b'W', b'!'];

    fn read_opf_from_epub(epub_path: &std::path::Path) -> Vec<u8> {
        let file = std::fs::File::open(epub_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let container_xml = {
            let mut c = archive.by_name("META-INF/container.xml").unwrap();
            let mut buf = Vec::new();
            c.read_to_end(&mut buf).unwrap();
            buf
        };
        let opf_path = crate::epub::find_opf_path(&container_xml).unwrap();
        let mut opf_entry = archive.by_name(&opf_path).unwrap();
        let mut buf = Vec::new();
        opf_entry.read_to_end(&mut buf).unwrap();
        buf
    }

    #[test]
    fn rewrites_metadata_no_cover() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.epub");
        let dst = dir.path().join("dst.epub");
        std::fs::write(&src, build_epub_no_cover("Old Title")).unwrap();

        let sidecar = make_sidecar("New Title");
        enrich_epub(&src, &dst, &sidecar, None).unwrap();

        let opf_bytes = read_opf_from_epub(&dst);
        let parsed = parse_sidecar(&opf_bytes).unwrap();
        assert_eq!(parsed.title, "New Title");
    }

    #[test]
    fn replaces_existing_epub2_cover() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.epub");
        let dst = dir.path().join("dst.epub");
        std::fs::write(&src, build_epub2_with_cover("Dune")).unwrap();

        let sidecar = make_sidecar("Dune");
        enrich_epub(&src, &dst, &sidecar, Some(NEW_COVER)).unwrap();

        // Read cover entry from dst
        let file = std::fs::File::open(&dst).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut cover_entry = archive.by_name("cover.jpg").unwrap();
        let mut buf = Vec::new();
        cover_entry.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, NEW_COVER);

        // The manifest item must have properties="cover-image" so e-readers
        // and our own extractor can find it after the EPUB 2 meta tag was
        // replaced by enrichment.
        let opf_bytes = read_opf_from_epub(&dst);
        let opf_str = std::str::from_utf8(&opf_bytes).unwrap();
        assert!(opf_str.contains("cover-image"), "manifest should have cover-image property");
        assert!(
            crate::opf::extract_cover_href(&opf_bytes).is_some(),
            "extract_cover_href must find cover in enriched EPUB"
        );
    }

    #[test]
    fn replaces_existing_epub3_cover() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.epub");
        let dst = dir.path().join("dst.epub");
        std::fs::write(&src, build_epub3_with_cover("Foundation")).unwrap();

        let sidecar = make_sidecar("Foundation");
        enrich_epub(&src, &dst, &sidecar, Some(NEW_COVER)).unwrap();

        let file = std::fs::File::open(&dst).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut cover_entry = archive.by_name("images/cover.jpg").unwrap();
        let mut buf = Vec::new();
        cover_entry.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, NEW_COVER);
    }

    #[test]
    fn adds_cover_to_epub_without_one() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.epub");
        let dst = dir.path().join("dst.epub");
        std::fs::write(&src, build_epub_no_cover("Neuromancer")).unwrap();

        let sidecar = make_sidecar("Neuromancer");
        enrich_epub(&src, &dst, &sidecar, Some(NEW_COVER)).unwrap();

        let file = std::fs::File::open(&dst).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();

        let cover_buf = {
            let mut cover_entry = archive.by_name("cover.jpg").unwrap();
            let mut buf = Vec::new();
            cover_entry.read_to_end(&mut buf).unwrap();
            buf
        };
        assert_eq!(cover_buf, NEW_COVER);

        // OPF should have cover-image manifest entry
        let opf_str = {
            let mut opf_entry = archive.by_name("content.opf").unwrap();
            let mut buf = Vec::new();
            opf_entry.read_to_end(&mut buf).unwrap();
            String::from_utf8(buf).unwrap()
        };
        assert!(opf_str.contains("cover-image"), "OPF should declare cover-image");
    }

    #[test]
    fn preserves_non_opf_entries() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.epub");
        let dst = dir.path().join("dst.epub");

        // Build EPUB with an extra chapter file
        let opf = r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>Book</dc:title>
  </metadata>
  <manifest>
    <item id="ch1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="ch1"/></spine>
</package>"#;

        let cursor = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zip.start_file("mimetype", stored).unwrap();
        zip.write_all(b"application/epub+zip").unwrap();
        zip.start_file("META-INF/container.xml", stored).unwrap();
        zip.write_all(CONTAINER_XML).unwrap();
        zip.start_file("content.opf", stored).unwrap();
        zip.write_all(opf.as_bytes()).unwrap();
        zip.start_file("chapter1.xhtml", stored).unwrap();
        zip.write_all(b"<html><body>Hello</body></html>").unwrap();
        std::fs::write(&src, zip.finish().unwrap().into_inner()).unwrap();

        enrich_epub(&src, &dst, &make_sidecar("Book"), None).unwrap();

        let file = std::fs::File::open(&dst).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut ch = archive.by_name("chapter1.xhtml").unwrap();
        let mut buf = Vec::new();
        ch.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, b"<html><body>Hello</body></html>");
    }

    /// Verify that genres in the sidecar appear as `dc:subject` elements in
    /// the enriched EPUB's OPF. Genres stored only in the `spinnaker:metadata`
    /// blob would be invisible to e-readers.
    #[test]
    fn genres_written_as_dc_subject_in_enriched_epub() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.epub");
        let dst = dir.path().join("dst.epub");
        std::fs::write(&src, build_epub_no_cover("Test")).unwrap();

        let mut sidecar = make_sidecar("Test");
        sidecar.genres = vec!["Fantasy".to_string(), "Epic Fantasy".to_string()];

        enrich_epub(&src, &dst, &sidecar, None).unwrap();

        let opf_bytes = read_opf_from_epub(&dst);
        let opf_str = std::str::from_utf8(&opf_bytes).expect("utf8");

        for genre in &sidecar.genres {
            let expected = format!("<dc:subject>{genre}</dc:subject>");
            assert!(opf_str.contains(&expected), "expected {expected:?} in enriched EPUB OPF");
        }

        let parsed = parse_sidecar(&opf_bytes).expect("parse sidecar");
        assert_eq!(parsed.genres, sidecar.genres, "genres must roundtrip through enriched EPUB");
    }

    #[test]
    fn mimetype_is_first_entry() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.epub");
        let dst = dir.path().join("dst.epub");
        std::fs::write(&src, build_epub_no_cover("Test")).unwrap();

        enrich_epub(&src, &dst, &make_sidecar("Test"), None).unwrap();

        let file = std::fs::File::open(&dst).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let first = archive.by_index(0).unwrap();
        assert_eq!(first.name(), "mimetype");
    }

    // ── Convention cover (id="cover", no meta, no properties) ────────────────

    fn build_epub_convention_cover(title: &str) -> Vec<u8> {
        let opf = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>{title}</dc:title>
  </metadata>
  <manifest>
    <item id="cover" href="cover.jpeg" media-type="image/jpeg"/>
    <item id="ch1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="ch1"/></spine>
</package>"#
        );
        build_epub_zip(&opf, Some(("cover.jpeg", FAKE_JPEG)))
    }

    #[test]
    fn convention_cover_replaced_not_duplicated() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.epub");
        let dst = dir.path().join("dst.epub");
        std::fs::write(&src, build_epub_convention_cover("Test")).unwrap();

        enrich_epub(&src, &dst, &make_sidecar("Test"), Some(NEW_COVER)).unwrap();

        let file = std::fs::File::open(&dst).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();

        // Cover bytes should be replaced at the EXISTING path (cover.jpeg)
        let cover_buf = {
            let mut entry = archive.by_name("cover.jpeg").unwrap();
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).unwrap();
            buf
        };
        assert_eq!(cover_buf, NEW_COVER, "cover.jpeg should have new cover bytes");

        // There must NOT be a duplicate cover.jpg entry
        assert!(archive.by_name("cover.jpg").is_err(), "cover.jpg should not exist — no duplicate");

        // OPF must have properties="cover-image" on the existing item
        let opf_bytes = read_opf_from_epub(&dst);
        let opf_str = std::str::from_utf8(&opf_bytes).unwrap();
        assert!(
            opf_str.contains(r#"properties="cover-image""#),
            "manifest item should have cover-image property"
        );

        // Extractor must find the cover
        assert!(
            crate::opf::extract_cover_href(&opf_bytes).is_some(),
            "extract_cover_href must find cover in enriched EPUB"
        );
    }

    #[test]
    fn enriched_epub2_cover_roundtrips_through_extractor() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.epub");
        let dst = dir.path().join("dst.epub");
        std::fs::write(&src, build_epub2_with_cover("Roundtrip")).unwrap();

        enrich_epub(&src, &dst, &make_sidecar("Roundtrip"), Some(NEW_COVER)).unwrap();

        let opf_bytes = read_opf_from_epub(&dst);
        let href = crate::opf::extract_cover_href(&opf_bytes).expect("cover must be findable after enrichment");
        assert_eq!(href, "cover.jpg");
    }

    // ── ensure_cover_image_property unit tests ───────────────────────────────

    #[test]
    fn ensure_adds_property_when_missing() {
        let opf = r#"<manifest><item id="cover" href="cover.jpeg" media-type="image/jpeg"/></manifest>"#;
        let result = super::ensure_cover_image_property(opf, "cover").unwrap();
        assert!(result.contains(r#"properties="cover-image""#));
    }

    #[test]
    fn ensure_appends_to_existing_properties() {
        let opf = r#"<manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="cover" href="cover.jpg" media-type="image/jpeg" properties="svg"/></manifest>"#;
        let result = super::ensure_cover_image_property(opf, "cover").unwrap();
        assert!(result.contains(r#"properties="svg cover-image""#));
        // nav item should be untouched
        assert!(result.contains(r#"properties="nav""#));
    }

    #[test]
    fn ensure_is_idempotent() {
        let opf = r#"<manifest><item id="ci" href="cover.jpg" media-type="image/jpeg" properties="cover-image"/></manifest>"#;
        let result = super::ensure_cover_image_property(opf, "ci").unwrap();
        assert!(result.contains(r#"properties="cover-image""#));
        // Must not duplicate the property
        assert_eq!(result.matches("cover-image").count(), 1);
    }

    #[test]
    fn replace_metadata_with_indented_closing_tags() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.epub");
        let dst = dir.path().join("dst.epub");
        let opf = r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>Old Title</dc:title>
  </metadata>
  <manifest>
    <item id="ch1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>"#;
        let epub = build_epub_zip(opf, None);
        std::fs::write(&src, epub).unwrap();
        enrich_epub(&src, &dst, &make_sidecar("New Title"), None).unwrap();
        let opf_bytes = read_opf_from_epub(&dst);
        let parsed = crate::opf::parse_sidecar(&opf_bytes).unwrap();
        assert_eq!(parsed.title, "New Title");
        let opf_str = std::str::from_utf8(&opf_bytes).unwrap();
        assert!(opf_str.contains("<spine>"), "spine must be preserved");
        assert!(opf_str.contains("ch1"), "spine itemref must be preserved");
    }

    #[test]
    fn inject_cover_preserves_xml_comment_in_manifest() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.epub");
        let dst = dir.path().join("dst.epub");
        let opf = r#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>Test</dc:title>
  </metadata>
  <manifest>
    <!-- primary content -->
    <item id="ch1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="ch1"/></spine>
</package>"#;
        let epub = build_epub_zip(opf, None);
        std::fs::write(&src, epub).unwrap();
        const NEW_COVER: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0];
        enrich_epub(&src, &dst, &make_sidecar("Test"), Some(NEW_COVER)).unwrap();
        let opf_bytes = read_opf_from_epub(&dst);
        let opf_str = std::str::from_utf8(&opf_bytes).unwrap();
        assert!(opf_str.contains("cover-image"), "cover item should be in manifest");
        assert!(opf_str.contains("primary content"), "xml comment should survive");
    }
}
