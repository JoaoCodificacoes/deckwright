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
        ..Default::default()
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
fn stage(trigger: Trigger, kind: EffectKind) -> Stage {
    Stage {
        trigger,
        delay_ms: 0,
        effects: vec![effect(kind)],
    }
}
#[test]
fn canonical_roundtrip_all_effects_triggers_and_transition() {
    let mut timeline = Timeline {
        stages: vec![
            stage(Trigger::OnSlideStart, EffectKind::Appear),
            stage(Trigger::OnClick, EffectKind::FadeIn),
            stage(
                Trigger::WithPrevious,
                EffectKind::WipeIn {
                    direction: Direction::Up,
                },
            ),
            stage(
                Trigger::AfterPrevious,
                EffectKind::Motion {
                    from_x: 0.0,
                    from_y: -0.1133,
                    to_x: 0.0,
                    to_y: 0.0,
                    accelerate: 0.4,
                    decelerate: 0.4,
                },
            ),
            stage(Trigger::OnClick, EffectKind::FadeOut),
        ],
    };
    timeline.stages[3].delay_ms = -120;
    timeline.stages[2].effects.push(Effect {
        delay_ms: 20,
        ..effect(EffectKind::FadeIn)
    });
    for transition in [
        Transition::None,
        Transition::Fade { duration_ms: 700 },
        Transition::Morph { duration_ms: 900 },
    ] {
        let xml = compile_animated_slide(&slide(), &timeline, &transition).unwrap();
        let parsed = parse_slide_timing(&xml, "roundtrip").unwrap();
        assert!(parsed.opaque.is_empty(), "{:?}", parsed.opaque);
        assert_eq!(parsed.timeline, timeline);
        assert_eq!(parsed.transition, transition);
        assert_eq!(
            compile_animated_slide(&slide(), &parsed.timeline, &parsed.transition).unwrap(),
            xml
        );
    }
}
#[test]
fn nonfinite_easing_and_nonzero_endpoint_rejected() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 101.0] {
        let t = Timeline {
            stages: vec![stage(
                Trigger::OnClick,
                EffectKind::Motion {
                    from_x: value,
                    from_y: 0.0,
                    to_x: 0.0,
                    to_y: 0.0,
                    accelerate: 0.4,
                    decelerate: 0.4,
                },
            )],
        };
        assert!(compile_animated_slide(&slide(), &t, &Transition::None).is_err());
    }
    // A non-zero endpoint is valid (layout-relative paths, timing.md wave 10);
    // out-of-range endpoints and invalid easing are not.
    for (to, accel, decel) in [(101.0, 0.4, 0.4), (0.0, 0.6, 0.6), (0.0, -0.1, 0.4)] {
        let t = Timeline {
            stages: vec![stage(
                Trigger::OnClick,
                EffectKind::Motion {
                    from_x: 0.0,
                    from_y: 0.0,
                    to_x: to,
                    to_y: 0.0,
                    accelerate: accel,
                    decelerate: decel,
                },
            )],
        };
        assert!(compile_animated_slide(&slide(), &t, &Transition::None).is_err());
    }
}
#[test]
fn invalid_targets_durations_and_noncausal_overlap_rejected() {
    let mut t = Timeline {
        stages: vec![stage(Trigger::OnClick, EffectKind::FadeIn)],
    };
    t.stages[0].effects[0].target = 99;
    assert!(compile_animated_slide(&slide(), &t, &Transition::None).is_err());
    t.stages[0].effects[0].target = 2;
    t.stages[0].effects[0].duration_ms = 0;
    assert!(compile_animated_slide(&slide(), &t, &Transition::None).is_err());
    t.stages[0].effects[0].duration_ms = 600;
    t.stages[0].delay_ms = -1;
    assert!(compile_animated_slide(&slide(), &t, &Transition::None).is_err());
    t.stages[0].delay_ms = 0;
    t.stages
        .push(stage(Trigger::AfterPrevious, EffectKind::FadeOut));
    t.stages[1].delay_ms = -601;
    assert!(compile_animated_slide(&slide(), &t, &Transition::None).is_err());
}
#[test]
fn duplicate_ids_missing_targets_and_unknown_effects_are_not_exact() {
    let t = Timeline {
        stages: vec![stage(Trigger::OnClick, EffectKind::FadeIn)],
    };
    let xml = String::from_utf8(compile_animated_slide(&slide(), &t, &Transition::None).unwrap())
        .unwrap();
    let duplicate = xml.replacen(
        "id=\"2\" dur=\"indefinite\"",
        "id=\"1\" dur=\"indefinite\"",
        1,
    );
    assert!(parse_slide_timing(duplicate.as_bytes(), "duplicate").is_err());
    assert!(
        parse_slide_timing(
            xml.replace("spid=\"2\"", "spid=\"99\"").as_bytes(),
            "missing"
        )
        .is_err()
    );
    let unknown = parse_slide_timing(
        xml.replace("filter=\"fade\"", "filter=\"zoom\"").as_bytes(),
        "unknown",
    )
    .unwrap();
    assert_eq!(unknown.opaque.len(), 1);
    assert_eq!(unknown.opaque[0].targets, vec![2]);
    assert!(unknown.timeline.stages.is_empty());
}
#[test]
fn exit_build_and_visibility_order_and_fallback_are_complete() {
    let t = Timeline {
        stages: vec![stage(Trigger::OnClick, EffectKind::FadeOut)],
    };
    let xml = String::from_utf8(
        compile_animated_slide(&slide(), &t, &Transition::Morph { duration_ms: 900 }).unwrap(),
    )
    .unwrap();
    assert!(xml.contains("spid=\"2\" grpId=\"1\""));
    assert!(xml.contains("val=\"hidden\""));
    assert!(xml.contains("delay=\"599\""));
    assert!(xml.find("AlternateContent").unwrap() < xml.find("<p:timing>").unwrap());
    assert!(xml.contains("mc:Fallback"));
    assert!(xml.contains("p14:dur=\"900\""));
}
#[test]
fn no_timing_is_empty() {
    assert_eq!(
        parse_slide_timing(
            &deckwright::builder::compile_slide(&slide()).unwrap(),
            "static"
        )
        .unwrap(),
        ParsedTiming::default()
    );
}
