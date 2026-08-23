use std::{io::Read, path::Path};

use bb_core::{Error as CoreError, pipeline::ExtractedMetadata};

/// Read the raw OPF XML bytes from an EPUB file.
pub(crate) fn read_epub_opf_xml(path: &Path) -> Result<String, crate::Error> {
    let (bytes, _dir) = read_opf_bytes_and_dir(path)?;
    String::from_utf8(bytes).map_err(|e| crate::Error::InvalidValue(e.to_string()))
}

pub(crate) fn extract_epub_metadata(path: &Path) -> Result<ExtractedMetadata, CoreError> {
    let (opf_bytes, opf_dir) = read_opf_bytes_and_dir(path).map_err(|e| CoreError::Infrastructure(e.to_string()))?;
    let mut meta = crate::opf::extract_metadata(&opf_bytes).map_err(|e| CoreError::Infrastructure(e.to_string()))?;

    // Extract cover image if the OPF manifest declares one.
    if let Some(cover_href) = crate::opf::extract_cover_href(&opf_bytes) {
        let cover_path = resolve_zip_path(&opf_dir, &cover_href);
        if let Ok(bytes) = read_zip_entry(path, &cover_path) {
            meta.cover_bytes = Some(bytes);
        }
    }

    Ok(meta)
}

/// Returns the raw OPF XML bytes and the OPF file's parent directory within
/// the ZIP (e.g. `"OEBPS"` for `"OEBPS/content.opf"`, or `""` for a root OPF).
fn read_opf_bytes_and_dir(path: &Path) -> Result<(Vec<u8>, String), crate::Error> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    let opf_path = {
        let mut container = archive.by_name("META-INF/container.xml")?;
        let mut buf = Vec::new();
        container.read_to_end(&mut buf)?;
        find_opf_path(&buf)?
    };

    let opf_dir = match opf_path.rfind('/') {
        Some(pos) => opf_path[..pos].to_string(),
        None => String::new(),
    };

    let mut opf_file = archive.by_name(&opf_path)?;
    let mut buf = Vec::new();
    opf_file.read_to_end(&mut buf)?;
    Ok((buf, opf_dir))
}

/// Resolve a manifest href relative to the OPF directory.
///
/// For example, `opf_dir = "OEBPS"` and `href = "images/cover.jpg"` yields
/// `"OEBPS/images/cover.jpg"`.  A root-level OPF (`opf_dir = ""`) returns
/// the href unchanged.
pub(crate) fn resolve_zip_path(opf_dir: &str, href: &str) -> String {
    if opf_dir.is_empty() { href.to_string() } else { format!("{opf_dir}/{href}") }
}

/// Read the raw bytes of a single entry from the EPUB ZIP archive.
fn read_zip_entry(epub_path: &Path, entry_path: &str) -> Result<Vec<u8>, crate::Error> {
    let file = std::fs::File::open(epub_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut entry = archive.by_name(entry_path)?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf)?;
    Ok(buf)
}

