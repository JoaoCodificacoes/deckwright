//! animMotion parsing and validation for general motion paths (timing.md, Wave 10).
use deckwright::{model::*, timing::*, units::Emu};

fn slide() -> Slide {
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

fn motion(from: (f64, f64), to: (f64, f64)) -> EffectKind {
    EffectKind::Motion {
        from_x: from.0,
        from_y: from.1,
        to_x: to.0,
        to_y: to.1,
        accelerate: 0.4,
        decelerate: 0.4,
    }
}

fn timeline(kind: EffectKind) -> Timeline {
    Timeline {
        stages: vec![Stage {
            trigger: Trigger::OnClick,
            delay_ms: 0,
            effects: vec![Effect {
                target: 2,
                delay_ms: 0,
                duration_ms: 600,
                kind,
            }],
        }],
    }
}

/// The compiled XML for a single motion, with the animMotion element edited by `edit`.
fn edited_motion_xml(kind: EffectKind, edit: impl Fn(&str) -> String) -> String {
    let xml = compile_animated_slide(&slide(), &timeline(kind), &Transition::None).unwrap();
    let xml = String::from_utf8_lossy(&xml).into_owned();
    let start = xml.find("<p:animMotion").expect("compiled motion");
    let end = start
        + xml[start..].find("</p:animMotion>").expect("closing tag")
        + "</p:animMotion>".len();
    let mut out = xml.clone();
    out.replace_range(start..end, &edit(&xml[start..end]));
    out
}

#[test]
fn empty_pts_types_and_zero_rctr_are_accepted_and_dropped() {
    let kind = motion((0.0, -0.1), (0.0, 0.0));
    let xml = edited_motion_xml(kind.clone(), |m| {
        m.replacen("rAng=\"0\">", "rAng=\"0\" ptsTypes=\"\">", 1)
            .replacen(
                "</p:animMotion>",
                "<p:rCtr x=\"0\" y=\"0\"/></p:animMotion>",
                1,
            )
    });
    assert!(xml.contains("ptsTypes=\"\""), "fixture edit failed");
    assert!(
        xml.contains("<p:rCtr x=\"0\" y=\"0\"/>"),
        "fixture edit failed"
    );
    let parsed = parse_slide_timing(xml.as_bytes(), "pts").unwrap();
    assert!(parsed.opaque.is_empty(), "{:?}", parsed.opaque);
    assert_eq!(parsed.timeline, timeline(kind));
}

#[test]
fn nonzero_rctr_is_opaque() {
    let xml = edited_motion_xml(motion((0.0, -0.1), (0.0, 0.0)), |m| {
        m.replacen(
            "</p:animMotion>",
            "<p:rCtr x=\"5\" y=\"0\"/></p:animMotion>",
            1,
        )
    });
    let parsed = parse_slide_timing(xml.as_bytes(), "rctr").unwrap();
    assert!(
        parsed.opaque.iter().any(|o| o.reason.contains("rCtr")),
        "{:?}",
        parsed.opaque
    );
}

#[test]
fn other_pts_types_are_opaque() {
    let xml = edited_motion_xml(motion((0.0, -0.1), (0.0, 0.0)), |m| {
        m.replacen("rAng=\"0\">", "rAng=\"0\" ptsTypes=\"AA\">", 1)
    });
    let parsed = parse_slide_timing(xml.as_bytes(), "pts-other").unwrap();
    assert!(
        parsed.opaque.iter().any(|o| o.targets == vec![2]),
        "{:?}",
        parsed.opaque
    );
}

#[test]
fn nonzero_endpoint_validates_and_round_trips() {
    for kind in [
        motion((0.0, 0.0), (0.25, 0.0)),
        motion((-0.5, 0.1), (0.3, -0.2)),
        motion((0.0, 0.0), (100.0, -100.0)),
    ] {
        let t = timeline(kind);
        validate_timeline(&t).unwrap();
        let xml = compile_animated_slide(&slide(), &t, &Transition::None).unwrap();
        let parsed = parse_slide_timing(&xml, "roundtrip").unwrap();
        assert!(parsed.opaque.is_empty(), "{:?}", parsed.opaque);
        assert_eq!(parsed.timeline, t);
        assert_eq!(
            compile_animated_slide(&slide(), &parsed.timeline, &parsed.transition).unwrap(),
            xml
        );
    }
}

#[test]
fn out_of_range_coordinates_are_still_rejected() {
    let bad = [
        motion((101.0, 0.0), (0.0, 0.0)),
        motion((0.0, 0.0), (0.0, -100.5)),
        motion((0.0, f64::NAN), (0.0, 0.0)),
        motion((0.0, 0.0), (f64::INFINITY, 0.0)),
    ];
    for kind in bad {
        let t = timeline(kind);
        assert!(validate_timeline(&t).is_err());
        assert!(compile_animated_slide(&slide(), &t, &Transition::None).is_err());
    }
}
