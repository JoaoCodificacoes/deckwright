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
