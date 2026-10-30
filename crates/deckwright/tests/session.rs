use deckwright::session::Server;
use serde_json::{Value, json};
use std::{
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    process,
};

const CONTENT_TYPES: &str = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Default Extension="rels" ContentType="application/xml"/></Types>"#;
const PRESENTATION: &str = r#"<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:sldIdLst><p:sldId id="256" r:id="rId1"/><p:sldId id="257" r:id="rId2"/></p:sldIdLst><p:sldSz cx="12192000" cy="6858000"/></p:presentation>"#;
const RELS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="slide" Target="slides/slide1.xml"/><Relationship Id="rId2" Type="slide" Target="slides/slide2.xml"/></Relationships>"#;

fn slide(name: &str) -> String {
    format!(
        r#"<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:sp><p:nvSpPr><p:cNvPr id="2" name="{name}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr></p:sp></p:spTree></p:cSld></p:sld>"#
    )
}

/// Writes a two-slide package. `box_name` sets slide 1's shape name, so a rewrite
/// with a different value also changes the file length.
fn write_deck(path: &Path, box_name: &str) {
    let parts = [
        ("[Content_Types].xml", CONTENT_TYPES.to_string()),
        ("ppt/presentation.xml", PRESENTATION.to_string()),
        ("ppt/_rels/presentation.xml.rels", RELS.to_string()),
        ("ppt/slides/slide1.xml", slide(box_name)),
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
    let dir = std::env::temp_dir().join(format!("deckwright-session-{}-{test}", process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn send(server: &mut Server, request: Value) -> Value {
    let line = server.handle_line(&request.to_string());
    assert!(!line.contains('\n'), "response must be one line: {line}");
    serde_json::from_str(&line).unwrap()
}

fn open(server: &mut Server, id: u64, path: &Path) -> Value {
    send(
        server,
        json!({"id": id, "op": "open", "path": path.to_str().unwrap()}),
    )
}

fn state(server: &mut Server, id: u64, handle: &str) -> Value {
    send(
        server,
        json!({"id": id, "op": "state", "handle": handle, "slide": 1, "at_ms": 0}),
    )
}

fn error_code(response: &Value) -> &str {
    assert_eq!(response["ok"], false, "{response}");
    response["error"]["code"].as_str().unwrap()
}

#[test]
fn open_then_query_ops_return_versioned_envelopes() {
    let dir = workdir("envelopes");
    let path = dir.join("deck.pptx");
    write_deck(&path, "Box");
    let mut server = Server::new(8);

    let opened = open(&mut server, 1, &path);
    assert_eq!(opened["id"], 1);
    assert_eq!(opened["ok"], true);
    assert_eq!(opened["result"]["handle"], "h1");
    assert_eq!(opened["result"]["revision"], 1);
    assert_eq!(opened["result"]["slides"], 2);

    let st = state(&mut server, 2, "h1");
    assert_eq!(st["ok"], true);
    assert_eq!(st["result"]["handle"], "h1");
    assert_eq!(st["result"]["revision"], 1);
    assert_eq!(st["result"]["report"]["schema"], "dw.query.state/1");

    let timeline = send(
        &mut server,
        json!({"id": 3, "op": "timeline", "handle": "h1", "slide": 1}),
    );
    assert_eq!(timeline["result"]["revision"], 1);
    assert_eq!(
        timeline["result"]["report"]["schema"],
        "dw.query.timeline/1"
    );

    let morph = send(
        &mut server,
        json!({"id": 4, "op": "morph", "handle": "h1", "from": 1, "to": 2}),
    );
    assert_eq!(morph["result"]["report"]["schema"], "dw.query.morph/1");

    let check = send(&mut server, json!({"id": 5, "op": "check", "handle": "h1"}));
    assert_eq!(check["ok"], true);
    assert_eq!(check["result"]["handle"], "h1");
    assert_eq!(check["result"]["revision"], 1);
    assert_eq!(check["result"]["report"]["schema"], "dw.check/1");

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn malformed_requests_are_parse_or_unknown_op_errors_with_echoed_ids() {
    let mut server = Server::new(8);

    let bad_json: Value = serde_json::from_str(&server.handle_line("{not json")).unwrap();
    assert_eq!(bad_json["id"], Value::Null);
    assert_eq!(error_code(&bad_json), "parse");

    let not_object: Value = serde_json::from_str(&server.handle_line("[1,2]")).unwrap();
    assert_eq!(not_object["id"], Value::Null);
    assert_eq!(error_code(&not_object), "parse");

    let unknown = send(&mut server, json!({"id": 7, "op": "frobnicate"}));
    assert_eq!(unknown["id"], 7);
    assert_eq!(error_code(&unknown), "unknown-op");

    let no_op = send(&mut server, json!({"id": 9}));
    assert_eq!(error_code(&no_op), "parse");

    let missing_arg = send(
        &mut server,
        json!({"id": 8, "op": "state", "handle": "h1", "slide": 1}),
    );
    assert_eq!(missing_arg["id"], 8);
    assert_eq!(error_code(&missing_arg), "parse");

    let wrong_type = send(
        &mut server,
        json!({"id": 10, "op": "state", "handle": "h1", "slide": "one", "at_ms": 0}),
    );
    assert_eq!(error_code(&wrong_type), "parse");
}

#[test]
fn never_issued_and_closed_handles_are_invalid() {
    let dir = workdir("invalid-handle");
    let path = dir.join("deck.pptx");
    write_deck(&path, "Box");
    let mut server = Server::new(8);
    open(&mut server, 1, &path);

    assert_eq!(error_code(&state(&mut server, 2, "h9")), "invalid-handle");

    let closed = send(&mut server, json!({"id": 3, "op": "close", "handle": "h1"}));
    assert_eq!(closed["result"]["closed"], true);
    assert_eq!(error_code(&state(&mut server, 4, "h1")), "invalid-handle");

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn out_of_range_slide_is_query_error_and_missing_path_is_io() {
    let dir = workdir("query-io");
    let path = dir.join("deck.pptx");
    write_deck(&path, "Box");
    let mut server = Server::new(8);
    open(&mut server, 1, &path);

    let out_of_range = send(
        &mut server,
        json!({"id": 2, "op": "state", "handle": "h1", "slide": 9, "at_ms": 0}),
    );
    assert_eq!(error_code(&out_of_range), "query");
    assert_eq!(
        error_code(&open(&mut server, 3, &dir.join("missing.pptx"))),
        "io"
    );

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn rewritten_file_is_stale_until_refresh_bumps_revision() {
    let dir = workdir("stale-refresh");
    let path = dir.join("deck.pptx");
    write_deck(&path, "Box");
    let mut server = Server::new(8);
    open(&mut server, 1, &path);

    // A longer name changes the length, so the metadata differs.
    write_deck(&path, "Box with a much longer name");
    assert_eq!(error_code(&state(&mut server, 2, "h1")), "stale");

    let refreshed = send(
        &mut server,
        json!({"id": 3, "op": "refresh", "handle": "h1"}),
    );
    assert_eq!(refreshed["result"]["handle"], "h1");
    assert_eq!(refreshed["result"]["revision"], 2);
    assert_eq!(refreshed["result"]["slides"], 2);

    let after = state(&mut server, 4, "h1");
    assert_eq!(after["ok"], true);
    assert_eq!(after["result"]["revision"], 2);
    assert_eq!(after["result"]["report"]["revision"], 2);

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn deleted_file_is_stale() {
    let dir = workdir("deleted");
    let path = dir.join("deck.pptx");
    write_deck(&path, "Box");
    let mut server = Server::new(8);
    open(&mut server, 1, &path);

    fs::remove_file(&path).unwrap();
    assert_eq!(error_code(&state(&mut server, 2, "h1")), "stale");
    let check = send(&mut server, json!({"id": 3, "op": "check", "handle": "h1"}));
    assert_eq!(error_code(&check), "stale");

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn least_recently_used_handle_is_evicted_and_handles_are_not_reused() {
    let dir = workdir("lru");
    let [a, b, c] = ["a", "b", "c"].map(|name| dir.join(format!("{name}.pptx")));
    for path in [&a, &b, &c] {
        write_deck(path, "Box");
    }
    let mut server = Server::new(2);

    assert_eq!(open(&mut server, 1, &a)["result"]["handle"], "h1");
    assert_eq!(open(&mut server, 2, &b)["result"]["handle"], "h2");
    assert_eq!(state(&mut server, 3, "h1")["ok"], true);
    assert_eq!(open(&mut server, 4, &c)["result"]["handle"], "h3");

    assert_eq!(error_code(&state(&mut server, 5, "h2")), "evicted");
    assert_eq!(state(&mut server, 6, "h1")["ok"], true);
    assert_eq!(state(&mut server, 7, "h3")["ok"], true);
    assert_eq!(open(&mut server, 8, &a)["result"]["handle"], "h4");

    fs::remove_dir_all(dir).unwrap();
}
