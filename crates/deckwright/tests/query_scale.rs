use deckwright::query::{Geometry, QuerySession, ShapeSnapshot, SlideSnapshot};
use deckwright::timing::*;

// Shape 2 is 50 x 30 at (100, 20): centre (125, 35). Shape 3 shares the geometry.
fn shape(id: u32) -> ShapeSnapshot {
    ShapeSnapshot {
        id,
        name: format!("!!s{id}"),
        text: String::new(),
        parent: None,
        z_order: id as usize,
        geometry: Some(Geometry {
            x: 100.0,
            y: 20.0,
            width: 50.0,
            height: 30.0,
        }),
        hidden: false,
        warnings: vec![],
    }
}

fn session(stages: Vec<Stage>) -> QuerySession {
    QuerySession::from_snapshots(vec![SlideSnapshot {
        part: "neutral".into(),
        width: 1000.0,
        height: 500.0,
        shapes: vec![shape(2), shape(3)],
        timing: ParsedTiming {
            timeline: Timeline { stages },
            ..Default::default()
        },
    }])
}

fn stage(trigger: Trigger, target: u32, kind: EffectKind) -> Stage {
    Stage {
        trigger,
        delay_ms: 0,
        effects: vec![Effect {
            target,
            delay_ms: 0,
            duration_ms: 1000,
            kind,
        }],
    }
}

fn grow(x: f64, repeat: Repeat, auto_reverse: bool) -> EffectKind {
    EffectKind::Scale {
        x,
        y: x,
        repeat,
        auto_reverse,
    }
}

fn once() -> Repeat {
    Repeat::Count { thousandths: 1000 }
}

/// (x, y, width, height) of shape 2 at sample time `t`.
fn rect(s: &QuerySession, clicks: &[u64], t: u64) -> (f64, f64, f64, f64) {
    let report = s.state(1, clicks, t).unwrap();
    let g = report
        .shapes
        .iter()
        .find(|x| x.id == 2)
        .unwrap()
        .geometry
        .clone()
        .unwrap();
    (g.x, g.y, g.width, g.height)
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}

fn assert_rect(actual: (f64, f64, f64, f64), expected: (f64, f64, f64, f64)) {
    close(actual.0, expected.0);
    close(actual.1, expected.1);
    close(actual.2, expected.2);
    close(actual.3, expected.3);
}

#[test]
fn mid_forward_factor_scales_about_centre() {
    let s = session(vec![stage(Trigger::OnClick, 2, grow(2.0, once(), false))]);
    // Half way through the forward pass the factor is 1.5: 75 x 45 centred on (125, 35).
    assert_rect(rect(&s, &[0], 500), (87.5, 12.5, 75.0, 45.0));
}

#[test]
fn end_of_iteration_reaches_the_full_factor() {
    let s = session(vec![stage(Trigger::OnClick, 2, grow(2.0, once(), false))]);
    assert_rect(rect(&s, &[0], 1000), (75.0, 5.0, 100.0, 60.0));
}

#[test]
fn auto_reverse_returns_to_one_at_twice_the_duration() {
    let s = session(vec![stage(Trigger::OnClick, 2, grow(2.0, once(), true))]);
    assert_rect(rect(&s, &[0], 1000), (75.0, 5.0, 100.0, 60.0));
    // The return half: factor 1.5 at 1.5 d.
    assert_rect(rect(&s, &[0], 1500), (87.5, 12.5, 75.0, 45.0));
    assert_rect(rect(&s, &[0], 2000), (100.0, 20.0, 50.0, 30.0));
    assert_rect(rect(&s, &[0], 9000), (100.0, 20.0, 50.0, 30.0));
}

#[test]
fn finite_count_holds_its_active_end() {
    let s = session(vec![stage(
        Trigger::OnClick,
        2,
        grow(2.0, Repeat::Count { thousandths: 2500 }, false),
    )]);
    // 2.5 passes end half way through the third forward pass: factor 1.5, then held.
    assert_rect(rect(&s, &[0], 2500), (87.5, 12.5, 75.0, 45.0));
    assert_rect(rect(&s, &[0], 99_999), (87.5, 12.5, 75.0, 45.0));
}

