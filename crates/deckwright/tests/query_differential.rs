//! Differential check: `QuerySession::state` against a naive reference written from
//! docs/decisions/{timing,query}.md and tests/query.rs. The reference shares no code with
//! src/query.rs: stage anchors come from a plain loop, effect windows are absolute, and
//! each shape folds its started effects in time order.
use deckwright::query::{Geometry, QuerySession, ShapeSnapshot, SlideSnapshot, StateReport};
use deckwright::timing::*;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

const W: f64 = 1000.0;
const H: f64 = 500.0;

fn shape(id: u32, geometry: Option<Geometry>) -> ShapeSnapshot {
    let (name, text, parent, z_order) = (format!("!!s{id}"), String::new(), None, id as usize);
    ShapeSnapshot {
        id,
        name,
        text,
        parent,
        z_order,
        geometry,
        hidden: false,
        warnings: vec![],
    }
}

fn slide(shapes: Vec<ShapeSnapshot>, stages: Vec<Stage>) -> SlideSnapshot {
    let timing = ParsedTiming {
        timeline: Timeline { stages },
        ..Default::default()
    };
    SlideSnapshot {
        part: "neutral".into(),
        width: W,
        height: H,
        shapes,
        timing,
    }
}

/// Absolute begin of every effect in document order; `None` when its stage has not started.
/// OnClick consumes clicks in order. WithPrevious anchors at the previous stage start and
/// AfterPrevious at its latest effect end; each stage adds its own delay.
fn windows<'a>(tl: &'a Timeline, clicks: &[u64]) -> Vec<(Option<i128>, &'a Effect)> {
    let (mut starts, mut out, mut next_click) = (Vec::<Option<i128>>::new(), vec![], clicks.iter());
    for (i, stage) in tl.stages.iter().enumerate() {
        let anchor = match stage.trigger {
            Trigger::OnSlideStart => Some(0),
            Trigger::OnClick => next_click.next().map(|c| i128::from(*c)),
            Trigger::WithPrevious => starts[i - 1],
            Trigger::AfterPrevious => starts[i - 1].and_then(|s| {
                let ends = tl.stages[i - 1].effects.iter();
                ends.map(|e| s + i128::from(e.delay_ms + e.duration_ms as i64))
                    .max()
            }),
        };
        let start = anchor.map(|a| a + i128::from(stage.delay_ms));
        starts.push(start);
        out.extend(
            stage
                .effects
                .iter()
                .map(|e| (start.map(|s| s + i128::from(e.delay_ms)), e)),
        );
    }
    out
}

fn entrance(kind: &EffectKind) -> bool {
    matches!(
        kind,
        EffectKind::Appear | EffectKind::FadeIn | EffectKind::WipeIn { .. }
    )
}

fn progress(elapsed: i128, duration_ms: u32) -> f64 {
    (elapsed as f64 / f64::from(duration_ms)).clamp(0.0, 1.0)
}

/// Path position: the integral of a velocity that ramps up over [0,a], is constant, and
/// ramps down over [1-d,1]. Its total area is 1.
fn eased(p: f64, a: f64, d: f64) -> f64 {
    let speed = 1.0 / (1.0 - (a + d) / 2.0);
    let ramp = |x: f64, r: f64| if r > 0.0 { (x / r).min(1.0) } else { 1.0 };
    let v = |s: f64| speed * ramp(s, a).min(ramp(1.0 - s, d));
    let mut cuts = vec![0.0, p];
    cuts.extend([a, 1.0 - d].into_iter().filter(|x| (0.0..p).contains(x)));
    cuts.sort_by(f64::total_cmp);
    cuts.windows(2)
        .map(|w| (w[1] - w[0]) * (v(w[0]) + v(w[1])) / 2.0)
        .sum()
}

