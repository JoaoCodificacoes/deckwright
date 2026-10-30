# Check rules, version 1 (timing and bounds extension)

`dw check` keeps the `dw.check/1` envelope and schema. New rules add findings
with these exact `rule` names. `slide` is the slide part name; `shape` is the
numeric shape ID as a string, or null.

| Rule | Severity | When |
|---|---|---|
| `timing-parse` | error | `timing::parse_slide_timing` fails for the slide. |
| `timing-invalid` | error | Parsed timeline fails `timing::validate_timeline`. Defensive guard: the parser currently reports constructs outside the valid supported subset (including overflowing delays and motion paths not ending at the static position) as opaque timing, so this rule fires only if that parser invariant breaks. |
| `timing-opaque` | warning | One finding per `OpaqueTiming` entry. The message includes the reason, and `shape` is its first target or null. Timing semantics for those targets are unknown. |
| `off-slide` | warning | A shape's resolved geometry lies entirely outside the slide rectangle, either at slide start (no clicks, t = 0) or in the settled final state (every OnClick stage clicked, sampled after all effects end). |
| `partially-off-slide` | warning | Resolved geometry intersects the slide but extends past an edge, in either of the same two states. |

Bounds use `QuerySession` snapshots and `state()`, so they inherit its rules.
Shapes with unknown geometry (`null`) get no bounds verdict. The existing
"Bounds unknown" limitation is replaced by one that says bounds cover resolved
axis-aligned geometry only and skip rotated, placeholder-inherited and
opaque-timing shapes. A shape hidden in a sampled state is not checked in that
state. The settled state is sampled with one click per OnClick stage at
1 ms spacing, at `t = last click + max(effect end) + 1`.

Each (rule, slide, shape) finding is reported at most once. Existing rules and
their severities are unchanged. Heuristic warnings never change the exit status.

## Scoped suppressions (wave 6)

A `Suppression { rule: String, slide: Option<usize>, shape: Option<String> }`
matches findings with that rule. If `slide` is set, the finding must be on that
one-based slide in presentation order. If `shape` is set, the finding must have
that shape ID. `check::check_package_with(package, &[Suppression])` moves
matching findings into a new top-level `suppressed` array, in the same finding
format. `check_package(p)` equals `check_package_with(p, &[])`.

Only warnings can be suppressed. A suppression that would match an error
finding leaves it in `findings` and adds a limitation naming the suppression.
Each suppression that matches nothing adds one limitation saying it is unused.

The CLI flag is `dw check <pptx> --json [--suppress RULE[@SLIDE[:SHAPE]]]...`.
It is repeatable, and malformed values are usage errors. The schema
`check-v1.json` adds `suppressed` (an array of findings) to the required
properties, and the envelope stays `dw.check/1`.
