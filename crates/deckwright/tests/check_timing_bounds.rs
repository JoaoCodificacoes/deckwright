use deckwright::{
    Limits, Package,
    check::{CheckReport, check_package},
    model::*,
    timing::*,
    units::Emu,
};
use std::io::{Cursor, Write};

const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const SLIDE: &str = "ppt/slides/slide1.xml";

/// Slide size is 1000 x 500 units, matching the coordinates used below.
fn package(slide: &[u8]) -> Package {
    let presentation = format!(
        "<p:presentation xmlns:p=\"{P}\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><p:sldIdLst><p:sldId id=\"256\" r:id=\"rId1\"/></p:sldIdLst><p:sldSz cx=\"1000\" cy=\"500\"/></p:presentation>"
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
            b"<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"slide\" Target=\"slides/slide1.xml\"/></Relationships>".to_vec(),
        ),
        (SLIDE, slide.to_vec()),
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

fn sp(id: u32, x: i64, y: i64, w: i64, h: i64) -> String {
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"s{id}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"{x}\" y=\"{y}\"/><a:ext cx=\"{w}\" cy=\"{h}\"/></a:xfrm></p:spPr></p:sp>"
    )
}

fn model(x: i64, y: i64) -> Slide {
    Slide {
        objects: vec![Object {
            key: "box".into(),
            id: 2,
            name: "Box".into(),
            morph_name: None,
            description: None,
            kind: ObjectKind::Shape {
                textbox: false,
                lock_no_group: false,
                placeholder: None,
                properties: Properties {
                    transform: Some(Transform {
                        x: Emu(x),
                        y: Emu(y),
                        width: Emu(100),
                        height: Emu(100),
                        ..Default::default()
                    }),
                    geometry: Some(Geometry {
                        preset: "rect".into(),
                        adjustments: vec![],
                    }),
                    ..Default::default()
                },
                style: None,
                text: None,
            },
        }],
    }
}

fn effect(kind: EffectKind) -> Effect {
    Effect {
        target: 2,
        delay_ms: 0,
        duration_ms: 600,
        kind,
    }
}

fn click(kind: EffectKind) -> Stage {
    Stage {
        trigger: Trigger::OnClick,
        delay_ms: 0,
        effects: vec![effect(kind)],
    }
}

fn timed(x: i64, stages: Vec<Stage>) -> Vec<u8> {
    compile_animated_slide(&model(x, 100), &Timeline { stages }, &Transition::None).unwrap()
}

fn new_rules(report: &CheckReport) -> Vec<(&str, Option<&str>)> {
    report
        .findings
        .iter()
        .filter(|f| {
            matches!(
                f.rule.as_str(),
                "timing-parse"
                    | "timing-invalid"
                    | "timing-opaque"
                    | "off-slide"
                    | "partially-off-slide"
            )
        })
        .map(|f| (f.rule.as_str(), f.shape.as_deref()))
        .collect()
}

#[test]
fn clean_deck_has_no_timing_or_bounds_findings() {
    let report = check_package(&package(&raw_slide(&format!(
        "{}{}{}",
        sp(2, 100, 100, 200, 100),
        sp(3, 0, 0, 1000, 500),
        sp(4, 500, 10, 0, 100)
    ))));
    assert!(!report.has_errors());
    assert!(report.findings.is_empty(), "{:?}", report.findings);

    let timed_report = check_package(&package(&timed(100, vec![click(EffectKind::FadeIn)])));
    assert!(!timed_report.has_errors());
    assert!(
        new_rules(&timed_report).is_empty(),
        "{:?}",
        timed_report.findings
    );
}

#[test]
fn shape_fully_off_right_edge_warns_once() {
    let report = check_package(&package(&raw_slide(&sp(2, 1100, 100, 100, 100))));
    assert!(!report.has_errors());
    assert_eq!(new_rules(&report), vec![("off-slide", Some("2"))]);
    let finding = &report.findings[0];
    assert_eq!(finding.severity, "warning");
    assert_eq!(finding.slide.as_deref(), Some(SLIDE));
}

#[test]
fn shape_straddling_bottom_edge_is_partial_once() {
    let report = check_package(&package(&raw_slide(&sp(2, 100, 450, 100, 100))));
    assert!(!report.has_errors());
    assert_eq!(new_rules(&report), vec![("partially-off-slide", Some("2"))]);
    assert_eq!(report.findings[0].severity, "warning");
}

