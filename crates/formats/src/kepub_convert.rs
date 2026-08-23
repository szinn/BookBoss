//! In-house EPUB → KEPUB conversion.
//!
//! Implements the same transformation as the `kepubify` CLI tool:
//!
//! 1. Every non-whitespace text node inside `<body>` (excluding `<pre>`) is
//!    wrapped in `<span class="koboSpan" id="kobo.{chapter}.{span}">`.
//! 2. A minimal Kobo CSS `<style>` block is injected before `</head>`.
//! 3. All XHTML content documents in the ZIP are processed in order; all other
//!    entries are copied verbatim.
//! 4. `mimetype` is always written first, uncompressed, as the EPUB spec
//!    requires.
//!
//! The output file uses the `.kepub.epub` double extension by convention — the
//! caller is responsible for the file name; this function writes to whatever
//! path `dest` points to.
//!
//! # Span ID scheme
//!
//! `id="kobo.{chapter}.{span}"` where both indices are 1-based. `chapter`
//! increments once per XHTML file processed (in ZIP entry order). `span`
//! resets to 1 for each new chapter.

use std::{
    io::{Read, Write},
    path::Path,
};

use quick_xml::{
    Reader, Writer,
    events::{BytesEnd, BytesStart, BytesText, Event},
};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::Error;

// ── Public entry point
// ─────────────────────────────────────────────────────

/// Converts an enriched EPUB to KEPUB format, writing the output to `dest`.
///
/// Opens `epub_path` as a ZIP archive, processes each XHTML content document
/// with [`inject_kobo_spans`], and repacks the result. All non-XHTML entries
/// are copied verbatim (preserving their original compression).
pub fn convert_to_kepub(epub_path: &Path, dest: &Path) -> Result<(), Error> {
    let src_file = std::fs::File::open(epub_path)?;
    let mut src = ZipArchive::new(src_file)?;

    // Collect entry names and indices up front (ZipArchive requires single
    // mutable borrow when calling by_index, so we can't enumerate + read
    // simultaneously).
    let entry_count = src.len();
    let mut entries: Vec<(usize, String)> = Vec::with_capacity(entry_count);
    for i in 0..entry_count {
        let f = src.by_index(i)?;
        entries.push((i, f.name().to_string()));
    }

    let dest_file = std::fs::File::create(dest)?;
    let mut output = ZipWriter::new(std::io::BufWriter::new(dest_file));

    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // `mimetype` must be the very first entry and must be uncompressed.
    if let Some(&(idx, _)) = entries.iter().find(|(_, n)| n == "mimetype") {
        let mut f = src.by_index(idx)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        output.start_file("mimetype", stored)?;
        output.write_all(&buf)?;
    }

    let mut chapter: usize = 0;

    for (i, name) in &entries {
        if name == "mimetype" {
            continue; // already written first
        }

        let lower = name.to_ascii_lowercase();
        let is_xhtml = lower.ends_with(".xhtml") || lower.ends_with(".html") || lower.ends_with(".htm");

        let mut f = src.by_index(*i)?;
        let compression = f.compression();
        let mut raw = Vec::new();
        f.read_to_end(&mut raw)?;

        if is_xhtml {
            chapter += 1;
            let processed = inject_kobo_spans(&raw, chapter)?;
            output.start_file(name, deflated)?;
            output.write_all(&processed)?;
        } else {
            let opts = SimpleFileOptions::default().compression_method(compression);
            output.start_file(name, opts)?;
            output.write_all(&raw)?;
        }
    }

    let mut buf_writer = output.finish()?;
    buf_writer.flush()?;
    Ok(())
}

// ── XHTML span injection
// ─────────────────────────────────────────────────────

