//! Typed, deliberately bounded animation semantics. All times are milliseconds.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timeline {
    pub stages: Vec<Stage>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stage {
    pub trigger: Trigger,
    pub delay_ms: i64,
    pub effects: Vec<Effect>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    OnClick,
    WithPrevious,
    AfterPrevious,
    OnSlideStart,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effect {
    pub target: u32,
    pub delay_ms: i64,
    pub duration_ms: u32,
    pub kind: EffectKind,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum EffectKind {
    Appear,
    FadeIn,
    WipeIn {
        direction: Direction,
    },
    FadeOut,
    /// Relative to slide width/height; the static position is the resting END.
    /// At progress zero the offset is (from_x, from_y); at one it is (to_x,to_y).
    Motion {
        from_x: f64,
        from_y: f64,
        to_x: f64,
        to_y: f64,
        accelerate: f64,
        decelerate: f64,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Transition {
    #[default]
    None,
    Fade {
        duration_ms: u32,
    },
    Morph {
        duration_ms: u32,
    },
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParsedTiming {
    pub timeline: Timeline,
    pub transition: Transition,
    pub opaque: Vec<OpaqueTiming>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpaqueTiming {
    pub targets: Vec<u32>,
    pub reason: String,
}

use crate::{
    Error, Result,
    model::{Object, ObjectKind, Slide},
    xml::Node,
};
use std::collections::BTreeSet;
const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
fn bad(part: &str, message: impl Into<String>) -> Error {
    Error::InvalidPart {
        part: part.into(),
        message: message.into(),
    }
}

/// Validate finite units and scheduling arithmetic before compilation or evaluation.
pub fn validate_timeline(timeline: &Timeline) -> Result<()> {
    for (i, s) in timeline.stages.iter().enumerate() {
        if s.effects.is_empty() {
            return Err(bad("timing", format!("stage {i} has no effects")));
        }
        if i == 0 && matches!(s.trigger, Trigger::WithPrevious | Trigger::AfterPrevious) {
            return Err(bad("timing", "first stage cannot refer to previous stage"));
        }
        for e in &s.effects {
            if e.target == 0 || e.duration_ms == 0 {
                return Err(bad("timing", "effect requires nonzero target and duration"));
            }
            s.delay_ms
                .checked_add(e.delay_ms)
                .and_then(|v| v.checked_add(i64::from(e.duration_ms)))
                .ok_or_else(|| bad("timing", "delay/duration overflow"))?;
            if let EffectKind::Motion {
                from_x,
                from_y,
                to_x,
                to_y,
                accelerate,
                decelerate,
            } = e.kind
            {
                if ![from_x, from_y, to_x, to_y, accelerate, decelerate]
                    .iter()
                    .all(|x| x.is_finite())
                    || [from_x, from_y, to_x, to_y].iter().any(|x| x.abs() > 100.0)
                    || to_x != 0.0
                    || to_y != 0.0
                    || accelerate < 0.0
                    || decelerate < 0.0
                    || accelerate + decelerate > 1.0
                {
                    return Err(bad(
                        "timing",
                        "motion requires finite coordinates within 100 slide units and easing fractions summing to <=1",
                    ));
                }
            }
        }
    }
    Ok(())
}
fn targets(objects: &[Object], out: &mut BTreeSet<u32>) {
    for o in objects {
        out.insert(o.id);
        if let ObjectKind::Group { children, .. } = &o.kind {
            targets(children, out);
        }
    }
}
fn ct(id: &mut u32) -> u32 {
    let n = *id;
    *id += 1;
    n
}
fn target(id: u32) -> String {
    format!("<p:tgtEl><p:spTgt spid=\"{id}\"/></p:tgtEl>")
}
fn visibility(id: &mut u32, shape: u32, hidden: bool, delay: u32) -> String {
    let n = ct(id);
    format!(
        "<p:set><p:cBhvr><p:cTn id=\"{n}\" dur=\"1\" fill=\"hold\"><p:stCondLst><p:cond delay=\"{delay}\"/></p:stCondLst></p:cTn>{}<p:attrNameLst><p:attrName>style.visibility</p:attrName></p:attrNameLst></p:cBhvr><p:to><p:strVal val=\"{}\"/></p:to></p:set>",
        target(shape),
        if hidden { "hidden" } else { "visible" }
    )
}
fn effect_xml(e: &Effect, id: &mut u32, node_type: &str) -> String {
    let n = ct(id);
    let behavior = ct(id);
    let (preset, class) = match e.kind {
        EffectKind::Appear => (1, "entr"),
        EffectKind::WipeIn { .. } => (22, "entr"),
        EffectKind::FadeOut => (10, "exit"),
        EffectKind::Motion { .. } => (0, "path"),
        _ => (10, "entr"),
    };
    let subtype = match e.kind {
        EffectKind::WipeIn {
            direction: Direction::Down,
        } => 4,
        EffectKind::WipeIn {
            direction: Direction::Up,
        } => 1,
        EffectKind::WipeIn {
            direction: Direction::Left,
        } => 8,
        EffectKind::WipeIn {
            direction: Direction::Right,
        } => 2,
        _ => 0,
    };
    let ease = if let EffectKind::Motion {
        accelerate,
        decelerate,
        ..
    } = e.kind
    {
        format!(
            " accel=\"{:.0}\" decel=\"{:.0}\"",
            accelerate * 100000.0,
            decelerate * 100000.0
        )
    } else {
        String::new()
    };
    let mut children = String::new();
    if matches!(
        e.kind,
        EffectKind::Appear | EffectKind::FadeIn | EffectKind::WipeIn { .. }
    ) {
        children += &visibility(id, e.target, false, 0);
    }
    let bhvr = format!(
        "<p:cBhvr><p:cTn id=\"{behavior}\" dur=\"{}\" fill=\"hold\"/>{}</p:cBhvr>",
        e.duration_ms,
        target(e.target)
    );
    match e.kind {
        EffectKind::Appear => {}
        EffectKind::Motion {
            from_x,
            from_y,
            to_x,
            to_y,
            ..
        } => {
            children += &format!(
                "<p:animMotion origin=\"layout\" path=\"M {from_x} {from_y} L {to_x} {to_y} E\" pathEditMode=\"relative\" rAng=\"0\">{bhvr}</p:animMotion>"
            )
        }
        _ => {
            let filter = match &e.kind {
                EffectKind::WipeIn { direction } => format!(
                    "wipe({})",
                    match direction {
                        Direction::Up => "down",
                        Direction::Down => "up",
                        Direction::Left => "right",
                        Direction::Right => "left",
                    }
                ),
                _ => "fade".into(),
            };
            children += &format!(
                "<p:animEffect transition=\"{}\" filter=\"{filter}\">{bhvr}</p:animEffect>",
                if matches!(e.kind, EffectKind::FadeOut) {
                    "out"
                } else {
                    "in"
                }
            );
        }
    }
    if matches!(e.kind, EffectKind::FadeOut) {
        children += &visibility(id, e.target, true, e.duration_ms.saturating_sub(1));
    }
    format!(
        "<p:par><p:cTn id=\"{n}\" dur=\"{}\" presetID=\"{preset}\" presetClass=\"{class}\" presetSubtype=\"{subtype}\" fill=\"hold\" grpId=\"{}\" nodeType=\"{node_type}\"{ease}><p:stCondLst><p:cond delay=\"{}\"/></p:stCondLst><p:childTnLst>{children}</p:childTnLst></p:cTn></p:par>",
        e.duration_ms,
        if class == "exit" { 1 } else { 0 },
        e.delay_ms
    )
}
/// Compile fresh timing from typed semantics; imported XML is never copied.
pub fn compile_animated_slide(
    slide: &Slide,
    timeline: &Timeline,
    transition: &Transition,
) -> Result<Vec<u8>> {
    validate_timeline(timeline)?;
    let mut ids = BTreeSet::new();
    targets(&slide.objects, &mut ids);
    for e in timeline.stages.iter().flat_map(|s| &s.effects) {
        if !ids.contains(&e.target) {
            return Err(bad("timing", format!("target {} does not exist", e.target)));
        }
        if e.delay_ms < 0 {
            return Err(bad(
                "timing",
                "negative effect delay must be represented by a stage overlap",
            ));
        }
    }
    let mut xml = String::from_utf8(crate::builder::compile_slide(slide)?)
        .map_err(|e| bad("timing", e.to_string()))?;
    let duration = match transition {
        Transition::None => None,
        Transition::Fade { duration_ms } | Transition::Morph { duration_ms } => Some(*duration_ms),
    };
    if duration == Some(0) {
        return Err(bad("timing", "transition duration must be positive"));
    }
    let transition = match transition {
        Transition::None => String::new(),
        Transition::Fade { duration_ms } => format!(
            "<p:transition xmlns:p14=\"http://schemas.microsoft.com/office/powerpoint/2010/main\" p14:dur=\"{duration_ms}\"><p:fade/></p:transition>"
        ),
        Transition::Morph { duration_ms } => format!(
            "<mc:AlternateContent xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\"><mc:Choice xmlns:p159=\"http://schemas.microsoft.com/office/powerpoint/2015/09/main\" Requires=\"p159\"><p:transition xmlns:p14=\"http://schemas.microsoft.com/office/powerpoint/2010/main\" p14:dur=\"{duration_ms}\"><p159:morph option=\"byObject\"/></p:transition></mc:Choice><mc:Fallback><p:transition xmlns:p14=\"http://schemas.microsoft.com/office/powerpoint/2010/main\" p14:dur=\"{duration_ms}\"><p:fade/></p:transition></mc:Fallback></mc:AlternateContent>"
        ),
    };
    let mut id = 3;
    let mut body = String::new();
    let mut block = String::new();
    let mut block_click = false;
    let mut previous = None;
    let mut previous_end = 0_i64;
    for s in &timeline.stages {
        if matches!(s.trigger, Trigger::OnClick | Trigger::OnSlideStart) && !block.is_empty() {
            let wrapper = ct(&mut id);
            body += &format!(
                "<p:par><p:cTn id=\"{wrapper}\" fill=\"hold\"><p:stCondLst><p:cond delay=\"{}\"/></p:stCondLst><p:childTnLst>{block}</p:childTnLst></p:cTn></p:par>",
                if block_click { "indefinite" } else { "0" }
            );
            block.clear();
        }
        if matches!(s.trigger, Trigger::OnClick | Trigger::OnSlideStart) {
            block_click = matches!(s.trigger, Trigger::OnClick);
        }
        let n = ct(&mut id);
        let node = match s.trigger {
            Trigger::OnClick => "clickEffect",
            Trigger::WithPrevious => "withEffect",
            Trigger::AfterPrevious => "afterEffect",
            Trigger::OnSlideStart => "withEffect",
        };
        let condition = match s.trigger {
            Trigger::OnClick => format!("<p:cond delay=\"{}\"/>", s.delay_ms),
            Trigger::OnSlideStart => format!("<p:cond delay=\"{}\"/>", s.delay_ms),
            _ => {
                let delay = if matches!(s.trigger, Trigger::AfterPrevious) {
                    previous_end
                        .checked_add(s.delay_ms)
                        .ok_or_else(|| bad("timing", "overlap overflow"))?
                } else {
                    s.delay_ms
                };
                if delay < 0 {
                    return Err(bad("timing", "overlap precedes previous stage start"));
                }
                format!(
                    "<p:cond evt=\"onBegin\" delay=\"{delay}\"><p:tn val=\"{}\"/></p:cond>",
                    previous.ok_or_else(|| bad("timing", "missing previous stage"))?
                )
            }
        };
        if matches!(s.trigger, Trigger::OnClick | Trigger::OnSlideStart) && s.delay_ms < 0 {
            return Err(bad("timing", "negative external trigger delay"));
        }
        let children = s
            .effects
            .iter()
            .enumerate()
            .map(|(i, e)| effect_xml(e, &mut id, if i == 0 { node } else { "withEffect" }))
            .collect::<String>();
        block += &format!(
            "<p:par><p:cTn id=\"{n}\" fill=\"hold\" nodeType=\"{node}\"><p:stCondLst>{condition}</p:stCondLst><p:childTnLst>{children}</p:childTnLst></p:cTn></p:par>"
        );
        previous = Some(n);
        previous_end = s
            .effects
            .iter()
            .map(|e| e.delay_ms + i64::from(e.duration_ms))
            .max()
            .unwrap_or(0);
    }
    if !block.is_empty() {
        let wrapper = ct(&mut id);
        body += &format!(
            "<p:par><p:cTn id=\"{wrapper}\" fill=\"hold\"><p:stCondLst><p:cond delay=\"{}\"/></p:stCondLst><p:childTnLst>{block}</p:childTnLst></p:cTn></p:par>",
            if block_click { "indefinite" } else { "0" }
        );
    }
    let mut builds = BTreeSet::new();
    for e in timeline.stages.iter().flat_map(|s| &s.effects) {
        if !matches!(e.kind, EffectKind::Motion { .. }) {
            builds.insert((e.target, matches!(e.kind, EffectKind::FadeOut) as u8));
        }
    }
    let timing = if body.is_empty() {
        String::new()
    } else {
        format!(
            "<p:timing><p:tnLst><p:par><p:cTn id=\"1\" dur=\"indefinite\" restart=\"never\" nodeType=\"tmRoot\"><p:childTnLst><p:seq concurrent=\"1\" nextAc=\"seek\"><p:cTn id=\"2\" dur=\"indefinite\" nodeType=\"mainSeq\"><p:childTnLst>{body}</p:childTnLst></p:cTn><p:prevCondLst><p:cond evt=\"onPrev\" delay=\"0\"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:prevCondLst><p:nextCondLst><p:cond evt=\"onNext\" delay=\"0\"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:nextCondLst></p:seq></p:childTnLst></p:cTn></p:par></p:tnLst><p:bldLst>{}</p:bldLst></p:timing>",
            builds
                .iter()
                .map(|(target, group)| format!(
                    "<p:bldP spid=\"{target}\" grpId=\"{group}\" animBg=\"1\"/>"
                ))
                .collect::<String>()
        )
    };
    let at = xml
        .rfind("</p:sld>")
        .ok_or_else(|| bad("timing", "static compiler omitted slide end"))?;
    xml.insert_str(at, &(transition + &timing));
    Ok(xml.into_bytes())
}

fn attr<'a>(n: &'a Node, key: &str) -> Option<&'a str> {
    n.attrs.get(key).map(String::as_str)
}
fn child<'a>(n: &'a Node, name: &str) -> Option<&'a Node> {
    n.children
        .iter()
        .find(|c| c.name == name && c.namespace == P)
}
fn number<T: std::str::FromStr>(n: &Node, key: &str, part: &str) -> Result<T> {
    attr(n, key)
        .ok_or_else(|| bad(part, format!("{} missing {key}", n.name)))?
        .parse()
        .map_err(|_| bad(part, format!("invalid {} {key}", n.name)))
}
fn delay(n: &Node, part: &str) -> Result<i64> {
    match child(n, "stCondLst")
        .and_then(|c| child(c, "cond"))
        .and_then(|c| attr(c, "delay"))
    {
        None | Some("indefinite") => Ok(0),
        Some(v) => v.parse().map_err(|_| bad(part, "invalid timing delay")),
    }
}
fn opaque(n: &Node, reason: impl Into<String>) -> OpaqueTiming {
    OpaqueTiming {
        targets: n
            .descendants("spTgt")
            .filter_map(|x| attr(x, "spid").and_then(|s| s.parse().ok()))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        reason: reason.into(),
    }
}
fn read_effect(n: &Node, offset: i64, part: &str) -> Result<Effect> {
    let supported = match attr(n, "presetClass") {
        Some("entr") => matches!(attr(n, "presetID"), Some("1" | "10" | "22")),
        Some("exit") => attr(n, "presetID") == Some("10"),
        Some("path") => attr(n, "presetID") == Some("0"),
        _ => false,
    };
    if !supported {
        return Err(bad(part, "unsupported preset class or ID"));
    }
    let shape = n
        .descendants("spTgt")
        .next()
        .ok_or_else(|| bad(part, "effect missing shape target"))?;
    let target = number(shape, "spid", part)?;
    if n.descendants("spTgt")
        .any(|s| attr(s, "spid") != attr(shape, "spid"))
    {
        return Err(bad(part, "effect has multiple targets"));
    }
    let motion = n.descendants("animMotion").next();
    let anim = n.descendants("animEffect").next();
    let (kind, duration) = if let Some(m) = motion {
        if attr(m, "origin") != Some("layout") || attr(m, "pathEditMode") != Some("relative") {
            return Err(bad(part, "unsupported motion coordinate origin"));
        }
        let path = attr(m, "path").ok_or_else(|| bad(part, "missing motion path"))?;
        let fields = path.split_whitespace().collect::<Vec<_>>();
        if fields.len() != 7 || fields[0] != "M" || fields[3] != "L" || fields[6] != "E" {
            return Err(bad(part, "only relative line motion is supported"));
        }
        let f = |i: usize| {
            fields[i]
                .parse::<f64>()
                .map_err(|_| bad(part, "invalid motion coordinate"))
        };
        let fraction = |key: &str| -> Result<f64> {
            attr(n, key)
                .unwrap_or("0")
                .parse::<f64>()
                .map(|x| x / 100000.0)
                .map_err(|_| bad(part, "invalid easing"))
        };
        let duration = m
            .descendants("cTn")
            .next()
            .ok_or_else(|| bad(part, "motion missing duration"))?;
        (
            EffectKind::Motion {
                from_x: f(1)?,
                from_y: f(2)?,
                to_x: f(4)?,
                to_y: f(5)?,
                accelerate: fraction("accel")?,
                decelerate: fraction("decel")?,
            },
            number(duration, "dur", part)?,
        )
    } else if let Some(a) = anim {
        let filter = attr(a, "filter").unwrap_or("");
        let exit = attr(a, "transition") == Some("out");
        let kind = match (filter, exit) {
            ("fade", false) => EffectKind::FadeIn,
            ("fade", true) => EffectKind::FadeOut,
            ("wipe(up)", false) => EffectKind::WipeIn {
                direction: Direction::Down,
            },
            ("wipe(down)", false) => EffectKind::WipeIn {
                direction: Direction::Up,
            },
            ("wipe(left)", false) => EffectKind::WipeIn {
                direction: Direction::Right,
            },
            ("wipe(right)", false) => EffectKind::WipeIn {
                direction: Direction::Left,
            },
            _ => return Err(bad(part, format!("unsupported effect filter {filter}"))),
        };
        let duration = a
            .descendants("cTn")
            .next()
            .ok_or_else(|| bad(part, "effect missing duration"))?;
        (kind, number(duration, "dur", part)?)
    } else if attr(n, "presetID") == Some("1") && n.descendants("set").next().is_some() {
        (EffectKind::Appear, number(n, "dur", part)?)
    } else {
        return Err(bad(part, "unknown preset or behavior"));
    };
    let effect = Effect {
        target,
        delay_ms: offset,
        duration_ms: duration,
        kind,
    };
    validate_timeline(&Timeline {
        stages: vec![Stage {
            trigger: Trigger::OnSlideStart,
            delay_ms: 0,
            effects: vec![effect.clone()],
        }],
    })?;
    Ok(effect)
}
fn source_effects(
    n: &Node,
    offset: i64,
    part: &str,
    effects: &mut Vec<Effect>,
    unknown: &mut Vec<OpaqueTiming>,
) -> Result<()> {
    let offset = if n.name == "cTn" {
        offset
            .checked_add(delay(n, part)?)
            .ok_or_else(|| bad(part, "delay overflow"))?
    } else {
        offset
    };
    if n.name == "cTn" && attr(n, "presetID").is_some() {
        match read_effect(n, offset, part) {
            Ok(e) => effects.push(e),
            Err(e) => unknown.push(opaque(n, e.to_string())),
        };
        return Ok(());
    }
    for c in &n.children {
        source_effects(c, offset, part, effects, unknown)?;
    }
    Ok(())
}
/// Parse the explicitly supported subset. Unsupported behavior remains opaque.
pub fn parse_slide_timing(xml: &[u8], part: &str) -> Result<ParsedTiming> {
    let root = crate::xml::parse(xml, part, 256)?;
    let mut result = ParsedTiming::default();
    fn morph(n: &Node) -> bool {
        (n.name == "morph"
            && n.namespace == "http://schemas.microsoft.com/office/powerpoint/2015/09/main")
            || n.children.iter().any(morph)
    }
    if let Some(t) = root.descendants("transition").next() {
        let duration = attr(
            t,
            "{http://schemas.microsoft.com/office/powerpoint/2010/main}dur",
        )
        .map(|v| {
            v.parse::<u32>()
                .map_err(|_| bad(part, "invalid transition duration"))
        })
        .transpose()?
        .unwrap_or(match attr(t, "spd") {
            Some("slow") => 1000,
            Some("fast") => 250,
            _ => 500,
        });
        if duration == 0 {
            return Err(bad(part, "zero transition duration"));
        }
        if morph(&root) {
            result.transition = Transition::Morph {
                duration_ms: duration,
            };
            fn unsupported_morph(n: &Node) -> bool {
                (n.name == "morph" && attr(n, "option") != Some("byObject"))
                    || n.children.iter().any(unsupported_morph)
            }
            if unsupported_morph(&root) {
                result
                    .opaque
                    .push(opaque(t, "only byObject Morph is supported"));
            }
        } else if child(t, "fade").is_some() {
            result.transition = Transition::Fade {
                duration_ms: duration,
            };
        } else {
            result.opaque.push(opaque(t, "unsupported transition"));
        }
    }
    let Some(timing) = root.descendants("timing").next() else {
        return Ok(result);
    };
    fn inspect(n: &Node, unknown: &mut Vec<OpaqueTiming>) {
        let allowed = matches!(
            n.name.as_str(),
            "timing"
                | "tnLst"
                | "par"
                | "seq"
                | "cTn"
                | "childTnLst"
                | "stCondLst"
                | "cond"
                | "tn"
                | "tgtEl"
                | "sldTgt"
                | "spTgt"
                | "cBhvr"
                | "attrNameLst"
                | "attrName"
                | "set"
                | "to"
                | "strVal"
                | "animEffect"
                | "animMotion"
                | "prevCondLst"
                | "nextCondLst"
                | "bldLst"
                | "bldP"
        );
        if n.namespace != P || !allowed {
            unknown.push(opaque(n, format!("unsupported timing element {}", n.name)));
            return;
        }
        if n.name == "cTn"
            && n.attrs.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "id" | "dur"
                        | "restart"
                        | "nodeType"
                        | "fill"
                        | "presetID"
                        | "presetClass"
                        | "presetSubtype"
                        | "grpId"
                        | "accel"
                        | "decel"
                )
            })
        {
            unknown.push(opaque(n, "unsupported time node attributes"));
        }
        if n.name == "cond"
            && attr(n, "evt").is_some_and(|e| !matches!(e, "onBegin" | "onPrev" | "onNext"))
        {
            unknown.push(opaque(n, "unsupported trigger event"));
        }
        if n.name == "stCondLst" && n.children.len() != 1 {
            unknown.push(opaque(n, "multiple start conditions are unsupported"));
        }
        if n.name == "spTgt" && !n.children.is_empty() {
            unknown.push(opaque(n, "text/subshape targets are unsupported"));
        }
        for c in &n.children {
            inspect(c, unknown);
        }
    }
    inspect(timing, &mut result.opaque);
    let mut ids = BTreeSet::new();
    for n in timing.descendants("cTn") {
        let id: u32 = number(n, "id", part)?;
        if id == 0 || !ids.insert(id) {
            return Err(bad(part, "duplicate or zero timing node ID"));
        }
    }
    let shape_ids = root
        .descendants("cNvPr")
        .filter_map(|n| attr(n, "id").and_then(|s| s.parse::<u32>().ok()))
        .collect::<BTreeSet<_>>();
    for n in timing.descendants("spTgt") {
        let id: u32 = number(n, "spid", part)?;
        if !shape_ids.contains(&id) {
            return Err(bad(part, format!("timing references missing target {id}")));
        }
    }
    let main = timing
        .descendants("cTn")
        .find(|n| attr(n, "nodeType") == Some("mainSeq"));
    let Some(list) = main.and_then(|n| child(n, "childTnLst")) else {
        result
            .opaque
            .push(opaque(timing, "missing supported main sequence"));
        return Ok(result);
    };
    let mut previous_end = 0_i64;
    let mut groups = Vec::new();
    for group in &list.children {
        let nested = child(group, "cTn").and_then(|n| child(n, "childTnLst"));
        let canonical = nested.is_some_and(|list| {
            list.children.iter().all(|c| {
                child(c, "cTn")
                    .is_some_and(|n| attr(n, "nodeType").is_some() && attr(n, "presetID").is_none())
            })
        });
        if canonical {
            if let Some(nested) = nested {
                groups.extend(nested.children.iter());
            }
        } else {
            groups.push(group);
        }
    }
    for group in groups {
        let Some(n) = child(group, "cTn") else {
            result
                .opaque
                .push(opaque(group, "unsupported main sequence child"));
            continue;
        };
        let indefinite = child(n, "stCondLst")
            .and_then(|x| child(x, "cond"))
            .and_then(|x| attr(x, "delay"))
            == Some("indefinite");
        if indefinite {
            let mut effects = Vec::new();
            source_effects(n, 0, part, &mut effects, &mut result.opaque)?;
            if !effects.is_empty() {
                result.timeline.stages.push(Stage {
                    trigger: Trigger::OnClick,
                    delay_ms: 0,
                    effects,
                });
            }
            continue;
        }
        let Some(node_type) = attr(n, "nodeType") else {
            result
                .opaque
                .push(opaque(n, "unsupported scheduling container"));
            continue;
        };
        let cond = child(n, "stCondLst").and_then(|x| child(x, "cond"));
        let trigger = match node_type {
            "clickEffect" => Trigger::OnClick,
            "afterEffect" => Trigger::AfterPrevious,
            "withEffect" => {
                if cond.is_some_and(|c| child(c, "tn").is_some()) {
                    Trigger::WithPrevious
                } else {
                    Trigger::OnSlideStart
                }
            }
            _ => {
                result.opaque.push(opaque(n, "unsupported stage node type"));
                continue;
            }
        };
        let mut stage_delay = delay(n, part)?;
        if matches!(trigger, Trigger::AfterPrevious) {
            stage_delay -= previous_end;
        }
        let mut effects = Vec::new();
        if let Some(children) = child(n, "childTnLst") {
            for c in &children.children {
                source_effects(c, 0, part, &mut effects, &mut result.opaque)?;
            }
        }
        if !effects.is_empty() {
            previous_end = effects
                .iter()
                .map(|e| e.delay_ms + i64::from(e.duration_ms))
                .max()
                .unwrap_or(0);
            result.timeline.stages.push(Stage {
                trigger,
                delay_ms: stage_delay,
                effects,
            });
        }
    }
    // Effects outside the supported preset containers must never disappear silently.
    let covered = result
        .timeline
        .stages
        .iter()
        .map(|s| s.effects.len())
        .sum::<usize>();
    if covered == 0 && timing.descendants("cBhvr").next().is_some() && result.opaque.is_empty() {
        result
            .opaque
            .push(opaque(timing, "unrecognized timing behavior"));
    }
    Ok(result)
}
