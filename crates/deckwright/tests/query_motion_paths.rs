//! Layout-relative motion paths (query.md rule 6): the offset is from + (to - from) * eased(p)
//! times the slide size, holds at `to` after the end, and only the latest-begun started motion
//! on a shape sets it.
use deckwright::query::{Geometry, QuerySession, ShapeSnapshot, ShapeState, SlideSnapshot};
use deckwright::timing::*;

const W: f64 = 1000.0;
const H: f64 = 500.0;
// Static (layout) position of every shape on the test slide.
const X0: f64 = 100.0;
const Y0: f64 = 20.0;

fn shape(id: u32) -> ShapeSnapshot {
    ShapeSnapshot {
        id,
        name: format!("!!{id}"),
        text: String::new(),
        parent: None,
        z_order: id as usize,
        geometry: Some(Geometry {
            x: X0,
            y: Y0,
            width: 50.0,
            height: 30.0,
        }),
        hidden: false,
        warnings: vec![],
    }
}

fn effect(target: u32, kind: EffectKind, delay_ms: i64, duration_ms: u32) -> Effect {
    Effect {
        target,
        delay_ms,
        duration_ms,
        kind,
    }
}

fn motion(from: (f64, f64), to: (f64, f64), duration_ms: u32) -> (EffectKind, u32) {
    (
        EffectKind::Motion {
            from_x: from.0,
            from_y: from.1,
            to_x: to.0,
            to_y: to.1,
            accelerate: 0.0,
            decelerate: 0.0,
        },
        duration_ms,
    )
}

fn stage(trigger: Trigger, effects: Vec<Effect>) -> Stage {
    Stage {
        trigger,
        delay_ms: 0,
        effects,
    }
}

fn session(stages: Vec<Stage>) -> QuerySession {
    QuerySession::from_snapshots(vec![SlideSnapshot {
        part: "neutral".into(),
        width: W,
        height: H,
        shapes: vec![shape(2), shape(3)],
        timing: ParsedTiming {
            timeline: Timeline { stages },
            ..Default::default()
        },
    }])
}

fn look(s: &QuerySession, clicks: &[u64], t: u64, id: u32) -> ShapeState {
    let report = s.state(1, clicks, t).unwrap();
    report.shapes.into_iter().find(|x| x.id == id).unwrap()
}

/// Geometry (x, y) of shape 2 at time `t`, with the given click history.
fn at(s: &QuerySession, clicks: &[u64], t: u64) -> (f64, f64) {
    let g = look(s, clicks, t, 2).geometry.unwrap();
    (g.x, g.y)
}

fn assert_near(actual: (f64, f64), expected: (f64, f64)) {
    assert!(
        (actual.0 - expected.0).abs() < 1e-9 && (actual.1 - expected.1).abs() < 1e-9,
        "{actual:?} != {expected:?}"
    );
}

#[test]
fn single_path_to_nonzero_endpoint_holds_its_offset() {
    let (kind, dur) = motion((0.0, 0.0), (0.25, 0.0), 1000);
    let s = session(vec![stage(Trigger::OnClick, vec![effect(2, kind, 0, dur)])]);
    // Starts at the static position, moves linearly, and holds at +0.25 slide widths.
    assert_near(at(&s, &[0], 0), (X0, Y0));
    assert_near(at(&s, &[0], 500), (X0 + 125.0, Y0));
    assert_near(at(&s, &[0], 1000), (X0 + 250.0, Y0));
    assert_near(at(&s, &[0], 5000), (X0 + 250.0, Y0));
}

