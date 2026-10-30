//! In-process query benchmark: one slide-state evaluation and one adjacent Morph
//! diff, each timed together with serde_json serialization. Std only.
//! `DW_BENCH_INPUT=deck.pptx` benchmarks a local deck; otherwise a synthetic
//! 25-slide workload is built. `DW_BENCH_SAMPLES` overrides the 50 samples.
use deckwright::{
    Limits,
    query::{Geometry, QuerySession, ShapeSnapshot, SlideSnapshot, TimelineReport},
    timing::{
        Direction, Effect, EffectKind, ParsedTiming, Stage, Timeline, Transition, Trigger,
        validate_timeline,
    },
};
use serde_json::{Value, json};
use std::{cmp::Reverse, hint::black_box, time::Instant};

const BUDGET_MS: f64 = 5.0;
const WARMUPS: usize = 5;
const DEFAULT_SAMPLES: usize = 50;
const RESAMPLES: usize = 2000;
const SEED: u64 = 0x9E37_79B9_7F4A_7C15;
const CLICK_SPACING_MS: i64 = 500;

/// Nearest-rank percentile, identical to bench/run.py: sorted[ceil(n*f)-1].
fn nearest_rank(sorted: &[f64], fraction: f64) -> f64 {
    let rank = (sorted.len() as f64 * fraction).ceil() as usize;
    sorted[rank.max(1) - 1]
}

/// Same semantics as Python's statistics.median.
fn median_sorted(sorted: &[f64]) -> f64 {
    let n = sorted.len();
    if n.is_multiple_of(2) {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    } else {
        sorted[n / 2]
    }
}

struct XorShift64(u64);

