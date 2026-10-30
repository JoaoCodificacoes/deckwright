use deckwright::{
    model::{Geometry, Object, ObjectKind, Properties, Slide, Transform},
    timing::{Effect, EffectKind, Stage, Timeline, Transition, Trigger, compile_animated_slide},
    units::Emu,
};
use serde_json::Value;
use std::{
    fs,
    io::{Cursor, Write},
    path::PathBuf,
    process::{Command, Output},
};

const CONTENT_TYPES: &str = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Default Extension="rels" ContentType="application/xml"/></Types>"#;
const PRESENTATION: &str = r#"<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:sldIdLst><p:sldId id="256" r:id="rId1"/><p:sldId id="257" r:id="rId2"/></p:sldIdLst><p:sldSz cx="12192000" cy="6858000"/></p:presentation>"#;
const RELS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="slide" Target="slides/slide1.xml"/><Relationship Id="rId2" Type="slide" Target="slides/slide2.xml"/></Relationships>"#;

fn object(id: u32, name: &str) -> Object {
    Object {
        key: name.into(),
        id,
        name: name.into(),
        morph_name: None,
        description: None,
        kind: ObjectKind::Shape {
            textbox: false,
            lock_no_group: false,
            placeholder: None,
            properties: Properties {
                transform: Some(Transform {
                    x: Emu(100),
                    y: Emu(100),
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
    }
}

fn slide(objects: Vec<Object>) -> Slide {
    Slide { objects }
}

fn fade_on_click() -> Timeline {
    Timeline {
        stages: vec![Stage {
            trigger: Trigger::OnClick,
            delay_ms: 0,
            effects: vec![Effect {
                target: 2,
                delay_ms: 0,
                duration_ms: 600,
                kind: EffectKind::FadeIn,
            }],
        }],
    }
}

/// Writes a two-slide package: slide 1 fades shape 2 in on click and both
/// slides carry a shape named "!!a"; slide 2 declares a Morph transition.
fn fixture(test: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "deckwright-query-cli-{}-{test}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("deck.pptx");
    let slide1 = compile_animated_slide(
        &slide(vec![object(2, "Box"), object(3, "!!a")]),
        &fade_on_click(),
        &Transition::None,
    )
    .unwrap();
    let slide2 = compile_animated_slide(
        &slide(vec![object(2, "!!a")]),
        &Timeline { stages: vec![] },
        &Transition::Morph { duration_ms: 900 },
    )
    .unwrap();
    let parts: Vec<(&str, Vec<u8>)> = vec![
        ("[Content_Types].xml", CONTENT_TYPES.as_bytes().to_vec()),
        ("ppt/presentation.xml", PRESENTATION.as_bytes().to_vec()),
        ("ppt/_rels/presentation.xml.rels", RELS.as_bytes().to_vec()),
        ("ppt/slides/slide1.xml", slide1),
        ("ppt/slides/slide2.xml", slide2),
    ];
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content) in parts {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&content).unwrap();
    }
    fs::write(&path, zip.finish().unwrap().into_inner()).unwrap();
    (dir, path)
}

fn dw(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_dw"))
        .args(args)
        .output()
        .unwrap()
}

fn json_ok(out: &Output) -> Value {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout.iter().filter(|b| **b == b'\n').count(), 1);
    assert!(out.stdout.ends_with(b"\n"));
    serde_json::from_slice(&out.stdout).unwrap()
}

fn usage_error(out: &Output) {
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).starts_with("dw: "),
        "{out:?}"
    );
}