#[test]
fn four_segment_chain_is_continuous_and_returns_to_static() {
    // Points a, b, c, d are offsets from the static position; the last segment ends at (0, 0).
    let a = (-0.1, 0.05);
    let b = (0.2, 0.1);
    let c = (0.3, -0.2);
    let d = (-0.4, 0.3);
    let origin = (0.0, 0.0);
    let segments = [(a, b), (b, c), (c, d), (d, origin)];
    let stages = segments
        .iter()
        .enumerate()
        .map(|(i, &(from, to))| {
            let (kind, dur) = motion(from, to, 1000);
            let trigger = if i == 0 {
                Trigger::OnClick
            } else {
                Trigger::AfterPrevious
            };
            stage(trigger, vec![effect(2, kind, 0, dur)])
        })
        .collect();
    let s = session(stages);
    let pos = |o: (f64, f64)| (X0 + o.0 * W, Y0 + o.1 * H);
    // Each boundary lands on the shared point, from both the ending and the starting segment.
    assert_near(at(&s, &[0], 0), pos(a));
    assert_near(at(&s, &[0], 1000), pos(b));
    assert_near(at(&s, &[0], 2000), pos(c));
    assert_near(at(&s, &[0], 3000), pos(d));
    assert_near(at(&s, &[0], 4000), pos(origin));
    assert_near(at(&s, &[0], 9000), pos(origin));
    // Inside a segment the offset is linear between its endpoints.
    let mid_bc = ((b.0 + c.0) / 2.0, (b.1 + c.1) / 2.0);
    assert_near(at(&s, &[0], 1500), pos(mid_bc));
    assert_near(at(&s, &[0], 3500), pos((d.0 / 2.0, d.1 / 2.0)));
}

#[test]
fn later_begun_motion_overrides_and_offsets_are_not_summed() {
    // A moves right over [0, 2000). B starts at 500 and moves down over [500, 1500).
    let (ka, da) = motion((0.0, 0.0), (0.5, 0.0), 2000);
    let (kb, db) = motion((0.0, 0.0), (0.0, 0.2), 1000);
    let s = session(vec![
        stage(Trigger::OnClick, vec![effect(2, ka, 0, da)]),
        stage(Trigger::OnClick, vec![effect(2, kb, 0, db)]),
    ]);
    // At 1000 both are active at progress one half. Summing would give (250, 50); the latest
    // begun, B, sets the offset alone: (0, 0.1 * H).
    assert_near(at(&s, &[0, 500], 1000), (X0, Y0 + 0.1 * H));
    // A has finished and B holds at its end. A's (0.5, 0) does not come back.
    assert_near(at(&s, &[0, 500], 2500), (X0, Y0 + 0.2 * H));
    // Before B begins (its click has not happened), A alone sets the offset.
    // Progress 250 / 2000 of a 0.5 slide-width path: offset 0.0625 * W.
    assert_near(at(&s, &[0], 250), (X0 + 0.0625 * W, Y0));
}

#[test]
fn ties_in_begin_time_go_to_the_later_effect_in_document_order() {
    let (ka, da) = motion((0.0, 0.0), (0.5, 0.0), 1000);
    let (kb, db) = motion((0.0, 0.0), (0.0, 0.25), 1000);
    let s = session(vec![stage(
        Trigger::OnClick,
        vec![effect(2, ka, 0, da), effect(2, kb, 0, db)],
    )]);
    assert_near(at(&s, &[0], 1000), (X0, Y0 + 0.25 * H));
}

#[test]
fn pending_motion_does_nothing() {
    // A has started; B waits for a click that never comes and must not add its offset.
    let (ka, da) = motion((0.0, 0.0), (0.25, 0.0), 1000);
    let (kb, db) = motion((0.0, 0.0), (0.0, 0.5), 1000);
    let s = session(vec![
        stage(Trigger::OnClick, vec![effect(2, ka, 0, da)]),
        stage(Trigger::OnClick, vec![effect(2, kb, 0, db)]),
    ]);
    assert_near(at(&s, &[0], 1500), (X0 + 250.0, Y0));
    // With no clicks at all nothing has started: the static position.
    assert_near(at(&s, &[], 5000), (X0, Y0));
}

#[test]
fn motion_on_one_shape_leaves_another_at_its_static_position() {
    let (k, d) = motion((0.0, 0.0), (0.25, 0.0), 1000);
    let s = session(vec![stage(Trigger::OnClick, vec![effect(2, k, 0, d)])]);
    let other = look(&s, &[0], 1000, 3).geometry.unwrap();
    assert_near((other.x, other.y), (X0, Y0));
}
