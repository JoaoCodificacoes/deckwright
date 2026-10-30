# MCP transport, version 1

`dw-mcp` (crate `crates/dw-mcp`, binary `dw-mcp [--max-open N]`) is a Model
Context Protocol server over stdio. It wraps `deckwright::session::Server`
(`docs/decisions/session.md`) and adds no query logic. It depends only on
`deckwright` and `serde_json`.

**Framing.** One JSON-RPC 2.0 message per line on stdin. Responses go one per
line on stdout, flushed per line. Nothing else is written to stdout; diagnostics
go to stderr. Messages without an `id` are notifications and never get a
response. The server exits 0 at end of input.

**Methods.**
- `initialize`: returns `{"protocolVersion": v, "capabilities": {"tools": {}},
  "serverInfo": {"name": "deckwright", "version": <crate version>}}`. `v` echoes
  the client's `params.protocolVersion` when it is `2025-06-18` or `2025-03-26`,
  and otherwise is `2025-06-18`.
- `notifications/initialized`: no response.
- `ping`: returns `{}`.
- `tools/list`: returns `{"tools": [...]}` with the seven tools below. Each tool
  has `name`, `description` and an `inputSchema` JSON Schema of type object,
  listing its arguments, the required ones, and `additionalProperties: false`.
- `tools/call`: takes `{"name", "arguments"}`, calls the session op of the same
  name without the `dw_` prefix, and passes `arguments` through as the request
  fields. On success the result is `{"content": [{"type": "text", "text":
  <compact JSON of the op result>}], "structuredContent": <op result>,
  "isError": false}`. A session error (`ok: false`) becomes
  `{"content": [{"type": "text", "text": "<code>: <message>"}],
  "structuredContent": {"error": {"code", "message"}}, "isError": true}`. Tool
  errors are results, not JSON-RPC errors.

**Tools.** `dw_open {path}`, `dw_state {handle, slide, at_ms, clicks_ms?}`,
`dw_timeline {handle, slide}`, `dw_morph {handle, from, to}`, `dw_check
{handle}`, `dw_refresh {handle}`, `dw_close {handle}`. Integers are non-negative,
and `clicks_ms` is an array of non-negative integers.

**JSON-RPC errors.**
- `-32700` for a parse error, with `id: null`;
- `-32600` for an invalid request (not an object, wrong `jsonrpc`, or missing
  `method`);
- `-32601` for an unknown method;
- `-32602` for an unknown tool name, or `arguments` that are not an object.

The `--max-open` validation matches `dw serve`.
