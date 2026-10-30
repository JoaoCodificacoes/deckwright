use deckwright::{
    Limits, Package,
    check::{CheckReport, Suppression, check_package, check_package_with},
    session::Server,
};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Cursor, Write},
};

const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const SLIDE1: &str = "ppt/slides/slide1.xml";
const SLIDE2: &str = "ppt/slides/slide2.xml";

/// Two slides, 1000 x 500 units. `order` lists slide numbers in presentation order.
fn deck(order: &[u32], slide1: &[u8], slide2: &[u8]) -> Package {
    let ids: String = order
        .iter()
        .map(|n| format!("<p:sldId id=\"{}\" r:id=\"rId{n}\"/>", 255 + n))
        .collect();
    let presentation = format!(
        "<p:presentation xmlns:p=\"{P}\" xmlns:r=\"{R}\"><p:sldIdLst>{ids}</p:sldIdLst><p:sldSz cx=\"1000\" cy=\"500\"/></p:presentation>"
    );
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in [
        (
            "[Content_Types].xml",
            b"<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"xml\" ContentType=\"application/xml\"/><Default Extension=\"rels\" ContentType=\"application/xml\"/></Types>".to_vec(),
        ),
        ("ppt/presentation.xml", presentation.into_bytes()),
        (
            "ppt/_rels/presentation.xml.rels",
            b"<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"slide\" Target=\"slides/slide1.xml\"/><Relationship Id=\"rId2\" Type=\"slide\" Target=\"slides/slide2.xml\"/></Relationships>".to_vec(),
        ),
        (SLIDE1, slide1.to_vec()),
        (SLIDE2, slide2.to_vec()),
    ] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&data).unwrap();
    }
    Package::from_bytes(zip.finish().unwrap().into_inner(), Limits::default()).unwrap()
}

fn raw_slide(body: &str) -> Vec<u8> {
    format!("<p:sld xmlns:p=\"{P}\" xmlns:a=\"{A}\"><p:cSld><p:spTree>{body}</p:spTree></p:cSld></p:sld>")
        .into_bytes()
}

fn sp(id: u32, name: &str, x: i64, y: i64) -> String {
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"{name}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"{x}\" y=\"{y}\"/><a:ext cx=\"100\" cy=\"100\"/></a:xfrm></p:spPr></p:sp>"
    )
}

/// Slide 1: shape 2 is off the right edge (off-slide warning).
/// Slide 2: shapes 2 and 3 share the explicit Morph name (duplicate-morph-name warning on 3).
fn warned_deck(order: &[u32]) -> Package {
    deck(
        order,
        &raw_slide(&sp(2, "Late", 1100, 100)),
        &raw_slide(&format!(
            "{}{}",
            sp(2, "!!same", 100, 100),
            sp(3, "!!same", 300, 100)
        )),
    )
}

/// Slide 1 holds two shapes with ID 2: a duplicate-shape-id error.
fn error_deck() -> Package {
    deck(
        &[1, 2],
        &raw_slide(&format!("{}{}", sp(2, "a", 100, 100), sp(2, "b", 300, 100))),
        &raw_slide(&sp(2, "c", 100, 100)),
    )
}

fn rule(rule: &str) -> Suppression {
    Suppression {
        rule: rule.into(),
        slide: None,
        shape: None,
    }
}

fn scoped(rule: &str, slide: usize, shape: Option<&str>) -> Suppression {
    Suppression {
        rule: rule.into(),
        slide: Some(slide),
        shape: shape.map(str::to_owned),
    }
}

/// (rule, slide part, shape) for each finding, in report order.
fn keys(findings: &[deckwright::check::Finding]) -> Vec<(&str, Option<&str>, Option<&str>)> {
    findings
        .iter()
        .map(|f| (f.rule.as_str(), f.slide.as_deref(), f.shape.as_deref()))
        .collect()
}

fn unused(report: &CheckReport) -> Vec<&String> {
    report
        .limitations
        .iter()
        .filter(|l| l.contains("is unused"))
        .collect()
}

#[test]
fn without_suppressions_nothing_moves_and_suppressed_is_present() {
    let package = warned_deck(&[1, 2]);
    let report = check_package_with(&package, &[]);
    // Rule-pass findings come before bounds findings, so compare as a sorted set.
    let mut found = keys(&report.findings);
    found.sort();
    assert_eq!(
        found,
        vec![
            ("duplicate-morph-name", Some(SLIDE2), Some("3")),
            ("off-slide", Some(SLIDE1), Some("2")),
        ]
    );
    assert!(report.suppressed.is_empty());
    assert!(unused(&report).is_empty());
    assert_eq!(
        check_package(&package).findings.len(),
        report.findings.len()
    );
    let value = serde_json::to_value(&report).unwrap();
    assert_eq!(value["suppressed"], json!([]));
}

#[test]
fn rule_only_suppresses_every_matching_warning() {
    let report = check_package_with(&warned_deck(&[1, 2]), &[rule("off-slide")]);
    assert_eq!(
        keys(&report.findings),
        vec![("duplicate-morph-name", Some(SLIDE2), Some("3"))]
    );
    assert_eq!(
        keys(&report.suppressed),
        vec![("off-slide", Some(SLIDE1), Some("2"))]
    );
    assert!(unused(&report).is_empty());
}

