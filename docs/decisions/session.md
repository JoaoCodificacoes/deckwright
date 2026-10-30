# Persistent query session, version 1

`dw serve` reads one JSON request per line on stdin and writes exactly one JSON
response line per request on stdout, in order, flushing after each line. It exits
0 at end of input. The protocol logic lives in `deckwright::session::Server`, with
`fn handle_line(&mut self, line: &str) -> String`. The CLI only loops over lines.

Request: `{"id": <any JSON>, "op": "<op>", ...args}`. Every response echoes `id`
(null if the request could not be parsed). It has either `"ok": true` with a
`result`, or `"ok": false` with `"error": {"code": "<code>", "message": "..."}`.

| op | args | result |
|---|---|---|
| `open` | `path` | `{"handle": "h<N>", "revision": 1, "slides": <count>}` |
| `state` | `handle`, `slide`, `at_ms`, optional `clicks_ms` (array) | `{"handle", "revision", "report": <dw.query.state/1>}` |
| `timeline` | `handle`, `slide` | same envelope with the timeline report |
| `morph` | `handle`, `from`, `to` | same envelope with the Morph report |
| `check` | `handle` | same envelope with the `dw.check/1` report |
| `refresh` | `handle` | re-reads the file and increments the revision: `{"handle", "revision", "slides"}` |
| `close` | `handle` | `{"handle", "closed": true}` |

Handles are `h1`, `h2`, … and are never reused within a process.

Error codes:
- `parse` for invalid JSON, a missing `op`, or wrong argument types;
- `unknown-op`;
- `invalid-handle` for a handle never issued or already closed;
- `evicted` when the handle was dropped by the cache;
- `stale`, below;
- `query` for errors returned by `QuerySession` (out-of-range slide, bad click history);
- `io` for open or refresh failures, including an invalid package.

**Staleness.** `open` records the file length and modification time. Every
handle operation except `close` and `refresh` re-reads that metadata first. If it
differs, or the file is gone, the result is `stale` and nothing is answered from
the old snapshot. `refresh` rebuilds the session from the current file under the
same handle and bumps `revision`. The `check` op uses the package bytes captured
at open/refresh time, so it never mixes revisions.

**Bounded cache.** `Server::new(max_open)` holds at most `max_open` sessions
(CLI flag `--max-open N`, default 8, minimum 1). Opening one more evicts the
least recently used handle; any successful op on a handle counts as use. A
later op on an evicted handle returns `evicted`, which is distinct from
`invalid-handle`.
