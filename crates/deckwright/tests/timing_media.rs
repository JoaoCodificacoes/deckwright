use deckwright::{model::*, timing::*, units::Emu};

fn shape(id: u32) -> Object {
    Object {
        key: format!("s{id}"),
        id,
        name: format!("Shape {id}"),
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
    }
}

fn slide() -> Slide {
    Slide {
        objects: vec![shape(2), shape(3)],
    }
}

fn media(target: u32, delay_ms: i64, duration_ms: u32, from_ms: u32, volume: u32) -> Effect {
    Effect {
        target,
        delay_ms,
        duration_ms,
        kind: EffectKind::MediaPlay { from_ms, volume },
    }
}

fn click(effects: Vec<Effect>) -> Stage {
    Stage {
        trigger: Trigger::OnClick,
        delay_ms: 0,
        effects,
    }
}

fn compile(t: &Timeline) -> String {
    String::from_utf8(compile_animated_slide(&slide(), t, &Transition::None).unwrap()).unwrap()
}

fn one_media() -> Timeline {
    Timeline {
        stages: vec![click(vec![media(2, 0, 3000, 2500, 80000)])],
    }
}

#[test]
fn media_compile_parse_round_trip_with_exact_xml() {
    let t = one_media();
    let xml = compile(&t);
    let cmd = "<p:cmd type=\"call\" cmd=\"playFrom(2.5)\"><p:cBhvr><p:cTn id=\"5\" dur=\"3000\" fill=\"hold\"/><p:tgtEl><p:spTgt spid=\"2\"/></p:tgtEl></p:cBhvr></p:cmd>";
    assert!(xml.contains(cmd), "missing {cmd} in {xml}");
    let effect = "<p:par><p:cTn id=\"4\" dur=\"3000\" presetID=\"1\" presetClass=\"mediacall\" presetSubtype=\"0\" fill=\"hold\" grpId=\"0\" nodeType=\"clickEffect\"><p:stCondLst><p:cond delay=\"0\"/></p:stCondLst><p:childTnLst>";
    assert!(xml.contains(&format!("{effect}{cmd}</p:childTnLst></p:cTn></p:par>")));
    // The media node follows the main sequence inside the root time node.
    let video = "<p:video><p:cMediaNode vol=\"80000\"><p:cTn id=\"7\" fill=\"hold\" display=\"0\"><p:stCondLst><p:cond delay=\"indefinite\"/></p:stCondLst></p:cTn><p:tgtEl><p:spTgt spid=\"2\"/></p:tgtEl></p:cMediaNode></p:video>";
    assert!(
        xml.contains(&format!("</p:seq>{video}</p:childTnLst>")),
        "{xml}"
    );

    let parsed = parse_slide_timing(xml.as_bytes(), "round-trip").unwrap();
    assert!(parsed.opaque.is_empty(), "{:?}", parsed.opaque);
    assert_eq!(parsed.timeline, t);
    assert_eq!(
        compile_animated_slide(&slide(), &parsed.timeline, &parsed.transition).unwrap(),
        xml.as_bytes()
    );
}

#[test]
fn one_media_node_per_target_in_target_order() {
    let t = Timeline {
        stages: vec![
            click(vec![
                media(3, 0, 1000, 0, 50000),
                media(2, 0, 1000, 0, 80000),
            ]),
            click(vec![media(2, 0, 500, 1000, 80000)]),
        ],
    };
    let xml = compile(&t);
    assert_eq!(xml.matches("<p:video>").count(), 2, "{xml}");
    let first = xml.find("spid=\"2\"/></p:tgtEl></p:cMediaNode>").unwrap();
    let second = xml.find("vol=\"50000\"").unwrap();
    assert!(first < second);
    let parsed = parse_slide_timing(xml.as_bytes(), "two").unwrap();
    assert!(parsed.opaque.is_empty(), "{:?}", parsed.opaque);
    assert_eq!(parsed.timeline, t);
}

#[test]
fn media_validation_rejects_bad_start_volume_and_mixed_volumes() {
    let bad_start = Timeline {
        stages: vec![click(vec![media(2, 0, 1000, 150, 80000)])],
    };
    assert!(validate_timeline(&bad_start).is_err());
    assert!(compile_animated_slide(&slide(), &bad_start, &Transition::None).is_err());

    let loud = Timeline {
        stages: vec![click(vec![media(2, 0, 1000, 0, 100_001)])],
    };
    assert!(validate_timeline(&loud).is_err());

    let mixed = Timeline {
        stages: vec![
            click(vec![media(2, 0, 1000, 0, 80000)]),
            click(vec![media(2, 0, 1000, 1000, 50000)]),
        ],
    };
    assert!(validate_timeline(&mixed).is_err());

    let limit = Timeline {
        stages: vec![click(vec![media(2, 0, 1000, 0, 100_000)])],
    };
    assert!(validate_timeline(&limit).is_ok());
}

/// Removes the one `p:video` element from compiled XML.
fn without_video(xml: &str) -> String {
    let start = xml.find("<p:video>").unwrap();
    let end = xml.find("</p:video>").unwrap() + "</p:video>".len();
    format!("{}{}", &xml[..start], &xml[end..])
}

#[test]
fn media_call_without_its_video_node_is_opaque() {
    let xml = compile(&one_media());
    let parsed = parse_slide_timing(without_video(&xml).as_bytes(), "no-video").unwrap();
    assert!(parsed.timeline.stages.is_empty());
    assert!(
        parsed.opaque.iter().any(|o| o.targets == vec![2]),
        "{:?}",
        parsed.opaque
    );
}

#[test]
fn extra_audio_or_duplicate_media_node_is_opaque() {
    let xml = compile(&one_media());
    let video_start = xml.find("<p:video>").unwrap();
    let video_end = xml.find("</p:video>").unwrap() + "</p:video>".len();
    let video = &xml[video_start..video_end];

    // Copies reuse the node, so give them a fresh timing ID to keep the file otherwise valid.
    let fresh = video.replace("id=\"7\"", "id=\"90\"");
    let audio = fresh
        .replacen("<p:video>", "<p:audio>", 1)
        .replacen("</p:video>", "</p:audio>", 1);
    let with_audio = format!("{}{audio}{}", &xml[..video_end], &xml[video_end..]);
    let parsed = parse_slide_timing(with_audio.as_bytes(), "audio").unwrap();
    assert!(
        parsed.opaque.iter().any(|o| o.targets == vec![2]),
        "{:?}",
        parsed.opaque
    );

    let with_duplicate = format!("{}{fresh}{}", &xml[..video_end], &xml[video_end..]);
    let parsed = parse_slide_timing(with_duplicate.as_bytes(), "duplicate").unwrap();
    assert!(
        parsed.opaque.iter().any(|o| o.targets == vec![2]),
        "{:?}",
        parsed.opaque
    );
}

#[test]
fn media_effect_active_duration_is_its_duration() {
    assert_eq!(effect_active_ms(&media(2, 0, 4000, 0, 80000)), Some(4000));
}
