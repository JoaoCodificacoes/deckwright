//! Open once, snapshot once, then evaluate explicit slide-entry event schedules.
use crate::{
    Error, Limits, Package, Result,
    timing::*,
    xml::{self, Node},
};
use serde::Serialize;
use std::{collections::BTreeMap, path::Path};
const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R: &str = "{http://schemas.openxmlformats.org/officeDocument/2006/relationships}id";
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Geometry {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Debug, Serialize)]
pub struct ShapeSnapshot {
    pub id: u32,
    pub name: String,
    pub text: String,
    pub parent: Option<u32>,
    pub z_order: usize,
    pub geometry: Option<Geometry>,
    pub hidden: bool,
    pub warnings: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct SlideSnapshot {
    pub part: String,
    pub width: f64,
    pub height: f64,
    pub shapes: Vec<ShapeSnapshot>,
    pub timing: ParsedTiming,
}
pub struct QuerySession {
    slides: Vec<SlideSnapshot>,
    /// sldSz of the presentation. Kept apart from `slides` so it is known with zero slides.
    size: (f64, f64),
}
/// Playback of a media call on one shape. Only present for shapes with a media call.
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct MediaState {
    pub playing: bool,
    pub position_ms: u64,
}
#[derive(Debug, Serialize)]
pub struct ShapeState {
    pub id: u32,
    pub name: String,
    pub text: String,
    pub parent: Option<u32>,
    pub z_order: usize,
    pub visible: Option<bool>,
    pub opacity: Option<f64>,
    pub wipe_fraction: Option<f64>,
    pub geometry: Option<Geometry>,
    pub media: Option<MediaState>,
    pub warnings: Vec<String>,
}
#[derive(Debug, Serialize)]
pub struct StateReport {
    pub schema: &'static str,
    pub revision: u32,
    pub schema_version: u32,
    pub slide: usize,
    pub sample_time_ms: u64,
    pub clicks_ms: Vec<u64>,
    pub shapes: Vec<ShapeState>,
    pub warnings: Vec<String>,
}
#[derive(Debug, Serialize)]
pub struct TimelineReport {
    pub schema: &'static str,
    pub revision: u32,
    pub schema_version: u32,
    pub slide: usize,
    pub timing: ParsedTiming,
}
#[derive(Debug, Serialize)]
pub struct MorphMatch {
    pub name: String,
    pub from_id: u32,
    pub to_id: u32,
    pub delta: Option<Geometry>,
}
#[derive(Debug, Serialize)]
pub struct MorphReport {
    pub schema: &'static str,
    pub revision: u32,
    pub schema_version: u32,
    pub from: usize,
    pub to: usize,
    pub explicit_matches: Vec<MorphMatch>,
    pub fade_out: Vec<u32>,
    pub fade_in: Vec<u32>,
    pub warnings: Vec<String>,
}
fn bad(part: &str, message: impl Into<String>) -> Error {
    Error::InvalidPart {
        part: part.into(),
        message: message.into(),
    }
}
fn child<'a>(n: &'a Node, name: &str, ns: &str) -> Option<&'a Node> {
    n.children
        .iter()
        .find(|n| n.name == name && n.namespace == ns)
}
fn number(n: &Node, k: &str) -> Option<f64> {
    n.attrs.get(k)?.parse().ok()
}
/// Part a `sldId` resolves to, or None when its relationship is missing or has no internal target.
fn slide_target<'a>(package: &'a Package, id: &Node) -> Option<&'a str> {
    id.attrs
        .get(R)
        .and_then(|rid| {
            package
                .relationships()
                .iter()
                .find(|r| r.source == "ppt/presentation.xml" && r.id == *rid)
        })
        .and_then(|r| r.resolved_target.as_deref())
}
/// Slide parts in presentation order, parsed without a session. Used only where no session can be
/// built, so suppressions still resolve slide numbers. None when the presentation does not parse
/// or any `sldId` does not resolve.
pub(crate) fn slide_order(package: &Package) -> Option<Vec<String>> {
    let bytes = package.part("ppt/presentation.xml")?;
    let root = xml::parse(bytes, "ppt/presentation.xml", usize::MAX).ok()?;
    root.descendants("sldId")
        .map(|id| slide_target(package, id).map(str::to_owned))
        .collect()
}
impl QuerySession {
    pub fn from_snapshots(slides: Vec<SlideSnapshot>) -> Self {
        let size = slides.first().map_or((0.0, 0.0), |s| (s.width, s.height));
        Self { slides, size }
    }
    pub fn open(path: impl AsRef<Path>, limits: Limits) -> Result<Self> {
        Self::from_package(&Package::open(path, limits)?, limits)
    }
    pub fn from_package(package: &Package, limits: Limits) -> Result<Self> {
        let bytes = package
            .part("ppt/presentation.xml")
            .ok_or_else(|| bad("ppt/presentation.xml", "missing presentation"))?;
        let root = xml::parse(bytes, "ppt/presentation.xml", limits.max_xml_depth)?;
        let size = root.descendants("sldSz").next();
        let width = size.and_then(|n| number(n, "cx")).unwrap_or(0.0);
        let height = size.and_then(|n| number(n, "cy")).unwrap_or(0.0);
        let mut slides = Vec::new();
        for id in root.descendants("sldId") {
            let rel = slide_target(package, id)
                .ok_or_else(|| bad("ppt/presentation.xml", "unresolved slide"))?;
            let bytes = package.part(rel).ok_or_else(|| bad(rel, "missing slide"))?;
            let node = xml::parse(bytes, rel, limits.max_xml_depth)?;
            let mut shapes = Vec::new();
            if let Some(tree) = node.descendants("spTree").next() {
                snapshot_children(tree, None, None, &mut shapes);
            }
            slides.push(SlideSnapshot {
                part: rel.to_owned(),
                width,
                height,
                shapes,
                timing: crate::timing::parse_slide_timing(bytes, rel)?,
            });
        }
        Ok(Self {
            slides,
            size: (width, height),
        })
    }
    /// Number of slides in presentation order.
    pub fn slide_count(&self) -> usize {
        self.slides.len()
    }
    /// Part name of a one-based slide in presentation order.
    pub fn slide_part(&self, slide: usize) -> Result<&str> {
        Ok(&self.slide(slide)?.part)
    }
    /// Slide width and height in EMU, from the snapshot.
    pub fn slide_size(&self, slide: usize) -> Result<(f64, f64)> {
        let s = self.slide(slide)?;
        Ok((s.width, s.height))
    }
    /// sldSz of the presentation, also when there are no slides. Zero when absent or unparsable.
    pub(crate) fn presentation_size(&self) -> (f64, f64) {
        self.size
    }
    fn slide(&self, index: usize) -> Result<&SlideSnapshot> {
        index
            .checked_sub(1)
            .and_then(|i| self.slides.get(i))
            .ok_or_else(|| {
                bad(
                    "query",
                    format!("slide {index} outside 1..={}", self.slides.len()),
                )
            })
    }
    pub fn timeline(&self, slide: usize) -> Result<TimelineReport> {
        Ok(TimelineReport {
            schema: "dw.query.timeline/1",
            revision: 1,
            schema_version: 1,
            slide,
            timing: self.slide(slide)?.timing.clone(),
        })
    }
    pub fn state(
        &self,
        slide: usize,
        clicks_ms: &[u64],
        sample_time_ms: u64,
    ) -> Result<StateReport> {
        let s = self.slide(slide)?;
        if clicks_ms.windows(2).any(|w| w[0] > w[1])
            || clicks_ms.iter().any(|t| *t > sample_time_ms)
        {
            return Err(bad(
                "query",
                "click history must be ordered and no later than sample time",
            ));
        }
        let mut shapes: Vec<_> = s
            .shapes
            .iter()
            .map(|x| ShapeState {
                id: x.id,
                name: x.name.clone(),
                text: x.text.clone(),
                parent: x.parent,
                z_order: x.z_order,
                visible: Some(!x.hidden),
                opacity: Some(1.0),
                wipe_fraction: Some(1.0),
                geometry: x.geometry.clone(),
                media: None,
                warnings: x.warnings.clone(),
            })
            .collect();
        let mut click = 0;
        let mut warnings: Vec<String> = s.timing.opaque.iter().map(|o| o.reason.clone()).collect();
        let mut previous_start = Some(0_i128);
        let mut previous_end = Some(0_i128);
        let mut scheduled: Vec<(&Effect, Option<i128>)> = Vec::new();
        for stage in &s.timing.timeline.stages {
            let anchor = match stage.trigger {
                Trigger::OnSlideStart => Some(0),
                Trigger::OnClick => {
                    let t = clicks_ms.get(click).map(|t| i128::from(*t));
                    click += 1;
                    t
                }
                Trigger::WithPrevious => previous_start,
                Trigger::AfterPrevious => previous_end,
            };
            let start = anchor.map(|a| a + i128::from(stage.delay_ms));
            previous_start = start;
            previous_end = start;
            for effect in &stage.effects {
                if !shapes.iter().any(|x| x.id == effect.target) {
                    warnings.push(format!(
                        "Timing target {} is absent from slide",
                        effect.target
                    ));
                }
                let begin = start.map(|a| a + i128::from(effect.delay_ms));
                // An indefinite effect has no end, so it leaves later AfterPrevious stages unanchored.
                let end = begin
                    .zip(effect_active_ms(effect))
                    .map(|(a, d)| a + i128::from(d));
                previous_end = match (previous_end, end) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    _ => None,
                };
                scheduled.push((effect, begin));
            }
        }
        let now = i128::from(sample_time_ms);
        let mut tracks: Vec<Track> = s.shapes.iter().map(|x| Track::new(x.hidden)).collect();
        // Rule 2: only a shape's first effect in document order can hide it initially.
        for (effect, _) in &scheduled {
            let Some(i) = shapes.iter().position(|x| x.id == effect.target) else {
                continue;
            };
            let track = &mut tracks[i];
            if !track.seen {
                track.seen = true;
                if matches!(
                    effect.kind,
                    EffectKind::Appear | EffectKind::FadeIn | EffectKind::WipeIn { .. }
                ) {
                    track.visible = false;
                    track.timing_hidden = true;
                }
            }
        }
        // Rules 3-5: started effects in begin order; the stable sort keeps document order on ties.
        let mut started: Vec<(i128, &Effect)> = scheduled
            .iter()
            .filter_map(|(effect, begin)| begin.filter(|b| *b <= now).map(|b| (b, *effect)))
            .collect();
        started.sort_by_key(|(begin, _)| *begin);
        // Media playback: every shape with a media call starts idle; the latest-begun started call wins.
        let mut media: Vec<Option<MediaState>> = shapes
            .iter()
            .map(|x| {
                scheduled
                    .iter()
                    .any(|(e, _)| {
                        e.target == x.id && matches!(e.kind, EffectKind::MediaPlay { .. })
                    })
                    .then_some(MediaState {
                        playing: false,
                        position_ms: 0,
                    })
            })
            .collect();
        for (begin, effect) in started {
            let Some(i) = shapes.iter().position(|x| x.id == effect.target) else {
                continue;
            };
            let track = &mut tracks[i];
            let active = now < begin + i128::from(effect.duration_ms);
            let p = progress(begin, effect, now);
            if let EffectKind::MediaPlay { from_ms, .. } = effect.kind {
                let elapsed = (now - begin).min(i128::from(effect.duration_ms));
                media[i] = Some(MediaState {
                    playing: active,
                    position_ms: u64::from(from_ms) + elapsed as u64,
                });
            }
            match effect.kind {
                EffectKind::Appear | EffectKind::FadeIn | EffectKind::WipeIn { .. } => {
                    track.visible = true;
                    track.timing_hidden = false;
                }
                EffectKind::FadeOut => {
                    track.visible = active;
                    track.timing_hidden = !active;
                }
                EffectKind::Motion { .. }
                | EffectKind::Scale { .. }
                | EffectKind::MediaPlay { .. } => {}
            }
            if active {
                match effect.kind {
                    EffectKind::FadeIn => track.fade = Some(p),
                    EffectKind::FadeOut => track.fade = Some(1.0 - p),
                    EffectKind::WipeIn { .. } => track.wipe = p,
                    _ => {}
                }
            }
        }
        for (shape, track) in shapes.iter_mut().zip(&tracks) {
            shape.visible = Some(track.visible);
            shape.opacity = Some(if track.timing_hidden {
                0.0
            } else {
                track.fade.unwrap_or(1.0)
            });
            shape.wipe_fraction = Some(track.wipe);
        }
        for (shape, m) in shapes.iter_mut().zip(media) {
            shape.media = m;
        }
        // Rule 6: started motion is additive and applied in document order; pending motion does nothing.
        for (effect, begin) in &scheduled {
            if let EffectKind::Motion {
                from_x,
                from_y,
                to_x,
                to_y,
                accelerate,
                decelerate,
            } = &effect.kind
                && let Some(shape) = shapes.iter_mut().find(|x| x.id == effect.target)
            {
                let Some(b) = begin.filter(|b| *b <= now) else {
                    continue;
                };
                let p = ease(progress(b, effect, now), *accelerate, *decelerate);
                if let Some(g) = shape.geometry.as_mut() {
                    g.x += (from_x + (to_x - from_x) * p - to_x) * s.width;
                    g.y += (from_y + (to_y - from_y) * p - to_y) * s.height;
                }
            }
        }
        // Started scales run after motion, about the shape's current centre. They multiply, so
        // their order does not change the result.
        for (effect, begin) in &scheduled {
            let Some(b) = begin.filter(|b| *b <= now) else {
                continue;
            };
            let Some((fx, fy)) = scale_factors(effect, now - b) else {
                continue;
            };
            if let Some(shape) = shapes.iter_mut().find(|x| x.id == effect.target)
                && let Some(g) = shape.geometry.as_mut()
            {
                g.x += g.width * (1.0 - fx) / 2.0;
                g.y += g.height * (1.0 - fy) / 2.0;
                g.width *= fx;
                g.height *= fy;
            }
        }
        for opaque in &s.timing.opaque {
            for shape in &mut shapes {
                if opaque.targets.is_empty() || opaque.targets.contains(&shape.id) {
                    shape.visible = None;
                    shape.opacity = None;
                    shape.wipe_fraction = None;
                    shape.geometry = None;
                    shape.media = None;
                    shape.warnings.push(opaque.reason.clone());
                }
            }
        }
        // Snapshot order is parent-before-child, so inherited uncertainty propagates.
        for i in 0..shapes.len() {
            if let Some(parent) = shapes[i].parent
                && let Some(p) = shapes.iter().position(|x| x.id == parent)
            {
                shapes[i].visible = match (shapes[p].visible, shapes[i].visible) {
                    (Some(false), _) | (_, Some(false)) => Some(false),
                    (Some(true), v) => v,
                    _ => None,
                };
                if shapes[p].geometry.is_none() {
                    shapes[i].geometry = None;
                    shapes[i]
                        .warnings
                        .push("parent transform is unknown".into());
                } else if let Some(original) = s
                    .shapes
                    .iter()
                    .find(|x| x.id == parent)
                    .and_then(|x| x.geometry.as_ref())
                {
                    let movement = shapes[p]
                        .geometry
                        .as_ref()
                        .map(|g| (g.x - original.x, g.y - original.y));
                    if let (Some(g), Some((dx, dy))) = (shapes[i].geometry.as_mut(), movement) {
                        g.x += dx;
                        g.y += dy;
                    }
                }
                shapes[i].opacity = match (shapes[p].opacity, shapes[i].opacity) {
                    (Some(a), Some(b)) => Some(a * b),
                    _ => None,
                };
            }
        }
        Ok(StateReport {
            schema: "dw.query.state/1",
            revision: 1,
            schema_version: 1,
            slide,
            sample_time_ms,
            clicks_ms: clicks_ms.to_vec(),
            shapes,
            warnings,
        })
    }
    pub fn morph(&self, from: usize, to: usize) -> Result<MorphReport> {
        let a = self.slide(from)?;
        let b = self.slide(to)?;
        let mut left = BTreeMap::<&str, Vec<&ShapeSnapshot>>::new();
        let mut right = BTreeMap::<&str, Vec<&ShapeSnapshot>>::new();
        for (s, map) in [(a, &mut left), (b, &mut right)] {
            for shape in &s.shapes {
                if shape.name.starts_with("!!") {
                    map.entry(&shape.name).or_default().push(shape);
                }
            }
        }
        let mut report=MorphReport{schema:"dw.query.morph/1",revision:1,schema_version:1,from,to,explicit_matches:vec![],fade_out:vec![],fade_in:vec![],warnings:vec!["Automatic matching is unknown; fade candidates are not an exact PowerPoint playback prediction".into()]};
        if !matches!(b.timing.transition, Transition::Morph { .. }) {
            report
                .warnings
                .push("Destination does not declare a Morph transition".into());
        }
        for (label, map) in [("source", &left), ("destination", &right)] {
            for (name, shapes) in map {
                if shapes.len() > 1 {
                    report.warnings.push(format!(
                        "Duplicate explicit Morph name {name} on {label}; pairing is ambiguous"
                    ));
                }
            }
        }
        for (name, ls) in &left {
            if let Some(rs) = right.get(name) {
                if ls.len() == 1 && rs.len() == 1 {
                    let (x, y) = (ls[0], rs[0]);
                    let delta =
                        x.geometry
                            .as_ref()
                            .zip(y.geometry.as_ref())
                            .map(|(x, y)| Geometry {
                                x: y.x - x.x,
                                y: y.y - x.y,
                                width: y.width - x.width,
                                height: y.height - x.height,
                            });
                    report.explicit_matches.push(MorphMatch {
                        name: (*name).into(),
                        from_id: x.id,
                        to_id: y.id,
                        delta,
                    });
                } else {
                    report.warnings.push(format!(
                        "Duplicate explicit Morph name {name}; pairing is ambiguous"
                    ));
                }
            }
        }
        for x in &a.shapes {
            if !report.explicit_matches.iter().any(|m| m.from_id == x.id) {
                report.fade_out.push(x.id);
            }
        }
        for x in &b.shapes {
            if !report.explicit_matches.iter().any(|m| m.to_id == x.id) {
                report.fade_in.push(x.id);
            }
        }
        Ok(report)
    }
}
/// Per-shape visibility state accumulated while applying started effects.
struct Track {
    visible: bool,
    /// True when visibility or opacity is withheld by timing (rule 2 or a finished fade-out).
    timing_hidden: bool,
    seen: bool,
    fade: Option<f64>,
    wipe: f64,
}
impl Track {
    fn new(hidden: bool) -> Self {
        Self {
            visible: !hidden,
            timing_hidden: false,
            seen: false,
            fade: None,
            wipe: 1.0,
        }
    }
}
/// Progress of an effect that has started (`begin <= now`), clamped to `[0, 1]`.
fn progress(begin: i128, effect: &Effect, now: i128) -> f64 {
    if effect.duration_ms == 0 {
        1.0
    } else {
        ((now - begin) as f64 / f64::from(effect.duration_ms)).clamp(0.0, 1.0)
    }
}
/// Factors (x, y) of a Scale effect `elapsed` ms after its begin. Within an iteration the forward
/// progress is local/d, and with `auto_reverse` the second half runs back down. A finite effect
/// past its active end holds the value just before that end (fill hold), so an exact boundary
/// belongs to the iteration that just finished. Otherwise a boundary restarts at the first
/// factor, which matches `t mod iteration`.
fn scale_factors(effect: &Effect, elapsed: i128) -> Option<(f64, f64)> {
    let EffectKind::Scale {
        x, y, auto_reverse, ..
    } = &effect.kind
    else {
        return None;
    };
    let d = i128::from(effect.duration_ms);
    let iteration = if *auto_reverse { 2 * d } else { d };
    let (t, held) = match effect_active_ms(effect).map(i128::from) {
        Some(active) if elapsed >= active => (active, true),
        _ => (elapsed, false),
    };
    let local = if held && t > 0 && t % iteration == 0 {
        iteration
    } else {
        t % iteration
    } as f64;
    let d = d as f64;
    let forward = if *auto_reverse && local > d {
        1.0 - (local - d) / d
    } else {
        local / d
    };
    Some((1.0 + (x - 1.0) * forward, 1.0 + (y - 1.0) * forward))
}
fn ease(p: f64, a: f64, d: f64) -> f64 {
    let speed = 1.0 / (1.0 - (a + d) / 2.0);
    if a > 0.0 && p < a {
        speed * p * p / (2.0 * a)
    } else if d > 0.0 && p > 1.0 - d {
        1.0 - speed * (1.0 - p) * (1.0 - p) / (2.0 * d)
    } else {
        speed * (p - a / 2.0)
    }
}
fn snapshot_children(
    tree: &Node,
    parent: Option<u32>,
    parent_map: Option<(f64, f64, f64, f64)>,
    out: &mut Vec<ShapeSnapshot>,
) {
    for n in &tree.children {
        if n.namespace != P
            || !matches!(
                n.name.as_str(),
                "sp" | "pic" | "grpSp" | "cxnSp" | "graphicFrame"
            )
        {
            continue;
        }
        let nv = n
            .children
            .iter()
            .find(|c| c.namespace == P && c.name.starts_with("nv"));
        let meta = nv.and_then(|n| child(n, "cNvPr", P));
        let Some(id) = meta
            .and_then(|n| n.attrs.get("id"))
            .and_then(|s| s.parse().ok())
        else {
            continue;
        };
        let props = child(n, if n.name == "grpSp" { "grpSpPr" } else { "spPr" }, P);
        let xfrm = props
            .and_then(|n| child(n, "xfrm", A))
            .or_else(|| child(n, "xfrm", P));
        let mut warnings = Vec::new();
        let mut geometry = xfrm.and_then(|x| {
            let off = child(x, "off", A)?;
            let ext = child(x, "ext", A)?;
            Some(Geometry {
                x: number(off, "x")?,
                y: number(off, "y")?,
                width: number(ext, "cx")?,
                height: number(ext, "cy")?,
            })
        });
        let unsupported = xfrm.is_some_and(|x| {
            x.attrs.get("rot").is_some_and(|r| r != "0")
                || x.attrs.get("flipH").is_some_and(|v| v != "0")
                || x.attrs.get("flipV").is_some_and(|v| v != "0")
        });
        if unsupported {
            geometry = None;
            warnings.push("rotated or flipped geometry is unresolved".into());
        }
        if let (Some(g), Some((sx, sy, tx, ty))) = (&mut geometry, parent_map) {
            g.x = g.x * sx + tx;
            g.y = g.y * sy + ty;
            g.width *= sx;
            g.height *= sy;
        }
        if parent.is_some() && parent_map.is_none() {
            geometry = None;
            warnings.push("parent coordinate mapping is unresolved".into());
        }
        if geometry.is_none() {
            warnings.push("geometry or placeholder inheritance is unresolved".into());
        }
        let child_map = if n.name == "grpSp" {
            xfrm.and_then(|x| {
                let off = child(x, "chOff", A)?;
                let ext = child(x, "chExt", A)?;
                let g = geometry.as_ref()?;
                let (cx, cy) = (number(ext, "cx")?, number(ext, "cy")?);
                if cx == 0.0 || cy == 0.0 {
                    return None;
                }
                let (sx, sy) = (g.width / cx, g.height / cy);
                Some((
                    sx,
                    sy,
                    g.x - number(off, "x")? * sx,
                    g.y - number(off, "y")? * sy,
                ))
            })
        } else {
            None
        };
        out.push(ShapeSnapshot {
            id,
            name: meta
                .and_then(|n| n.attrs.get("name"))
                .cloned()
                .unwrap_or_default(),
            text: n
                .descendants("t")
                .map(|n| n.text.as_str())
                .collect::<Vec<_>>()
                .join(""),
            parent,
            z_order: out.len(),
            geometry,
            hidden: meta
                .and_then(|n| n.attrs.get("hidden"))
                .is_some_and(|v| v == "1" || v == "true"),
            warnings,
        });
        if n.name == "grpSp" {
            snapshot_children(n, Some(id), child_map, out);
        }
    }
}