#[test]
fn edge_contact_from_inside_is_not_a_finding() {
    let report = check_package(&package(&raw_slide(&sp(2, 900, 400, 100, 100))));
    assert!(new_rules(&report).is_empty(), "{:?}", report.findings);
}

#[test]
fn fade_in_hidden_at_start_reports_only_the_settled_state() {
    let report = check_package(&package(&timed(1100, vec![click(EffectKind::FadeIn)])));
    assert!(!report.has_errors());
    assert_eq!(new_rules(&report), vec![("off-slide", Some("2"))]);
    assert!(report.findings[0].message.contains("settled"));
}

#[test]
fn fade_out_hidden_in_settled_state_is_not_checked_there() {
    // Motion starts on-slide (static position is the resting END) and ends off-slide,
    // then a finished FadeOut hides the shape in the settled state.
    let motion = Stage {
        trigger: Trigger::OnSlideStart,
        delay_ms: 0,
        effects: vec![effect(EffectKind::Motion {
            from_x: -0.5,
            from_y: 0.0,
            to_x: 0.0,
            to_y: 0.0,
            accelerate: 0.0,
            decelerate: 0.0,
        })],
    };
    let report = check_package(&package(&timed(
        1100,
        vec![motion, click(EffectKind::FadeOut)],
    )));
    assert!(new_rules(&report).is_empty(), "{:?}", report.findings);
}

#[test]
fn opaque_timing_warns_naming_its_target() {
    let xml = String::from_utf8(timed(100, vec![click(EffectKind::FadeIn)])).unwrap();
    let xml = xml.replace("filter=\"fade\"", "filter=\"zoom\"");
    let report = check_package(&package(xml.as_bytes()));
    assert!(!report.has_errors());
    let finding = report
        .findings
        .iter()
        .find(|f| f.rule == "timing-opaque")
        .expect("timing-opaque finding");
    assert_eq!(finding.severity, "warning");
    assert_eq!(finding.shape.as_deref(), Some("2"));
    assert!(finding.message.contains('2'));
}

#[test]
fn malformed_timing_is_a_parse_error() {
    let xml = String::from_utf8(timed(100, vec![click(EffectKind::FadeIn)])).unwrap();
    let xml = xml.replace("spid=\"2\"", "spid=\"99\"");
    let report = check_package(&package(xml.as_bytes()));
    assert!(report.has_errors());
    let finding = report
        .findings
        .iter()
        .find(|f| f.rule == "timing-parse")
        .expect("timing-parse finding");
    assert_eq!(finding.severity, "error");
    assert_eq!(finding.shape, None);
}

#[test]
fn rotated_partially_off_shape_gets_no_bounds_verdict() {
    let body = "<p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"r\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm rot=\"5400000\"><a:off x=\"900\" y=\"450\"/><a:ext cx=\"200\" cy=\"100\"/></a:xfrm></p:spPr></p:sp>";
    let report = check_package(&package(&raw_slide(body)));
    assert!(new_rules(&report).is_empty(), "{:?}", report.findings);
}

#[test]
fn report_keeps_the_check_schema_shape() {
    let xml = String::from_utf8(timed(1100, vec![click(EffectKind::FadeIn)])).unwrap();
    let xml = xml.replace("filter=\"fade\"", "filter=\"zoom\"");
    let report = check_package(&package(xml.as_bytes()));
    assert!(!report.findings.is_empty());
    let value = serde_json::to_value(&report).unwrap();
    let object = value.as_object().unwrap();
    assert_eq!(object.len(), 5);
    assert_eq!(object["schema"], "dw.check/1");
    assert_eq!(object["schema_version"], 1);
    assert_eq!(object["imported"], true);
    assert!(object["limitations"].is_array());
    for finding in object["findings"].as_array().unwrap() {
        let finding = finding.as_object().unwrap();
        assert_eq!(finding.len(), 6);
        assert!(matches!(
            finding["severity"].as_str(),
            Some("error" | "warning")
        ));
        assert!(finding["rule"].is_string());
        assert!(finding["slide"].is_string() || finding["slide"].is_null());
        assert!(finding["shape"].is_string() || finding["shape"].is_null());
        assert!(finding["message"].is_string());
        assert!(finding["fix_hint"].is_string());
    }
}
