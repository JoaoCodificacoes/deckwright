//! MCP transport overhead: one `state` request through `session::Server::handle_line`
//! (path `session`) and the equivalent `tools/call` `dw_state` through
//! `McpServer::handle` (path `mcp`). Both timings include request handling and response
//! serialization; request lines are prebuilt. Std and serde_json only.
//! `DW_BENCH_SAMPLES` overrides the 50 samples. Local, advisory numbers; never gated.
use deckwright::{
    Limits,
    model::{Geometry, Object, ObjectKind, Properties, Slide, Transform},
    session::Server,
    template,
    timing::{Effect, EffectKind, Stage, Timeline, Transition, Trigger, compile_animated_slide},
    units::Emu,
};
use dw_mcp::McpServer;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    hint::black_box,
    path::Path,
    time::Instant,
};

const WARMUPS: usize = 5;
const DEFAULT_SAMPLES: usize = 50;
const SLIDES: usize = 3;
/// One-based slide carrying the click fade.
const SLIDE: u64 = 2;
const AT_MS: u64 = 1000;
const CLICKS_MS: [u64; 1] = [0];

/// Nearest-rank percentile, identical to crates/deckwright/benches/query.rs.
fn nearest_rank(sorted: &[f64], fraction: f64) -> f64 {
    let rank = (sorted.len() as f64 * fraction).ceil() as usize;
    sorted[rank.max(1) - 1]
}

fn median_sorted(sorted: &[f64]) -> f64 {
    let n = sorted.len();
    if n.is_multiple_of(2) {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    } else {
        sorted[n / 2]
    }
}

fn rect(id: u32) -> Object {
    Object {
        key: format!("box{id}"),
        id,
        name: format!("Box {id}"),
        morph_name: None,
        description: None,
        hyperlink: None,
        kind: ObjectKind::Shape {
            textbox: false,
            lock_no_group: false,
            placeholder: None,
            properties: Properties {
                transform: Some(Transform {
                    x: Emu(600_000 + i64::from(id - 2) * 2_800_000),
                    y: Emu(2_000_000),
                    width: Emu(1_828_800),
                    height: Emu(914_400),
                    ..Default::default()
                }),
                geometry: Some(Geometry {
                    preset: "rect".into(),
                    adjustments: vec![],
                }),
                ..Default::default()
            },
            style: None,
            text: None,
        },
    }
}

/// Writes a three-slide blank deck whose slides are compiled from typed semantics.
/// Slide 2 has four shapes and a click fade on shape 2.
fn build_deck(path: &Path) {
    let shapes = Slide {
        objects: (2..6).map(rect).collect(),
        ..Default::default()
    };
    let fade = Timeline {
        stages: vec![Stage {
            trigger: Trigger::OnClick,
            delay_ms: 0,
            effects: vec![Effect {
                target: 2,
                delay_ms: 0,
                duration_ms: 600,
                kind: EffectKind::FadeIn,
            }],
        }],
    };
    let mut replacements = BTreeMap::new();
    for number in 1..=SLIDES {
        let timeline = if number as u64 == SLIDE {
            fade.clone()
        } else {
            Timeline { stages: vec![] }
        };
        let bytes = compile_animated_slide(&shapes, &timeline, &Transition::None)
            .expect("compile synthetic slide");
        replacements.insert(format!("ppt/slides/slide{number}.xml"), bytes);
    }
    template::blank(SLIDES, Emu(12_192_000), Emu(6_858_000))
        .and_then(|deck| deck.edited(replacements, &BTreeSet::new(), Limits::default()))
        .and_then(|deck| deck.write(path))
        .expect("write synthetic deck");
}

/// Runs `run` after warmups; returns raw nanoseconds and the response size in bytes.
fn measure(samples: usize, mut run: impl FnMut() -> usize) -> (Vec<u64>, usize) {
    let mut bytes = 0;
    for _ in 0..WARMUPS {
        bytes = black_box(run());
    }
    let mut ns = Vec::with_capacity(samples);
    for _ in 0..samples {
        let started = Instant::now();
        bytes = black_box(run());
        ns.push(started.elapsed().as_nanos() as u64);
    }
    (ns, bytes)
}

