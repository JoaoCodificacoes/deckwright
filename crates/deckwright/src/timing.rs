//! Typed, deliberately bounded animation semantics. All times are milliseconds.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timeline { pub stages: Vec<Stage> }
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stage { pub trigger: Trigger, pub delay_ms: i64, pub effects: Vec<Effect> }
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Trigger { OnClick, WithPrevious, AfterPrevious, OnSlideStart }
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effect { pub target: u32, pub delay_ms: i64, pub duration_ms: u32, pub kind: EffectKind }
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag="type", rename_all="snake_case", deny_unknown_fields)]
pub enum EffectKind {
    Appear,
    FadeIn,
    WipeIn { direction: Direction },
    FadeOut,
    /// Relative to slide width/height; the static position is the resting END.
    /// At progress zero the offset is (from_x, from_y); at one it is (to_x,to_y).
    Motion { from_x: f64, from_y: f64, to_x: f64, to_y: f64, accelerate: f64, decelerate: f64 },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub enum Direction { Left, Right, Up, Down }
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag="type", rename_all="snake_case", deny_unknown_fields)]
pub enum Transition { #[default] None, Fade, Morph }
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParsedTiming { pub timeline: Timeline, pub transition: Transition, pub opaque: Vec<OpaqueTiming> }
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpaqueTiming { pub targets: Vec<u32>, pub reason: String }