/// (visible, opacity, wipe_fraction, geometry) of one shape at time `t`, per query.md rules
/// 1-7. Started effects are folded in (begin, document index) order; a fade or wipe only
/// counts while active, and a finished fade holds nothing.
fn reference(
    shape: &ShapeSnapshot,
    w: &[(Option<i128>, &Effect)],
    t: i128,
) -> (bool, f64, f64, Geometry) {
    // `w` is in document order, so `mine.first()` is the shape's first effect in the file.
    let mine: Vec<_> = w
        .iter()
        .enumerate()
        .filter(|(_, (_, e))| e.target == shape.id)
        .map(|(i, (b, e))| (*b, i, *e))
        .collect();
    let hidden_first = mine.first().is_some_and(|(_, _, e)| entrance(&e.kind));
    let mut started: Vec<_> = mine
        .iter()
        .filter_map(|(b, i, e)| b.filter(|b| *b <= t).map(|b| (b, *i, *e)))
        .collect();
    started.sort_by_key(|(b, i, _)| (*b, *i));
    let (mut visible, mut opacity, mut wipe) = (!hidden_first, 1.0, 1.0);
    for (begin, _, e) in &started {
        let elapsed = t - begin;
        let p = progress(elapsed, e.duration_ms);
        let active = elapsed < i128::from(e.duration_ms);
        match e.kind {
            EffectKind::Appear | EffectKind::FadeIn | EffectKind::WipeIn { .. } => visible = true,
            EffectKind::FadeOut => visible = p < 1.0,
            EffectKind::Motion { .. } => {}
        }
        if active {
            match e.kind {
                EffectKind::FadeIn => opacity = p,
                EffectKind::FadeOut => opacity = 1.0 - p,
                EffectKind::WipeIn { .. } => wipe = p,
                _ => {}
            }
        }
    }
    if !visible {
        opacity = 0.0;
    }
    // Started motion offsets add up from the resting endpoint; a pending motion does nothing.
    let mut g = shape
        .geometry
        .clone()
        .expect("generated shapes have geometry");
    for (b, _, e) in &mine {
        if let EffectKind::Motion {
            from_x,
            from_y,
            accelerate,
            decelerate,
            ..
        } = e.kind
        {
            let Some(b) = b.filter(|b| *b <= t) else {
                continue;
            };
            let p = progress(t - b, e.duration_ms);
            let k = 1.0 - eased(p, accelerate, decelerate);
            g.x += from_x * k * W;
            g.y += from_y * k * H;
        }
    }
    (visible, opacity, wipe, g)
}

fn compare(
    report: &StateReport,
    slide: &SlideSnapshot,
    clicks: &[u64],
    sample: u64,
) -> Result<(), TestCaseError> {
    let w = windows(&slide.timing.timeline, clicks);
    prop_assert_eq!(report.shapes.len(), slide.shapes.len());
    let near = |a: f64, b: f64| (a - b).abs() <= 1e-9;
    for (got, shape) in report.shapes.iter().zip(&slide.shapes) {
        let want = reference(shape, &w, i128::from(sample));
        let (visible, opacity, wipe, g) = (want.0, want.1, want.2, &want.3);
        let geo = got.geometry.as_ref().map(|x| (x.x, x.y, x.width, x.height));
        let geometry_ok = geo.is_some_and(|(x, y, cx, cy)| {
            near(x, g.x) && near(y, g.y) && near(cx, g.width) && near(cy, g.height)
        });
        let ok = got.visible == Some(visible)
            && got.opacity.is_some_and(|v| near(v, opacity))
            && got.wipe_fraction.is_some_and(|v| near(v, wipe))
            && geometry_ok;
        prop_assert!(
            ok,
            "shape {}: state {:?} vs reference {:?}",
            shape.id,
            got,
            want
        );
    }
    Ok(())
}

// ---- strategies: only the supported, valid subset ----

fn effect_kind() -> impl Strategy<Value = EffectKind> {
    let dirs = vec![
        Direction::Left,
        Direction::Right,
        Direction::Up,
        Direction::Down,
    ];
    let wipe =
        proptest::sample::select(dirs).prop_map(|direction| EffectKind::WipeIn { direction });
    let motion = (-1.0..=1.0, -1.0..=1.0, 0.0..=1.0, 0.0..=1.0).prop_map(|(x, y, a, d)| {
        let decelerate = d * (1.0 - a);
        EffectKind::Motion {
            from_x: x,
            from_y: y,
            to_x: 0.0,
            to_y: 0.0,
            accelerate: a,
            decelerate,
        }
    });
    prop_oneof![
        Just(EffectKind::Appear),
        Just(EffectKind::FadeIn),
        Just(EffectKind::FadeOut),
        wipe,
        motion
    ]
}

