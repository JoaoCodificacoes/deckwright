use crate::{
    Limits, Package,
    query::{Geometry, QuerySession},
    timing::{self, ParsedTiming, Trigger},
    xml,
};
use serde::Serialize;
use std::{collections::BTreeSet, path::Path};

#[derive(Debug, Serialize)]
pub struct Finding {
    pub rule: String,
    pub severity: String,
    pub slide: Option<String>,
    pub shape: Option<String>,
    pub message: String,
    pub fix_hint: String,
}
#[derive(Debug, Serialize)]
pub struct CheckReport {
    pub schema: String,
    pub schema_version: u32,
    /// False means import failed and structural checks could not run.
    pub imported: bool,
    pub findings: Vec<Finding>,
    pub limitations: Vec<String>,
}
impl CheckReport {
    pub fn has_errors(&self) -> bool {
        self.findings.iter().any(|f| f.severity == "error")
    }
    fn add(
        &mut self,
        rule: &str,
        severity: &str,
        slide: Option<&str>,
        shape: Option<&str>,
        message: String,
        hint: &str,
    ) {
        self.findings.push(Finding {
            rule: rule.into(),
            severity: severity.into(),
            slide: slide.map(str::to_owned),
            shape: shape.map(str::to_owned),
            message,
            fix_hint: hint.into(),
        });
    }
}
/// Includes import failures in the versioned report; never reports a failed import as clean.
pub fn check_path(path: impl AsRef<Path>, limits: Limits) -> CheckReport {
    match Package::open(path, limits) {
        Ok(package) => check_package(&package),
        Err(error) => {
            let mut report = CheckReport {
                schema: "dw.check/1".into(),
                schema_version: 1,
                imported: false,
                findings: vec![],
                limitations: vec!["Structural rules did not run because import failed.".into()],
            };
            report.add(
                if matches!(error, crate::Error::Relationship { .. }) {
                    "relationship-import"
                } else {
                    "package-import"
                },
                "error",
                None,
                None,
                error.to_string(),
                "Repair the reported package/import error and rerun checks.",
            );
            report
        }
    }
}
/// Encoded slide-local checks. Does not evaluate Office playback or inherited geometry.
pub fn check_package(package: &Package) -> CheckReport {
    let mut report = CheckReport { schema: "dw.check/1".into(), schema_version: 1, imported: true, findings: vec![],
        limitations: vec![
            "Only transitional PresentationML namespace rules are checked; Strict OOXML and full schema validation are unknown.".into(),
            "Bounds unknown: inherited, rotated and grouped geometry is not resolved; no bounds verdict is emitted.".into(),
            "Morph checks cover duplicate explicit !! names within each slide, not matching or playback.".into(),
            "Timing checks cover encoded spTgt shape IDs only; timing semantics and playback are unknown.".into(),
        ] };
    let mut parsed_xml = BTreeSet::new();
    for name in package.part_names().filter(|n| is_slide_part(n)) {
        // Import has already validated XML with the caller's limits. A repeat parse
        // uses unlimited depth so custom import limits remain supported.
        let root = match xml::parse(package.part(name).unwrap_or_default(), name, usize::MAX) {
            Ok(root) => root,
            Err(error) => {
                report.add(
                    "slide-xml",
                    "error",
                    Some(name),
                    None,
                    error.to_string(),
                    "Repair slide XML.",
                );
                continue;
            }
        };
        parsed_xml.insert(name);
        let mut ids = BTreeSet::new();
        let mut morph_names = BTreeSet::new();
        for node in root.descendants("cNvPr") {
            if let Some(id) = node.attrs.get("id")
                && !ids.insert(shape_key(id))
            {
                report.add(
                    "duplicate-shape-id",
                    "error",
                    Some(name),
                    Some(id),
                    format!("Shape ID {id} appears more than once."),
                    "Assign unique shape IDs within this slide and update references.",
                );
            }
            if let Some(label) = node.attrs.get("name").filter(|s| s.starts_with("!!"))
                && !morph_names.insert(label.clone())
            {
                report.add(
                    "duplicate-morph-name",
                    "warning",
                    Some(name),
                    node.attrs.get("id").map(String::as_str),
                    format!("Explicit Morph name {label} appears more than once."),
                    "Use a unique explicit Morph name for each shape within this slide.",
                );
            }
        }
        for target in root.descendants("spTgt") {
            if let Some(id) = target.attrs.get("spid")
                && !ids.contains(&shape_key(id))
            {
                report.add("missing-timing-target", "error", Some(name), Some(id), format!("Timing references absent shape ID {id}."), "Point the timing target at an existing slide shape or remove the stale timing node.");
            }
        }
    }
    let bounds = match (
        QuerySession::from_package(package, Limits::default()),
        slide_layout(package),
    ) {
        (Ok(session), Some((parts, width, height))) if width > 0.0 && height > 0.0 => {
            Some(Bounds {
                session,
                parts,
                width,
                height,
            })
        }
        (Ok(_), _) => {
            report.limitations.push(
                "Bounds skipped: slide size or slide order is unavailable; no bounds findings were emitted.".into(),
            );
            None
        }
        (Err(error), _) => {
            report.limitations.push(format!(
                "Bounds skipped: the query session could not be built ({error}); no bounds findings were emitted."
            ));
            None
        }
    };
    for name in package.part_names().filter(|n| is_slide_part(n)) {
        if !parsed_xml.contains(name) {
            continue;
        }
        let start = report.findings.len();
        check_slide_timing(
            &mut report,
            name,
            package.part(name).unwrap_or_default(),
            bounds.as_ref(),
        );
        report.findings[start..].sort_by(|a, b| {
            (a.rule.as_str(), shape_number(a)).cmp(&(b.rule.as_str(), shape_number(b)))
        });
    }
    report
}

