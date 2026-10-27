# Query reports, version 1

`QuerySession::open(path, Limits)` parses slide geometry and timing once. Reuse the
session for `state(slide, clicks_ms, sample_time_ms)`, `timeline(slide)` and
`morph(from, to)`. Slide numbers are one based. Reports implement `Serialize` and
carry `schema_version: 1`. `from_package(&Package, Limits)` borrows an existing
package during snapshot construction. `from_snapshots` accepts the same semantic
inputs for neutral authored tests.

Times are integer milliseconds since slide entry. `clicks_ms` is the complete,
nondecreasing actual click history at the sample time; future clicks are errors.
An extra click does not cancel effects still running from earlier clicks. Stage
anchors follow the shared timing contract; effects retain their final sampled
properties. No `(click_number, elapsed_time)` interpretation is implicit. Geometry
uses EMUs in slide space. Motion uses slide-relative path fractions and the
static resting endpoint. Opacity and wipe fraction range from zero to one.

`null` means unknown, never hidden. Unsupported opaque timing invalidates affected
properties with reasons. Geometry requiring placeholder inheritance, group
rotation or flips is unresolved. Parent visibility and opacity propagate to
children. Reports describe the supported semantics, not a raster rendering.

Morph pairs only unique matching `!!` names. Numeric shape IDs are slide-local
references, separate from matching names and authored source identity. Duplicate
names are ambiguous and generate warnings. Unpaired IDs are fade candidates;
automatic PowerPoint pairing remains unknown. A geometry delta is destination
minus source, including extent changes. A missing Morph transition is reported.
