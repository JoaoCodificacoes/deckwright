//! Auto-start main-sequence groups: a group whose cTn start conditions are exactly
//! `[delay="indefinite"]` then `[evt="onBegin" delay="0"]` on the mainSeq node starts with
//! the slide. Its first stage is `OnSlideStart`. docs/decisions/timing.md, Wave 10.
use deckwright::query::{QuerySession, ShapeSnapshot, SlideSnapshot};
use deckwright::timing::*;
use deckwright::{model::*, units::Emu};

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

fn fade_in_then_fade_out(first_delay: i64) -> Timeline {
    Timeline {
        stages: vec![
            Stage {
                trigger: Trigger::OnSlideStart,
                delay_ms: first_delay,
                effects: vec![effect(EffectKind::FadeIn, 0, 600)],
            },
            Stage {
                trigger: Trigger::AfterPrevious,
                delay_ms: 0,
                effects: vec![effect(EffectKind::FadeOut, 0, 500)],
            },
        ],
    }
}

// Hand-written PowerPoint-shaped XML. The auto-start group is stage ids 3 (group), 4 (first
// stage, afterEffect with delay 250), and 8 (second stage, afterEffect after stage 4 ends at
// 600). Effect bodies are the same shape as the compiler emits for FadeIn and FadeOut.
const HEAD: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">"#,
    r#"<p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/><p:sp><p:nvSpPr><p:cNvPr id="2" name="Box"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr/></p:sp></p:spTree></p:cSld>"#,
    r#"<p:timing><p:tnLst><p:par><p:cTn id="1" dur="indefinite" restart="never" nodeType="tmRoot"><p:childTnLst>"#,
    r#"<p:seq concurrent="1" nextAc="seek"><p:cTn id="2" dur="indefinite" nodeType="mainSeq"><p:childTnLst>"#,
);
const STAGES: &str = concat!(
    r#"<p:par><p:cTn id="4" fill="hold" nodeType="afterEffect"><p:stCondLst><p:cond delay="250"/></p:stCondLst><p:childTnLst>"#,
    r#"<p:par><p:cTn id="5" dur="600" presetID="10" presetClass="entr" presetSubtype="0" fill="hold" grpId="0" nodeType="afterEffect"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst>"#,
    r#"<p:set><p:cBhvr><p:cTn id="6" dur="1" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst></p:cTn><p:tgtEl><p:spTgt spid="2"/></p:tgtEl><p:attrNameLst><p:attrName>style.visibility</p:attrName></p:attrNameLst></p:cBhvr><p:to><p:strVal val="visible"/></p:to></p:set>"#,
    r#"<p:animEffect transition="in" filter="fade"><p:cBhvr><p:cTn id="7" dur="600" fill="hold"/><p:tgtEl><p:spTgt spid="2"/></p:tgtEl></p:cBhvr></p:animEffect>"#,
    r#"</p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn></p:par>"#,
    r#"<p:par><p:cTn id="8" fill="hold" nodeType="afterEffect"><p:stCondLst><p:cond evt="onBegin" delay="600"><p:tn val="4"/></p:cond></p:stCondLst><p:childTnLst>"#,
    r#"<p:par><p:cTn id="9" dur="500" presetID="10" presetClass="exit" presetSubtype="0" fill="hold" grpId="1" nodeType="afterEffect"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst>"#,
    r#"<p:animEffect transition="out" filter="fade"><p:cBhvr><p:cTn id="10" dur="500" fill="hold"/><p:tgtEl><p:spTgt spid="2"/></p:tgtEl></p:cBhvr></p:animEffect>"#,
    r#"<p:set><p:cBhvr><p:cTn id="11" dur="1" fill="hold"><p:stCondLst><p:cond delay="499"/></p:stCondLst></p:cTn><p:tgtEl><p:spTgt spid="2"/></p:tgtEl><p:attrNameLst><p:attrName>style.visibility</p:attrName></p:attrNameLst></p:cBhvr><p:to><p:strVal val="hidden"/></p:to></p:set>"#,
    r#"</p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn></p:par>"#,
);
const TAIL: &str = concat!(
    r#"</p:childTnLst></p:cTn>"#,
    r#"<p:prevCondLst><p:cond evt="onPrev" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:prevCondLst>"#,
    r#"<p:nextCondLst><p:cond evt="onNext" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:nextCondLst></p:seq>"#,
    r#"</p:childTnLst></p:cTn></p:par></p:tnLst>"#,
    r#"<p:bldLst><p:bldP spid="2" grpId="0" animBg="1"/><p:bldP spid="2" grpId="1" animBg="1"/></p:bldLst></p:timing></p:sld>"#,
);