/// Timing validity, opaque timing and sampled slide bounds for one slide part.
fn check_slide_timing(report: &mut CheckReport, name: &str, bytes: &[u8], bounds: Option<&Bounds>) {
    let timing = match timing::parse_slide_timing(bytes, name) {
        Ok(timing) => timing,
        Err(error) => {
            report.add(
                "timing-parse",
                "error",
                Some(name),
                None,
                error.to_string(),
                "Repair the slide timing XML or regenerate it from supported effects.",
            );
            return;
        }
    };
    if let Err(error) = timing::validate_timeline(&timing.timeline) {
        report.add(
            "timing-invalid",
            "error",
            Some(name),
            None,
            error.to_string(),
            "Correct the timing stage, effect or motion values reported by validation.",
        );
    }
    for opaque in &timing.opaque {
        let targets = if opaque.targets.is_empty() {
            "no shape".to_owned()
        } else {
            opaque
                .targets
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        };
        report.add(
            "timing-opaque",
            "warning",
            Some(name),
            opaque.targets.first().map(u32::to_string).as_deref(),
            format!(
                "Timing for {targets} is opaque and not modeled: {}",
                opaque.reason
            ),
            "Replace unsupported timing with supported effects, or accept that these targets are unverified.",
        );
    }
    if let Some(bounds) = bounds
        && let Some(number) = bounds.parts.iter().position(|p| p == name).map(|i| i + 1)
    {
        bounds_findings(report, name, bounds, number, &timing);
    }
}

/// Samples the start and settled states; each (rule, shape) is reported at most once.
fn bounds_findings(
    report: &mut CheckReport,
    name: &str,
    bounds: &Bounds,
    number: usize,
    timing: &ParsedTiming,
) {
    let (clicks, settled_time) = settled_schedule(timing);
    let states = [
        ("at slide start", bounds.session.state(number, &[], 0)),
        (
            "in the settled final state",
            bounds.session.state(number, &clicks, settled_time),
        ),
    ];
    let mut seen = BTreeSet::new();
    for (label, state) in states {
        let state = match state {
            Ok(state) => state,
            Err(error) => {
                report.limitations.push(format!(
                    "Bounds skipped for {name}: sampling failed ({error})."
                ));
                return;
            }
        };
        for shape in &state.shapes {
            // A shape hidden in this sampled state is not checked in that state.
            if shape.visible == Some(false) {
                continue;
            }
            let Some(geometry) = &shape.geometry else {
                continue;
            };
            let Some(placement) = placement(geometry, bounds.width, bounds.height) else {
                continue;
            };
            let id = shape.id.to_string();
            if !seen.insert((placement.rule(), shape.id)) {
                continue;
            }
            let message = match placement {
                Placement::Outside => format!(
                    "Shape {id} lies entirely outside the {}x{} slide {label}.",
                    bounds.width, bounds.height
                ),
                Placement::Partial => {
                    format!("Shape {id} extends past a slide edge {label}.")
                }
            };
            report.add(
                placement.rule(),
                "warning",
                Some(name),
                Some(&id),
                message,
                "Move or resize the shape so its resolved geometry stays on the slide, or hide it in that state.",
            );
        }
    }
}