impl XorShift64 {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

fn interval(sorted: &[f64]) -> [f64; 2] {
    [nearest_rank(sorted, 0.025), nearest_rank(sorted, 0.975)]
}

/// 95% bootstrap intervals for (p95, median); fixed seed, resampled with replacement.
fn bootstrap_ci(sorted: &[f64]) -> ([f64; 2], [f64; 2]) {
    let mut rng = XorShift64(SEED);
    let n = sorted.len() as u64;
    let (mut p95s, mut medians) = (Vec::new(), Vec::new());
    let mut buf = vec![0.0; sorted.len()];
    for _ in 0..RESAMPLES {
        for slot in &mut buf {
            *slot = sorted[(rng.next_u64() % n) as usize];
        }
        buf.sort_by(f64::total_cmp);
        p95s.push(nearest_rank(&buf, 0.95));
        medians.push(median_sorted(&buf));
    }
    p95s.sort_by(f64::total_cmp);
    medians.sort_by(f64::total_cmp);
    (interval(&p95s), interval(&medians))
}

/// Known-answer checks for the statistics code; runs only in debug builds.
fn self_check() {
    let ranks: Vec<f64> = (1..=50u32).map(f64::from).collect();
    assert_eq!(nearest_rank(&ranks, 0.95), 48.0);
    assert_eq!(nearest_rank(&ranks, 0.5), 25.0);
    assert_eq!(median_sorted(&ranks), 25.5);
    let ten: Vec<f64> = (1..=10u32).map(f64::from).collect();
    assert_eq!(nearest_rank(&ten, 0.95), 10.0);
    assert_eq!(median_sorted(&ten), 5.5);
    let (p95, median) = bootstrap_ci(&[3.0; 20]);
    assert_eq!(p95, [3.0, 3.0]);
    assert_eq!(median, [3.0, 3.0]);
    let (p95, _) = bootstrap_ci(&ranks);
    assert!(p95[0] <= p95[1] && p95[0] >= 1.0 && p95[1] <= 50.0);
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(1)
}

fn synthetic_kind(i: u32) -> EffectKind {
    match i % 4 {
        0 => EffectKind::FadeIn,
        1 => EffectKind::WipeIn {
            direction: Direction::Left,
        },
        2 => EffectKind::FadeOut,
        _ => EffectKind::Motion {
            from_x: -0.5,
            from_y: 0.0,
            to_x: 0.0,
            to_y: 0.0,
            accelerate: 0.2,
            decelerate: 0.2,
        },
    }
}

/// 25 slides of 30 shapes (first 15 named `!!nK`, shared by adjacent slides)
/// and a 20-stage on-click timeline with a Morph transition.
fn synthetic_slides() -> Result<Vec<SlideSnapshot>, String> {
    let timeline = Timeline {
        stages: (0..20u32)
            .map(|i| Stage {
                trigger: Trigger::OnClick,
                delay_ms: 0,
                effects: vec![Effect {
                    target: 2 + (i * 7) % 30,
                    delay_ms: 0,
                    duration_ms: 400 + 50 * (i % 5),
                    kind: synthetic_kind(i),
                }],
            })
            .collect(),
    };
    validate_timeline(&timeline).map_err(|e| e.to_string())?;
    Ok((0..25usize)
        .map(|k| SlideSnapshot {
            part: format!("ppt/slides/slide{}.xml", k + 1),
            width: 12_192_000.0,
            height: 6_858_000.0,
            shapes: (0..30u32)
                .map(|j| ShapeSnapshot {
                    id: 2 + j,
                    name: if j < 15 {
                        format!("!!n{j}")
                    } else {
                        format!("shape{j}")
                    },
                    text: format!("slide {k} shape {j}"),
                    parent: None,
                    z_order: j as usize,
                    geometry: Some(Geometry {
                        x: f64::from(j % 6) * 1.0e6,
                        y: f64::from(j / 6) * 1.0e6,
                        width: 1.5e6,
                        height: 1.0e6,
                    }),
                    hidden: false,
                    warnings: vec![],
                })
                .collect(),
            timing: ParsedTiming {
                timeline: timeline.clone(),
                transition: Transition::Morph { duration_ms: 700 },
                opaque: vec![],
            },
        })
        .collect())
}

/// Input loading is timed once and reported separately from the samples.
fn load_session() -> (QuerySession, Value, Value) {
    let started = Instant::now();
    let path = std::env::var_os("DW_BENCH_INPUT").filter(|p| !p.is_empty());
    let (session, source, phase) = match path {
        Some(path) => {
            let session = QuerySession::open(&path, Limits::default())
                .unwrap_or_else(|e| fail(&format!("failed to open DW_BENCH_INPUT: {e}")));
            let bytes = std::fs::metadata(&path).map_or(0, |m| m.len());
            (
                session,
                json!({"kind": "file", "file_bytes": bytes}),
                "open",
            )
        }
        None => {
            let slides = synthetic_slides().unwrap_or_else(|e| fail(&e));
            let source = json!({"kind": "synthetic", "shapes_per_slide": 30,
                "click_stages_per_slide": 20});
            (QuerySession::from_snapshots(slides), source, "build")
        }
    };
    let open = json!({"phase": phase, "ms": started.elapsed().as_secs_f64() * 1e3});
    (session, source, open)
}

fn effect_count(report: &TimelineReport) -> usize {
    report
        .timing
        .timeline
        .stages
        .iter()
        .map(|s| s.effects.len())
        .sum()
}

/// One click per on-click stage at 500 ms spacing. The sample time is the middle
/// of the last effect under nominal start times (chained triggers are approximated).
fn state_plan(timeline: &Timeline) -> (Vec<u64>, u64) {
    let mut clicks = Vec::new();
    let mut start = 0;
    let mut sample = 0;
    for stage in &timeline.stages {
        if stage.trigger == Trigger::OnClick {
            start = clicks.len() as i64 * CLICK_SPACING_MS;
            clicks.push(start as u64);
        } else if stage.trigger == Trigger::OnSlideStart {
            start = 0;
        }
        for effect in &stage.effects {
            sample = start + stage.delay_ms + effect.delay_ms + i64::from(effect.duration_ms) / 2;
        }
    }
    let last_click = clicks.last().copied().unwrap_or(0);
    (clicks, (sample.max(0) as u64).max(last_click))
}

/// Warmups, then timed samples. Any error skips the operation instead of panicking.
fn run(mut op: impl FnMut() -> Result<usize, String>, samples: usize, meta: Value) -> Value {
    let outcome = (|| -> Result<(Vec<u64>, usize), String> {
        for _ in 0..WARMUPS {
            black_box(op()?);
        }
        let mut ns = Vec::with_capacity(samples);
        let mut bytes = 0;
        for _ in 0..samples {
            let started = Instant::now();
            let n = op()?;
            ns.push(started.elapsed().as_nanos() as u64);
            bytes = black_box(n);
        }
        Ok((ns, bytes))
    })();
    match outcome {
        Ok((ns, bytes)) => measured(meta, &ns, bytes),
        Err(reason) => skipped(&reason),
    }
}

fn measured(mut out: Value, ns: &[u64], output_bytes: usize) -> Value {
    let mut ms: Vec<f64> = ns.iter().map(|&n| n as f64 / 1e6).collect();
    ms.sort_by(f64::total_cmp);
    let p95 = nearest_rank(&ms, 0.95);
    let (ci_p95, ci_median) = bootstrap_ci(&ms);
    let verdict = if p95 <= BUDGET_MS { "pass" } else { "fail" };
    let mean = ms.iter().sum::<f64>() / ms.len() as f64;
    let fields = [
        ("status", json!("measured")),
        ("budget_ms", json!(BUDGET_MS)),
        ("verdict", json!(verdict)),
        ("output_bytes", json!(output_bytes)),
        (
            "stats_ms",
            json!({"median": median_sorted(&ms), "p95": p95, "min": ms[0],
                "max": ms[ms.len() - 1], "mean": mean}),
        ),
        ("ci95_ms", json!({"p95": ci_p95, "median": ci_median})),
        ("samples_ns", json!(ns)),
    ];
    for (key, value) in fields {
        out[key] = value;
    }
    out
}

fn skipped(reason: &str) -> Value {
    json!({"status": "skipped", "reason": reason, "budget_ms": BUDGET_MS, "verdict": "skipped"})
}

fn bench_state(session: &QuerySession, slide_count: usize, samples: usize) -> Value {
    let (count, slide) = (1..=slide_count)
        .map(|s| (session.timeline(s).map_or(0, |t| effect_count(&t)), s))
        .max_by_key(|&(n, s)| (n, Reverse(s)))
        .unwrap_or((0, 0));
    if count == 0 {
        return skipped("no slide in the deck has timing effects");
    }
    let Ok(report) = session.timeline(slide) else {
        return skipped("state slide could not be read");
    };
    let (clicks, sample_ms) = state_plan(&report.timing.timeline);
    let op = || -> Result<usize, String> {
        let state = session
            .state(black_box(slide), black_box(&clicks), black_box(sample_ms))
            .map_err(|e| e.to_string())?;
        Ok(serde_json::to_vec(&state).map_err(|e| e.to_string())?.len())
    };
    let meta = json!({"slide": slide, "effects": count, "clicks": clicks.len(),
        "sample_time_ms": sample_ms});
    run(op, samples, meta)
}

fn bench_morph(session: &QuerySession, slide_count: usize, samples: usize) -> Value {
    if slide_count < 2 {
        return skipped("deck has fewer than two slides, so there is no adjacent pair");
    }
    let best = (1..slide_count)
        .filter_map(|from| {
            let matches = session.morph(from, from + 1).ok()?.explicit_matches.len();
            Some((matches, from))
        })
        .max_by_key(|&(n, from)| (n, Reverse(from)));
    let Some((matches, from)) = best else {
        return skipped("no adjacent slide pair could be evaluated");
    };
    let op = || -> Result<usize, String> {
        let report = session
            .morph(black_box(from), black_box(from + 1))
            .map_err(|e| e.to_string())?;
        Ok(serde_json::to_vec(&report)
            .map_err(|e| e.to_string())?
            .len())
    };
    let meta = json!({"from": from, "to": from + 1, "explicit_matches": matches});
    run(op, samples, meta)
}

fn main() {
    if cfg!(debug_assertions) {
        self_check();
    }
    let samples = match std::env::var("DW_BENCH_SAMPLES") {
        Ok(v) => v
            .parse::<usize>()
            .ok()
            .filter(|&n| n >= 1)
            .unwrap_or_else(|| fail("DW_BENCH_SAMPLES must be a positive integer")),
        Err(_) => DEFAULT_SAMPLES,
    };
    let (session, mut source, open) = load_session();
    let slide_count = (1..).take_while(|&i| session.timeline(i).is_ok()).count();
    source["slides"] = json!(slide_count);
    let mut root = json!({
        "schema": "dw.bench.query/1",
        "input": source,
        "open": open,
        "protocol": {
            "warmups": WARMUPS,
            "samples": samples,
            "timed": "one state() or morph() call plus serde_json::to_vec",
            "percentile": "nearest-rank, sorted[ceil(n*f)-1] as in bench/run.py",
            "median": "statistics.median semantics",
            "bootstrap": {"resamples": RESAMPLES, "seed": format!("{SEED:#018X}"),
                "generator": "xorshift64 (13, 7, 17)",
                "interval": "2.5th and 97.5th bootstrap percentiles, nearest-rank"},
        },
        "cargo_profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "operations": {
            "state": bench_state(&session, slide_count, samples),
            "morph": bench_morph(&session, slide_count, samples),
        },
    });
    if let Some(version) = option_env!("RUSTC_VERSION") {
        root["rustc"] = json!(version);
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&root).unwrap_or_default()
    );
}
