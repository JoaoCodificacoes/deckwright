# Three-slide static authoring demonstrator

The Rust example compiles editable native text/shapes/pictures/connectors/groups
from typed declarative data. It does not read source slide XML. Original assets,
theme, masters and layouts are imported as opaque template dependencies.

Private source preparation (outside timed builds):

```sh
python3 examples/thesis/extract_source.py /path/to/frozen/reference.pptx fixtures/private/thesis-source
cargo run --release -p deckwright --example thesis -- fixtures/private/thesis-source build/thesis-static.pptx
python3 examples/thesis/verify_static.py /path/to/frozen/reference.pptx build/thesis-static.pptx
python3 examples/thesis/mutation_check.py target/release/examples/thesis fixtures/private/thesis-source
```

The extractor requires the authorized SHA-256 frozen in the plan. It fails on
unknown authored elements/attributes and produces typed `slides/s01.json` through
`s03.json`, a template containing three **empty** slides, and a placement/media
manifest. Private wording/assets/data remain ignored; none are checked in.
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

This milestone deliberately omits transitions and timing, displaying all static
objects. It preserves source content quirks (including field text and document
form labels); potential defects require review. PowerPoint/font/line-break
comparison and performance qualification are separate evidence gates.
Output uses create-new semantics: choose a fresh path on each run.

`verify_static.py` independently compares every source/native object subtree,
including nested group members, after excluding only the documented key carrier
and explicit text whitespace attribute. It also verifies exact bytes for every
retained imported dependency/media part. A mismatch fails with slide/object
context. This is a semantic fidelity check; PowerPoint visual QA is still required.