struct Bounds {
    session: QuerySession,
    /// Slide parts in QuerySession order; index + 1 is the query slide number.
    parts: Vec<String>,
    width: f64,
    height: f64,
}

#[derive(Clone, Copy, PartialEq)]
enum Placement {
    Outside,
    Partial,
}

impl Placement {
    fn rule(self) -> &'static str {
        match self {
            Placement::Outside => "off-slide",
            Placement::Partial => "partially-off-slide",
        }
    }
}

/// Tolerance in slide units, so exact edge contact survives float scaling.
const EPSILON: f64 = 1e-6;

/// None when the geometry is fully inside. Positive-area shapes are outside when they
/// have no positive-area overlap; zero-area shapes are outside only when strictly beyond an edge.
fn placement(g: &Geometry, width: f64, height: f64) -> Option<Placement> {
    let (x0, x1) = (g.x.min(g.x + g.width), g.x.max(g.x + g.width));
    let (y0, y1) = (g.y.min(g.y + g.height), g.y.max(g.y + g.height));
    if x1 - x0 > 0.0 && y1 - y0 > 0.0 {
        let overlap_x = x1.min(width) - x0.max(0.0);
        let overlap_y = y1.min(height) - y0.max(0.0);
        if overlap_x <= EPSILON || overlap_y <= EPSILON {
            return Some(Placement::Outside);
        }
    } else if x1 < -EPSILON || x0 > width + EPSILON || y1 < -EPSILON || y0 > height + EPSILON {
        return Some(Placement::Outside);
    }
    if x0 < -EPSILON || y0 < -EPSILON || x1 > width + EPSILON || y1 > height + EPSILON {
        Some(Placement::Partial)
    } else {
        None
    }
}

/// One click per OnClick stage at 1 ms spacing, sampled at last click + max effect end + 1.
/// Effect ends are measured from their click anchor, which bounds every click group.
fn settled_schedule(timing: &ParsedTiming) -> (Vec<u64>, u64) {
    let clicks = timing
        .timeline
        .stages
        .iter()
        .filter(|s| s.trigger == Trigger::OnClick)
        .count();
    let times: Vec<u64> = (0..clicks as u64).collect();
    let last = times.last().copied().unwrap_or(0);
    let (mut previous_start, mut previous_end, mut max_end) = (0_i64, 0_i64, 0_i64);
    for stage in &timing.timeline.stages {
        let anchor = match stage.trigger {
            Trigger::OnSlideStart | Trigger::OnClick => 0,
            Trigger::WithPrevious => previous_start,
            Trigger::AfterPrevious => previous_end,
        };
        let start = anchor.saturating_add(stage.delay_ms);
        previous_start = start;
        previous_end = start;
        for effect in &stage.effects {
            let end = start
                .saturating_add(effect.delay_ms)
                .saturating_add(i64::from(effect.duration_ms));
            previous_end = previous_end.max(end);
            max_end = max_end.max(end);
        }
    }
    (times, last + max_end.max(0) as u64 + 1)
}

/// Slide parts in QuerySession order, with the same sldSz source as query.rs.
fn slide_layout(package: &Package) -> Option<(Vec<String>, f64, f64)> {
    const R: &str = "{http://schemas.openxmlformats.org/officeDocument/2006/relationships}id";
    let bytes = package.part("ppt/presentation.xml")?;
    let root = xml::parse(bytes, "ppt/presentation.xml", usize::MAX).ok()?;
    let size = root.descendants("sldSz").next();
    let number = |key: &str| {
        size.and_then(|n| n.attrs.get(key))
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    let mut parts = Vec::new();
    for id in root.descendants("sldId") {
        let target = id
            .attrs
            .get(R)
            .and_then(|rid| {
                package
                    .relationships()
                    .iter()
                    .find(|r| r.source == "ppt/presentation.xml" && r.id == *rid)
            })
            .and_then(|r| r.resolved_target.clone())?;
        parts.push(target);
    }
    Some((parts, number("cx"), number("cy")))
}

fn is_slide_part(name: &str) -> bool {
    name.starts_with("ppt/slides/") && !name.contains("/_rels/") && name.ends_with(".xml")
}

fn shape_number(finding: &Finding) -> Option<u32> {
    finding.shape.as_deref().and_then(|s| s.parse().ok())
}

fn shape_key(id: &str) -> String {
    // OOXML drawing IDs have unsigned-integer value semantics.
    id.trim()
        .parse::<u32>()
        .map(|value| value.to_string())
        .unwrap_or_else(|_| id.to_owned())
}
