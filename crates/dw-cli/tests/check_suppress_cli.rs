use deckwright::{Limits, Package, template, units::Emu};
use std::{
    collections::BTreeMap,
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";

fn raw_slide(body: &str) -> Vec<u8> {
    format!("<p:sld xmlns:p=\"{P}\" xmlns:a=\"{A}\"><p:cSld><p:spTree>{body}</p:spTree></p:cSld></p:sld>")
        .into_bytes()
}

fn sp(id: u32, name: &str, x: i64) -> String {
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"{name}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"{x}\" y=\"914400\"/><a:ext cx=\"1828800\" cy=\"914400\"/></a:xfrm></p:spPr></p:sp>"
    )
}

/// 12192000 EMU wide. Slide 1 has an off-slide shape (2); slide 2 has a duplicate Morph name (3).
fn write_deck(dir: &Path, slide1: Vec<u8>, slide2: Vec<u8>) -> PathBuf {
    let replacements = BTreeMap::from([
        ("ppt/slides/slide1.xml".to_owned(), slide1),
        ("ppt/slides/slide2.xml".to_owned(), slide2),
    ]);
    let deck: Package = template::blank(2, Emu(12_192_000), Emu(6_858_000))
        .unwrap()
        .edited(replacements, &BTreeSet::new(), Limits::default())
        .unwrap();
    let path = dir.join("deck.pptx");
    deck.write(&path).unwrap();
    path
}

fn warned_deck(dir: &Path) -> PathBuf {
    write_deck(
        dir,
        raw_slide(&sp(2, "Late", 13_000_000)),
        raw_slide(&format!(
            "{}{}",
            sp(2, "!!same", 1_000_000),
            sp(3, "!!same", 4_000_000)
        )),
    )
}

fn duplicate_id_deck(dir: &Path) -> PathBuf {
    write_deck(
        dir,
        raw_slide(&format!(
            "{}{}",
            sp(2, "a", 1_000_000),
            sp(2, "b", 4_000_000)
        )),
        raw_slide(&sp(2, "c", 1_000_000)),
    )
}

fn check(input: &PathBuf, extra: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_dw"))
        .arg("check")
        .arg(input)
        .arg("--json")
        .args(extra)
        .output()
        .unwrap()
}

fn dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn repeated_suppress_flags_move_each_warning() {
    let dir = dir("deckwright-suppress-cli-multi");
    let input = warned_deck(&dir);
    let output = check(
        &input,
        &[
            "--suppress",
            "off-slide",
            "--suppress",
            "duplicate-morph-name@2:3",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["schema"], "dw.check/1");
    assert_eq!(report["findings"], serde_json::json!([]));
    assert_eq!(report["suppressed"].as_array().unwrap().len(), 2);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn non_matching_scope_keeps_the_warning_and_reports_it_unused() {
    let dir = dir("deckwright-suppress-cli-unused");
    let input = warned_deck(&dir);
    let output = check(&input, &["--suppress", "off-slide@2"]);
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["suppressed"], serde_json::json!([]));
    assert_eq!(report["findings"].as_array().unwrap().len(), 2);
    let limitations = report["limitations"].as_array().unwrap();
    assert!(
        limitations
            .iter()
            .any(|l| l.as_str().unwrap().contains("is unused")),
        "{limitations:?}"
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn suppressing_an_error_keeps_it_and_exits_one() {
    let dir = dir("deckwright-suppress-cli-error");
    let input = duplicate_id_deck(&dir);
    let output = check(&input, &["--suppress", "duplicate-shape-id"]);
    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["suppressed"], serde_json::json!([]));
    assert!(
        report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["rule"] == "duplicate-shape-id" && f["severity"] == "error")
    );
    assert!(
        report["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l.as_str().unwrap().contains("did not apply")),
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn malformed_suppress_values_are_usage_errors() {
    let dir = dir("deckwright-suppress-cli-malformed");
    let input = warned_deck(&dir);
    for value in [
        "@",
        "@x",
        "rule@1:",
        "rule@",
        "rule@0",
        "rule@x",
        "rule:3",
        "rule@1:2:3",
        "",
    ] {
        let output = check(&input, &["--suppress", value]);
        assert_eq!(output.status.code(), Some(1), "{value:?}");
        assert!(output.stdout.is_empty(), "{value:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.starts_with("dw: "), "{value:?}: {stderr}");
    }
    let missing_value = Command::new(env!("CARGO_BIN_EXE_dw"))
        .arg("check")
        .arg(&input)
        .arg("--json")
        .arg("--suppress")
        .output()
        .unwrap();
    assert_eq!(missing_value.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&missing_value.stderr).starts_with("dw: "));
    fs::remove_dir_all(dir).unwrap();
}
