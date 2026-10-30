use deckwright::query::{Geometry, MediaState, QuerySession, ShapeSnapshot, SlideSnapshot};
use deckwright::timing::*;

// Shape 2 is 50 x 30 at (100, 20). Shape 3 is an unrelated shape with no media.
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

/// A stage that starts at slide entry plus `delay_ms` and holds one effect.
fn at(delay_ms: i64, target: u32, duration_ms: u32, kind: EffectKind) -> Stage {
    Stage {
        trigger: Trigger::OnSlideStart,
        delay_ms,
        effects: vec![Effect {
            target,
            delay_ms: 0,
            duration_ms,
            kind,
        }],
    }
}

fn media(from_ms: u32) -> EffectKind {
    EffectKind::MediaPlay {
        from_ms,
        volume: 80000,
    }
}

fn media_of(s: &QuerySession, clicks: &[u64], t: u64, id: u32) -> Option<MediaState> {
    s.state(1, clicks, t)
        .unwrap()
        .shapes
        .iter()
        .find(|x| x.id == id)
        .unwrap()
        .media
}

fn state(playing: bool, position_ms: u64) -> Option<MediaState> {
    Some(MediaState {
        playing,
        position_ms,
    })
}

#[test]
fn before_start_the_call_is_idle_at_zero() {
    let s = session(vec![at(1000, 2, 3000, media(2500))]);
    assert_eq!(media_of(&s, &[], 500, 2), state(false, 0));
}

#[test]
fn mid_play_reports_from_plus_elapsed() {
    let s = session(vec![at(1000, 2, 3000, media(2500))]);
    assert_eq!(media_of(&s, &[], 1000, 2), state(true, 2500));
    assert_eq!(media_of(&s, &[], 2750, 2), state(true, 4250));
}

#[test]
fn after_end_the_position_is_held_at_the_end() {
    let s = session(vec![at(1000, 2, 3000, media(2500))]);
    assert_eq!(media_of(&s, &[], 3999, 2), state(true, 5499));
    assert_eq!(media_of(&s, &[], 4000, 2), state(false, 5500));
    assert_eq!(media_of(&s, &[], 90_000, 2), state(false, 5500));
}

#[test]
fn a_later_call_overrides_an_earlier_one() {
    let s = session(vec![
        at(0, 2, 5000, media(0)),
        at(2000, 2, 1000, media(1000)),
    ]);
    // Both are active at 2500: the later call governs.
    assert_eq!(media_of(&s, &[], 1000, 2), state(true, 1000));
    assert_eq!(media_of(&s, &[], 2500, 2), state(true, 1500));
    // The later call has ended and holds; the earlier call no longer governs.
    assert_eq!(media_of(&s, &[], 4000, 2), state(false, 2000));
}

#[test]
fn a_shape_without_media_has_null_media() {
    let s = session(vec![at(0, 2, 1000, media(0))]);
    assert_eq!(media_of(&s, &[], 500, 3), None);
    let report = s.state(1, &[], 500).unwrap();
    let json = serde_json::to_value(&report).unwrap();
    assert!(json["shapes"][1]["media"].is_null());
}

#[test]
fn state_json_media_is_null_or_exactly_playing_and_position() {
    let s = session(vec![at(0, 2, 1000, media(300))]);
    let json = serde_json::to_value(s.state(1, &[], 500).unwrap()).unwrap();
    for shape in json["shapes"].as_array().unwrap() {
        let media = &shape["media"];
        assert!(media.is_null() || media.as_object().unwrap().len() == 2);
    }
    let media = &json["shapes"][0]["media"];
    assert_eq!(media["playing"], serde_json::json!(true));
    assert_eq!(media["position_ms"], serde_json::json!(800));
}

#[test]
fn an_after_previous_stage_starts_at_the_media_end() {
    let s = session(vec![
        at(0, 2, 2000, media(0)),
        Stage {
            trigger: Trigger::AfterPrevious,
            delay_ms: 0,
            effects: vec![Effect {
                target: 3,
                delay_ms: 0,
                duration_ms: 500,
                kind: EffectKind::Appear,
            }],
        },
    ]);
    let before = s.state(1, &[], 1999).unwrap();
    assert_eq!(before.shapes[1].visible, Some(false));
    let at_end = s.state(1, &[], 2000).unwrap();
    assert_eq!(at_end.shapes[1].visible, Some(true));
}

#[test]
fn media_leaves_visibility_and_geometry_unchanged() {
    let s = session(vec![at(0, 2, 1000, media(0))]);
    let quiet = session(vec![]);
    for t in [0, 500, 1000, 5000] {
        let with = s.state(1, &[], t).unwrap();
        let without = quiet.state(1, &[], t).unwrap();
        let (a, b) = (&with.shapes[0], &without.shapes[0]);
        assert_eq!(a.visible, b.visible);
        assert_eq!(a.opacity, b.opacity);
        assert_eq!(a.wipe_fraction, b.wipe_fraction);
        assert_eq!(a.geometry, b.geometry);
    }
}