/// A slide whose first main-sequence group has the start conditions `conds`.
fn sld(conds: &str) -> String {
    format!(
        "{HEAD}<p:par><p:cTn id=\"3\" fill=\"hold\"><p:stCondLst>{conds}</p:stCondLst><p:childTnLst>{STAGES}</p:childTnLst></p:cTn></p:par>{TAIL}"
    )
}

/// Auto-start group whose effects are direct children, so it is not flattened into stages.
fn direct_effects_group() -> String {
    format!(
        "{HEAD}<p:par><p:cTn id=\"3\" fill=\"hold\"><p:stCondLst>{AUTO_START}</p:stCondLst><p:childTnLst>{}</p:childTnLst></p:cTn></p:par>{TAIL}",
        r#"<p:par><p:cTn id="5" dur="600" presetID="10" presetClass="entr" presetSubtype="0" fill="hold" grpId="0" nodeType="clickEffect"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst><p:set><p:cBhvr><p:cTn id="6" dur="1" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst></p:cTn><p:tgtEl><p:spTgt spid="2"/></p:tgtEl><p:attrNameLst><p:attrName>style.visibility</p:attrName></p:attrNameLst></p:cBhvr><p:to><p:strVal val="visible"/></p:to></p:set><p:animEffect transition="in" filter="fade"><p:cBhvr><p:cTn id="7" dur="600" fill="hold"/><p:tgtEl><p:spTgt spid="2"/></p:tgtEl></p:cBhvr></p:animEffect></p:childTnLst></p:cTn></p:par>"#
    )
}

const AUTO_START: &str =
    r#"<p:cond delay="indefinite"/><p:cond evt="onBegin" delay="0"><p:tn val="2"/></p:cond>"#;

#[test]
fn auto_start_group_parses_first_stage_as_on_slide_start() {
    let parsed = parse_slide_timing(sld(AUTO_START).as_bytes(), "autostart").unwrap();
    assert!(parsed.opaque.is_empty(), "{:?}", parsed.opaque);
    assert_eq!(parsed.transition, Transition::None);
    assert_eq!(
        parsed.timeline,
        Timeline {
            stages: vec![
                // The first stage keeps its own delay, measured from the slide start.
                Stage {
                    trigger: Trigger::OnSlideStart,
                    delay_ms: 250,
                    effects: vec![effect(EffectKind::FadeIn, 0, 600)],
                },
                // Ends at 600 relative to stage one, so the cond delay 600 gives 0 here.
                Stage {
                    trigger: Trigger::AfterPrevious,
                    delay_ms: 0,
                    effects: vec![effect(EffectKind::FadeOut, 0, 500)],
                },
            ],
        }
    );
}

#[test]
fn auto_start_group_with_direct_effects_is_on_slide_start() {
    let parsed = parse_slide_timing(direct_effects_group().as_bytes(), "direct").unwrap();
    assert!(parsed.opaque.is_empty(), "{:?}", parsed.opaque);
    assert_eq!(
        parsed.timeline,
        Timeline {
            stages: vec![Stage {
                trigger: Trigger::OnSlideStart,
                delay_ms: 0,
                effects: vec![effect(EffectKind::FadeIn, 0, 600)],
            }],
        }
    );
}

#[test]
fn other_start_condition_forms_stay_opaque() {
    let forms = [
        // Target is not the mainSeq node.
        r#"<p:cond delay="indefinite"/><p:cond evt="onBegin" delay="0"><p:tn val="99"/></p:cond>"#,
        // Different event.
        r#"<p:cond delay="indefinite"/><p:cond evt="onEnd" delay="0"><p:tn val="2"/></p:cond>"#,
        // Non-zero delay.
        r#"<p:cond delay="indefinite"/><p:cond evt="onBegin" delay="500"><p:tn val="2"/></p:cond>"#,
        // Extra condition.
        r#"<p:cond delay="indefinite"/><p:cond evt="onBegin" delay="0"><p:tn val="2"/></p:cond><p:cond delay="0"/>"#,
        // Reversed order.
        r#"<p:cond evt="onBegin" delay="0"><p:tn val="2"/></p:cond><p:cond delay="indefinite"/>"#,
    ];
    for conds in forms {
        let parsed = parse_slide_timing(sld(conds).as_bytes(), "autostart").unwrap();
        assert!(
            !parsed.opaque.is_empty(),
            "expected opaque for {conds}, got {:?}",
            parsed.timeline
        );
    }
}

