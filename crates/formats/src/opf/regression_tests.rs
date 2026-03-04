/// Regression tests for real-world OPF metadata that previously parsed
/// incorrectly. Each test captures a specific quirk and uses an insta
/// snapshot so any future change in output is explicit and intentional.
///
/// To add a new regression test:
///   1. Add a `#[test]` fn with a name describing the quirk.
///   2. Paste the minimal failing OPF as a `br#"..."#` byte literal.
///   3. Call `extract_metadata` or `parse_sidecar` and assert with
///      `insta::assert_debug_snapshot!(result)`.
///   4. Run `INSTA_UPDATE=always cargo test -p bb-formats <test_name>` to
///      generate the snapshot.
use super::{extract_metadata, parse_sidecar, write_sidecar};

#[test]
fn extract_metadata_opf2_full() {
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
    insta::assert_debug_snapshot!(meta);
}

#[test]
fn isbn10_classified_correctly() {
    let opf = br#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"
            xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>Test</dc:title>
    <dc:identifier opf:scheme="ISBN">0441013597</dc:identifier>
  </metadata>
  <manifest/>
  <spine/>
</package>"#;

    let meta = extract_metadata(opf).expect("parse failed");
    insta::assert_debug_snapshot!(meta);
}

#[test]
fn isbn13_classified_correctly() {
    let opf = br#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"
            xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>Test</dc:title>
    <dc:identifier opf:scheme="ISBN">9780441013593</dc:identifier>
  </metadata>
  <manifest/>
  <spine/>
</package>"#;

    let meta = extract_metadata(opf).expect("parse failed");
    insta::assert_debug_snapshot!(meta);
}

/// OPF 2 with a plain `<dc:date>` year — verifies the year is
/// extracted correctly and not dropped.
#[test]
fn opf2_plain_year_date() {
    let opf = br#"<?xml version="1.0" encoding="utf-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/"
            xmlns:opf="http://www.idpf.org/2007/opf">
    <dc:title>Dune</dc:title>
    <dc:creator opf:role="aut" opf:file-as="Herbert, Frank">Frank Herbert</dc:creator>
    <dc:date>1965</dc:date>
    <dc:language>en</dc:language>
    <dc:identifier opf:scheme="ISBN">9780441013593</dc:identifier>
  </metadata>
  <manifest/>
  <spine/>
</package>"#;

    let meta = extract_metadata(opf).expect("parse failed");
    insta::assert_debug_snapshot!(meta);
}

#[test]
fn sidecar_roundtrip() {
    let original = super::write::tests::full_test_sidecar();
    let bytes = write_sidecar(&original).expect("write failed");
    let parsed = parse_sidecar(&bytes).expect("parse failed");
    insta::assert_debug_snapshot!(parsed);
}
