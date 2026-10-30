use deckwright::{
    Limits, Package,
    check::check_package,
    model::{Color, Fill, Geometry, Line, Object, ObjectKind, Properties, Slide, Transform},
    query::QuerySession,
    template,
    timing::{Effect, EffectKind, Stage, Timeline, Transition, Trigger, compile_animated_slide},
    units::Emu,
};
use std::collections::{BTreeMap, BTreeSet};

const W: i64 = 12_192_000;
const H: i64 = 6_858_000;

fn blank(slides: usize) -> Package {
    template::blank(slides, Emu(W), Emu(H)).unwrap()
}

fn errors(package: &Package) -> usize {
    check_package(package)
        .findings
        .iter()
        .filter(|f| f.severity == "error")
        .count()
}

fn shape(key: &str, id: u32, name: &str, preset: &str, rect: [i64; 4], rgb: &str) -> Object {
    Object {
        key: key.into(),
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
                    x: Emu(rect[0]),
                    y: Emu(rect[1]),
                    width: Emu(rect[2]),
                    height: Emu(rect[3]),
                    ..Default::default()
                }),
                geometry: Some(Geometry {
                    preset: preset.into(),
                    adjustments: vec![],
                }),
                fill: Some(Fill::Solid {
                    color: Color::Rgb {
                        value: rgb.into(),
                        alpha: None,
                    },
                }),
                line: Some(Line {
                    width: None,
                    fill: Some(Fill::None),
                }),
                effects: None,
                custom: None,
            },
            style: None,
            text: None,
        },
    }
}

fn effect(target: u32, duration_ms: u32, kind: EffectKind) -> Effect {
    Effect {
        target,
        delay_ms: 0,
        duration_ms,
        kind,
    }
}

/// Same two-slide shape as the public example: a morphing `!!box` and a dot.
fn morph_deck(package: &Package) -> Package {
    let slides = [
        (
            Slide {
                objects: vec![shape(
                    "box",
                    2,
                    "!!box",
                    "roundRect",
                    [600_000, 2_400_000, 2_000_000, 2_000_000],
                    "2563EB",
                )],
            },
            Timeline::default(),
            Transition::None,
        ),
        (
            Slide {
                objects: vec![
                    shape(
                        "box",
                        2,
                        "!!box",
                        "roundRect",
                        [8_600_000, 2_000_000, 3_000_000, 3_000_000],
                        "F97316",
                    ),
                    shape(
                        "dot",
                        4,
                        "dot",
                        "roundRect",
                        [4_600_000, 2_900_000, 1_500_000, 1_500_000],
                        "10B981",
                    ),
                ],
            },
            Timeline {
                stages: vec![Stage {
                    trigger: Trigger::OnClick,
                    delay_ms: 0,
                    effects: vec![effect(4, 800, EffectKind::FadeIn)],
                }],
            },
            Transition::Morph { duration_ms: 2000 },
        ),
    ];
    let mut replacements = BTreeMap::new();
    for (i, (slide, timeline, transition)) in slides.iter().enumerate() {
        replacements.insert(
            format!("ppt/slides/slide{}.xml", i + 1),
            compile_animated_slide(slide, timeline, transition).unwrap(),
        );
    }
    package
        .edited(replacements, &BTreeSet::new(), Limits::default())
        .unwrap()
}

#[test]
fn blank_decks_open_with_requested_slide_count_and_no_errors() {
    for slides in [1, 3, 25] {
        let package = blank(slides);
        let reopened = Package::from_bytes(package.to_bytes().unwrap(), Limits::default()).unwrap();
        let session = QuerySession::from_package(&reopened, Limits::default()).unwrap();
        assert!(session.timeline(slides).is_ok(), "slide {slides} missing");
        assert!(
            session.timeline(slides + 1).is_err(),
            "extra slide for {slides}"
        );
        assert_eq!(errors(&reopened), 0, "check errors for {slides} slides");
    }
}

#[test]
fn blank_bytes_are_deterministic() {
    let a = blank(3).to_bytes().unwrap();
    let b = blank(3).to_bytes().unwrap();
    assert_eq!(a, b);
}

#[test]
fn bounds_are_rejected() {
    let h = Emu(H);
    assert!(template::blank(0, Emu(W), h).is_err());
    assert!(template::blank(1001, Emu(W), h).is_err());
    assert!(template::blank(1, Emu(914_399), h).is_err());
    assert!(template::blank(1, Emu(W), Emu(914_399)).is_err());
    assert!(template::blank(1, Emu(51_206_401), h).is_err());
    assert!(template::blank(1000, Emu(W), h).is_ok());
    assert!(template::blank(1, Emu(914_400), Emu(51_206_400)).is_ok());
}

#[test]
fn relationships_resolve_to_the_blank_layout_and_master() {
    let package = blank(3);
    let rels = package.relationships();
    for n in 1..=3 {
        let source = format!("ppt/slides/slide{n}.xml");
        assert!(
            rels.iter().any(|r| r.source == source
                && r.relationship_type.ends_with("/slideLayout")
                && r.resolved_target.as_deref() == Some("ppt/slideLayouts/slideLayout1.xml")),
            "{source} lacks a layout relationship"
        );
    }
    let resolved = |source: &str, kind: &str| {
        rels.iter().any(|r| {
            r.source == source && r.relationship_type.ends_with(kind) && r.resolved_target.is_some()
        })
    };
    assert!(resolved(
        "ppt/slideMasters/slideMaster1.xml",
        "/slideLayout"
    ));
    assert!(resolved("ppt/slideMasters/slideMaster1.xml", "/theme"));
    assert!(resolved(
        "ppt/slideLayouts/slideLayout1.xml",
        "/slideMaster"
    ));
    let layout =
        std::str::from_utf8(package.part("ppt/slideLayouts/slideLayout1.xml").unwrap()).unwrap();
    assert!(layout.contains(r#"type="blank""#));
}

#[test]
fn morph_deck_matches_box_with_expected_delta_and_transition() {
    let package = morph_deck(&blank(2));
    assert_eq!(errors(&package), 0);
    let session = QuerySession::from_package(&package, Limits::default()).unwrap();
    let report = session.morph(1, 2).unwrap();
    let matched = report
        .explicit_matches
        .iter()
        .find(|m| m.name == "!!box")
        .expect("!!box explicit match");
    let delta = matched.delta.as_ref().expect("delta");
    assert_eq!(delta.x, 8_000_000.0);
    assert_eq!(delta.width, 1_000_000.0);
    assert_eq!(delta.height, 1_000_000.0);
    let timeline = session.timeline(2).unwrap();
    assert_eq!(
        timeline.timing.transition,
        Transition::Morph { duration_ms: 2000 }
    );
}
