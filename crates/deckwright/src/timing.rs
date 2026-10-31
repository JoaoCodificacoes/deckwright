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
    /// Grow/Shrink emphasis (`presetClass="emph" presetID="6"`). `x` and `y` are the factors
    /// reached at the end of one forward pass. See docs/decisions/timing.md.
    Scale {
        x: f64,
        y: f64,
        repeat: Repeat,
        auto_reverse: bool,
    },
    /// PowerPoint media call (`presetClass="mediacall" presetID="1"`). Plays a video or audio
    /// picture from `from_ms` for `Effect.duration_ms`. `volume` is in thousandths of a percent.
    /// See docs/decisions/timing.md.
    MediaPlay {
        from_ms: u32,
        volume: u32,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Repeat {
    /// Thousandths of a repetition, 1..=1_000_000; 1000 means once.
    Count {
        thousandths: u32,
    },
    Indefinite,
}
/// Active duration in milliseconds: one iteration (doubled by `auto_reverse`) times the
/// repeat count. `None` for an indefinite repeat, which never ends.
pub fn effect_active_ms(effect: &Effect) -> Option<u64> {
    match &effect.kind {
        EffectKind::Scale {
            repeat,
            auto_reverse,
            ..
        } => {
            let iteration = u64::from(effect.duration_ms) * if *auto_reverse { 2 } else { 1 };
            match repeat {
                Repeat::Count { thousandths } => Some(iteration * u64::from(*thousandths) / 1000),
                Repeat::Indefinite => None,
            }
        }
        _ => Some(u64::from(effect.duration_ms)),
    }
}
/// Latest end of a stage's effects relative to the stage start. `None` when any effect is
/// indefinite, so a following `AfterPrevious` stage is never anchored.
fn effects_end(effects: &[Effect]) -> Option<i64> {
    let mut latest: Option<i64> = None;
    for e in effects {
        let end = i64::try_from(effect_active_ms(e)?)
            .ok()?
            .checked_add(e.delay_ms)?;
        latest = Some(latest.map_or(end, |l| l.max(end)));
    }
    Some(latest.unwrap_or(0))
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
use std::collections::{BTreeMap, BTreeSet};
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
            if let EffectKind::Scale { x, y, repeat, .. } = &e.kind
                && (![*x, *y]
                    .iter()
                    .all(|f| f.is_finite() && *f > 0.0 && *f <= 10.0)
                    || [*x, *y].iter().any(|f| (f * 100_000.0).round() < 1.0)
                    || matches!(repeat, Repeat::Count { thousandths }
                        if !(1..=1_000_000).contains(thousandths)))
            {
                return Err(bad(
                    "timing",
                    "scale requires finite factors in (0, 10] and a repeat count of 1..=1000000 thousandths",
                ));
            }
            if let EffectKind::MediaPlay { from_ms, volume } = e.kind
                && (from_ms % 100 != 0 || volume > 100_000)
            {
                return Err(bad(
                    "timing",
                    "media call requires from_ms in whole tenths of a second and volume 0..=100000",
                ));
            }
            if let EffectKind::Motion {
                from_x,
                from_y,
                to_x,
                to_y,
                accelerate,
                decelerate,
            } = e.kind
                && (![from_x, from_y, to_x, to_y, accelerate, decelerate]
                    .iter()
                    .all(|x| x.is_finite())
                    || [from_x, from_y, to_x, to_y].iter().any(|x| x.abs() > 100.0)
                    || to_x != 0.0
                    || to_y != 0.0
                    || accelerate < 0.0
                    || decelerate < 0.0
                    || accelerate + decelerate > 1.0)
            {
                return Err(bad(
                    "timing",
                    "motion requires finite coordinates within 100 slide units and easing fractions summing to <=1",
                ));
            }
        }
    }
    let mut volumes = BTreeMap::new();
    for e in timeline.stages.iter().flat_map(|s| &s.effects) {
        if let EffectKind::MediaPlay { volume, .. } = e.kind
            && volumes
                .insert(e.target, volume)
                .is_some_and(|v| v != volume)
        {
            return Err(bad("timing", "media calls on one target need one volume"));
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
/// Scale factors are stored in the XML as integers in hundred-thousandths.
fn scale_units(factor: f64) -> i64 {
    (factor * 100_000.0).round() as i64
}
fn effect_xml(e: &Effect, id: &mut u32, node_type: &str) -> String {
    let n = ct(id);
    let behavior = ct(id);
    let (preset, class) = match e.kind {
        EffectKind::Appear => (1, "entr"),
        EffectKind::WipeIn { .. } => (22, "entr"),
        EffectKind::FadeOut => (10, "exit"),
        EffectKind::Motion { .. } => (0, "path"),
        EffectKind::Scale { .. } => (6, "emph"),
        EffectKind::MediaPlay { .. } => (1, "mediacall"),
        _ => (10, "entr"),
    };
    let repeat = if let EffectKind::Scale {
        repeat,
        auto_reverse,
        ..
    } = &e.kind
    {
        let count = match repeat {
            Repeat::Count { thousandths } => thousandths.to_string(),
            Repeat::Indefinite => "indefinite".into(),
        };
        format!(
            " repeatCount=\"{count}\"{}",
            if *auto_reverse { " autoRev=\"1\"" } else { "" }
        )
    } else {
        String::new()
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
        EffectKind::Scale { x, y, .. } => {
            children += &format!(
                "<p:animScale>{bhvr}<p:by x=\"{}\" y=\"{}\"/></p:animScale>",
                scale_units(x),
                scale_units(y)
            )
        }
        EffectKind::MediaPlay { from_ms, .. } => {
            // from_ms is a whole number of tenths of a second, so the seconds are exact.
            children += &format!(
                "<p:cmd type=\"call\" cmd=\"playFrom({}.{})\">{bhvr}</p:cmd>",
                from_ms / 1000,
                (from_ms % 1000) / 100
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
        "<p:par><p:cTn id=\"{n}\" dur=\"{}\" presetID=\"{preset}\" presetClass=\"{class}\" presetSubtype=\"{subtype}\" fill=\"hold\" grpId=\"{}\" nodeType=\"{node_type}\"{ease}{repeat}><p:stCondLst><p:cond delay=\"{}\"/></p:stCondLst><p:childTnLst>{children}</p:childTnLst></p:cTn></p:par>",
        e.duration_ms,
        if class == "exit" { 1 } else { 0 },
        e.delay_ms
    )
}
/// Start conditions of a main-sequence group wrapper. A click group waits for a click. The
/// first group of a timeline that starts with `OnSlideStart` uses the auto-start pair, which
/// `auto_start` reads back. Other groups keep their base `delay="0"` condition.
fn group_conditions(click: bool, auto: bool) -> &'static str {
    if click {
        r#"<p:cond delay="indefinite"/>"#
    } else if auto {
        r#"<p:cond delay="indefinite"/><p:cond evt="onBegin" delay="0"><p:tn val="2"/></p:cond>"#
    } else {
        r#"<p:cond delay="0"/>"#
    }
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
    // A timeline that starts with OnSlideStart uses the auto-start pair for its first group.
    let auto = matches!(
        timeline.stages.first().map(|s| &s.trigger),
        Some(Trigger::OnSlideStart)
    );
    let mut id = 3;
    let mut body = String::new();
    let mut block = String::new();
    let mut block_click = false;
    let mut previous = None;
    let mut previous_end = Some(0_i64);
    for s in &timeline.stages {
        if matches!(s.trigger, Trigger::OnClick | Trigger::OnSlideStart) && !block.is_empty() {
            let wrapper = ct(&mut id);
            // Groups are emitted in order, so an empty body means this is the first group.
            let conditions = group_conditions(block_click, auto && body.is_empty());
            body += &format!(
                "<p:par><p:cTn id=\"{wrapper}\" fill=\"hold\"><p:stCondLst>{conditions}</p:stCondLst><p:childTnLst>{block}</p:childTnLst></p:cTn></p:par>"
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
                        .and_then(|end| end.checked_add(s.delay_ms))
                        .ok_or_else(|| {
                            bad(
                                "timing",
                                "after-previous stage follows an indefinite effect or overflows",
                            )
                        })?
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
        previous_end = effects_end(&s.effects);
    }
    if !block.is_empty() {
        let wrapper = ct(&mut id);
        let conditions = group_conditions(block_click, auto && body.is_empty());
        body += &format!(
            "<p:par><p:cTn id=\"{wrapper}\" fill=\"hold\"><p:stCondLst>{conditions}</p:stCondLst><p:childTnLst>{block}</p:childTnLst></p:cTn></p:par>"
        );
    }
    let mut builds = BTreeSet::new();
    for e in timeline.stages.iter().flat_map(|s| &s.effects) {
        if !matches!(e.kind, EffectKind::Motion { .. }) {
            builds.insert((e.target, matches!(e.kind, EffectKind::FadeOut) as u8));
        }
    }
    // One media node per target, after the main sequence, in target order.
    let mut media_volumes = BTreeMap::new();
    for e in timeline.stages.iter().flat_map(|s| &s.effects) {
        if let EffectKind::MediaPlay { volume, .. } = e.kind {
            media_volumes.insert(e.target, volume);
        }
    }
    let media = media_volumes
        .iter()
        .map(|(target, volume)| {
            let n = ct(&mut id);
            format!(
                "<p:video><p:cMediaNode vol=\"{volume}\"><p:cTn id=\"{n}\" fill=\"hold\" display=\"0\"><p:stCondLst><p:cond delay=\"indefinite\"/></p:stCondLst></p:cTn><p:tgtEl><p:spTgt spid=\"{target}\"/></p:tgtEl></p:cMediaNode></p:video>"
            )
        })
        .collect::<String>();
    let timing = if body.is_empty() {
        String::new()
    } else {
        format!(
            "<p:timing><p:tnLst><p:par><p:cTn id=\"1\" dur=\"indefinite\" restart=\"never\" nodeType=\"tmRoot\"><p:childTnLst><p:seq concurrent=\"1\" nextAc=\"seek\"><p:cTn id=\"2\" dur=\"indefinite\" nodeType=\"mainSeq\"><p:childTnLst>{body}</p:childTnLst></p:cTn><p:prevCondLst><p:cond evt=\"onPrev\" delay=\"0\"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:prevCondLst><p:nextCondLst><p:cond evt=\"onNext\" delay=\"0\"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:nextCondLst></p:seq>{media}</p:childTnLst></p:cTn></p:par></p:tnLst><p:bldLst>{}</p:bldLst></p:timing>",
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
fn is_p(n: &Node, name: &str) -> bool {
    n.name == name && n.namespace == P
}
/// `n` is the presentationML element `name` with exactly these attributes and child count.
fn exact(n: &Node, name: &str, keys: &[&str], children: usize) -> bool {
    is_p(n, name)
        && n.attrs.len() == keys.len()
        && n.children.len() == children
        && keys.iter().all(|k| n.attrs.contains_key(*k))
}
/// `group` is a main-sequence group whose cTn start conditions are exactly
/// `<cond delay="indefinite"/>` followed by `<cond evt="onBegin" delay="0"><tn val="{main}"/></cond>`.
fn auto_start(group: &Node, main: u32) -> bool {
    let Some(list) = child(group, "cTn").and_then(|n| child(n, "stCondLst")) else {
        return false;
    };
    let [first, second] = list.children.as_slice() else {
        return false;
    };
    let main = main.to_string();
    exact(first, "cond", &["delay"], 0)
        && attr(first, "delay") == Some("indefinite")
        && exact(second, "cond", &["evt", "delay"], 1)
        && attr(second, "evt") == Some("onBegin")
        && attr(second, "delay") == Some("0")
        && exact(&second.children[0], "tn", &["val"], 0)
        && attr(&second.children[0], "val") == Some(main.as_str())
}
/// A `p:video` node placed after the main sequence: its target and volume. It must match
/// `compile_animated_slide` exactly, with no other attributes or children.
fn media_node(v: &Node, part: &str) -> Result<(u32, u32)> {
    let fail = || bad(part, "invalid media node");
    let [m] = v.children.as_slice() else {
        return Err(fail());
    };
    let [timer, tgt] = m.children.as_slice() else {
        return Err(fail());
    };
    let [st] = timer.children.as_slice() else {
        return Err(fail());
    };
    let [cond] = st.children.as_slice() else {
        return Err(fail());
    };
    let [sp] = tgt.children.as_slice() else {
        return Err(fail());
    };
    let ok = v.attrs.is_empty()
        && exact(m, "cMediaNode", &["vol"], 2)
        && exact(timer, "cTn", &["id", "fill", "display"], 1)
        && attr(timer, "fill") == Some("hold")
        && attr(timer, "display") == Some("0")
        && exact(st, "stCondLst", &[], 1)
        && exact(cond, "cond", &["delay"], 0)
        && attr(cond, "delay") == Some("indefinite")
        && exact(tgt, "tgtEl", &[], 1)
        && exact(sp, "spTgt", &["spid"], 0);
    if !ok {
        return Err(fail());
    }
    Ok((number(sp, "spid", part)?, number(m, "vol", part)?))
}
/// A media call: exactly one `playFrom(s.t)` command on the target, in the form the compiler
/// writes. The volume comes from that target's `p:video` node.
fn media_call(
    n: &Node,
    target: u32,
    volumes: &BTreeMap<u32, u32>,
    part: &str,
) -> Result<(EffectKind, u32)> {
    let malformed = || bad(part, "malformed media call");
    if n.descendants("cmd").count() != 1
        || ["animEffect", "animMotion", "animScale", "set"]
            .iter()
            .any(|k| n.descendants(k).next().is_some())
    {
        return Err(malformed());
    }
    let cmd = child(n, "childTnLst")
        .and_then(|l| child(l, "cmd"))
        .ok_or_else(malformed)?;
    let [b] = cmd.children.as_slice() else {
        return Err(malformed());
    };
    let [timer, tgt] = b.children.as_slice() else {
        return Err(malformed());
    };
    if !exact(cmd, "cmd", &["type", "cmd"], 1)
        || attr(cmd, "type") != Some("call")
        || !exact(b, "cBhvr", &[], 2)
        || !exact(timer, "cTn", &["id", "dur", "fill"], 0)
        || attr(timer, "fill") != Some("hold")
        || !exact(tgt, "tgtEl", &[], 1)
    {
        return Err(malformed());
    }
    let (secs, tenth) = attr(cmd, "cmd")
        .and_then(|c| c.strip_prefix("playFrom(")?.strip_suffix(')'))
        .and_then(|c| c.split_once('.'))
        .filter(|(_, t)| t.len() == 1 && t.bytes().all(|b| b.is_ascii_digit()))
        .ok_or_else(malformed)?;
    let from_ms = secs
        .parse::<u32>()
        .ok()
        .and_then(|s| s.checked_mul(1000))
        .and_then(|ms| ms.checked_add(100 * u32::from(tenth.as_bytes()[0] - b'0')))
        .ok_or_else(malformed)?;
    // Only the canonical spelling is accepted, so every accepted file recompiles identically.
    let canonical = format!("playFrom({}.{})", from_ms / 1000, (from_ms % 1000) / 100);
    if attr(cmd, "cmd") != Some(canonical.as_str()) {
        return Err(malformed());
    }
    let volume = *volumes
        .get(&target)
        .ok_or_else(|| bad(part, "media call has no matching p:video node"))?;
    Ok((
        EffectKind::MediaPlay { from_ms, volume },
        number(timer, "dur", part)?,
    ))
}
/// Opaque entries for `p:video` nodes that are malformed, duplicated for one target, or not used
/// by a media call in the parsed timeline.
fn media_problems(
    placed: &[&Node],
    media: &[Result<(u32, u32)>],
    timeline: &Timeline,
) -> Vec<OpaqueTiming> {
    let mut counts = BTreeMap::<u32, usize>::new();
    for (target, _) in media.iter().flatten() {
        *counts.entry(*target).or_default() += 1;
    }
    let used: BTreeSet<u32> = timeline
        .stages
        .iter()
        .flat_map(|s| &s.effects)
        .filter(|e| matches!(e.kind, EffectKind::MediaPlay { .. }))
        .map(|e| e.target)
        .collect();
    placed
        .iter()
        .zip(media)
        .filter_map(|(v, m)| match m {
            Err(e) => Some(opaque(v, e.to_string())),
            Ok((t, _)) if counts[t] > 1 => Some(opaque(v, "duplicate media timing for one target")),
            Ok((t, _)) if !used.contains(t) => Some(opaque(v, "media timing without a media call")),
            Ok(_) => None,
        })
        .collect()
}
fn read_effect(n: &Node, offset: i64, part: &str, volumes: &BTreeMap<u32, u32>) -> Result<Effect> {
    let supported = match attr(n, "presetClass") {
        Some("entr") => matches!(attr(n, "presetID"), Some("1" | "10" | "22")),
        Some("exit") => attr(n, "presetID") == Some("10"),
        Some("path") => attr(n, "presetID") == Some("0"),
        Some("emph") => attr(n, "presetID") == Some("6"),
        Some("mediacall") => attr(n, "presetID") == Some("1"),
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
    let (kind, duration) = if attr(n, "presetClass") == Some("mediacall") {
        media_call(n, target, volumes, part)?
    } else if let Some(m) = motion {
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
    } else if let Some(s) = n.descendants("animScale").next() {
        if s.children.len() != 2 {
            return Err(bad(part, "scale behaviour must hold cBhvr and by only"));
        }
        let repeat = match attr(n, "repeatCount") {
            Some("indefinite") => Repeat::Indefinite,
            Some(v) => Repeat::Count {
                thousandths: v
                    .parse()
                    .map_err(|_| bad(part, "invalid scale repeatCount"))?,
            },
            None => return Err(bad(part, "scale missing repeatCount")),
        };
        let auto_reverse = match attr(n, "autoRev") {
            None => false,
            Some("1") => true,
            Some(_) => return Err(bad(part, "invalid scale autoRev")),
        };
        let by = child(s, "by").ok_or_else(|| bad(part, "scale missing by"))?;
        let factor =
            |key: &str| -> Result<f64> { Ok(number::<i64>(by, key, part)? as f64 / 100_000.0) };
        let duration = s
            .descendants("cTn")
            .next()
            .ok_or_else(|| bad(part, "scale missing duration"))?;
        (
            EffectKind::Scale {
                x: factor("x")?,
                y: factor("y")?,
                repeat,
                auto_reverse,
            },
            number(duration, "dur", part)?,
        )
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
    volumes: &BTreeMap<u32, u32>,
) -> Result<()> {
    let offset = if n.name == "cTn" {
        offset
            .checked_add(delay(n, part)?)
            .ok_or_else(|| bad(part, "delay overflow"))?
    } else {
        offset
    };
    if n.name == "cTn" && attr(n, "presetID").is_some() {
        match read_effect(n, offset, part, volumes) {
            Ok(e) => effects.push(e),
            Err(e) => unknown.push(opaque(n, e.to_string())),
        };
        return Ok(());
    }
    for c in &n.children {
        source_effects(c, offset, part, effects, unknown, volumes)?;
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
    fn inspect<'a>(
        n: &'a Node,
        stack: &mut Vec<&'a Node>,
        placed: &[&Node],
        auto: &[&Node],
        unknown: &mut Vec<OpaqueTiming>,
    ) {
        // Placed media nodes are validated by `media_node`, so their contents are not inspected.
        if is_p(n, "video") && placed.iter().any(|p| std::ptr::eq(*p, n)) {
            return;
        }
        let parent = stack.last().map_or("", |p| p.name.as_str());
        // A media command is only valid directly inside a media call effect.
        let media_cmd = n.name == "cmd"
            && parent == "childTnLst"
            && stack.len() >= 2
            && stack[stack.len() - 2].name == "cTn"
            && attr(stack[stack.len() - 2], "presetClass") == Some("mediacall");
        let allowed = media_cmd
            || matches!(
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
                    | "animScale"
                    | "prevCondLst"
                    | "nextCondLst"
                    | "bldLst"
                    | "bldP"
            )
            || (n.name == "by" && parent == "animScale");
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
                        | "repeatCount"
                        | "autoRev"
                )
            })
        {
            unknown.push(opaque(n, "unsupported time node attributes"));
        }
        // Repeats are only recognised on the Grow/Shrink effect node itself.
        if n.name == "cTn"
            && (n.attrs.contains_key("repeatCount") || n.attrs.contains_key("autoRev"))
            && !(attr(n, "presetID") == Some("6") && attr(n, "presetClass") == Some("emph"))
        {
            unknown.push(opaque(n, "repeat placement outside emphasis scale"));
        }
        if n.name == "cond"
            && attr(n, "evt").is_some_and(|e| !matches!(e, "onBegin" | "onPrev" | "onNext"))
        {
            unknown.push(opaque(n, "unsupported trigger event"));
        }
        // Auto-start pairs are validated by `auto_start`, the only multiple-condition form.
        if n.name == "stCondLst"
            && n.children.len() != 1
            && !auto.iter().any(|a| std::ptr::eq(*a, n))
        {
            unknown.push(opaque(n, "multiple start conditions are unsupported"));
        }
        if n.name == "spTgt" && !n.children.is_empty() {
            unknown.push(opaque(n, "text/subshape targets are unsupported"));
        }
        stack.push(n);
        for c in &n.children {
            inspect(c, stack, placed, auto, unknown);
        }
        stack.pop();
    }
    // Media nodes must follow the main sequence in the root time node.
    let placed: Vec<&Node> = timing
        .descendants("cTn")
        .find(|n| attr(n, "nodeType") == Some("tmRoot"))
        .and_then(|n| child(n, "childTnLst"))
        .map(|l| {
            l.children
                .iter()
                .skip_while(|c| c.name != "seq")
                .filter(|c| is_p(c, "video"))
                .collect()
        })
        .unwrap_or_default();
    let main = timing
        .descendants("cTn")
        .find(|n| attr(n, "nodeType") == Some("mainSeq"));
    let main_id = main
        .and_then(|n| attr(n, "id"))
        .and_then(|v| v.parse::<u32>().ok());
    let main_list = main.and_then(|n| child(n, "childTnLst"));
    let auto_conds: Vec<&Node> = match (main_list, main_id) {
        (Some(list), Some(id)) => list
            .children
            .iter()
            .filter(|g| auto_start(g, id))
            .filter_map(|g| child(g, "cTn"))
            .filter_map(|n| child(n, "stCondLst"))
            .collect(),
        _ => Vec::new(),
    };
    inspect(
        timing,
        &mut Vec::new(),
        &placed,
        &auto_conds,
        &mut result.opaque,
    );
    let media: Vec<Result<(u32, u32)>> = placed.iter().map(|v| media_node(v, part)).collect();
    let mut counts = BTreeMap::<u32, usize>::new();
    for (target, _) in media.iter().flatten() {
        *counts.entry(*target).or_default() += 1;
    }
    let volumes: BTreeMap<u32, u32> = media
        .iter()
        .flatten()
        .filter(|(t, _)| counts[t] == 1)
        .copied()
        .collect();
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
    let Some(list) = main_list else {
        result
            .opaque
            .push(opaque(timing, "missing supported main sequence"));
        result
            .opaque
            .extend(media_problems(&placed, &media, &result.timeline));
        return Ok(result);
    };
    let mut previous_end = Some(0_i64);
    // Each entry carries whether it is the first stage of an auto-start group.
    let mut groups: Vec<(&Node, bool)> = Vec::new();
    for group in &list.children {
        let nested = child(group, "cTn").and_then(|n| child(n, "childTnLst"));
        let canonical = nested.is_some_and(|list| {
            list.children.iter().all(|c| {
                child(c, "cTn")
                    .is_some_and(|n| attr(n, "nodeType").is_some() && attr(n, "presetID").is_none())
            })
        });
        let auto = main_id.is_some_and(|id| auto_start(group, id));
        if canonical {
            if let Some(nested) = nested {
                groups.extend(
                    nested
                        .children
                        .iter()
                        .enumerate()
                        .map(|(i, c)| (c, auto && i == 0)),
                );
            }
        } else {
            groups.push((group, auto));
        }
    }
    for (group, auto) in groups {
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
            source_effects(n, 0, part, &mut effects, &mut result.opaque, &volumes)?;
            if !effects.is_empty() {
                previous_end = effects_end(&effects);
                result.timeline.stages.push(Stage {
                    trigger: if auto {
                        Trigger::OnSlideStart
                    } else {
                        Trigger::OnClick
                    },
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
        let mut trigger = match node_type {
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
        // The first stage of an auto-start group starts with the slide. Its delay is
        // absolute, like any OnSlideStart stage, so it is not measured from a previous stage.
        if auto {
            if cond.is_some_and(|c| child(c, "tn").is_some()) {
                result
                    .opaque
                    .push(opaque(n, "auto-start stage refers to another stage"));
                continue;
            }
            trigger = Trigger::OnSlideStart;
        }
        let mut stage_delay = delay(n, part)?;
        if matches!(trigger, Trigger::AfterPrevious) {
            let Some(end) = previous_end else {
                result.opaque.push(opaque(
                    n,
                    "after-previous stage follows an indefinite effect",
                ));
                continue;
            };
            stage_delay -= end;
        }
        let mut effects = Vec::new();
        if let Some(children) = child(n, "childTnLst") {
            for c in &children.children {
                source_effects(c, 0, part, &mut effects, &mut result.opaque, &volumes)?;
            }
        }
        if !effects.is_empty() {
            previous_end = effects_end(&effects);
            result.timeline.stages.push(Stage {
                trigger,
                delay_ms: stage_delay,
                effects,
            });
        }
    }
    result
        .opaque
        .extend(media_problems(&placed, &media, &result.timeline));
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

/// Canonical timing form: stage order, triggers and stage delays are kept, and
/// the effects of each stage are stable-sorted by `target` only.
///
/// Effects within one stage are concurrent, so the order of effects on different
/// targets is not semantic. Effects on the same target keep their relative order,
/// because later effects determine the final properties of that shape. Stages are
/// never merged and stage delays are never folded into effects, since
/// `WithPrevious` and `AfterPrevious` anchors depend on stage boundaries.
///
/// The result evaluates identically to the input, and canonicalizing is idempotent.
pub fn canonicalize(timeline: &Timeline) -> Timeline {
    Timeline {
        stages: timeline
            .stages
            .iter()
            .map(|s| {
                let mut effects = s.effects.clone();
                // `sort_by_key` is stable: same-target order is preserved.
                effects.sort_by_key(|e| e.target);
                Stage {
                    trigger: s.trigger.clone(),
                    delay_ms: s.delay_ms,
                    effects,
                }
            })
            .collect(),
    }
}

/// Compares two parsed timings by their canonical form and transition.
///
/// Returns `None` when either side has opaque timing, because equivalence is
/// unknown in that case rather than false. Otherwise returns `Some(true)` exactly
/// when the canonical timelines and the transitions are equal.
pub fn equivalent(a: &ParsedTiming, b: &ParsedTiming) -> Option<bool> {
    if !a.opaque.is_empty() || !b.opaque.is_empty() {
        return None;
    }
    Some(canonicalize(&a.timeline) == canonicalize(&b.timeline) && a.transition == b.transition)
}
