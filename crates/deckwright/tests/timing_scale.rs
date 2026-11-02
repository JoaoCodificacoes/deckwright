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
        ..Default::default()
    }
}

fn scale(x: f64, y: f64, repeat: Repeat, auto_reverse: bool) -> EffectKind {
    EffectKind::Scale {
        x,
        y,
        repeat,
        auto_reverse,
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

fn effect(duration_ms: u32, kind: EffectKind) -> Effect {
    Effect {
        target: 2,
        delay_ms: 0,
        duration_ms,
        kind,
    }
}

#[test]
fn scale_compile_parse_round_trip_with_exact_attributes() {
    let cases = [
        (
            Repeat::Count { thousandths: 1000 },
            false,
            "repeatCount=\"1000\"",
            false,
        ),
        (
            Repeat::Count { thousandths: 2500 },
            true,
            "repeatCount=\"2500\" autoRev=\"1\"",
            true,
        ),
        (
            Repeat::Indefinite,
            false,
            "repeatCount=\"indefinite\"",
            false,
        ),
        (
            Repeat::Indefinite,
            true,
            "repeatCount=\"indefinite\" autoRev=\"1\"",
            true,
        ),
    ];
    for (repeat, auto_reverse, attrs, expect_auto) in cases {
        let t = timeline(scale(1.5, 0.5, repeat.clone(), auto_reverse));
        let xml =
            String::from_utf8(compile_animated_slide(&slide(), &t, &Transition::None).unwrap())
                .unwrap();
        // The effect node: preset 6 emphasis, with the repeat attributes on its cTn only.
        let effect_node = format!(
            "presetID=\"6\" presetClass=\"emph\" presetSubtype=\"0\" fill=\"hold\" grpId=\"0\" nodeType=\"clickEffect\" {attrs}"
        );
        assert!(xml.contains(&effect_node), "missing {effect_node} in {xml}");
        assert!(xml.contains("<p:animScale><p:cBhvr><p:cTn id="));
        assert!(xml.contains("dur=\"600\" fill=\"hold\"/><p:tgtEl><p:spTgt spid=\"2\"/></p:tgtEl></p:cBhvr><p:by x=\"150000\" y=\"50000\"/></p:animScale>"));
        assert_eq!(xml.contains("autoRev=\"1\""), expect_auto);

        let parsed = parse_slide_timing(xml.as_bytes(), "roundtrip").unwrap();
        assert!(parsed.opaque.is_empty(), "{:?}", parsed.opaque);
        assert_eq!(parsed.timeline, t);
        assert_eq!(
            compile_animated_slide(&slide(), &parsed.timeline, &parsed.transition).unwrap(),
            xml.as_bytes()
        );
    }
}

#[test]
fn repeat_placement_outside_scale_is_opaque() {
    let fade = timeline(EffectKind::FadeIn);
    let xml =
        String::from_utf8(compile_animated_slide(&slide(), &fade, &Transition::None).unwrap())
            .unwrap();
    let moved = xml.replacen(
        "presetID=\"10\" presetClass=\"entr\"",
        "presetID=\"10\" presetClass=\"entr\" repeatCount=\"1000\"",
        1,
    );
    assert_ne!(moved, xml);
    let parsed = parse_slide_timing(moved.as_bytes(), "placement").unwrap();
    assert!(!parsed.opaque.is_empty());
}

#[test]
fn other_emph_presets_parse_as_opaque() {
    let t = timeline(scale(2.0, 2.0, Repeat::Count { thousandths: 1000 }, false));
    let xml = String::from_utf8(compile_animated_slide(&slide(), &t, &Transition::None).unwrap())
        .unwrap();
    let other = xml.replacen("presetID=\"6\"", "presetID=\"7\"", 1);
    let parsed = parse_slide_timing(other.as_bytes(), "emph").unwrap();
    assert!(!parsed.opaque.is_empty());
    assert!(parsed.timeline.stages.is_empty());
}

#[test]
fn scale_validation_bounds() {
    let ok = [
        (1.0, 1.0, Repeat::Count { thousandths: 1 }),
        (
            10.0,
            0.5,
            Repeat::Count {
                thousandths: 1_000_000,
            },
        ),
        (2.0, 2.0, Repeat::Indefinite),
    ];
    for (x, y, repeat) in ok {
        let t = timeline(scale(x, y, repeat, false));
        assert!(compile_animated_slide(&slide(), &t, &Transition::None).is_ok());
    }
    let bad = [
        (0.0, 1.0, Repeat::Indefinite),
        (1.0, 0.0, Repeat::Indefinite),
        (10.5, 1.0, Repeat::Indefinite),
        (f64::NAN, 1.0, Repeat::Indefinite),
        (1.0, f64::INFINITY, Repeat::Indefinite),
        (2.0, 2.0, Repeat::Count { thousandths: 0 }),
        (
            2.0,
            2.0,
            Repeat::Count {
                thousandths: 1_000_001,
            },
        ),
    ];
    for (x, y, repeat) in bad {
        let t = timeline(scale(x, y, repeat.clone(), false));
        assert!(
            validate_timeline(&t).is_err(),
            "accepted {x} {y} {repeat:?}"
        );
        assert!(compile_animated_slide(&slide(), &t, &Transition::None).is_err());
    }
}

#[test]
fn effect_active_ms_values() {
    let count = |d, thousandths, auto_reverse| {
        effect_active_ms(&effect(
            d,
            scale(2.0, 2.0, Repeat::Count { thousandths }, auto_reverse),
        ))
    };
    assert_eq!(count(600, 1000, false), Some(600));
    assert_eq!(count(1000, 2500, false), Some(2500));
    assert_eq!(count(1000, 2500, true), Some(5000));
    assert_eq!(count(1000, 1, false), Some(1));
    assert_eq!(
        effect_active_ms(&effect(1000, scale(2.0, 2.0, Repeat::Indefinite, true))),
        None
    );
    assert_eq!(
        effect_active_ms(&effect(700, EffectKind::FadeIn)),
        Some(700)
    );
}
