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

## Same-target effect semantics (decided 2026-10-30, pending oracle confirmation)

These rules resolve the differential-test findings (a)–(d). They follow the
SMIL sandwich model PowerPoint timing is based on. Until rendered PowerPoint
recordings confirm them, they are coordinator decisions, not observed facts.

1. **Started effects only.** An effect has started when its stage is anchored
   (its click happened or its predecessor chain is anchored) and
   `begin <= t`. Effects that have not started change nothing, except rule 2.
2. **Initial visibility.** A shape whose first effect in document order is an
   entrance (`Appear`, `FadeIn`, `WipeIn`) starts hidden. A later pending
   entrance never re-hides a shape that is already visible.
3. **Priority.** Started effects apply in order of `begin`, ties broken by
   document order. The latest-begun visibility effect sets `visible`: an
   entrance sets true; a `FadeOut` sets true while active and false once
   finished.
4. **Opacity.** If the shape is hidden by timing (rule 2, or a finished
   `FadeOut` under rule 3), opacity is 0. Otherwise the latest-begun fade that
   is still active (`begin <= t < end`) sets opacity (`FadeIn`: p;
   `FadeOut`: 1 - p). With no active fade, opacity is 1. A finished fade does
   not hold its opacity, so a shape revealed after an exit is fully opaque.
5. **Wipe.** The latest-begun active `WipeIn` sets `wipe_fraction` to p.
   Otherwise it is 1, including while a pending wipe keeps the shape hidden.
6. **Motion.** Offsets of started motions are additive, and the static
   transform is the resting endpoint. A motion that has not started does
   nothing, like any other effect (rule 1). The shape stays at its static
   position until the motion begins, then jumps to the path start. Earlier
   versions placed a pending motion at its start offset; the neutral Morph
   demo showed that contradicts rule 1 and expected PowerPoint behaviour.
7. Unchanged as well: opaque timing, `hidden` shapes, parent propagation and
   effects at `p` from `[0,1]` clamped at the end.
