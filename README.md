# deckwright

A performance-focused Rust framework for authoring editable PowerPoint decks and inspecting animation state as data.

The first release will faithfully reconstruct the 25-slide thesis reference, including its original assets, Morph transitions, click builds, motion paths, emphasis effects and embedded video.

## Plan

See [the implementation plan](docs/superpowers/plans/2026-10-25-thesis-reconstruction-performance.md) for scope, acceptance criteria and proposed performance budgets.

Initial targets on a fixed benchmark host: full build under 1 second, warm single-slide edit under 250 ms, warm state query under 5 ms, and core peak memory under 128 MiB. These are engineering targets, not measured results.

## Status

Implemented: bounded OPC import/export, deterministic roundtrips, JSON inspection,
structural checks, and a typed static authoring API. The thesis example rebuilds
the first three slides as editable native objects. Their PowerPoint static PNGs
match the frozen reference exactly; this does not establish animation equivalence.

The reference inspection benchmark records 205.32 ms p95 and 78.75 MiB peak RSS
on the documented WSL host. See [the measurement PR](https://github.com/JoaoCodificacoes/deckwright/pull/14) for
the exact revision, workloads and remaining qualification gates.

## Install

```sh
cargo install --locked --path crates/dw-cli   # dw: inspect, roundtrip, check, query, serve
cargo install --locked --path crates/dw-mcp   # dw-mcp: MCP stdio server for agents
```

The `Install` workflow repeats this into an empty root on every change, then
drives the installed binaries against the neutral Morph demo deck.

## Try it

```sh
cargo run --release -p dw-cli -- inspect /path/to/deck.pptx --json
cargo run --release -p dw-cli -- check /path/to/deck.pptx --json
cargo run --release -p dw-cli -- roundtrip /path/to/deck.pptx --output /path/to/new-output.pptx
cargo run --release -p dw-cli -- query state /path/to/deck.pptx --slide 2 --at-ms 1500 --clicks-ms 1000 --json
cargo run --release -p dw-cli -- query morph /path/to/deck.pptx --from 1 --to 2 --json
cargo run --release -p dw-cli -- serve --max-open 8          # JSON-lines session (docs/decisions/session.md)
cargo run --release -p dw-mcp -- --max-open 8                # MCP stdio server (docs/decisions/mcp.md)
cargo run --release -p deckwright --example morph_demo -- /tmp/morph-demo.pptx
cargo test --workspace --locked
```

Checks return a nonzero exit status on errors and report unsupported coverage
explicitly. Output paths must be new. The [three-slide example](examples/thesis/README.md)
documents private source preparation and typed text/layout edits. The neutral
[Morph demo](examples/morph_demo/README.md) builds a public animated deck from a generated
blank template. Timing compilation and state/Morph queries are implemented; their
PowerPoint oracle verification and the full 25-slide reconstruction remain pending.

## Reference assets

The thesis deck, extracted media, detailed slide inventory and private oracle evidence remain local and are excluded from Git. Implementation must preserve original wording and visuals and keep native slide objects editable. Use generated neutral fixtures in public CI.

## Development

Work through the numbered issues in dependency order. Each issue includes its scope and acceptance gate. Record correctness and performance evidence before marking work complete.
