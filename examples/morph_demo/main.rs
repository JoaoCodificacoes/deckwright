//! Public Morph demo: three neutral slides built on `template::blank`.
//! Usage: `morph_demo <new-output.pptx>`. Refuses to overwrite an existing file.
use deckwright::{
    Error, Limits, Package, Result,
    check::check_package,
    model::{
        BodyProperties, Color, Fill, Geometry, Line, Object, ObjectKind, Paragraph, Properties,
        RunStyle, Slide, TextBody, TextRun, Transform,
    },
    query::QuerySession,
    template,
    timing::{Effect, EffectKind, Stage, Timeline, Transition, Trigger, compile_animated_slide},
    units::Emu,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

const WIDTH: i64 = 12_192_000;
const HEIGHT: i64 = 6_858_000;

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidPart {
        part: "morph demo".into(),
        message: message.into(),
    }
}

fn label(text: &str, rgb: &str, size: u32) -> TextBody {
    TextBody {
        body: BodyProperties {
            anchor: Some("ctr".into()),
            ..Default::default()
        },
        list_style: false,
        paragraphs: vec![Paragraph {
            properties: None,
            end_style: None,
            runs: vec![TextRun {
                text: text.into(),
                style: Some(RunStyle {
                    size: Some(size),
                    color: Some(Color::Rgb {
                        value: rgb.into(),
                        alpha: None,
                    }),
                    ..Default::default()
                }),
                field: None,
                hyperlink: None,
            }],
        }],
    }
}

/// A preset shape with a solid fill and no outline. `rect` is x, y, width, height in EMU.
fn shape(
    key: &str,
    id: u32,
    name: &str,
    preset: &str,
    rect: [i64; 4],
    rgb: &str,
    text: Option<TextBody>,
) -> Object {
    Object {
        key: key.into(),
        id,
        name: name.into(),
        morph_name: None,
        description: None,
        hyperlink: None,
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
                    ..Default::default()
                }),
                effects: None,
                custom: None,
            },
            style: None,
            text,
        },
    }
}

fn stage(trigger: Trigger, delay_ms: i64, effects: Vec<Effect>) -> Stage {
    Stage {
        trigger,
        delay_ms,
        effects,
    }
}

fn effect(target: u32, delay_ms: i64, duration_ms: u32, kind: EffectKind) -> Effect {
    Effect {
        target,
        delay_ms,
        duration_ms,
        kind,
    }
}

/// Slide 1 title bar and box, then slide 2 box/dot, then slide 3 box/arrow.
fn slides() -> [(Slide, Timeline, Transition); 3] {
    let title = shape(
        "title",
        3,
        "Title bar",
        "rect",
        [0, 0, WIDTH, 1_000_000],
        "1F2937",
        Some(label("Deckwright Morph demo", "FFFFFF", 3200)),
    );
    let first = shape(
        "box",
        2,
        "!!box",
        "roundRect",
        [600_000, 2_400_000, 2_000_000, 2_000_000],
        "2563EB",
        Some(label("box", "FFFFFF", 2400)),
    );
    let second = shape(
        "box",
        2,
        "!!box",
        "roundRect",
        [8_600_000, 2_000_000, 3_000_000, 3_000_000],
        "F97316",
        None,
    );
    let dot = shape(
        "dot",
        4,
        "dot",
        "roundRect",
        [4_600_000, 2_900_000, 1_500_000, 1_500_000],
        "10B981",
        None,
    );
    let third = shape(
        "box",
        2,
        "!!box",
        "roundRect",
        [5_346_000, 1_500_000, 1_500_000, 1_500_000],
        "7C3AED",
        None,
    );
    let arrow = shape(
        "arrow",
        5,
        "arrow",
        "rect",
        [8_000_000, 4_200_000, 2_500_000, 1_200_000],
        "DC2626",
        None,
    );
    let morph = Transition::Morph { duration_ms: 2000 };
    [
        (
            Slide {
                objects: vec![title, first],
                ..Default::default()
            },
            Timeline::default(),
            Transition::None,
        ),
        (
            Slide {
                objects: vec![second, dot],
                ..Default::default()
            },
            Timeline {
                stages: vec![stage(
                    Trigger::OnClick,
                    0,
                    vec![effect(4, 0, 800, EffectKind::FadeIn)],
                )],
            },
            morph.clone(),
        ),
        (
            Slide {
                objects: vec![third, arrow],
                ..Default::default()
            },
            Timeline {
                stages: vec![
                    stage(
                        Trigger::OnClick,
                        0,
                        vec![effect(
                            5,
                            0,
                            1500,
                            EffectKind::Motion {
                                from_x: -0.6,
                                from_y: 0.0,
                                to_x: 0.0,
                                to_y: 0.0,
                                accelerate: 0.2,
                                decelerate: 0.3,
                            },
                        )],
                    ),
                    stage(
                        Trigger::AfterPrevious,
                        500,
                        vec![effect(5, 0, 700, EffectKind::FadeOut)],
                    ),
                ],
            },
            morph,
        ),
    ]
}

fn build(output: &Path) -> Result<()> {
    if output.exists() {
        return Err(invalid(format!(
            "{} already exists; choose a fresh output",
            output.display()
        )));
    }
    let limits = Limits::default();
    let template = template::blank(3, Emu(WIDTH), Emu(HEIGHT))?;
    let mut replacements = BTreeMap::new();
    for (index, (slide, timeline, transition)) in slides().iter().enumerate() {
        replacements.insert(
            format!("ppt/slides/slide{}.xml", index + 1),
            compile_animated_slide(slide, timeline, transition)?,
        );
    }
    let package = template.edited(replacements, &BTreeSet::new(), limits)?;
    package.write(output)?;

    let reopened = QuerySession::open(output, limits)?;
    let slide_count = Package::open(output, limits)?.inventory()["slides"]
        .as_array()
        .map_or(0, Vec::len);
    let morph = reopened.morph(1, 2)?;
    let matched = morph
        .explicit_matches
        .iter()
        .find(|m| m.name == "!!box")
        .ok_or_else(|| invalid("no explicit !!box match between slides 1 and 2"))?;
    let check = check_package(&Package::open(output, limits)?);
    let errors = check
        .findings
        .iter()
        .filter(|f| f.severity == "error")
        .count();
    let summary = serde_json::json!({
        "slides": slide_count,
        "morph_1_to_2": {
            "name": matched.name,
            "delta": matched.delta,
        },
        "check_errors": errors,
    });
    println!("{summary}");
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 1 {
        return Err(invalid("usage: morph_demo <new-output.pptx>"));
    }
    build(&args[0])
}
