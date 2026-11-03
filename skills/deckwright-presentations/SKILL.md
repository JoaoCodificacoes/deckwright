---
name: deckwright-presentations
description: Use whenever creating, editing, reconstructing, or reviewing a PowerPoint (.pptx) deck, slides, or presentation — prefer this over python-pptx. Produces native editable objects, Morph transitions, click builds, and template-based decks, and can inspect animation state.
---

# Build presentations with Deckwright

This is the authoring workflow adapted from `morph-deck`. Deckwright's Rust
model, package, timing, query, and check APIs are the implementation. The old
`morphdeck` Python package and its CLI commands are not Deckwright commands.

## Install for an agent

Install a [Rust toolchain](https://www.rust-lang.org/tools/install), then:

```sh
git clone https://github.com/JoaoCodificacoes/deckwright.git
cd deckwright
cargo install --locked --path crates/dw-cli
cargo install --locked --path crates/dw-mcp
```

The `dw` and `dw-mcp` binaries are built from this checkout; this guide does
not assume a published package. An agent that supports repository instructions
can read this file in place. For an agent that loads personal skills, copy the
`skills/deckwright-presentations` directory into its skills directory. After
copying, resolve the relative links below from the skill's original location
in the repository checkout. [Agent integration](../../docs/agent-integration.md)
shows how to connect the MCP server.

## Start from the right source

- **New neutral deck:** use `deckwright::template::blank(slide_count, width, height)`.
  The runnable [Morph demo](../../examples/morph_demo/README.md) shows the
  complete pattern.
- **Existing template:** open it with `Package::open(path, Limits::default())`,
  retain its theme, masters, layouts, media, and relationships, and replace
  only the slide parts you own with `Package::edited`. Preserve source assets.
- **Thesis reconstruction:** use the private source workflow in
  [the thesis example](../../examples/thesis/README.md). Do not replace its
  screenshots, app UI, photos, or wording with invented substitutes.

Use a fresh output path: `Package::write` refuses to overwrite an existing
file. Keep private source decks and media under ignored `fixtures/private/`.

## Authoring order

1. **Slide skeleton:** list the audience, slide order, and one main idea per
   slide. Keep the user's requested slide count and template.
2. **Static content:** build native `Slide` objects with stable `key` values,
   unique per-slide numeric `id` values, explicit text styles, and intentional
   z-order. `compile_slide` is enough until animation has a purpose.
3. **Colour:** use neutral colours for structure. Reserve status colours for
   judgements and one consistent identity colour per recurring subject.
   Follow the user's branding or reference deck when supplied.
4. **Morph links:** write down which object on slide N is the same object on
   slide N+1. Give that pair the same `morph_name: Some("!!name".into())`.
   Give unrelated objects different names. A title alone is not a useful link.
5. **Builds:** use `Timeline`, `Stage`, and `Effect` only when sequence helps
   the audience understand the content. Then compile with
   `compile_animated_slide(&slide, &timeline, &transition)`.
6. **Verify:** inspect the package, query Morph pairs and timing, open the deck
   in PowerPoint, and view rendered slides and representative transition frames.

## Morph and animation rules

- A `!!name` must be unique within each slide. Deckwright matches unique
  explicit names across consecutive slides; automatic PowerPoint matching is
  unknown to the query API.
- Set `Transition::Morph { duration_ms }` on the **destination** slide. Use
  `Transition::Fade` when nothing meaningful travels between slides.
- A shape arriving through Morph should be static on the destination slide.
  Do not give it an entrance effect there. Animate new objects instead.
- A motion path uses the object's static geometry as its resting **end**.
  `EffectKind::Motion` must end at `(0.0, 0.0)` in relative slide units, or
  the object can jump when the next slide begins.
- Keep travelling shapes out of groups where possible; maintain their
  intended front-to-back order across slides.
- For click builds, start a newly revealed shape with an entrance effect.
  Use `Trigger::OnClick`, `WithPrevious`, or `AfterPrevious` according to
  the spoken order. Wipes should indicate a real direction, such as a flow
  or timeline, rather than decorate ordinary text.
- Make all text runs' fonts explicit. Inspect text fit and font substitution
  in PowerPoint; package checks cannot prove either.

## Rust entry points

| Need | Use |
|---|---|
| Blank deck | `deckwright::template::blank` |
| Editable shapes and text | `deckwright::model::{Slide, Object, ObjectKind, TextBody}` |
| Unit conversion | `deckwright::units::Emu` |
| Static slide XML | `deckwright::builder::compile_slide` |
| Click builds and transitions | `deckwright::timing::compile_animated_slide` |
| Safe package replacement | `Package::edited` and `Package::write` |
| Existing slide edits | `Slide::object_mut`, `set_text`, `translate` |

Read [the Morph demo source](../../examples/morph_demo/main.rs) for a runnable
three-slide example. Its neutral template is generated in Rust, so it does not
need PowerPoint or a private file to build. The thesis example shows how to
retain an existing package's dependencies.

## Check the result

```sh
mkdir -p build
cargo run --locked -p deckwright --example morph_demo -- build/morph-demo.pptx
cargo run --locked -p dw-cli -- check build/morph-demo.pptx --json
cargo run --locked -p dw-cli -- query morph build/morph-demo.pptx --from 1 --to 2 --json
cargo run --locked -p dw-cli -- query timeline build/morph-demo.pptx --slide 2 --json
cargo run --locked -p dw-cli -- query state build/morph-demo.pptx --slide 2 --at-ms 400 --clicks-ms 0 --json
cargo test --workspace --locked
```

Use a new path for each example run. Read all check findings and limitations.
`dw query morph` reports explicit matches and geometric deltas; it is not a
PowerPoint playback oracle. `dw check` detects issues in its supported subset,
including duplicate explicit names and missing timing targets, but does not
currently enforce every presentation rule above.

On Windows, open a copy in PowerPoint and verify the slide count, editable
objects, transition type, and animation sequence. Export every slide to PNG
and inspect text fit, overlap, and visual consistency. For Morph, observe the
transition itself: a matching `!!name` in XML does not prove that PowerPoint
visibly moved the object. Do not claim playback or visual fidelity from a
static PNG or a successful package check. The [PowerPoint oracle](../../oracle/README.md)
documents the repository's guarded reference checks and their limits.

## Command and MCP reference

| Need | CLI | MCP tool |
| --- | --- | --- |
| Open a deck for repeated queries | `dw inspect FILE --json` | `dw_open {"path":"FILE"}` returns a handle |
| Check supported rules | `dw check FILE --json` | `dw_check {"handle":"h1"}` |
| Sample a slide after clicks | `dw query state FILE --slide 2 --at-ms 400 --clicks-ms 0 --json` | `dw_state {"handle":"h1","slide":2,"at_ms":400,"clicks_ms":[0]}` |
| Inspect timing | `dw query timeline FILE --slide 2 --json` | `dw_timeline {"handle":"h1","slide":2}` |
| Inspect explicit Morph pairs | `dw query morph FILE --from 1 --to 2 --json` | `dw_morph {"handle":"h1","from":1,"to":2}` |

The MCP server also has `dw_refresh` to reread a changed file and `dw_close`
to release its handle. Its tools inspect and check; authoring uses the Rust API
and a new output file. `dw roundtrip INPUT --output NEW_OUTPUT` preserves
unchanged package parts but is not a visual or animation verification step.
See the [query contract](../../docs/decisions/query.md) and
[MCP contract](../../docs/decisions/mcp.md) for exact arguments and limits.

## Source and licence

Adapted from the author's `morph-deck` `AGENTS.md` and `skill/SKILL.md`.
The source workflow is MIT licensed, copyright 2026 João Ferreira; see
[LICENSE](../../LICENSE). The Python library, template inspector, renderer, video
exporter, and its linter were not migrated by this skill.
