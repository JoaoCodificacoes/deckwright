use deckwright::query::{Geometry, QuerySession, ShapeSnapshot, SlideSnapshot};
use deckwright::timing::*;
use proptest::prelude::*;

fn effect(target: u32, delay_ms: i64, duration_ms: u32, kind: EffectKind) -> Effect {
    Effect {
        target,
        delay_ms,
        duration_ms,
        kind,
    }
}

fn stage(trigger: Trigger, delay_ms: i64, effects: Vec<Effect>) -> Stage {
    Stage {
        trigger,
        delay_ms,
        effects,
    }
}

fn timing(stages: Vec<Stage>) -> ParsedTiming {
    ParsedTiming {
        timeline: Timeline { stages },
        ..Default::default()
    }
}

/// Stage 0: shape 2 fades in, shape 3 appears, shape 4 fades in.
/// Stage 1: shape 2 fades out after the previous stage.
fn base() -> Timeline {
    Timeline {
        stages: vec![
            stage(
                Trigger::OnClick,
                0,
                vec![
                    effect(2, 0, 1000, EffectKind::FadeIn),
                    effect(3, 100, 500, EffectKind::Appear),
                    effect(4, 0, 300, EffectKind::FadeIn),
                ],
            ),
            stage(
                Trigger::AfterPrevious,
                0,
                vec![effect(2, 0, 400, EffectKind::FadeOut)],
            ),
        ],
    }
}

#[test]
fn canonicalize_is_idempotent() {
    let once = canonicalize(&base());
    assert_eq!(canonicalize(&once), once);
}

#[test]
fn reordering_different_targets_gives_equal_canonical_forms() {
    let t = base();
    let mut r = t.clone();
    r.stages[0].effects.reverse();
    assert_ne!(t, r);
    assert_eq!(canonicalize(&t), canonicalize(&r));
}

#[test]
fn swapping_same_target_effects_changes_canonical_form() {
    // Two effects on shape 2: the later one determines the final state, so order matters.
    let mut a = base();
    a.stages[0].effects = vec![
        effect(2, 0, 1000, EffectKind::FadeIn),
        effect(2, 0, 500, EffectKind::FadeOut),
    ];
    let mut b = a.clone();
    b.stages[0].effects.swap(0, 1);
    assert_ne!(canonicalize(&a), canonicalize(&b));
}

#[test]
fn stage_order_triggers_and_delays_are_preserved() {
    let t = base();

    let mut moved = t.clone();
    let e = moved.stages[1].effects.remove(0);
    moved.stages[0].effects.push(e);
    assert_ne!(canonicalize(&t), canonicalize(&moved));

    let mut delayed = t.clone();
    delayed.stages[1].delay_ms = 50;
    assert_ne!(canonicalize(&t), canonicalize(&delayed));

    let mut retriggered = t.clone();
    retriggered.stages[1].trigger = Trigger::WithPrevious;
    assert_ne!(canonicalize(&t), canonicalize(&retriggered));
}

#[test]
fn equivalent_compares_canonical_form_and_transition() {
    let a = timing(base().stages);

    let mut reordered = a.clone();
    reordered.timeline.stages[0].effects.rotate_left(1);
    assert_eq!(equivalent(&a, &reordered), Some(true));

    let mut longer = a.clone();
    longer.timeline.stages[1].effects[0].duration_ms = 999;
    assert_eq!(equivalent(&a, &longer), Some(false));

    let mut faded = a.clone();
    faded.transition = Transition::Fade { duration_ms: 700 };
    assert_eq!(equivalent(&a, &faded), Some(false));
}

#[test]
fn equivalent_is_unknown_with_opaque_timing() {
    let a = timing(base().stages);
    let mut opaque = a.clone();
    opaque.opaque.push(OpaqueTiming {
        targets: vec![2],
        reason: "unrecognized".into(),
    });
    assert_eq!(equivalent(&a, &opaque), None);
    assert_eq!(equivalent(&opaque, &a), None);
}