fn stage(trigger: Trigger, targets: Vec<u32>) -> impl Strategy<Value = Stage> {
    let delay = if matches!(trigger, Trigger::OnClick | Trigger::OnSlideStart) {
        (0..=1000i64).boxed()
    } else {
        (-500..=1000i64).boxed()
    };
    let effect = (
        proptest::sample::select(targets),
        0..=1000i64,
        1..=2000u32,
        effect_kind(),
    );
    let effects = prop::collection::vec(effect.prop_map(|(t, d, n, k)| eff(t, d, n, k)), 1..=3);
    (delay, effects).prop_map(move |(d, es)| stage_of(trigger.clone(), d, es))
}

fn timeline(targets: Vec<u32>) -> impl Strategy<Value = Timeline> {
    let any = |triggers: Vec<Trigger>| {
        let targets = targets.clone();
        proptest::sample::select(triggers).prop_flat_map(move |t| stage(t, targets.clone()))
    };
    let first = any(vec![Trigger::OnClick, Trigger::OnSlideStart]);
    let rest = vec![
        Trigger::OnClick,
        Trigger::OnSlideStart,
        Trigger::WithPrevious,
        Trigger::AfterPrevious,
    ];
    (first, prop::collection::vec(any(rest), 0..=4)).prop_map(|(first, rest)| Timeline {
        stages: std::iter::once(first).chain(rest).collect(),
    })
}

fn case() -> impl Strategy<Value = (SlideSnapshot, Vec<u64>, u64)> {
    let ids = proptest::collection::btree_set(2u32..=40, 1..=4)
        .prop_map(|s| s.into_iter().collect::<Vec<u32>>());
    ids.prop_flat_map(|ids| {
        let boxes =
            prop::collection::vec((0.0..900.0, 0.0..400.0, 1.0..100.0, 1.0..100.0), ids.len());
        // Nondecreasing click history of at most six clicks; sample at or after the last one.
        let clicks = prop::collection::vec(0u64..=9000, 0..=6).prop_flat_map(|mut c| {
            c.sort_unstable();
            let lo = c.last().copied().unwrap_or(0);
            (Just(c), lo..=10_000)
        });
        (Just(ids.clone()), boxes, timeline(ids), clicks).prop_map(
            |(ids, boxes, tl, (clicks, sample))| {
                let shapes = ids.iter().zip(boxes).map(|(&id, (x, y, width, height))| {
                    shape(
                        id,
                        Some(Geometry {
                            x,
                            y,
                            width,
                            height,
                        }),
                    )
                });
                (slide(shapes.collect(), tl.stages), clicks, sample)
            },
        )
    })
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 512,
        max_global_rejects: 65_536,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn state_agrees_with_naive_reference((slide, clicks, sample) in case()) {
        prop_assume!(validate_timeline(&slide.timing.timeline).is_ok());
        let session = QuerySession::from_snapshots(vec![slide.clone()]);
        let report = session
            .state(1, &clicks, sample)
            .map_err(|e| TestCaseError::fail(format!("state() errored: {e:?}")))?;
        compare(&report, &slide, &clicks, sample)?;
    }
}

// ---- pinned corners ----

fn stage_of(trigger: Trigger, delay_ms: i64, effects: Vec<Effect>) -> Stage {
    Stage {
        trigger,
        delay_ms,
        effects,
    }
}

fn eff(target: u32, delay_ms: i64, duration_ms: u32, kind: EffectKind) -> Effect {
    Effect {
        target,
        delay_ms,
        duration_ms,
        kind,
    }
}