/// Parse META-INF/container.xml and return the `full-path` of the rootfile.
pub(crate) fn find_opf_path(xml: &[u8]) -> Result<String, crate::Error> {
    use quick_xml::{Reader, XmlVersion, events::Event};
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e) | Event::Start(ref e)) if e.local_name().as_ref() == "rootfile" => {
                for attr in e.attributes() {
                    let attr = attr.map_err(quick_xml::Error::from)?;
                    if attr.key.as_ref() == "full-path" {
                        let val = attr.normalized_value(XmlVersion::Implicit1_0)?;
                        return Ok(val.into_owned());
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(e.into()),
            _ => {}
        }
    }
    Err(crate::Error::InvalidValue("container.xml: no rootfile found".to_string()))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::extract_epub_metadata;
    use crate::test_support::{CONTAINER_XML, build_test_epub};

    #[test]
    fn extracts_title_and_author() {
        let epub_bytes = build_test_epub();
        let path = std::env::temp_dir().join("bookboss_test_epub.epub");
        std::fs::write(&path, &epub_bytes).unwrap();

        let meta = extract_epub_metadata(&path).expect("extraction failed");

        assert_eq!(meta.title.as_deref(), Some("Dune"));
        let authors = meta.authors.as_ref().expect("authors missing");
        assert_eq!(authors[0].name, "Frank Herbert");

        let _ = std::fs::remove_file(&path);
    }

    // Minimal JPEG magic bytes (SOI marker).
    const FAKE_JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];

    fn build_epub2_with_cover() -> Vec<u8> {
        const OPF: &[u8] = br#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>Dune</dc:title>
    <meta name="cover" content="cover-img"/>
  </metadata>
  <manifest>
    <item id="cover-img" href="cover.jpg" media-type="image/jpeg"/>
  </manifest>
  <spine/>
</package>"#;

        let buf = Vec::new();
        let cursor = std::io::Cursor::new(buf);
        let mut zip = zip::ZipWriter::new(cursor);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        zip.start_file("META-INF/container.xml", opts).unwrap();
        zip.write_all(CONTAINER_XML).unwrap();
        zip.start_file("content.opf", opts).unwrap();
        zip.write_all(OPF).unwrap();
        zip.start_file("cover.jpg", opts).unwrap();
        zip.write_all(FAKE_JPEG).unwrap();

        zip.finish().unwrap().into_inner()
    }

    fn build_epub3_with_cover() -> Vec<u8> {
        const OPF: &[u8] = br#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Foundation</dc:title>
  </metadata>
  <manifest>
    <item id="cover" href="images/cover.jpg" media-type="image/jpeg" properties="cover-image"/>
  </manifest>
  <spine/>
</package>"#;

        let buf = Vec::new();
        let cursor = std::io::Cursor::new(buf);
        let mut zip = zip::ZipWriter::new(cursor);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

        zip.start_file("META-INF/container.xml", opts).unwrap();
        zip.write_all(CONTAINER_XML).unwrap();
        zip.start_file("content.opf", opts).unwrap();
        zip.write_all(OPF).unwrap();
        zip.start_file("images/cover.jpg", opts).unwrap();
        zip.write_all(FAKE_JPEG).unwrap();

        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn epub2_cover_extracted() {
        let path = std::env::temp_dir().join("bookboss_test_epub2_cover.epub");
        std::fs::write(&path, build_epub2_with_cover()).unwrap();

        let meta = extract_epub_metadata(&path).expect("extraction failed");

        assert!(meta.cover_bytes.is_some(), "expected cover_bytes to be populated");
        assert_eq!(meta.cover_bytes.as_deref(), Some(FAKE_JPEG));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn epub3_cover_extracted() {
        let path = std::env::temp_dir().join("bookboss_test_epub3_cover.epub");
        std::fs::write(&path, build_epub3_with_cover()).unwrap();

        let meta = extract_epub_metadata(&path).expect("extraction failed");

        assert!(meta.cover_bytes.is_some(), "expected cover_bytes to be populated");
        assert_eq!(meta.cover_bytes.as_deref(), Some(FAKE_JPEG));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn find_opf_path_handles_non_self_closing_rootfile() {
        const CONTAINER_XML_NON_SELF_CLOSING: &[u8] = br#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"></rootfile>
  </rootfiles>
</container>"#;

        let opf_path = super::find_opf_path(CONTAINER_XML_NON_SELF_CLOSING).expect("rootfile should be found");
        assert_eq!(opf_path, "OEBPS/content.opf");
    }

    #[test]
    fn epub_without_cover_returns_none() {
        let epub_bytes = build_test_epub();
        let path = std::env::temp_dir().join("bookboss_test_epub_no_cover.epub");
        std::fs::write(&path, &epub_bytes).unwrap();

        let meta = extract_epub_metadata(&path).expect("extraction failed");

        assert!(meta.cover_bytes.is_none());

        let _ = std::fs::remove_file(&path);
    }
}