fn shape(id: u32) -> ShapeSnapshot {
    ShapeSnapshot {
        id,
        name: format!("!!{id}"),
        text: "text".into(),
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

fn session(timing: ParsedTiming) -> QuerySession {
    QuerySession::from_snapshots(vec![SlideSnapshot {
        part: "neutral".into(),
        width: 1000.0,
        height: 500.0,
        shapes: vec![shape(2), shape(3), shape(4)],
        timing,
    }])
}

#[test]
fn query_states_match_for_canonical_form() {
    let mut reordered = base();
    reordered.stages[0].effects.swap(0, 2);
    let hand_built = Timeline {
        stages: vec![
            stage(
                Trigger::OnClick,
                0,
                vec![
                    effect(3, 0, 800, EffectKind::FadeIn),
                    effect(2, 200, 600, EffectKind::FadeIn),
                    effect(2, 0, 400, EffectKind::FadeOut),
                ],
            ),
            stage(
                Trigger::WithPrevious,
                100,
                vec![
                    effect(4, 0, 900, EffectKind::FadeIn),
                    effect(3, 0, 300, EffectKind::FadeOut),
                ],
            ),
        ],
    };
    let times = [0_u64, 50, 100, 300, 600, 900, 1100, 1500, 2500];
    for t in [base(), reordered, hand_built] {
        let original = session(timing(t.stages.clone()));
        let canon = session(timing(canonicalize(&t).stages));
        for &time in &times {
            let clicks: Vec<u64> = if time >= 100 { vec![100] } else { vec![] };
            for history in [vec![], clicks] {
                let a = serde_json::to_value(original.state(1, &history, time).unwrap());
                let b = serde_json::to_value(canon.state(1, &history, time).unwrap());
                assert_eq!(a.unwrap(), b.unwrap(), "time {time} clicks {history:?}");
            }
        }
    }
}

// ---- property: within-stage permutations that keep same-target order ----

fn effect_kind() -> impl Strategy<Value = EffectKind> {
    let motion = (-1.0..=1.0f64, -1.0..=1.0f64, 0.0..=1.0f64, 0.0..=1.0f64).prop_map(
        |(from_x, from_y, a, d)| EffectKind::Motion {
            from_x,
            from_y,
            to_x: 0.0,
            to_y: 0.0,
            accelerate: a,
            decelerate: d * (1.0 - a),
        },
    );
    prop_oneof![
        Just(EffectKind::Appear),
        Just(EffectKind::FadeIn),
        Just(EffectKind::FadeOut),
        Just(EffectKind::WipeIn {
            direction: Direction::Up
        }),
        motion,
    ]
}

fn valid_timeline() -> impl Strategy<Value = Timeline> {
    let trigger = prop_oneof![
        Just(Trigger::OnClick),
        Just(Trigger::WithPrevious),
        Just(Trigger::AfterPrevious),
        Just(Trigger::OnSlideStart),
    ];
    let effects =
        proptest::collection::vec((1..=4u32, 0..=500i64, 1..=1000u32, effect_kind()), 1..=3);
    let st = (trigger, 0..=500i64, effects).prop_map(|(trig, delay, effs)| {
        stage(
            trig,
            delay,
            effs.into_iter()
                .map(|(t, d, dur, k)| effect(t, d, dur, k))
                .collect(),
        )
    });
    proptest::collection::vec(st, 1..=4)
        .prop_map(|stages| Timeline { stages })
        .prop_filter("valid timeline", |t| {
            validate_timeline(t).is_ok()
                && t.stages[0].trigger != Trigger::WithPrevious
                && t.stages[0].trigger != Trigger::AfterPrevious
        })
}

/// Reassigns random keys so same-target effects keep their original relative order,
/// then sorts the stage by the assigned keys.
fn permute_stage(stage: &Stage, keys: &[u64]) -> Stage {
    let n = stage.effects.len();
    let mut assigned = vec![0u64; n];
    for target in stage.effects.iter().map(|e| e.target) {
        let idx: Vec<usize> = (0..n)
            .filter(|&i| stage.effects[i].target == target)
            .collect();
        let mut vals: Vec<u64> = idx.iter().map(|&i| keys[i]).collect();
        vals.sort_unstable();
        for (i, v) in idx.into_iter().zip(vals) {
            assigned[i] = v;
        }
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| (assigned[i], i));
    stage_with(
        stage,
        order
            .into_iter()
            .map(|i| stage.effects[i].clone())
            .collect(),
    )
}

fn stage_with(stage: &Stage, effects: Vec<Effect>) -> Stage {
    Stage {
        trigger: stage.trigger.clone(),
        delay_ms: stage.delay_ms,
        effects,
    }
}

fn multiset(t: &Timeline) -> Vec<String> {
    let mut v: Vec<String> = t
        .stages
        .iter()
        .flat_map(|s| s.effects.iter().map(|e| format!("{e:?}")))
        .collect();
    v.sort();
    v
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn same_target_preserving_permutations_are_canonically_equal(
        t in valid_timeline(),
        keys in proptest::collection::vec(proptest::collection::vec(any::<u64>(), 3), 4),
    ) {
        let permuted = Timeline {
            stages: t.stages.iter().zip(&keys).map(|(s, k)| permute_stage(s, k)).collect(),
        };
        prop_assert_eq!(canonicalize(&permuted), canonicalize(&t));
        prop_assert_eq!(multiset(&canonicalize(&t)), multiset(&t));
        prop_assert_eq!(canonicalize(&canonicalize(&t)), canonicalize(&t));
    }
}
