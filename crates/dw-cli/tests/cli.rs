use std::{fs, process::Command};
#[test]
fn inspect_roundtrip_and_output_protection() {
    let dir = std::env::temp_dir().join(format!("deckwright-cli-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let input = dir.join("input.zip");
    let output = dir.join("output.zip");
    fs::write(
        &input,
        include_bytes!("../../../fixtures/generated/package-preservation.zip"),
    )
    .unwrap();
    let inspect = Command::new(env!("CARGO_BIN_EXE_dw"))
        .args(["inspect", input.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    assert!(
        inspect.status.success(),
        "{}",
        String::from_utf8_lossy(&inspect.stderr)
    );
    let i: serde_json::Value = serde_json::from_slice(&inspect.stdout).unwrap();
    assert_eq!(i["schema_version"], 1);
    assert_eq!(i["package"]["part_count"], 3);
    let roundtrip = || {
        Command::new(env!("CARGO_BIN_EXE_dw"))
            .args([
                "roundtrip",
                input.to_str().unwrap(),
                "--output",
                output.to_str().unwrap(),
            ])
            .output()
            .unwrap()
    };
    assert!(roundtrip().status.success());
    let before = fs::read(&output).unwrap();
    assert!(!roundtrip().status.success());
    assert_eq!(fs::read(&output).unwrap(), before);
    let p = deckwright::Package::open(&output, deckwright::Limits::default()).unwrap();
    assert_eq!(p.part("media/payload.bin").unwrap(), [0, 255, 1, 2]);
    fs::remove_dir_all(dir).unwrap();
}
