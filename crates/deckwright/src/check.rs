use crate::{Limits, Package, xml};
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
        slide: Option<&str>,
        shape: Option<&str>,
        message: String,
        hint: &str,
    ) {
        self.findings.push(Finding {
            rule: rule.into(),
            severity: if rule == "duplicate-morph-name" {
                "warning"
            } else {
                "error"
            }
            .into(),
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
    for name in package
        .part_names()
        .filter(|n| n.starts_with("ppt/slides/") && !n.contains("/_rels/") && n.ends_with(".xml"))
    {
        // Import has already validated XML with the caller's limits. A repeat parse
        // uses unlimited depth so custom import limits remain supported.
        let root = match xml::parse(package.part(name).unwrap_or_default(), name, usize::MAX) {
            Ok(root) => root,
            Err(error) => {
                report.add(
                    "slide-xml",
                    Some(name),
                    None,
                    error.to_string(),
                    "Repair slide XML.",
                );
                continue;
            }
        };
        let mut ids = BTreeSet::new();
        let mut morph_names = BTreeSet::new();
        for node in root.descendants("cNvPr") {
            if let Some(id) = node.attrs.get("id") {
                if !ids.insert(shape_key(id)) {
                    report.add(
                        "duplicate-shape-id",
                        Some(name),
                        Some(id),
                        format!("Shape ID {id} appears more than once."),
                        "Assign unique shape IDs within this slide and update references.",
                    );
                }
            }
            if let Some(label) = node.attrs.get("name").filter(|s| s.starts_with("!!")) {
                if !morph_names.insert(label.clone()) {
                    report.add(
                        "duplicate-morph-name",
                        Some(name),
                        node.attrs.get("id").map(String::as_str),
                        format!("Explicit Morph name {label} appears more than once."),
                        "Use a unique explicit Morph name for each shape within this slide.",
                    );
                }
            }
        }
        for target in root.descendants("spTgt") {
            if let Some(id) = target.attrs.get("spid") {
                if !ids.contains(&shape_key(id)) {
                    report.add("missing-timing-target", Some(name), Some(id), format!("Timing references absent shape ID {id}."), "Point the timing target at an existing slide shape or remove the stale timing node.");
                }
            }
        }
    }
    report
}

fn shape_key(id: &str) -> String {
    // OOXML drawing IDs have unsigned-integer value semantics.
    id.trim()
        .parse::<u32>()
        .map(|value| value.to_string())
        .unwrap_or_else(|_| id.to_owned())
}