/// Processes a single XHTML document: injects `koboSpan` elements and a Kobo
/// `<style>` block. Returns the modified document as UTF-8 bytes.
///
/// `chapter` is the 1-based chapter index (increments once per XHTML file).
fn inject_kobo_spans(xhtml_bytes: &[u8], chapter: usize) -> Result<Vec<u8>, Error> {
    let mut reader = Reader::from_reader(xhtml_bytes);
    let mut writer = Writer::new(Vec::with_capacity(xhtml_bytes.len() + 512));
    let mut buf = Vec::new();

    let mut in_body = false;
    let mut pre_depth: u32 = 0;
    let mut style_injected = false;
    let mut span_counter: usize = 0;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(ref e) => {
                let local = local_name_lower(e.name().as_ref());
                if local == "body" {
                    in_body = true;
                } else if local == "pre" {
                    pre_depth += 1;
                }
                writer.write_event(Event::Start(e.borrow()))?;
            }
            Event::End(ref e) => {
                let local = local_name_lower(e.name().as_ref());

                if local == "head" && !style_injected {
                    // Inject Kobo style before </head>.
                    let mut style = BytesStart::new("style");
                    style.push_attribute(("type", "text/css"));
                    writer.write_event(Event::Start(style.borrow()))?;
                    writer.write_event(Event::Text(BytesText::new(".koboSpan{display:inline}")))?;
                    writer.write_event(Event::End(BytesEnd::new("style")))?;
                    style_injected = true;
                }

                if local == "body" {
                    in_body = false;
                } else if local == "pre" {
                    pre_depth = pre_depth.saturating_sub(1);
                }

                writer.write_event(Event::End(e.borrow()))?;
            }
            Event::Text(ref e) => {
                // Wrap non-whitespace text inside body (but not inside <pre>).
                let raw: &str = e.as_ref();
                let is_whitespace_only = raw.chars().all(|c| c.is_ascii_whitespace());

                if in_body && pre_depth == 0 && !is_whitespace_only {
                    span_counter += 1;
                    let span_id = format!("kobo.{chapter}.{span_counter}");
                    let mut span_start = BytesStart::new("span");
                    span_start.push_attribute(("class", "koboSpan"));
                    span_start.push_attribute(("id", span_id.as_str()));
                    writer.write_event(Event::Start(span_start.borrow()))?;
                    writer.write_event(Event::Text(e.borrow()))?;
                    writer.write_event(Event::End(BytesEnd::new("span")))?;
                } else {
                    writer.write_event(Event::Text(e.borrow()))?;
                }
            }
            Event::Eof => break,
            e => writer.write_event(e)?,
        }
        buf.clear();
    }

    Ok(writer.into_inner())
}

/// Extracts the local (un-namespaced) name from a qualified XML name, folded
/// to ASCII lowercase.
fn local_name_lower(qualified: &str) -> String {
    let local = qualified.split(':').next_back().unwrap_or(qualified);
    local.to_ascii_lowercase()
}

// ── Tests
// ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;

    use super::convert_to_kepub;
    use crate::test_support::CONTAINER_XML;

    const MINIMAL_OPF: &[u8] = br#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Test Book</dc:title>
  </metadata>
  <manifest>
    <item id="ch1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine><itemref idref="ch1"/></spine>
</package>"#;

    fn build_epub(chapters: &[(&str, &[u8])]) -> Vec<u8> {
        let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let cursor = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);

        zip.start_file("mimetype", stored).unwrap();
        zip.write_all(b"application/epub+zip").unwrap();

        zip.start_file("META-INF/container.xml", stored).unwrap();
        zip.write_all(CONTAINER_XML).unwrap();

        zip.start_file("content.opf", stored).unwrap();
        zip.write_all(MINIMAL_OPF).unwrap();

        for (name, content) in chapters {
            zip.start_file(*name, stored).unwrap();
            zip.write_all(content).unwrap();
        }

        zip.finish().unwrap().into_inner()
    }

    fn read_entry(epub_bytes: &[u8], name: &str) -> Vec<u8> {
        let cursor = std::io::Cursor::new(epub_bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        let mut entry = archive.by_name(name).unwrap();
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut buf).unwrap();
        buf
    }

    #[test]
    fn wraps_text_in_kobo_spans() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.epub");
        let dst = dir.path().join("dst.kepub.epub");

        let chapter_xhtml = br#"<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Ch1</title></head>
<body><p>Hello world.</p></body>
</html>"#;

        std::fs::write(&src, build_epub(&[("chapter1.xhtml", chapter_xhtml)])).unwrap();
        convert_to_kepub(&src, &dst).unwrap();

        let content = read_entry(&std::fs::read(&dst).unwrap(), "chapter1.xhtml");
        let s = String::from_utf8(content).unwrap();
        assert!(s.contains("koboSpan"), "should contain koboSpan class");
        assert!(s.contains("kobo.1.1"), "should contain span id kobo.1.1");
        assert!(s.contains("Hello world."), "should preserve text");
    }

    #[test]
    fn skips_whitespace_only_text() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.epub");
        let dst = dir.path().join("dst.kepub.epub");

        let chapter_xhtml = br#"<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Ch1</title></head>
<body>
  <p>Text</p>
