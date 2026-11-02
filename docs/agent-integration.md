# Use Deckwright with an agent

Deckwright offers two separate agent paths: a [presentation authoring skill](../skills/deckwright-presentations/SKILL.md) for planning and writing Rust slide code, and `dw-mcp` for inspecting and checking `.pptx` files. The MCP server does not create or edit slides.

## Build from source

Install a [Rust toolchain](https://www.rust-lang.org/tools/install), then clone the repository and run:

```sh
git clone https://github.com/JoaoCodificacoes/deckwright.git
cd deckwright
cargo install --locked --path crates/dw-cli
cargo install --locked --path crates/dw-mcp
```

These commands install binaries from the checkout. They do not depend on a registry release. Make sure `dw` and `dw-mcp` are on the agent process's `PATH`.

## Connect the MCP server

Configure your MCP client to launch `dw-mcp` as a **stdio** server. Client settings differ, but the command and arguments have this shape:

```json
{
  "command": "dw-mcp",
  "args": ["--max-open", "8"]
}
```

The server registers `dw_open`, `dw_state`, `dw_timeline`, `dw_morph`, `dw_check`, `dw_refresh`, and `dw_close`. An agent can open a deck, use the returned handle for subsequent calls, refresh it after an external edit, and close it when finished. Paths passed to `dw_open` must be accessible to the server process. The [MCP contract](decisions/mcp.md) gives the exact tool schemas and error behavior.

For example, ask an agent to open `demo.pptx`, check it, then report slide 2's state at 400 ms after a click at 0 ms. The underlying calls are:

```text
dw_open    {"path":"demo.pptx"}
dw_check   {"handle":"h1"}
dw_state   {"handle":"h1","slide":2,"at_ms":400,"clicks_ms":[0]}
dw_close   {"handle":"h1"}
```

`h1` is illustrative; use the actual handle returned by `dw_open`. Slide numbers are one based. Click times must be the complete, nondecreasing history at the sample time. Reports can contain unknown values and reasons for unsupported timing.

## Author and verify a deck

Point the agent at the [authoring skill](../skills/deckwright-presentations/SKILL.md) and the [runnable Morph demo](../examples/morph_demo/main.rs). The agent writes Rust code using `deckwright::model`, `template::blank` or `Package::open`, and `compile_slide` or `compile_animated_slide`. It writes to a new output path; `Package::write` refuses an existing file.

Then run `dw check`, relevant `dw query` commands, and inspect the result in PowerPoint. The CLI and MCP reports describe supported OOXML semantics; they do not prove rendered appearance, font substitution, or animation playback. See the [PowerPoint oracle notes](../oracle/README.md) for the repository's playback verification boundary.
