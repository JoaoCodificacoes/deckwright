//! MCP stdio protocol over `deckwright::session::Server`. Spec: docs/decisions/mcp.md.
//!
//! `McpServer::handle` answers one JSON-RPC message line. It owns no query logic:
//! each tool call becomes one session request line and the session response is mapped
//! to an MCP tool result.

use deckwright::session::Server;
use serde_json::{Map, Value, json};

const LATEST_VERSION: &str = "2025-06-18";
const SUPPORTED_VERSIONS: [&str; 2] = ["2025-06-18", "2025-03-26"];
const SESSION_OPS: [&str; 7] = [
    "open", "state", "timeline", "morph", "check", "refresh", "close",
];

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

struct RpcError {
    code: i64,
    message: String,
}

fn rpc_error(code: i64, message: impl Into<String>) -> RpcError {
    RpcError {
        code,
        message: message.into(),
    }
}

/// MCP server state: one session server, bounded by `max_open`.
pub struct McpServer {
    session: Server,
}

impl McpServer {
    pub fn new(max_open: usize) -> Self {
        Self {
            session: Server::new(max_open),
        }
    }

    /// Answers one input line with one JSON-RPC response line (no trailing newline).
    /// Returns `None` for notifications, meaning messages without an `id`.
    pub fn handle(&mut self, line: &str) -> Option<String> {
        let message = match serde_json::from_str::<Value>(line) {
            Ok(message) => message,
            Err(error) => {
                return Some(error_response(
                    Value::Null,
                    rpc_error(PARSE_ERROR, format!("parse error: {error}")),
                ));
            }
        };
        let Value::Object(object) = message else {
            return Some(error_response(
                Value::Null,
                rpc_error(INVALID_REQUEST, "request must be a JSON object"),
            ));
        };
        let id = object.get("id")?.clone();
        let outcome = match object.get("jsonrpc").and_then(Value::as_str) {
            Some("2.0") => self.dispatch(&id, &object),
            _ => Err(rpc_error(INVALID_REQUEST, "jsonrpc must be \"2.0\"")),
        };
        match outcome {
            Ok(Some(result)) => {
                Some(json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string())
            }
            Ok(None) => None,
            Err(error) => Some(error_response(id, error)),
        }
    }

    /// `Ok(None)` means the message needs no response.
    fn dispatch(
        &mut self,
        id: &Value,
        object: &Map<String, Value>,
    ) -> Result<Option<Value>, RpcError> {
        let method = object
            .get("method")
            .and_then(Value::as_str)
            .ok_or_else(|| rpc_error(INVALID_REQUEST, "method must be a string"))?;
        let params = object.get("params");
        match method {
            "notifications/initialized" => Ok(None),
            "initialize" => Ok(Some(initialize(params))),
            "ping" => Ok(Some(json!({}))),
            "tools/list" => Ok(Some(json!({"tools": tools()}))),
            "tools/call" => self.call_tool(id, params).map(Some),
            other => Err(rpc_error(
                METHOD_NOT_FOUND,
                format!("method not found: {other}"),
            )),
        }
    }

    fn call_tool(&mut self, id: &Value, params: Option<&Value>) -> Result<Value, RpcError> {
        let params = params
            .and_then(Value::as_object)
            .ok_or_else(|| rpc_error(INVALID_PARAMS, "params must be an object"))?;
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| rpc_error(INVALID_PARAMS, "name must be a string"))?;
        let op = name
            .strip_prefix("dw_")
            .filter(|op| SESSION_OPS.contains(op))
            .ok_or_else(|| rpc_error(INVALID_PARAMS, format!("unknown tool: {name}")))?;
        let mut request = match params.get("arguments") {
            None => Map::new(),
            Some(Value::Object(arguments)) => arguments.clone(),
            Some(_) => return Err(rpc_error(INVALID_PARAMS, "arguments must be an object")),
        };
        request.insert("id".into(), id.clone());
        request.insert("op".into(), json!(op));
        let line = Value::Object(request).to_string();
        // The session always answers with a JSON object envelope.
        let envelope: Value = serde_json::from_str(&self.session.handle_line(&line))
            .expect("session envelope is JSON");
        if envelope["ok"] == json!(true) {
            let result = envelope["result"].clone();
            Ok(json!({
                "content": [{"type": "text", "text": result.to_string()}],
                "structuredContent": result,
                "isError": false,
            }))
        } else {
            let error = envelope["error"].clone();
            let text = format!(
                "{}: {}",
                error["code"].as_str().unwrap_or("error"),
                error["message"].as_str().unwrap_or_default()
            );
            Ok(json!({
                "content": [{"type": "text", "text": text}],
                "structuredContent": {"error": error},
                "isError": true,
            }))
        }
    }
}

fn error_response(id: Value, error: RpcError) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": error.code, "message": error.message},
    })
    .to_string()
}

fn initialize(params: Option<&Value>) -> Value {
    let requested = params
        .and_then(|p| p.get("protocolVersion"))
        .and_then(Value::as_str);
    let version = match requested {
        Some(v) if SUPPORTED_VERSIONS.contains(&v) => v,
        _ => LATEST_VERSION,
    };
    json!({
        "protocolVersion": version,
        "capabilities": {"tools": {}},
        "serverInfo": {"name": "deckwright", "version": env!("CARGO_PKG_VERSION")},
    })
}

fn tool(op: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": format!("dw_{op}"),
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false,
        },
    })
}

fn tools() -> Vec<Value> {
    let handle = json!({"type": "string", "description": "Handle returned by dw_open, e.g. h1."});
    let slide = json!({"type": "integer", "minimum": 0, "description": "1-based slide number."});
    let at_ms = json!({"type": "integer", "minimum": 0, "description": "Time in milliseconds."});
    vec![
        tool(
            "open",
            "Open a .pptx file and return a handle for later queries.",
            json!({"path": {"type": "string", "description": "Path to the .pptx file."}}),
            &["path"],
        ),
        tool(
            "state",
            "Report the animated state of a slide at a time, with optional click times.",
            json!({
                "handle": handle,
                "slide": slide,
                "at_ms": at_ms,
                "clicks_ms": {"type": "array", "items": {"type": "integer", "minimum": 0}},
            }),
            &["handle", "slide", "at_ms"],
        ),
        tool(
            "timeline",
            "Report the animation timeline of a slide.",
            json!({"handle": handle, "slide": slide}),
            &["handle", "slide"],
        ),
        tool(
            "morph",
            "Report the Morph transition between two slides.",
            json!({
                "handle": handle,
                "from": {"type": "integer", "minimum": 0, "description": "1-based source slide."},
                "to": {"type": "integer", "minimum": 0, "description": "1-based target slide."},
            }),
            &["handle", "from", "to"],
        ),
        tool(
            "check",
            "Run the dw.check/1 checks on the snapshot behind a handle.",
            json!({"handle": handle}),
            &["handle"],
        ),
        tool(
            "refresh",
            "Re-read the file behind a handle and bump its revision.",
            json!({"handle": handle}),
            &["handle"],
        ),
        tool(
            "close",
            "Close a handle.",
            json!({"handle": handle}),
            &["handle"],
        ),
    ]
}
