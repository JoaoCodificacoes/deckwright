use serde_json::{Value, json};
use std::{
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

const CONTENT_TYPES: &str = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Default Extension="rels" ContentType="application/xml"/></Types>"#;
const PRESENTATION: &str = r#"<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:sldIdLst><p:sldId id="256" r:id="rId1"/><p:sldId id="257" r:id="rId2"/></p:sldIdLst><p:sldSz cx="12192000" cy="6858000"/></p:presentation>"#;
const RELS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="slide" Target="slides/slide1.xml"/><Relationship Id="rId2" Type="slide" Target="slides/slide2.xml"/></Relationships>"#;

fn slide(name: &str) -> String {
    format!(
        r#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:sp><p:nvSpPr><p:cNvPr id="2" name="{name}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr></p:sp></p:spTree></p:cSld></p:sld>"#
    )
}

fn write_deck(path: &Path) {
    let parts = [
        ("[Content_Types].xml", CONTENT_TYPES.to_string()),
        ("ppt/presentation.xml", PRESENTATION.to_string()),
        ("ppt/_rels/presentation.xml.rels", RELS.to_string()),
        ("ppt/slides/slide1.xml", slide("Box")),
        ("ppt/slides/slide2.xml", slide("!!a")),
    ];
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content) in parts {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(content.as_bytes()).unwrap();
    }
    fs::write(path, zip.finish().unwrap().into_inner()).unwrap();
}

fn workdir(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "deckwright-serve-cli-{}-{test}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn serve_answers_each_line_in_order_and_exits_zero() {
    let dir = workdir("ordered");
    let path = dir.join("deck.pptx");
    write_deck(&path);
    let requests = [
        json!({"id": 1, "op": "open", "path": path.to_str().unwrap()}).to_string(),
        json!({"id": 2, "op": "state", "handle": "h1", "slide": 1, "at_ms": 0}).to_string(),
        json!({"id": 3, "op": "check", "handle": "h1"}).to_string(),
    ];

    let mut child = Command::new(env!("CARGO_BIN_EXE_dw"))
        .args(["serve", "--max-open", "2"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        for request in &requests {
            writeln!(stdin, "{request}").unwrap();
        }
    }
    let out = child.wait_with_output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let text = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 3, "{text}");
    for (line, id) in lines.iter().zip([1, 2, 3]) {
        assert_eq!(line["id"], id);
        assert_eq!(line["ok"], true, "{line}");
    }
    assert_eq!(lines[0]["result"]["handle"], "h1");
    assert_eq!(lines[1]["result"]["report"]["schema"], "dw.query.state/1");
    assert_eq!(lines[2]["result"]["report"]["schema"], "dw.check/1");

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn max_open_must_be_a_positive_integer() {
    for args in [
        vec!["serve", "--max-open", "0"],
        vec!["serve", "--max-open", "many"],
        vec!["serve", "--bogus"],
    ] {
        let out: Output = Command::new(env!("CARGO_BIN_EXE_dw"))
            .args(&args)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1), "{args:?} {out:?}");
        assert!(out.stdout.is_empty(), "{args:?} {out:?}");
        assert!(
            String::from_utf8_lossy(&out.stderr).starts_with("dw: "),
            "{args:?} {out:?}"
        );
    }
}
