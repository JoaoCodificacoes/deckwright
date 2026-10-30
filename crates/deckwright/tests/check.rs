use deckwright::{
    Limits, Package,
    check::{check_package, check_path},
};
use std::io::{Cursor, Write};
fn package(body: &str) -> Package {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in [
        ("[Content_Types].xml", "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"xml\" ContentType=\"application/xml\"/></Types>".to_string()),
        ("ppt/slides/slide1.xml", format!("<p:sld xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" xmlns:x=\"urn:other\">{body}</p:sld>"))
    ] {
        zip.start_file(name, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(data.as_bytes()).unwrap();
    }
    Package::from_bytes(zip.finish().unwrap().into_inner(), Limits::default()).unwrap()
}
#[test]
fn valid_ids_names_and_targets_pass() {
    let report = check_package(&package(
        "<p:cNvPr id=\"1\" name=\"!!a\"/><p:cNvPr id=\"2\" name=\"!!b\"/><p:spTgt spid=\"2\"/>",
    ));
    assert!(!report.has_errors());
    assert!(report.findings.is_empty());
    assert_eq!(report.schema, "dw.check/1");
    assert!(
        report
            .limitations
            .iter()
            .any(|s| s.contains("Bounds cover resolved axis-aligned geometry"))
    );
}
#[test]
fn duplicate_shape_id_is_slide_local_error() {
    let report = check_package(&package("<p:cNvPr id=\"1\"/><p:cNvPr id=\"1\"/>"));
    assert!(report.has_errors());
    assert_eq!(report.findings[0].rule, "duplicate-shape-id");
    assert_eq!(report.findings[0].shape.as_deref(), Some("1"));
}
#[test]
fn duplicate_explicit_morph_names_warn_only() {
    let report = check_package(&package(
        "<p:cNvPr id=\"1\" name=\"!!a\"/><p:cNvPr id=\"2\" name=\"!!a\"/>",
    ));
    assert!(!report.has_errors());
    assert_eq!(report.findings[0].rule, "duplicate-morph-name");
    assert_eq!(report.findings[0].severity, "warning");
    assert!(
        check_package(&package(
            "<p:cNvPr id=\"1\" name=\"a\"/><p:cNvPr id=\"2\" name=\"a\"/>"
        ))
        .findings
        .is_empty()
    );
}
#[test]
fn absent_timing_target_errors_and_foreign_namespace_is_ignored() {
    let report = check_package(&package(
        "<p:cNvPr id=\"1\"/><p:spTgt spid=\"2\"/><x:spTgt spid=\"3\"/><x:cNvPr id=\"1\"/>",
    ));
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].rule, "missing-timing-target");
}
#[test]
fn import_failure_is_never_clean() {
    let report = check_path(
        "/path/that/does/not/exist/deckwright.pptx",
        Limits::default(),
    );
    assert!(!report.imported);
    assert!(report.has_errors());
}

#[test]
fn missing_relationship_target_is_import_error_report() {
    let path = std::env::temp_dir().join(format!(
        "dw-check-missing-target-{}.pptx",
        std::process::id()
    ));
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, xml) in [
        (
            "[Content_Types].xml",
            "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/></Types>",
        ),
        (
            "_rels/.rels",
            "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"r1\" Type=\"slide\" Target=\"missing.xml\"/></Relationships>",
        ),
    ] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(xml.as_bytes()).unwrap();
    }
    std::fs::write(&path, zip.finish().unwrap().into_inner()).unwrap();
    let report = check_path(&path, Limits::default());
    std::fs::remove_file(path).unwrap();
    assert!(!report.imported);
    assert!(report.has_errors());
    assert_eq!(report.findings[0].rule, "relationship-import");
}
#[test]
fn shape_ids_use_unsigned_integer_value_not_lexical_spelling() {
    let report = check_package(&package("<p:cNvPr id=\"1\"/><p:cNvPr id=\"01\"/>"));
    assert_eq!(report.findings[0].rule, "duplicate-shape-id");
    let report = check_package(&package("<p:cNvPr id=\"1\"/><p:spTgt spid=\"01\"/>"));
    assert!(report.findings.is_empty());
}
