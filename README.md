# Deckwright

Deckwright is a Rust toolkit for building editable PowerPoint slides and inspecting the animation data inside `.pptx` files. It can create a deck from a blank template, replace slides in an existing package, run structural checks, and answer questions such as “what is visible on slide 2 after the first click?” through a CLI or an MCP server.

The public [Morph demo](examples/morph_demo/README.md) builds three editable slides with an explicit Morph match, a click reveal, and a motion path. Watch the [26-second PowerPoint demo](https://github.com/JoaoCodificacoes/deckwright/releases/download/v0.1.0/public-demo.mp4) to see the transition, a selected native shape, and a real timing query. The [presentation authoring skill](skills/deckwright-presentations/SKILL.md) gives agents a workflow for using these APIs.

## Get started

Install a [Rust toolchain](https://www.rust-lang.org/tools/install), then install the CLI and MCP server:

```sh
cargo install dw-cli --version 0.1.1 --locked
cargo install dw-mcp --version 0.1.1 --locked
```

[Release archives](https://github.com/JoaoCodificacoes/deckwright/releases) provide prebuilt `dw` and `dw-mcp` binaries for Windows, Linux, and macOS.

To build and inspect the example deck, run these commands from a Deckwright checkout:

```sh
cargo run --locked -p deckwright --example morph_demo -- demo.pptx
dw check demo.pptx --json
dw query morph demo.pptx --from 1 --to 2 --json
dw query state demo.pptx --slide 2 --at-ms 400 --clicks-ms 0 --json
```

Choose a new output filename for each build; Deckwright refuses to overwrite an existing `.pptx`. Slide numbers are one based. Query times and click times are milliseconds since slide entry; `--clicks-ms` is the complete click history up to `--at-ms`.

## What works today

| Task | Entry point |
| --- | --- |
| Build editable slides with shapes, text, pictures, tables, charts, and timing in the supported model | `deckwright` Rust crate; see the [Morph demo source](examples/morph_demo/main.rs) |
| Open a `.pptx`, inspect its parts, preserve unchanged parts, and write a new package | `Package`; `dw inspect` and `dw roundtrip` |
| Check supported package, slide, timing, and bounds rules | `dw check` |
| Inspect slide state, timeline, and explicit Morph pairs | `dw query state`, `dw query timeline`, `dw query morph` |
| Give an agent the same checks and queries over stdio | `dw-mcp`; see [agent integration](docs/agent-integration.md) |

The [CLI contract](docs/decisions/query.md) defines query input and output. Reports represent the timing and geometry Deckwright understands; unsupported or ambiguous behavior is reported as unknown. Morph pairing currently covers unique explicit `!!name` matches. A passing check, successful query, or static render does **not** establish PowerPoint animation playback, font fit, or visual fidelity. Verify those in PowerPoint for any delivered deck.

## Choose a workflow

- [Create a presentation with an agent](skills/deckwright-presentations/SKILL.md)
- [Connect an agent to the MCP server](docs/agent-integration.md)
- [Compare Deckwright and python-pptx](docs/compare-python-pptx.md)
- [Run the neutral Morph demo](examples/morph_demo/README.md)
- [Read the benchmark method and results](docs/benchmarking.md)

## Project scope

Deckwright began as a reconstruction of a 25-slide thesis presentation. The [implementation plan](docs/superpowers/plans/2026-10-25-thesis-reconstruction-performance.md) records its fidelity and performance targets. The thesis source and extracted media are private and excluded from Git; the public demo uses generated neutral content. Preserve source wording, screenshots, photos, and app UI when editing a supplied deck.

The reference inspection benchmark measured 205.32 ms p95 and 78.75 MiB peak RSS on its documented WSL host; see [the measurement PR](https://github.com/JoaoCodificacoes/deckwright/pull/14) for the revision and workload. Build, edit, and query budgets in the plan are targets, not measured results. The full thesis reconstruction and PowerPoint playback acceptance remain separate gates.

## Development

```sh
cargo test --workspace --locked
```

See [AGENTS.md](AGENTS.md) for repository guidance and [the oracle notes](oracle/README.md) for PowerPoint verification limits.