fn pinned_slide(ids: &[u32], stages: Vec<Stage>) -> SlideSnapshot {
    let geometry = Geometry {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let shapes = ids
        .iter()
        .map(|&id| shape(id, Some(geometry.clone())))
        .collect();
    slide(shapes, stages)
}

fn pinned(ids: &[u32], stages: Vec<Stage>) -> QuerySession {
    QuerySession::from_snapshots(vec![pinned_slide(ids, stages)])
}

#[test]
fn negative_after_previous_overlap_starts_before_previous_effect_ends() {
    // Stage 1 runs [0, 1000). Stage 2 starts at 1000 - 500 = 500, overlapping it.
    let fade = |t| eff(t, 0, 1000, EffectKind::FadeIn);
    let s = pinned(
        &[2, 3],
        vec![
            stage_of(Trigger::OnSlideStart, 0, vec![fade(2)]),
            stage_of(Trigger::AfterPrevious, -500, vec![fade(3)]),
        ],
    );
    assert_eq!(s.state(1, &[], 499).unwrap().shapes[1].visible, Some(false));
    let at = s.state(1, &[], 500).unwrap();
    assert_eq!(
        (at.shapes[1].visible, at.shapes[1].opacity),
        (Some(true), Some(0.0))
    );
    assert_eq!(at.shapes[0].opacity, Some(0.5));
}

#[test]
fn with_previous_follows_a_delayed_stage_start() {
    // Click 100 + delay 300 gives stage 1 start 400. WithPrevious with delay 50 starts at the
    // previous stage start plus its own delay: 400 + 50 = 450.
    let s = pinned(
        &[2, 3],
        vec![
            stage_of(
                Trigger::OnClick,
                300,
                vec![eff(2, 0, 100, EffectKind::Appear)],
            ),
            stage_of(
                Trigger::WithPrevious,
                50,
                vec![eff(3, 0, 1000, EffectKind::FadeIn)],
            ),
        ],
    );
    assert_eq!(
        s.state(1, &[100], 449).unwrap().shapes[1].visible,
        Some(false)
    );
    let r = s.state(1, &[100], 500).unwrap();
    assert_eq!(r.shapes[1].visible, Some(true));
    assert!((r.shapes[1].opacity.unwrap() - 0.05).abs() < 1e-9);
}

#[test]
fn unclicked_stage_and_its_dependents_have_not_started() {
    // No click: the OnClick stage and its WithPrevious follower never start, while an
    // OnSlideStart stage on the same shape still runs.
    let s = pinned(
        &[2, 3],
        vec![
            stage_of(
                Trigger::OnClick,
                0,
                vec![eff(2, 0, 100, EffectKind::FadeIn)],
            ),
            stage_of(
                Trigger::WithPrevious,
                0,
                vec![eff(3, 0, 100, EffectKind::FadeIn)],
            ),
            stage_of(
                Trigger::OnSlideStart,
                0,
                vec![eff(3, 0, 1, EffectKind::Appear)],
            ),
        ],
    );
    let r = s.state(1, &[], 5000).unwrap();
    assert_eq!(
        (r.shapes[0].visible, r.shapes[0].opacity),
        (Some(false), Some(0.0))
    );
    assert_eq!(r.shapes[1].visible, Some(true));
}

#[test]
fn click_without_remaining_on_click_stage_changes_nothing() {
    let s = pinned(
        &[2],
        vec![stage_of(
            Trigger::OnClick,
            0,
            vec![eff(2, 0, 1000, EffectKind::FadeOut)],
        )],
    );
    let once = s.state(1, &[0], 500).unwrap();
    let extra = s.state(1, &[0, 200], 500).unwrap();
    assert_eq!(once.shapes[0].opacity, extra.shapes[0].opacity);
    assert_eq!(once.shapes[0].visible, extra.shapes[0].visible);
}

// ---- decided same-target cases (query.md, "Same-target effect semantics") ----

fn on(effects: Vec<Effect>) -> Stage {
    stage_of(Trigger::OnClick, 0, effects)
}

/// Each decided case as (slide, click history, sample time).
fn decided_a() -> (SlideSnapshot, Vec<u64>, u64) {
    let s = pinned_slide(
        &[11],
        vec![
            on(vec![eff(11, 0, 1, EffectKind::Appear)]),
            on(vec![
                eff(11, 172, 1, EffectKind::FadeOut),
                eff(11, 0, 1, EffectKind::Appear),
            ]),
        ],
    );
    (s, vec![4871], 4871)
}

fn decided_b() -> (SlideSnapshot, Vec<u64>, u64) {
    let wipe = EffectKind::WipeIn {
        direction: Direction::Left,
    };
    let s = pinned_slide(
        &[2],
        vec![
            on(vec![eff(2, 0, 1, EffectKind::Appear)]),
            on(vec![eff(2, 0, 1, wipe)]),
        ],
    );
    (s, vec![], 0)
}

fn decided_c() -> (SlideSnapshot, Vec<u64>, u64) {
    let s = pinned_slide(
        &[40],
        vec![
            on(vec![eff(40, 535, 1, EffectKind::FadeIn)]),
            on(vec![eff(40, 0, 1, EffectKind::FadeOut)]),
        ],
    );
    (s, vec![376, 502], 8616)
}

fn decided_d1() -> (SlideSnapshot, Vec<u64>, u64) {
    let s = pinned_slide(
        &[4],
        vec![
            on(vec![eff(4, 0, 1, EffectKind::FadeOut)]),
            on(vec![eff(4, 0, 1, EffectKind::Appear)]),
        ],
    );
    (s, vec![1482, 4186], 4186)
}

fn decided_d2() -> (SlideSnapshot, Vec<u64>, u64) {
    let s = pinned_slide(
        &[33],
        vec![
            on(vec![eff(33, 166, 1929, EffectKind::FadeIn)]),
            on(vec![eff(33, 0, 1, EffectKind::Appear)]),
        ],
    );
    (s, vec![6699, 6952], 6952)
}

/// (visible, opacity, wipe_fraction) of the first shape from `state()`.
type Case = (SlideSnapshot, Vec<u64>, u64);

fn state_of((s, clicks, t): Case) -> (Option<bool>, Option<f64>, Option<f64>) {
    let r = QuerySession::from_snapshots(vec![s])
        .state(1, &clicks, t)
        .unwrap();
    let sh = &r.shapes[0];
    (sh.visible, sh.opacity, sh.wipe_fraction)
}

#[test]
fn decided_a_pending_pair_does_not_rehide_a_revealed_shape() {
    let (v, o, _) = state_of(decided_a());
    assert_eq!((v, o), (Some(true), Some(1.0)));
}

#[test]
fn decided_b_pending_entrance_starts_hidden_with_unit_wipe() {
    assert_eq!(state_of(decided_b()), (Some(false), Some(0.0), Some(1.0)));
}

#[test]
fn decided_c_finished_fade_out_after_later_fade_in_reveals_fully() {
    let (v, o, _) = state_of(decided_c());
    assert_eq!((v, o), (Some(true), Some(1.0)));
}

#[test]
fn decided_d1_later_appear_restores_visibility_after_finished_fade() {
    let (v, o, _) = state_of(decided_d1());
    assert_eq!((v, o), (Some(true), Some(1.0)));
}

#[test]
fn decided_d2_appear_inside_running_fade_in_keeps_fade_opacity() {
    let (v, o, _) = state_of(decided_d2());
    assert_eq!(v, Some(true));
    assert!((o.unwrap() - (6952.0 - 6865.0) / 1929.0).abs() < 1e-9);
}

#[test]
fn reference_matches_decided_cases() {
    // (visible, opacity, wipe_fraction) of the first shape from the reference alone.
    let eval = |(s, clicks, t): Case| {
        let w = windows(&s.timing.timeline, &clicks);
        let (v, o, wp, _) = reference(&s.shapes[0], &w, i128::from(t));
        (v, o, wp)
    };
    assert_eq!(eval(decided_a()), (true, 1.0, 1.0));
    assert_eq!(eval(decided_b()), (false, 0.0, 1.0));
    assert_eq!(eval(decided_c()), (true, 1.0, 1.0));
    assert_eq!(eval(decided_d1()), (true, 1.0, 1.0));
    let (v, o, _) = eval(decided_d2());
    assert!(v);
    assert!((o - (6952.0 - 6865.0) / 1929.0).abs() < 1e-9);
}
