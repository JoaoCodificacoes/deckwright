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

## Media playback (wave 7, pending oracle confirmation)

`EffectKind::MediaPlay { from_ms: u32, volume: u32 }` is PowerPoint's media
call (`presetClass="mediacall" presetID="1"`) on a video or audio picture.
`Effect.duration_ms` is the play length, which PowerPoint writes as the cmd
behaviour's `dur`. `volume` is in thousandths of a percent, 0..=100000;
PowerPoint's default is 80000.

**Compiled XML.**
- The effect uses `<p:cmd type="call" cmd="playFrom({from_ms/1000:.1})">`
  (seconds, one decimal; validation requires `from_ms` to be a multiple of
  100), containing `<p:cBhvr><p:cTn id dur fill="hold"/><p:tgtEl><p:spTgt
  spid/></p:tgtEl></p:cBhvr>`.
- For each distinct MediaPlay target, the compiler also emits one
  `<p:video><p:cMediaNode vol="{volume}"><p:cTn id fill="hold"
  display="0"><p:stCondLst><p:cond delay="indefinite"/></p:stCondLst>
  </p:cTn><p:tgtEl><p:spTgt spid/></p:tgtEl></p:cMediaNode></p:video>`. It goes
  in the root `tmRoot` childTnLst, after the main sequence `p:seq`.
- Two MediaPlay effects on the same target with different volumes are a
  validation error.
- The parser accepts exactly this pairing: a mediacall effect needs a matching
  `p:video` node for its target. Anything else, including a `p:audio` node or
  extra cMediaNode timing, makes the slide timing opaque for that target.

**State.** Shapes with any MediaPlay effect get `media: { playing: bool,
position_ms: u64 }`; other shapes get `media: null`.
- Before the effect starts, `playing` is false and the position is 0.
- While it is active, `playing` is true and the position is
  `from_ms + (t - begin)`.
- After it ends, `playing` is false and the position stays at
  `from_ms + duration_ms` (hold).
- A later MediaPlay that starts on the same target overrides the earlier one,
  using the same begin-order rule as other effects.
- Visibility, opacity and geometry are unaffected.

The `state-v1.json` schema gains the nullable, required `media` property on
each shape. The effect's active duration is `duration_ms`, and media effects
never repeat.
