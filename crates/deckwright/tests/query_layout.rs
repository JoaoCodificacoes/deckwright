use deckwright::{
    Limits,
    query::{QuerySession, SlideSnapshot},
    template,
    timing::ParsedTiming,
    units::Emu,
};

const W: i64 = 12_192_000;
const H: i64 = 6_858_000;

fn snapshot(part: &str, width: f64, height: f64) -> SlideSnapshot {
    SlideSnapshot {
        part: part.into(),
        width,
        height,
        shapes: vec![],
        timing: ParsedTiming::default(),
    }
}

#[test]
fn accessors_follow_snapshot_order_and_reject_out_of_range() {
    let session = QuerySession::from_snapshots(vec![
        snapshot("ppt/slides/b.xml", 800.0, 600.0),
        snapshot("ppt/slides/a.xml", 1000.0, 500.0),
    ]);
    assert_eq!(session.slide_count(), 2);
    assert_eq!(session.slide_part(1).unwrap(), "ppt/slides/b.xml");
    assert_eq!(session.slide_part(2).unwrap(), "ppt/slides/a.xml");
    assert_eq!(session.slide_size(1).unwrap(), (800.0, 600.0));
    assert_eq!(session.slide_size(2).unwrap(), (1000.0, 500.0));
    for bad in [0, 3] {
        assert!(session.slide_part(bad).is_err(), "slide {bad}");
        assert!(session.slide_size(bad).is_err(), "slide {bad}");
    }
    assert!(session.state(3, &[], 0).is_err());
}

#[test]
fn empty_session_has_no_slides() {
    let session = QuerySession::from_snapshots(vec![]);
    assert_eq!(session.slide_count(), 0);
    assert!(session.slide_part(1).is_err());
    assert!(session.slide_size(1).is_err());
}

#[test]
fn package_session_reports_template_order_and_size() {
    let package = template::blank(3, Emu(W), Emu(H)).unwrap();
    let session = QuerySession::from_package(&package, Limits::default()).unwrap();
    assert_eq!(session.slide_count(), 3);
    assert_eq!(session.slide_part(1).unwrap(), "ppt/slides/slide1.xml");
    assert_eq!(session.slide_part(3).unwrap(), "ppt/slides/slide3.xml");
    assert_eq!(session.slide_size(1).unwrap(), (12192000.0, 6858000.0));
    assert_eq!(session.slide_size(3).unwrap(), (12192000.0, 6858000.0));
    assert!(session.slide_part(4).is_err());
}