</body>
</html>"#;

        std::fs::write(&src, build_epub(&[("chapter1.xhtml", chapter_xhtml)])).unwrap();
        convert_to_kepub(&src, &dst).unwrap();

        let content = read_entry(&std::fs::read(&dst).unwrap(), "chapter1.xhtml");
        let s = String::from_utf8(content).unwrap();
        // Only one span (the "Text" node), not for whitespace-only nodes
        assert_eq!(s.matches("koboSpan").count(), 2, "two occurrences per span (open + attr)");
    }

    #[test]
    fn does_not_wrap_pre_content() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.epub");
        let dst = dir.path().join("dst.kepub.epub");

        let chapter_xhtml = br#"<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Ch1</title></head>
<body><pre>preformatted text</pre></body>
</html>"#;

        std::fs::write(&src, build_epub(&[("chapter1.xhtml", chapter_xhtml)])).unwrap();
        convert_to_kepub(&src, &dst).unwrap();

        let content = read_entry(&std::fs::read(&dst).unwrap(), "chapter1.xhtml");
        let s = String::from_utf8(content).unwrap();
        // The CSS block contains ".koboSpan" but no <span> elements should be injected.
        assert!(!s.contains("class=\"koboSpan\""), "pre content should not be wrapped in spans");
    }

    #[test]
    fn injects_kobo_style() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.epub");
        let dst = dir.path().join("dst.kepub.epub");

        let chapter_xhtml = br#"<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Ch1</title></head>
<body><p>Text</p></body>
</html>"#;

        std::fs::write(&src, build_epub(&[("chapter1.xhtml", chapter_xhtml)])).unwrap();
        convert_to_kepub(&src, &dst).unwrap();

        let content = read_entry(&std::fs::read(&dst).unwrap(), "chapter1.xhtml");
        let s = String::from_utf8(content).unwrap();
        assert!(s.contains(".koboSpan"), "should inject kobo CSS");
    }

    #[test]
    fn chapter_indices_increment_across_files() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.epub");
        let dst = dir.path().join("dst.kepub.epub");

        let ch1 = br#"<?xml version="1.0" encoding="utf-8"?>
<html><head><title>Ch1</title></head><body><p>First</p></body></html>"#;
        let ch2 = br#"<?xml version="1.0" encoding="utf-8"?>
<html><head><title>Ch2</title></head><body><p>Second</p></body></html>"#;

        std::fs::write(&src, build_epub(&[("chapter1.xhtml", ch1), ("chapter2.xhtml", ch2)])).unwrap();
        convert_to_kepub(&src, &dst).unwrap();

        let epub_bytes = std::fs::read(&dst).unwrap();
        let s1 = String::from_utf8(read_entry(&epub_bytes, "chapter1.xhtml")).unwrap();
        let s2 = String::from_utf8(read_entry(&epub_bytes, "chapter2.xhtml")).unwrap();
        assert!(s1.contains("kobo.1.1"), "ch1 should use chapter 1");
        assert!(s2.contains("kobo.2.1"), "ch2 should use chapter 2");
    }

    #[test]
    fn non_xhtml_entries_are_copied_verbatim() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.epub");
        let dst = dir.path().join("dst.kepub.epub");

        let chapter_xhtml = br#"<?xml version="1.0" encoding="utf-8"?>
<html><head></head><body><p>Text</p></body></html>"#;
        let image_bytes: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0x00];

        let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let cursor = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        zip.start_file("mimetype", stored).unwrap();
        zip.write_all(b"application/epub+zip").unwrap();
        zip.start_file("META-INF/container.xml", stored).unwrap();
        zip.write_all(CONTAINER_XML).unwrap();
        zip.start_file("content.opf", stored).unwrap();
        zip.write_all(MINIMAL_OPF).unwrap();
        zip.start_file("chapter1.xhtml", stored).unwrap();
        zip.write_all(chapter_xhtml).unwrap();
        zip.start_file("images/cover.jpg", stored).unwrap();
        zip.write_all(image_bytes).unwrap();
        let epub_bytes = zip.finish().unwrap().into_inner();

        std::fs::write(&src, &epub_bytes).unwrap();
        convert_to_kepub(&src, &dst).unwrap();

        let out = std::fs::read(&dst).unwrap();
        let cover = read_entry(&out, "images/cover.jpg");
        assert_eq!(cover, image_bytes, "image should be copied verbatim");
    }

    #[test]
    fn mimetype_is_first_entry() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("src.epub");
        let dst = dir.path().join("dst.kepub.epub");

        let chapter_xhtml = b"<html><head></head><body></body></html>";
        std::fs::write(&src, build_epub(&[("chapter1.xhtml", chapter_xhtml)])).unwrap();
        convert_to_kepub(&src, &dst).unwrap();

        let out = std::fs::read(&dst).unwrap();
        let cursor = std::io::Cursor::new(out);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        assert_eq!(archive.by_index(0).unwrap().name(), "mimetype");
    }
}
