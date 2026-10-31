# Whole-deck authoring demonstrator

The Rust example compiles editable native text/shapes/pictures/connectors/groups,
custom geometry and video pictures from typed declarative data. It does not read
source slide XML. Original assets, theme, masters and layouts are imported as
opaque template dependencies. It builds every slide listed by the manifest, in
presentation order.

Private source preparation (outside timed builds):

```sh
python3 examples/thesis/extract_source.py /path/to/private.pptx fixtures/private/thesis-source --sha256 <hex> --dw /path/to/dw
cargo run --release -p deckwright --example thesis -- fixtures/private/thesis-source build/thesis.pptx
python3 examples/thesis/verify_static.py /path/to/private.pptx build/thesis.pptx --sha256 <hex>
python3 examples/thesis/mutation_check.py target/release/examples/thesis fixtures/private/thesis-source
```

Without `--sha256`, the extractor and verifier accept only the frozen reference
SHA-256 in the plan. `--sha256` authorizes another explicitly named source, must
match its digest exactly (otherwise nothing is written), and is recorded as
`source_sha256` in the manifest. Without `--dw`, the build is static: slides have
no timing, and the manifest's `omissions` lists `timing` and `transitions`.

The extractor fails on unknown authored elements/attributes and on any value outside
the supported domains. It produces, in `slides/sNN.json` (two digits, widened
beyond 99) for each slide in `sldIdLst` order, typed data. It also writes a
template containing **every** slide as an empty shell (timing, transition and
AlternateContent removed), and a placement/media manifest. With `--dw`, it runs
`dw query timeline SOURCE --slide N --json` per slide, with no shell, and writes
`slides/sNN.timing.json` with the `timing` object. Every entry in that object's
`opaque` list appears in `unsupported_authored_features` as
`{slide, targets, reason}`. The Rust example then compiles each slide with
`compile_animated_slide` when its timing file exists, otherwise with
`compile_slide`. Private wording, assets and data remain ignored; none are checked
in.

Supported authored additions include line dash and line ends, `endParaRPr`,
underline, custom geometry (guides, adjustments and non-standard text rectangles
fail closed) and video pictures (`videoFile`, `p14:media` and the media
hyperlink).

Each object's explicit `key` is distinct from `morph_name`. Array order is z-order.
Absent properties inherit from the imported placeholder/layout/master/theme;
present font/run, geometry and group child coordinates remain explicit.

Change JSON text or a transform before running the example to author changes.
The library also exposes `Slide::set_text`, `Slide::translate` and `object_mut`
for typed Rust changes; rich text is edited through its individual runs.

The output's `.manifest.json` sidecar maps each source key to original/generated
numeric IDs, geometry/crop and media hashes. Keys are additionally written into a
custom OOXML extension. That extension's PowerPoint save survival is provisional;
the sidecar remains authoritative until the sync milestone validates it.

`slides[].placements` is regenerated recursively from the current typed objects
on every build, including current numeric IDs, transforms, picture relationships,
crops and Morph names. `slides[].source_placements` separately preserves the
original extraction provenance by stable key; it has no `generated_id` claim.
New keys have a null source ID, while removed source objects remain in provenance.
Enum values and numeric domains in the supported model are checked before XML
emission; invalid text wrapping, font sizes, line widths and similar attributes
return an error identifying the authored source key.

Output uses create-new semantics: choose a fresh path on each run. Timing is
compiled only from `dw` typed output; it is not compared by `verify_static.py`,
which checks native objects only. PowerPoint/font/line-break comparison and
performance qualification are separate evidence gates.

`verify_static.py` independently compares every slide's native object subtree, in
sldIdLst order, including nested group members and the new elements, after
excluding only the documented key carrier and explicit text whitespace attribute.
It also verifies exact bytes for every retained imported dependency/media part. A
mismatch fails with slide/object context. This is a semantic fidelity check;
PowerPoint visual QA is still required.
