use std::{fs, process::Command};

#[test]
fn check_reports_json_and_uses_error_exit_status() {
    let dir = std::env::temp_dir().join(format!("deckwright-check-cli-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let input = dir.join("neutral.zip");
    fs::write(
        &input,
        include_bytes!("../../../fixtures/generated/package-preservation.zip"),
    )
    .unwrap();
    let check = |path: &std::path::Path| {
        Command::new(env!("CARGO_BIN_EXE_dw"))
            .arg("check")
            .arg(path)
            .arg("--json")
            .output()
            .unwrap()
    };
    let clean = check(&input);
    assert!(
        clean.status.success(),
        "{}",
        String::from_utf8_lossy(&clean.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&clean.stdout).unwrap();
    assert_eq!(report["schema"], "dw.check/1");
    assert_eq!(report["imported"], true);
    let missing = check(&dir.join("missing.pptx"));
    assert!(!missing.status.success());
    let report: serde_json::Value = serde_json::from_slice(&missing.stdout).unwrap();
    assert_eq!(report["schema"], "dw.check/1");
    assert_eq!(report["imported"], false);
    assert!(
        report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["severity"] == "error")
    );
    fs::remove_dir_all(dir).unwrap();
}
