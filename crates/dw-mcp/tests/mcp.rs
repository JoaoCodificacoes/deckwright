use dw_mcp::McpServer;
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

// The session opens files by path, so the bytes are written to a temp file per test.
const DECK: &[u8] = include_bytes!("../../../fixtures/generated/bench/neutral.pptx");

fn deck_path(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-mcp-{}-{test}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("deck.pptx");
    fs::write(&path, DECK).unwrap();
    path
}

fn server() -> McpServer {
    McpServer::new(8)
}

fn reply(server: &mut McpServer, request: Value) -> Value {
    let line = server
        .handle(&request.to_string())
        .expect("request with an id gets a response");
    serde_json::from_str(&line).expect("response is JSON")
}

fn call(server: &mut McpServer, id: u64, name: &str, arguments: Value) -> Value {
    let response = reply(
        server,
        json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
               "params": {"name": name, "arguments": arguments}}),
    );
    assert!(response.get("error").is_none(), "{response}");
    response["result"].clone()
}

#[test]
fn initialize_echoes_supported_versions_and_falls_back_to_latest() {
    let mut s = server();
    for requested in ["2025-06-18", "2025-03-26"] {
        let r = reply(
            &mut s,
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                   "params": {"protocolVersion": requested}}),
        );
        assert_eq!(r["result"]["protocolVersion"], requested);
        assert_eq!(r["result"]["serverInfo"]["name"], "deckwright");
        assert_eq!(r["result"]["capabilities"], json!({"tools": {}}));
    }
    let r = reply(
        &mut s,
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
               "params": {"protocolVersion": "2024-11-05"}}),
    );
    assert_eq!(r["result"]["protocolVersion"], "2025-06-18");
}

#[test]
fn messages_without_an_id_get_no_response() {
    let mut s = server();
    let notification = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    assert_eq!(s.handle(&notification.to_string()), None);
    let untagged = json!({"jsonrpc": "2.0", "method": "tools/list"});
    assert_eq!(s.handle(&untagged.to_string()), None);
}

#[test]
fn ping_returns_an_empty_object() {
    let mut s = server();
    let r = reply(&mut s, json!({"jsonrpc": "2.0", "id": 7, "method": "ping"}));
    assert_eq!(r, json!({"jsonrpc": "2.0", "id": 7, "result": {}}));
}

