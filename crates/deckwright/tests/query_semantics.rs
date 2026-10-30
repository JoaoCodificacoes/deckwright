use deckwright::query::{Geometry, QuerySession, ShapeSnapshot, ShapeState, SlideSnapshot};
use deckwright::timing::*;

fn shape(id: u32) -> ShapeSnapshot {
    ShapeSnapshot {
        id,
        name: format!("!!{id}"),
        text: String::new(),
        parent: None,
        z_order: 0,
        geometry: Some(Geometry {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
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

fn click(effects: Vec<Effect>) -> Stage {
    Stage {
        trigger: Trigger::OnClick,
        delay_ms: 0,
        effects,
    }
}

fn session(shape_ids: &[u32], stages: Vec<Stage>) -> QuerySession {
    QuerySession::from_snapshots(vec![SlideSnapshot {
        part: "neutral".into(),
        width: 1000.0,
        height: 500.0,
        shapes: shape_ids.iter().map(|id| shape(*id)).collect(),
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

#[test]
fn later_stage_without_click_does_not_hide_started_appear() {
    let s = session(
        &[11],
        vec![
            click(vec![effect(11, EffectKind::Appear, 0, 1000)]),
            click(vec![
                effect(11, EffectKind::FadeOut, 172, 1),
                effect(11, EffectKind::Appear, 0, 1),
            ]),
        ],
    );
    let r = look(&s, &[4871], 4871, 11);
    assert_eq!(r.visible, Some(true));
    assert_eq!(r.opacity, Some(1.0));
}

#[test]
fn pending_wipe_keeps_shape_hidden_with_full_wipe() {
    let s = session(
        &[2],
        vec![
            click(vec![effect(2, EffectKind::Appear, 0, 1)]),
            click(vec![effect(
                2,
                EffectKind::WipeIn {
                    direction: Direction::Left,
                },
                0,
                1,
            )]),
        ],
    );
    let r = look(&s, &[], 0, 2);
    assert_eq!(r.visible, Some(false));
    assert_eq!(r.opacity, Some(0.0));
    assert_eq!(r.wipe_fraction, Some(1.0));
}

#[test]
fn later_begun_fade_in_wins_over_earlier_fade_out() {
    let s = session(
        &[40],
        vec![
            click(vec![effect(40, EffectKind::FadeIn, 535, 1)]),
            click(vec![effect(40, EffectKind::FadeOut, 0, 1)]),
        ],
    );
    let r = look(&s, &[376, 502], 8616, 40);
    assert_eq!(r.visible, Some(true));
    assert_eq!(r.opacity, Some(1.0));
}

#[test]
fn reveal_after_exit_is_fully_opaque() {
    let s = session(
        &[4],
        vec![
            click(vec![effect(4, EffectKind::FadeOut, 0, 1)]),
            click(vec![effect(4, EffectKind::Appear, 0, 1)]),
        ],
    );
    let r = look(&s, &[1482, 4186], 4186, 4);
    assert_eq!(r.visible, Some(true));
    assert_eq!(r.opacity, Some(1.0));
}

#[test]
fn active_fade_in_keeps_its_progress_under_later_appear() {
    let s = session(
        &[33],
        vec![
            click(vec![effect(33, EffectKind::FadeIn, 166, 1929)]),
            click(vec![effect(33, EffectKind::Appear, 0, 1)]),
        ],
    );
    let r = look(&s, &[6699, 6952], 6952, 33);
    assert_eq!(r.visible, Some(true));
    let expected = (6952.0 - 6865.0) / 1929.0;
    assert!((r.opacity.unwrap() - expected).abs() < 1e-9);
}

#[test]
fn finished_fade_out_hides_with_zero_opacity() {
    let s = session(
        &[5],
        vec![click(vec![effect(5, EffectKind::FadeOut, 0, 1000)])],
    );
    let r = look(&s, &[0], 1000, 5);
    assert_eq!(r.visible, Some(false));
    assert_eq!(r.opacity, Some(0.0));
}

#[test]
fn pending_entrance_does_not_hide_fade_out_first_shape() {
    let s = session(
        &[5],
        vec![
            click(vec![effect(5, EffectKind::FadeOut, 0, 1)]),
            click(vec![effect(5, EffectKind::FadeIn, 0, 1)]),
        ],
    );
    let r = look(&s, &[], 0, 5);
    assert_eq!(r.visible, Some(true));
    assert_eq!(r.opacity, Some(1.0));
}

#[test]
fn same_begin_ties_apply_in_document_order() {
    let s = session(
        &[7],
        vec![
            click(vec![effect(7, EffectKind::FadeOut, 0, 1)]),
            Stage {
                trigger: Trigger::WithPrevious,
                delay_ms: 0,
                effects: vec![effect(7, EffectKind::Appear, 0, 1)],
            },
        ],
    );
    assert_eq!(look(&s, &[0], 5, 7).visible, Some(true));

    let s = session(
        &[7],
        vec![
            click(vec![effect(7, EffectKind::Appear, 0, 1)]),
            Stage {
                trigger: Trigger::WithPrevious,
                delay_ms: 0,
                effects: vec![effect(7, EffectKind::FadeOut, 0, 1)],
            },
        ],
    );
    assert_eq!(look(&s, &[0], 5, 7).visible, Some(false));
}