#[test]
fn rule_and_slide_suppress_only_that_slide() {
    let package = warned_deck(&[1, 2]);
    let report = check_package_with(&package, &[scoped("duplicate-morph-name", 2, None)]);
    assert_eq!(
        keys(&report.suppressed),
        vec![("duplicate-morph-name", Some(SLIDE2), Some("3"))]
    );
    assert_eq!(report.findings.len(), 1);
    assert_eq!(report.findings[0].rule, "off-slide");
    assert!(unused(&report).is_empty());
}

#[test]
fn rule_slide_and_shape_suppress_only_that_shape() {
    let package = warned_deck(&[1, 2]);
    let report = check_package_with(&package, &[scoped("off-slide", 1, Some("2"))]);
    assert_eq!(report.suppressed.len(), 1);
    assert_eq!(report.findings.len(), 1);
    assert!(unused(&report).is_empty());
}

#[test]
fn non_matching_slide_or_shape_keeps_the_finding_and_adds_unused_limitations() {
    let package = warned_deck(&[1, 2]);
    let report = check_package_with(
        &package,
        &[
            scoped("off-slide", 2, None),
            scoped("off-slide", 1, Some("9")),
            scoped("off-slide", 1, Some("3")),
        ],
    );
    assert!(report.suppressed.is_empty());
    assert_eq!(report.findings.len(), 2);
    assert_eq!(unused(&report).len(), 3, "{:?}", report.limitations);
    assert!(unused(&report)[0].contains("off-slide on slide 2"));
}

#[test]
fn errors_are_never_suppressed_and_the_attempt_is_a_limitation() {
    let package = error_deck();
    let report = check_package_with(&package, &[rule("duplicate-shape-id")]);
    assert!(report.has_errors());
    assert!(report.suppressed.is_empty());
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule == "duplicate-shape-id" && f.severity == "error"),
        "{:?}",
        report.findings
    );
    let refused: Vec<_> = report
        .limitations
        .iter()
        .filter(|l| l.contains("did not apply") && l.contains("duplicate-shape-id"))
        .collect();
    assert_eq!(refused.len(), 1, "{:?}", report.limitations);
    assert!(unused(&report).is_empty());
}

#[test]
fn slide_numbers_follow_presentation_order() {
    // Slide 2 (ppt/slides/slide2.xml) is listed first, so it is presentation slide 1.
    let package = warned_deck(&[2, 1]);
    let wrong = check_package_with(&package, &[scoped("duplicate-morph-name", 2, None)]);
    assert!(wrong.suppressed.is_empty());
    assert_eq!(unused(&wrong).len(), 1);

    let right = check_package_with(&package, &[scoped("duplicate-morph-name", 1, Some("3"))]);
    assert_eq!(
        keys(&right.suppressed),
        vec![("duplicate-morph-name", Some(SLIDE2), Some("3"))]
    );
}

#[test]
fn session_check_passes_suppress_through() {
    let dir = std::env::temp_dir().join(format!("deckwright-suppress-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("warned.pptx");
    warned_deck(&[1, 2]).write(&path).unwrap();

    let mut server = Server::new(4);
    let send = |server: &mut Server, request: Value| -> Value {
        serde_json::from_str(&server.handle_line(&request.to_string())).unwrap()
    };
    let open = send(
        &mut server,
        json!({"id": 1, "op": "open", "path": path.to_string_lossy()}),
    );
    assert_eq!(open["ok"], true, "{open}");
    let handle = open["result"]["handle"].clone();

    let plain = send(
        &mut server,
        json!({"id": 2, "op": "check", "handle": handle}),
    );
    assert_eq!(plain["result"]["report"]["suppressed"], json!([]));
    assert_eq!(
        plain["result"]["report"]["findings"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let suppressed = send(
        &mut server,
        json!({"id": 3, "op": "check", "handle": handle,
            "suppress": [{"rule": "off-slide", "slide": 1, "shape": "2"}]}),
    );
    assert_eq!(suppressed["ok"], true, "{suppressed}");
    let report = &suppressed["result"]["report"];
    assert_eq!(report["suppressed"].as_array().unwrap().len(), 1);
    assert_eq!(report["suppressed"][0]["rule"], "off-slide");
    assert_eq!(report["findings"].as_array().unwrap().len(), 1);

    for bad in [
        json!([{"rule": "off-slide", "slide": 0}]),
        json!([{"rule": ""}]),
        json!([{"rule": "off-slide", "slide": "1"}]),
        json!([{"rule": "off-slide", "shapes": "2"}]),
        json!("off-slide"),
    ] {
        let response = send(
            &mut server,
            json!({"id": 4, "op": "check", "handle": handle, "suppress": bad}),
        );
        assert_eq!(response["ok"], false, "{bad}");
        assert_eq!(response["error"]["code"], "parse", "{bad}");
    }
    fs::remove_dir_all(dir).unwrap();
}