#[test]
fn tools_list_describes_the_seven_session_tools() {
    let mut s = server();
    let r = reply(
        &mut s,
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    );
    let tools = r["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        [
            "dw_open",
            "dw_state",
            "dw_timeline",
            "dw_morph",
            "dw_check",
            "dw_refresh",
            "dw_close"
        ]
    );
    for tool in tools {
        let schema = &tool["inputSchema"];
        assert_eq!(schema["type"], "object", "{tool}");
        assert_eq!(schema["additionalProperties"], false, "{tool}");
        assert!(schema["required"].is_array(), "{tool}");
        assert!(!tool["description"].as_str().unwrap().is_empty());
    }
    let required = |name: &str| -> Vec<String> {
        let tool = tools.iter().find(|t| t["name"] == name).unwrap();
        tool["inputSchema"]["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(required("dw_open"), ["path"]);
    assert_eq!(required("dw_state"), ["handle", "slide", "at_ms"]);
    assert_eq!(required("dw_morph"), ["handle", "from", "to"]);
    assert_eq!(required("dw_close"), ["handle"]);
}

#[test]
fn open_then_check_returns_structured_and_text_results() {
    let path = deck_path("open-check");
    let mut s = server();
    let opened = call(
        &mut s,
        1,
        "dw_open",
        json!({"path": path.to_str().unwrap()}),
    );
    assert_eq!(opened["isError"], false);
    assert_eq!(opened["structuredContent"]["handle"], "h1");
    assert_eq!(opened["structuredContent"]["revision"], 1);
    assert_eq!(opened["structuredContent"]["slides"], 1);
    assert_eq!(
        serde_json::from_str::<Value>(opened["content"][0]["text"].as_str().unwrap()).unwrap(),
        opened["structuredContent"]
    );

    let checked = call(&mut s, 2, "dw_check", json!({"handle": "h1"}));
    assert_eq!(checked["isError"], false);
    assert_eq!(checked["structuredContent"]["handle"], "h1");
    assert_eq!(checked["structuredContent"]["revision"], 1);
    assert!(checked["structuredContent"]["report"].is_object());
    assert_eq!(
        serde_json::from_str::<Value>(checked["content"][0]["text"].as_str().unwrap()).unwrap(),
        checked["structuredContent"]
    );
}

#[test]
fn state_and_morph_run_against_the_open_deck() {
    let path = deck_path("state-morph");
    let mut s = server();
    call(
        &mut s,
        1,
        "dw_open",
        json!({"path": path.to_str().unwrap()}),
    );

    let state = call(
        &mut s,
        2,
        "dw_state",
        json!({"handle": "h1", "slide": 1, "at_ms": 0, "clicks_ms": []}),
    );
    assert_eq!(state["isError"], false);
    assert!(state["structuredContent"]["report"].is_object());

    // The bench deck has one slide, so a Morph between slides 1 and 2 is a query error.
    let morph = call(
        &mut s,
        3,
        "dw_morph",
        json!({"handle": "h1", "from": 1, "to": 2}),
    );
    assert_eq!(morph["isError"], true);
    assert!(
        morph["content"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("query: ")
    );
    assert_eq!(morph["structuredContent"]["error"]["code"], "query");
}

#[test]
fn invalid_handle_is_a_tool_error_not_a_protocol_error() {
    let mut s = server();
    let r = call(&mut s, 1, "dw_check", json!({"handle": "h99"}));
    assert_eq!(r["isError"], true);
    assert!(
        r["content"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("invalid-handle:")
    );
    assert_eq!(r["structuredContent"]["error"]["code"], "invalid-handle");
}

#[test]
fn bad_json_gives_parse_error_with_null_id() {
    let mut s = server();
    let r: Value = serde_json::from_str(&s.handle("{not json").unwrap()).unwrap();
    assert_eq!(r["id"], Value::Null);
    assert_eq!(r["error"]["code"], -32700);
}

#[test]
fn non_object_or_missing_method_is_invalid_request() {
    let mut s = server();
    let r: Value = serde_json::from_str(&s.handle("[1, 2]").unwrap()).unwrap();
    assert_eq!(r["id"], Value::Null);
    assert_eq!(r["error"]["code"], -32600);

    let r = reply(&mut s, json!({"jsonrpc": "2.0", "id": 5}));
    assert_eq!(r["id"], 5);
    assert_eq!(r["error"]["code"], -32600);
}

#[test]
fn unknown_method_is_method_not_found() {
    let mut s = server();
    let r = reply(
        &mut s,
        json!({"jsonrpc": "2.0", "id": 3, "method": "resources/list"}),
    );
    assert_eq!(r["id"], 3);
    assert_eq!(r["error"]["code"], -32601);
}

#[test]
fn unknown_tool_or_non_object_arguments_are_invalid_params() {
    let mut s = server();
    let r = reply(
        &mut s,
        json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call",
               "params": {"name": "dw_nope", "arguments": {}}}),
    );
    assert_eq!(r["error"]["code"], -32602);

    let r = reply(
        &mut s,
        json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call",
               "params": {"name": "dw_check", "arguments": 5}}),
    );
    assert_eq!(r["id"], 5);
    assert_eq!(r["error"]["code"], -32602);
}

#[test]
fn stdio_process_answers_each_request_and_exits_zero() {
    let requests = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
               "params": {"protocolVersion": "2025-06-18"}})
        .to_string(),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string(),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}).to_string(),
        json!({"jsonrpc": "2.0", "id": 3, "method": "ping"}).to_string(),
    ];
    let mut child = Command::new(env!("CARGO_BIN_EXE_dw-mcp"))
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
    let ids: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap()["id"].clone())
        .collect();
    assert_eq!(ids, [json!(1), json!(2), json!(3)]);
}

#[test]
fn max_open_zero_is_rejected_on_stderr() {
    let out = Command::new(env!("CARGO_BIN_EXE_dw-mcp"))
        .args(["--max-open", "0"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).starts_with("dw-mcp: "));
}
