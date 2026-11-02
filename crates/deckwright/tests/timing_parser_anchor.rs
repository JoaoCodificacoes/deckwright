use deckwright::query::{QuerySession, ShapeSnapshot, SlideSnapshot};
use deckwright::{model::*, timing::*, units::Emu};

fn slide() -> Slide {
    Slide {
        objects: vec![Object {
            key: "box".into(),
            id: 2,
            name: "Box".into(),
            morph_name: None,
            description: None,
            hyperlink: None,
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
        }],
    }
}

fn effect(kind: EffectKind, delay_ms: i64, duration_ms: u32) -> Effect {
    Effect {
        target: 2,
        delay_ms,
        duration_ms,
        kind,
    }
}

/// OnClick FadeIn (delay 200, duration 1000) ends at 1200. AfterPrevious FadeOut
/// (stage delay 300, duration 500) must begin at 1500.
fn click_then_after() -> Timeline {
    Timeline {
        stages: vec![
            Stage {
                trigger: Trigger::OnClick,
                delay_ms: 0,
                effects: vec![effect(EffectKind::FadeIn, 200, 1000)],
            },
            Stage {
                trigger: Trigger::AfterPrevious,
                delay_ms: 300,
                effects: vec![effect(EffectKind::FadeOut, 0, 500)],
            },
        ],
    }
}

// PowerPoint-shaped XML for the click group: the click group is an indefinite wrapper
// whose middle par has no nodeType, so it is not flattened into the nodeType path. The
// after-previous group is a top-level sibling with its own nodeType. Effect cTn blocks
// are copied from `compile_animated_slide(click_then_after())` output.
const FADE_IN: &str = r#"<p:par><p:cTn id="4" dur="1000" presetID="10" presetClass="entr" presetSubtype="0" fill="hold" grpId="0" nodeType="clickEffect"><p:stCondLst><p:cond delay="200"/></p:stCondLst><p:childTnLst><p:set><p:cBhvr><p:cTn id="6" dur="1" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst></p:cTn><p:tgtEl><p:spTgt spid="2"/></p:tgtEl><p:attrNameLst><p:attrName>style.visibility</p:attrName></p:attrNameLst></p:cBhvr><p:to><p:strVal val="visible"/></p:to></p:set><p:animEffect transition="in" filter="fade"><p:cBhvr><p:cTn id="5" dur="1000" fill="hold"/><p:tgtEl><p:spTgt spid="2"/></p:tgtEl></p:cBhvr></p:animEffect></p:childTnLst></p:cTn></p:par>"#;
const FADE_OUT: &str = r#"<p:par><p:cTn id="8" dur="500" presetID="10" presetClass="exit" presetSubtype="0" fill="hold" grpId="1" nodeType="afterEffect"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst><p:animEffect transition="out" filter="fade"><p:cBhvr><p:cTn id="9" dur="500" fill="hold"/><p:tgtEl><p:spTgt spid="2"/></p:tgtEl></p:cBhvr></p:animEffect><p:set><p:cBhvr><p:cTn id="10" dur="1" fill="hold"><p:stCondLst><p:cond delay="499"/></p:stCondLst></p:cTn><p:tgtEl><p:spTgt spid="2"/></p:tgtEl><p:attrNameLst><p:attrName>style.visibility</p:attrName></p:attrNameLst></p:cBhvr><p:to><p:strVal val="hidden"/></p:to></p:set></p:childTnLst></p:cTn></p:par>"#;

fn reproducer_xml() -> String {
    // The after-previous group's cond delay is absolute from the click start: 1200 + 300.
    concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">"#,
        r#"<p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/><p:sp><p:nvSpPr><p:cNvPr id="2" name="Box"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr/></p:sp></p:spTree></p:cSld>"#,
        r#"<p:timing><p:tnLst><p:par><p:cTn id="1" dur="indefinite" restart="never" nodeType="tmRoot"><p:childTnLst>"#,
        r#"<p:seq concurrent="1" nextAc="seek"><p:cTn id="2" dur="indefinite" nodeType="mainSeq"><p:childTnLst>"#,
        r#"<p:par><p:cTn id="11" fill="hold"><p:stCondLst><p:cond delay="indefinite"/></p:stCondLst><p:childTnLst>"#,
        r#"<p:par><p:cTn id="3" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst>"#,
    )
    .to_string()
        + FADE_IN
        + r#"</p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn></p:par>"#
        + r#"<p:par><p:cTn id="7" fill="hold" nodeType="afterEffect"><p:stCondLst><p:cond evt="onBegin" delay="1500"><p:tn val="3"/></p:cond></p:stCondLst><p:childTnLst>"#
        + FADE_OUT
        + r#"</p:childTnLst></p:cTn></p:par>"#
        + r#"</p:childTnLst></p:cTn></p:seq></p:childTnLst></p:cTn></p:par></p:tnLst></p:timing></p:sld>"#
}

#[test]
fn after_group_after_indefinite_click_is_anchored_to_click_end() {
    let parsed = parse_slide_timing(reproducer_xml().as_bytes(), "reproducer").unwrap();
    assert!(parsed.opaque.is_empty(), "{:?}", parsed.opaque);
    assert_eq!(parsed.timeline, click_then_after());

    let snapshot = SlideSnapshot {
        part: "reproducer".into(),
        width: 1000.0,
        height: 1000.0,
        shapes: vec![ShapeSnapshot {
            id: 2,
            name: "Box".into(),
            text: String::new(),
            parent: None,
            z_order: 0,
            geometry: None,
            hidden: false,
            warnings: vec![],
        }],
        timing: parsed,
    };
    let session = QuerySession::from_snapshots(vec![snapshot]);
    // The fade-out begins at 1500 and lasts 500 ms, so it is half done at 1750.
    let state = session.state(1, &[0], 1750).unwrap();
    assert_eq!(state.shapes[0].opacity, Some(0.5));
}

#[test]
fn click_then_after_roundtrips_to_canonical_timeline() {
    let timeline = click_then_after();
    let xml = compile_animated_slide(&slide(), &timeline, &Transition::None).unwrap();
    let parsed = parse_slide_timing(&xml, "roundtrip").unwrap();
    assert!(parsed.opaque.is_empty(), "{:?}", parsed.opaque);
    assert_eq!(parsed.timeline, canonicalize(&timeline));
    assert_eq!(parsed.timeline.stages[1].delay_ms, 300);
}