#[test]
fn state_before_click_hides_the_fade_target() {
    let (dir, path) = fixture("before-click");
    let p = path.to_str().unwrap();
    let v = json_ok(&dw(&[
        "query", "state", p, "--slide", "1", "--at-ms", "0", "--json",
    ]));
    assert_eq!(v["schema"], "dw.query.state/1");
    assert_eq!(v["shapes"][0]["name"], "Box");
    assert_eq!(v["shapes"][0]["visible"], false);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn state_mid_fade_is_strictly_between_zero_and_one() {
    let (dir, path) = fixture("mid-fade");
    let p = path.to_str().unwrap();
    let v = json_ok(&dw(&[
        "query",
        "state",
        p,
        "--slide",
        "1",
        "--clicks-ms",
        "100",
        "--at-ms",
        "400",
        "--json",
    ]));
    let opacity = v["shapes"][0]["opacity"].as_f64().unwrap();
    assert!(0.0 < opacity && opacity < 1.0, "{opacity}");
    // Flags may appear in any order.
    let reordered = json_ok(&dw(&[
        "query",
        "state",
        p,
        "--json",
        "--at-ms",
        "400",
        "--clicks-ms",
        "100",
        "--slide",
        "1",
    ]));
    assert_eq!(reordered, v);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn state_after_fade_is_fully_opaque() {
    let (dir, path) = fixture("after-fade");
    let p = path.to_str().unwrap();
    let v = json_ok(&dw(&[
        "query",
        "state",
        p,
        "--slide",
        "1",
        "--clicks-ms",
        "100",
        "--at-ms",
        "1000",
        "--json",
    ]));
    assert_eq!(v["shapes"][0]["opacity"], 1.0);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn timeline_reports_the_versioned_schema() {
    let (dir, path) = fixture("timeline");
    let p = path.to_str().unwrap();
    let v = json_ok(&dw(&["query", "timeline", p, "--slide", "1", "--json"]));
    assert_eq!(v["schema"], "dw.query.timeline/1");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn morph_reports_the_explicit_named_match() {
    let (dir, path) = fixture("morph");
    let p = path.to_str().unwrap();
    let v = json_ok(&dw(&[
        "query", "morph", p, "--from", "1", "--to", "2", "--json",
    ]));
    assert_eq!(v["schema"], "dw.query.morph/1");
    assert_eq!(v["explicit_matches"][0]["name"], "!!a");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn invalid_invocations_are_usage_or_input_errors() {
    let (dir, path) = fixture("errors");
    let p = path.to_str().unwrap();
    let missing = dir.join("missing.pptx");
    let m = missing.to_str().unwrap();
    let cases: Vec<Output> = vec![
        dw(&[
            "query", "state", m, "--slide", "1", "--at-ms", "0", "--json",
        ]),
        dw(&[
            "query", "state", p, "--slide", "0", "--at-ms", "0", "--json",
        ]),
        dw(&[
            "query", "state", p, "--slide", "3", "--at-ms", "0", "--json",
        ]),
        dw(&[
            "query",
            "state",
            p,
            "--slide",
            "1",
            "--clicks-ms",
            "500",
            "--at-ms",
            "400",
            "--json",
        ]),
        dw(&[
            "query",
            "state",
            p,
            "--slide",
            "1",
            "--clicks-ms",
            "300,100",
            "--at-ms",
            "400",
            "--json",
        ]),
        dw(&[
            "query",
            "state",
            p,
            "--slide",
            "1",
            "--clicks-ms",
            "",
            "--at-ms",
            "0",
            "--json",
        ]),
        dw(&[
            "query",
            "state",
            p,
            "--slide",
            "1",
            "--clicks-ms",
            "1,x",
            "--at-ms",
            "0",
            "--json",
        ]),
        dw(&[
            "query", "state", p, "--slide", "1", "--slide", "2", "--at-ms", "0", "--json",
        ]),
        dw(&[
            "query", "state", p, "--slide", "1", "--at-ms", "0", "--bogus", "1", "--json",
        ]),
        dw(&["query", "state", p, "--slide", "1", "--at-ms", "0"]),
        dw(&["query", "state", p, "--slide", "1", "--json"]),
        dw(&["query", "morph", p, "--from", "1", "--json"]),
    ];
    for out in &cases {
        usage_error(out);
    }
    fs::remove_dir_all(dir).unwrap();
}
