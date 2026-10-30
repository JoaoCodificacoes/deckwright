use deckwright::query::{Geometry, QuerySession, ShapeSnapshot, SlideSnapshot};
use deckwright::timing::*;
fn session(kind: EffectKind) -> QuerySession {
    QuerySession::from_snapshots(vec![snapshot(kind)])
}
fn snapshot(kind: EffectKind) -> SlideSnapshot {
    SlideSnapshot {
        part: "neutral".into(),
        width: 1000.0,
        height: 500.0,
        shapes: vec![ShapeSnapshot {
            id: 2,
            name: "!!one".into(),
            text: "hello".into(),
            parent: None,
            z_order: 0,
            geometry: Some(Geometry {
                x: 100.0,
                y: 20.0,
                width: 50.0,
                height: 30.0,
            }),
            hidden: false,
            warnings: vec![],
        }],
        timing: ParsedTiming {
            timeline: Timeline {
                stages: vec![Stage {
                    trigger: Trigger::OnClick,
                    delay_ms: 0,
                    effects: vec![Effect {
                        target: 2,
                        delay_ms: 0,
                        duration_ms: 1000,
                        kind,
                    }],
                }],
            },
            ..Default::default()
        },
    }
}
#[test]
fn fade_boundaries_and_overlap_history() {
    let s = session(EffectKind::FadeIn);
    assert_eq!(s.state(1, &[], 0).unwrap().shapes[0].visible, Some(false));
    assert_eq!(
        s.state(1, &[100], 600).unwrap().shapes[0].opacity,
        Some(0.5)
    );
    assert_eq!(
        s.state(1, &[100, 200], 1100).unwrap().shapes[0].opacity,
        Some(1.0)
    );
    assert!(s.state(0, &[], 0).is_err());
    assert!(s.state(1, &[20, 10], 30).is_err());
}
#[test]
fn motion_endpoint_is_static_position() {
    let s = session(EffectKind::Motion {
        from_x: -0.5,
        from_y: 0.0,
        to_x: 0.0,
        to_y: 0.0,
        accelerate: 0.0,
        decelerate: 0.0,
    });
    assert_eq!(
        s.state(1, &[0], 0).unwrap().shapes[0]
            .geometry
            .as_ref()
            .unwrap()
            .x,
        -400.0
    );
    assert_eq!(
        s.state(1, &[0], 500).unwrap().shapes[0]
            .geometry
            .as_ref()
            .unwrap()
            .x,
        -150.0
    );
    assert_eq!(
        s.state(1, &[0], 1000).unwrap().shapes[0]
            .geometry
            .as_ref()
            .unwrap()
            .x,
        100.0
    );
}
#[test]
fn overlapping_clicks_retain_earlier_active_effect_and_delay_boundaries() {
    let mut slide = snapshot(EffectKind::FadeIn);
    let mut second = slide.shapes[0].clone();
    second.id = 3;
    slide.shapes.push(second);
    let mut stage = slide.timing.timeline.stages[0].clone();
    stage.effects[0].target = 3;
    stage.effects[0].delay_ms = 100;
    slide.timing.timeline.stages.push(stage);
    let s = QuerySession::from_snapshots(vec![slide]);
    let at = s.state(1, &[0, 200], 300).unwrap();
    assert_eq!(at.shapes[0].opacity, Some(0.3));
    assert_eq!(at.shapes[1].opacity, Some(0.0));
    assert_eq!(
        s.state(1, &[0, 200], 299).unwrap().shapes[1].visible,
        Some(false)
    );
    assert!(s.state(1, &[301], 300).is_err());
}
#[test]
fn exit_wipe_easing_and_opaque_parent_states() {
    let s = session(EffectKind::FadeOut);
    assert_eq!(s.state(1, &[], 0).unwrap().shapes[0].visible, Some(true));
    assert_eq!(
        s.state(1, &[0], 1000).unwrap().shapes[0].visible,
        Some(false)
    );
    let s = session(EffectKind::WipeIn {
        direction: Direction::Left,
    });
    assert_eq!(
        s.state(1, &[0], 500).unwrap().shapes[0].wipe_fraction,
        Some(0.5)
    );
    let s = session(EffectKind::Motion {
        from_x: -0.5,
        from_y: 0.0,
        to_x: 0.0,
        to_y: 0.0,
        accelerate: 0.4,
        decelerate: 0.4,
    });
    let x = s.state(1, &[0], 200).unwrap().shapes[0]
        .geometry
        .as_ref()
        .unwrap()
        .x;
    assert!((x - (-358.3333333333333)).abs() < 1e-8);
    let mut slide = snapshot(EffectKind::FadeOut);
    let mut child = slide.shapes[0].clone();
    child.id = 3;
    child.parent = Some(2);
    slide.shapes.push(child);
    let s = QuerySession::from_snapshots(vec![slide.clone()]);
    assert_eq!(s.state(1, &[0], 500).unwrap().shapes[1].opacity, Some(0.5));
    slide.timing.opaque.push(OpaqueTiming {
        targets: vec![2],
        reason: "unsupported loop".into(),
    });
    let s = QuerySession::from_snapshots(vec![slide]);
    let r = s.state(1, &[0], 10000).unwrap();
    assert_eq!(r.shapes[0].visible, None);
    assert_eq!(r.shapes[1].visible, None);
    assert_eq!(r.shapes[1].geometry, None);
}
#[test]
fn morph_unique_names_and_duplicate_names_are_separate_from_ids() {
    let a = snapshot(EffectKind::Appear);
    let mut b = a.clone();
    b.shapes[0].id = 90;
    b.shapes[0].geometry.as_mut().unwrap().x += 70.0;
    b.timing.transition = Transition::Morph { duration_ms: 900 };
    let s = QuerySession::from_snapshots(vec![a.clone(), b.clone()]);
    let r = s.morph(1, 2).unwrap();
    assert_eq!(r.explicit_matches[0].to_id, 90);
    assert_eq!(r.explicit_matches[0].delta.as_ref().unwrap().x, 70.0);
    let mut duplicate = b.shapes[0].clone();
    duplicate.id = 91;
    b.shapes.push(duplicate);
    let s = QuerySession::from_snapshots(vec![a, b]);
    let r = s.morph(1, 2).unwrap();
    assert!(r.explicit_matches.is_empty());
    assert!(r.warnings.iter().any(|w| w.contains("Duplicate")));
    assert!(s.morph(1, 3).is_err());
}
#[test]
fn reports_are_serializable_for_schema_validation() {
    let s = session(EffectKind::FadeIn);
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/query-schema-samples");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, value) in [
        (
            "state",
            serde_json::to_value(s.state(1, &[0], 500).unwrap()).unwrap(),
        ),
        (
            "timeline",
            serde_json::to_value(s.timeline(1).unwrap()).unwrap(),
        ),
        (
            "morph",
            serde_json::to_value(s.morph(1, 1).unwrap()).unwrap(),
        ),
    ] {
        std::fs::write(
            dir.join(format!("{name}.json")),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
        assert_eq!(value["revision"], 1);
    }
}
#[test]
fn package_snapshot_resolves_namespaced_group_scaling_and_rejects_rotation() {
    use std::io::{Cursor, Write};
    let presentation = r#"<q:presentation xmlns:q="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><q:sldIdLst><q:sldId r:id="r1"/></q:sldIdLst><q:sldSz cx="1000" cy="500"/></q:presentation>"#;
    let slide = r#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><p:cSld><p:spTree><p:grpSp><p:nvGrpSpPr><p:cNvPr id="2" name="group" hidden="1"/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="100" y="200"/><a:ext cx="200" cy="300"/><a:chOff x="10" y="20"/><a:chExt cx="100" cy="100"/></a:xfrm></p:grpSpPr><p:sp><p:nvSpPr><p:cNvPr id="3" name="!!child"/></p:nvSpPr><p:spPr><a:xfrm><a:off x="20" y="30"/><a:ext cx="40" cy="10"/></a:xfrm></p:spPr><p:txBody><a:p><a:r><a:t>hello</a:t></a:r></a:p></p:txBody></p:sp></p:grpSp><p:sp><p:nvSpPr><p:cNvPr id="4" name="rotated"/></p:nvSpPr><p:spPr><a:xfrm rot="5400000"><a:off x="20" y="30"/><a:ext cx="40" cy="10"/></a:xfrm></p:spPr></p:sp><p:sp><p:nvSpPr><p:cNvPr id="5" name="placeholder"/></p:nvSpPr></p:sp><e:sp xmlns:e="urn:evil"><e:cNvPr id="99"/></e:sp></p:spTree></p:cSld></p:sld>"#;
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content) in [
        (
            "[Content_Types].xml",
            r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Default Extension="rels" ContentType="application/xml"/></Types>"#,
        ),
        ("ppt/presentation.xml", presentation),
        (
            "ppt/_rels/presentation.xml.rels",
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="r1" Type="slide" Target="slides/s.xml"/></Relationships>"#,
        ),
        ("ppt/slides/s.xml", slide),
    ] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(content.as_bytes()).unwrap();
    }
    let package = deckwright::Package::from_bytes(
        zip.finish().unwrap().into_inner(),
        deckwright::Limits::default(),
    )
    .unwrap();
    let s = QuerySession::from_package(&package, deckwright::Limits::default()).unwrap();
    let r = s.state(1, &[], 0).unwrap();
    assert_eq!(r.shapes.len(), 4);
    assert_eq!(
        r.shapes[1].geometry,
        Some(Geometry {
            x: 120.0,
            y: 230.0,
            width: 80.0,
            height: 30.0
        })
    );
    assert_eq!(r.shapes[1].visible, Some(false));
    assert_eq!(r.shapes[1].text, "hello");
    assert_eq!(r.shapes[1].name, "!!child");
    assert_eq!(r.shapes[2].geometry, None);
    assert_eq!(r.shapes[3].geometry, None);
}
#[test]
fn causal_after_previous_negative_overlap_and_with_previous() {
    let mut slide = snapshot(EffectKind::FadeIn);
    let mut next = slide.shapes[0].clone();
    next.id = 3;
    slide.shapes.push(next);
    let mut stage = slide.timing.timeline.stages[0].clone();
    stage.trigger = Trigger::AfterPrevious;
    stage.delay_ms = -500;
    stage.effects[0].target = 3;
    slide.timing.timeline.stages.push(stage);
    let s = QuerySession::from_snapshots(vec![slide]);
    assert_eq!(
        s.state(1, &[], 1000).unwrap().shapes[1].visible,
        Some(false)
    );
    let r = s.state(1, &[0], 750).unwrap();
    assert_eq!(r.shapes[0].opacity, Some(0.75));
    assert_eq!(r.shapes[1].opacity, Some(0.25));
}
