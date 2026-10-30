# Timing contract

`timing::{Timeline, Stage, Trigger, Effect, EffectKind, Direction, Transition,
ParsedTiming, OpaqueTiming}` is the shared typed contract. `Stage.delay_ms` is
relative to its trigger; `Effect.delay_ms` is relative to its stage. Both are
signed to express overlap. WithPrevious starts relative to the previous stage
start; AfterPrevious starts relative to its latest effect end. Clicks supply
OnClick stage anchors in order. OnSlideStart anchors at zero.

Motion coordinates are finite fractions of slide width/height. The static
shape transform is the resting endpoint; evaluated offsets are the interpolated
path minus its endpoint. Acceleration and deceleration are finite fractions in
[0,1] with sum <= 1. Durations are integer milliseconds. Unsupported imported
semantics are reported through `ParsedTiming.opaque`; partial known effects do
not make an opaque timeline exact.

Planned implementations (not yet present in this contract-only commit):
`parse_slide_timing(xml: &[u8], part: &str) -> crate::Result<ParsedTiming>`;
`compile_animated_slide(slide: &model::Slide, timeline: &Timeline,
transition: &Transition) -> crate::Result<Vec<u8>>`.

## Emphasis scale and repeats (wave 6, pending oracle confirmation)

`EffectKind::Scale { x: f64, y: f64, repeat: Repeat, auto_reverse: bool }` is
PowerPoint's Grow/Shrink emphasis (`presetClass="emph" presetID="6"`). `x` and
`y` are the factors reached at the end of one forward pass, with
0 < factor <= 10. `Repeat` is `#[serde(tag = "type", rename_all =
"snake_case")] enum Repeat { Count { thousandths: u32 }, Indefinite }`, where
`Count` is 1..=1_000_000 and 1000 means once.

**Compiled XML.**
- The effect cTn carries `repeatCount="<thousandths>"` or `"indefinite"`, plus
  `autoRev="1"` when `auto_reverse` is set.
- The behaviour is `<p:animScale><p:cBhvr><p:cTn id dur fill="hold"/>
  <p:tgtEl><p:spTgt spid/></p:tgtEl></p:cBhvr><p:by x="{x*100000}"
  y="{y*100000}"/></p:animScale>`, with the factors rounded to integers.
- The parser accepts exactly this shape. Every other `emph` preset, and any
  other repeat or autoRev placement, stays opaque.

**Scheduling.** One iteration lasts `duration_ms` (d), or 2d with
`auto_reverse`. The active duration is iteration × thousandths / 1000. For
scheduling, the effect end is its begin plus the active duration. An
`Indefinite` effect has no end, so a following `AfterPrevious` stage is never
anchored, the same as an unclicked stage.
`timing::effect_active_ms(&Effect) -> Option<u64>` returns the active duration,
or `None` for indefinite. Every other kind returns `Some(duration_ms)`.

**State.**
- Within an iteration at local time τ, forward progress is τ/d. With
  `auto_reverse`, the second half runs 1 - (τ - d)/d.
- The factor is `1 + (x - 1) * progress`, and the same for y.
- After a finite active duration ends, the value at that end is held (fill hold).
- Started scales compose multiplicatively and are applied after motion, about
  the shape's current center. The center stays fixed while width and height
  scale. A scale that has not started does nothing.
- Visibility and opacity are unaffected.
- `QuerySession::state` at any finite t is well defined even for indefinite
  repeats. No end state is ever fabricated for them.
