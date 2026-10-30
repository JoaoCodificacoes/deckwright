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

## CLI contract (frozen for T04)

```text
dw query state <pptx> --slide <N> --at-ms <T> [--clicks-ms <t1,t2,...>] --json
dw query timeline <pptx> --slide <N> --json
dw query morph <pptx> --from <N> --to <M> --json
```

Flags may appear in any order after `<pptx>`; each at most once. `--json` is
required. Integers are base-10 without signs. Omitting `--clicks-ms` means no
clicks; an empty value, a non-integer, a duplicate flag, an unknown flag or a
missing required flag is a usage error. The CLI opens `QuerySession::open(path,
Limits::default())` and calls `state`, `timeline` or `morph` unchanged; it adds
no evaluation logic. On success stdout is exactly one JSON report followed by a
newline and the exit status is 0. On any error (usage, I/O, invalid package, out
of range slide, invalid click history) stdout is empty, stderr starts with
`dw: ` and the exit status is 1.

## Canonical timing form (frozen for T04)

```rust
pub fn timing::canonicalize(timeline: &Timeline) -> Timeline;
pub fn timing::equivalent(a: &ParsedTiming, b: &ParsedTiming) -> Option<bool>;
```

`canonicalize` keeps stage order, triggers and delays, and stable-sorts the
effects of each stage by `target`. Effects in one stage are concurrent, so the
order of different targets is not semantic; the relative order of effects on
the same target is preserved because later effects determine final properties.
Stages are never merged and stage delays are never folded into effects, since
WithPrevious and AfterPrevious anchors depend on stage boundaries.
`canonicalize` is idempotent and evaluates identically to its input.

`equivalent` returns `None` when either side has opaque timing (equivalence is
unknown, not false), otherwise `Some(canonical timelines equal && transitions
equal)`.