#[test]
fn on_slide_start_then_after_previous_round_trips() {
    let slide = slide();
    for first_delay in [0, 250] {
        let timeline = fade_in_then_fade_out(first_delay);
        let xml = compile_animated_slide(&slide, &timeline, &Transition::None).unwrap();
        let text = String::from_utf8(xml.clone()).unwrap();
        assert!(
            text.contains(
                r#"<p:stCondLst><p:cond delay="indefinite"/><p:cond evt="onBegin" delay="0"><p:tn val="2"/></p:cond></p:stCondLst>"#
            ),
            "auto-start group not emitted"
        );
        let parsed = parse_slide_timing(&xml, "roundtrip").unwrap();
        assert!(parsed.opaque.is_empty(), "{:?}", parsed.opaque);
        assert_eq!(parsed.timeline, timeline);
        assert_eq!(
            compile_animated_slide(&slide, &parsed.timeline, &parsed.transition).unwrap(),
            xml
        );
    }
}

/// FNV-1a 64 of the bytes compiled at the base commit 5c3abf1 for the timeline below.
const ONCLICK_FIRST_BASE_HASH: u64 = 0x22e5_f8fb_5a2a_279f;
const ONCLICK_FIRST_BASE_LEN: usize = 6624;

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}

#[test]
fn on_click_first_timeline_compiles_as_at_base() {
    // Includes a mid-timeline OnSlideStart, which keeps its base emission.
    let timeline = Timeline {
        stages: vec![
            Stage {
                trigger: Trigger::OnClick,
                delay_ms: 0,
                effects: vec![effect(EffectKind::FadeIn, 200, 1000)],
            },
            Stage {
                trigger: Trigger::WithPrevious,
                delay_ms: 0,
                effects: vec![effect(EffectKind::Appear, 0, 1)],
            },
            Stage {
                trigger: Trigger::AfterPrevious,
                delay_ms: 300,
                effects: vec![effect(EffectKind::FadeOut, 0, 500)],
            },
            Stage {
                trigger: Trigger::OnSlideStart,
                delay_ms: 50,
                effects: vec![effect(EffectKind::FadeIn, 0, 600)],
            },
            Stage {
                trigger: Trigger::AfterPrevious,
                delay_ms: 0,
                effects: vec![effect(
                    EffectKind::WipeIn {
                        direction: Direction::Up,
                    },
                    0,
                    400,
                )],
            },
            Stage {
                trigger: Trigger::OnClick,
                delay_ms: 100,
                effects: vec![effect(EffectKind::FadeOut, 0, 700)],
            },
        ],
    };
    let xml = compile_animated_slide(&slide(), &timeline, &Transition::Fade { duration_ms: 700 })
        .unwrap();
    assert_eq!(xml.len(), ONCLICK_FIRST_BASE_LEN);
    assert_eq!(fnv1a(&xml), ONCLICK_FIRST_BASE_HASH);
}

#[test]
fn query_runs_auto_start_effect_without_clicks() {
    let xml =
        compile_animated_slide(&slide(), &fade_in_then_fade_out(0), &Transition::None).unwrap();
    let timing = parse_slide_timing(&xml, "query").unwrap();
    assert!(timing.opaque.is_empty(), "{:?}", timing.opaque);
    let session = QuerySession::from_snapshots(vec![SlideSnapshot {
        part: "autostart".into(),
        width: 1000.0,
        height: 500.0,
        shapes: vec![ShapeSnapshot {
            id: 2,
            name: "!!box".into(),
            text: String::new(),
            parent: None,
            z_order: 0,
            geometry: Some(deckwright::query::Geometry {
                x: 100.0,
                y: 100.0,
                width: 100.0,
                height: 100.0,
            }),
            hidden: false,
            warnings: vec![],
        }],
        timing,
    }]);
    // FadeIn is 600 ms from the slide start with no click history. It has begun at 0.
    let at_start = session.state(1, &[], 0).unwrap();
    assert_eq!(at_start.shapes[0].opacity, Some(0.0));
    let midway = session.state(1, &[], 300).unwrap();
    assert_eq!(midway.shapes[0].opacity, Some(0.5));
    let done = session.state(1, &[], 600).unwrap();
    assert_eq!(done.shapes[0].opacity, Some(1.0));
}