#[test]
fn indefinite_repeat_equals_value_at_t_mod_iteration() {
    let s = session(vec![stage(
        Trigger::OnClick,
        2,
        grow(2.0, Repeat::Indefinite, true),
    )]);
    // Iteration is 2d = 2000 ms; 1_001_500 mod 2000 = 1500.
    let large = rect(&s, &[0], 1_001_500);
    assert_rect(large, rect(&s, &[0], 1500));
    assert_rect(large, (87.5, 12.5, 75.0, 45.0));
}

#[test]
fn scaling_preserves_the_centre() {
    let s = session(vec![stage(Trigger::OnClick, 2, grow(3.0, once(), false))]);
    let (x, y, w, h) = rect(&s, &[0], 300);
    close(x + w / 2.0, 125.0);
    close(y + h / 2.0, 35.0);
    close(w, 50.0 * 1.6);
    close(h, 30.0 * 1.6);
}

#[test]
fn two_scales_compose_multiplicatively() {
    let s = session(vec![
        stage(Trigger::OnClick, 2, grow(2.0, once(), false)),
        stage(Trigger::OnClick, 2, grow(2.0, once(), false)),
    ]);
    // Two finished factors of 2 multiply to 4: 50 becomes 200 wide, centred on 125.
    let (x, _, w, _) = rect(&s, &[0, 0], 1000);
    close(w, 200.0);
    close(x + w / 2.0, 125.0);
}

#[test]
fn scale_applies_after_motion_about_the_moved_centre() {
    let motion = EffectKind::Motion {
        from_x: 0.2,
        from_y: 0.0,
        to_x: 0.0,
        to_y: 0.0,
        accelerate: 0.0,
        decelerate: 0.0,
    };
    let s = session(vec![
        stage(Trigger::OnClick, 2, motion),
        stage(Trigger::WithPrevious, 2, grow(2.0, once(), false)),
    ]);
    // Motion at p = 0.5 moves the shape 100 units right to x = 200 (centre 225). Scale at
    // 0.5 gives 1.5: width 75, left edge 225 - 37.5.
    let (x, _, w, _) = rect(&s, &[0], 500);
    close(w, 75.0);
    close(x, 187.5);
    close(x + w / 2.0, 225.0);
}

#[test]
fn after_previous_after_an_indefinite_scale_never_starts() {
    let s = session(vec![
        stage(Trigger::OnClick, 2, grow(2.0, Repeat::Indefinite, false)),
        stage(Trigger::AfterPrevious, 3, EffectKind::Appear),
    ]);
    let report = s.state(1, &[0], 1_000_000).unwrap();
    let late = report.shapes.iter().find(|x| x.id == 3).unwrap();
    assert_eq!((late.visible, late.opacity), (Some(false), Some(0.0)));
}

#[test]
fn after_previous_after_a_finite_scale_starts_at_its_active_end() {
    let s = session(vec![
        stage(
            Trigger::OnClick,
            2,
            grow(2.0, Repeat::Count { thousandths: 2500 }, false),
        ),
        stage(Trigger::AfterPrevious, 3, EffectKind::Appear),
    ]);
    let before = s.state(1, &[0], 2499).unwrap();
    assert_eq!(before.shapes[1].visible, Some(false));
    let at = s.state(1, &[0], 2500).unwrap();
    assert_eq!(at.shapes[1].visible, Some(true));
}

#[test]
fn pending_scale_changes_nothing() {
    let s = session(vec![stage(Trigger::OnClick, 2, grow(2.0, once(), false))]);
    let report = s.state(1, &[], 5000).unwrap();
    let shape = report.shapes.iter().find(|x| x.id == 2).unwrap();
    assert_eq!(shape.visible, Some(true));
    assert_eq!(shape.opacity, Some(1.0));
    assert_rect(rect(&s, &[], 5000), (100.0, 20.0, 50.0, 30.0));
}