/// (median, p95) in milliseconds, nearest-rank.
fn summarize(ns: &[u64]) -> (f64, f64) {
    let mut ms: Vec<f64> = ns.iter().map(|&n| n as f64 / 1e6).collect();
    ms.sort_by(f64::total_cmp);
    (median_sorted(&ms), nearest_rank(&ms, 0.95))
}

fn path_report(ns: &[u64], bytes: usize) -> Value {
    let (median, p95) = summarize(ns);
    json!({
        "stats_ms": {"median": median, "p95": p95},
        "response_bytes": bytes,
        "samples_ns": ns,
    })
}

fn handle_of(response: &Value) -> String {
    response["result"]["structuredContent"]["handle"]
        .as_str()
        .or_else(|| response["result"]["handle"].as_str())
        .expect("open answers a handle")
        .to_owned()
}

fn main() {
    let samples = std::env::var("DW_BENCH_SAMPLES")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(DEFAULT_SAMPLES);
    let dir = std::env::temp_dir().join(format!("dw-bench-transport-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("create bench directory");
    let path = dir.join("transport.pptx");
    build_deck(&path);
    let path_text = path.to_string_lossy().into_owned();

    let mut session = Server::new(8);
    let opened =
        session.handle_line(&json!({"id": 1, "op": "open", "path": path_text}).to_string());
    let opened: Value = serde_json::from_str(&opened).expect("session envelope is JSON");
    assert_eq!(opened["ok"], true, "{opened}");
    let session_handle = handle_of(&opened);
    let session_line = json!({
        "id": 2, "op": "state", "handle": session_handle,
        "slide": SLIDE, "at_ms": AT_MS, "clicks_ms": CLICKS_MS,
    })
    .to_string();

    let mut mcp = McpServer::new(8);
    let opened = mcp
        .handle(
            &json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": {"name": "dw_open", "arguments": {"path": path_text}}})
            .to_string(),
        )
        .expect("tools/call answers");
    let opened: Value = serde_json::from_str(&opened).expect("response is JSON");
    assert_eq!(opened["result"]["isError"], false, "{opened}");
    let mcp_handle = handle_of(&opened);
    let mcp_line = json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
    "params": {"name": "dw_state", "arguments": {
        "handle": mcp_handle, "slide": SLIDE, "at_ms": AT_MS, "clicks_ms": CLICKS_MS,
    }}})
    .to_string();

    // Sanity: both paths must answer successfully before anything is timed.
    let check: Value = serde_json::from_str(&session.handle_line(&session_line)).unwrap();
    assert_eq!(check["ok"], true, "{check}");
    let check: Value = serde_json::from_str(&mcp.handle(&mcp_line).unwrap()).unwrap();
    assert_eq!(check["result"]["isError"], false, "{check}");

    let (session_ns, session_bytes) = measure(samples, || {
        black_box(session.handle_line(black_box(&session_line))).len()
    });
    let (mcp_ns, mcp_bytes) = measure(samples, || {
        black_box(
            mcp.handle(black_box(&mcp_line))
                .expect("tools/call answers"),
        )
        .len()
    });
    fs::remove_dir_all(&dir).expect("remove bench directory");

    let (session_median, session_p95) = summarize(&session_ns);
    let (mcp_median, mcp_p95) = summarize(&mcp_ns);
    let report = json!({
        "schema": "dw.bench.mcp/1",
        "status": "measured",
        "advisory": true,
        "source": {"kind": "synthetic", "slides": SLIDES, "slide": SLIDE, "shapes": 4,
            "clicks": CLICKS_MS.len(), "at_ms": AT_MS},
        "warmups": WARMUPS,
        "samples": samples,
        "paths": {
            "session": path_report(&session_ns, session_bytes),
            "mcp": path_report(&mcp_ns, mcp_bytes),
        },
        "overhead_ms": {
            "median": mcp_median - session_median,
            "p95": mcp_p95 - session_p95,
        },
    });
    println!("{report}");
}
