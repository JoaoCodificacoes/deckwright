use deckwright::{
    Limits, Package,
    check::check_package,
    deck::{CoreProperties, Deck},
    package::Relationship,
    query::QuerySession,
    template,
    units::Emu,
};

const W: i64 = 12_192_000;
const H: i64 = 6_858_000;
const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfixture-bytes";
const LAYOUT: &str = "ppt/slideLayouts/slideLayout1.xml";
const REL_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn deck(slides: usize) -> Deck {
    Deck::from_package(template::blank(slides, Emu(W), Emu(H)).unwrap())
}

fn errors(package: &Package) -> Vec<String> {
    check_package(package)
        .findings
        .iter()
        .filter(|f| f.severity == "error")
        .map(|f| format!("{}: {}", f.rule, f.message))
        .collect()
}

fn rels_of<'a>(package: &'a Package, source: &str) -> Vec<&'a Relationship> {
    package
        .relationships()
        .iter()
        .filter(|r| r.source == source)
        .collect()
}

fn rel<'a>(package: &'a Package, source: &str, id: &str) -> &'a Relationship {
    rels_of(package, source)
        .into_iter()
        .find(|r| r.id == id)
        .unwrap_or_else(|| panic!("no {id} on {source}"))
}

fn text(package: &Package, part: &str) -> String {
    String::from_utf8(package.part(part).unwrap().to_vec()).unwrap()
}

fn parts_under(package: &Package, prefix: &str) -> Vec<String> {
    package
        .part_names()
        .filter(|n| n.starts_with(prefix) && !n.contains("_rels/"))
        .map(str::to_owned)
        .collect()
}

/// A slide with one picture bound to `rid`, built by hand in the template's style.
fn pic_slide(rid: &str) -> Vec<u8> {
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            r#"<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
            r#"xmlns:r="{rel}" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">"#,
            r#"<p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>"#,
            r#"<p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>"#,
            r#"<p:pic><p:nvPicPr><p:cNvPr id="2" name="Picture 1"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr>"#,
            r#"<p:blipFill><a:blip r:embed="{rid}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>"#,
            r#"<p:spPr><a:xfrm><a:off x="914400" y="914400"/><a:ext cx="1828800" cy="1828800"/></a:xfrm>"#,
            r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#,
            r#"</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#
        ),
        rel = REL_NS,
        rid = rid
    )
    .into_bytes()
}

#[test]
fn add_slide_from_blank_layout_appears_in_order_and_opens() {
    let mut d = deck(1);
    assert_eq!(d.slides(), vec!["ppt/slides/slide1.xml".to_string()]);
    let layouts = d.layouts();
    assert_eq!(layouts, vec![(LAYOUT.to_string(), "Blank".to_string())]);

    assert_eq!(d.add_slide(LAYOUT).unwrap(), "ppt/slides/slide2.xml");
    assert_eq!(d.add_slide(LAYOUT).unwrap(), "ppt/slides/slide3.xml");
    assert_eq!(
        d.slides(),
        vec![
            "ppt/slides/slide1.xml",
            "ppt/slides/slide2.xml",
            "ppt/slides/slide3.xml"
        ]
    );
    assert!(d.add_slide("ppt/slides/slide1.xml").is_err());

    let package = d.into_package();
    let session = QuerySession::from_package(&package, Limits::default()).unwrap();
    assert_eq!(session.slide_count(), 3);
    assert_eq!(session.slide_part(3).unwrap(), "ppt/slides/slide3.xml");
    assert!(errors(&package).is_empty(), "{:?}", errors(&package));
}

#[test]
fn add_image_twice_shares_one_media_part() {
    let mut d = deck(2);
    let first = d
        .add_image("ppt/slides/slide1.xml", PNG.to_vec(), "png")
        .unwrap();
    let second = d
        .add_image("ppt/slides/slide2.xml", PNG.to_vec(), "png")
        .unwrap();
    let again = d
        .add_image("ppt/slides/slide1.xml", PNG.to_vec(), "png")
        .unwrap();
    // Same slide, same bytes: the existing relationship is reused.
    assert_eq!(first, again);

    let package = d.into_package();
    assert_eq!(
        parts_under(&package, "ppt/media/"),
        vec!["ppt/media/image1.png"]
    );
    for (slide, id) in [
        ("ppt/slides/slide1.xml", &first),
        ("ppt/slides/slide2.xml", &second),
    ] {
        let r = rel(&package, slide, id);
        assert!(r.relationship_type.ends_with("/image"));
        assert_eq!(r.resolved_target.as_deref(), Some("ppt/media/image1.png"));
    }
    assert!(
        text(&package, "[Content_Types].xml")
            .contains(r#"<Default Extension="png" ContentType="image/png"/>"#)
    );
    assert!(errors(&package).is_empty(), "{:?}", errors(&package));
}

#[test]
fn add_image_rejects_bad_extension() {
    let mut d = deck(1);
    for ext in ["exe", "pdf", "", "webp", "png.exe"] {
        assert!(
            d.add_image("ppt/slides/slide1.xml", PNG.to_vec(), ext)
                .is_err(),
            "{ext:?} accepted"
        );
    }
    assert!(parts_under(&d.into_package(), "ppt/media/").is_empty());
}

#[test]
fn add_video_returns_three_resolving_relationships() {
    let mut d = deck(1);
    let links = d
        .add_video(
            "ppt/slides/slide1.xml",
            b"\x00\x00\x00\x18ftypmp42video".to_vec(),
            "mp4",
            PNG.to_vec(),
        )
        .unwrap();
    let package = d.into_package();
    let slide = "ppt/slides/slide1.xml";
    let video = rel(&package, slide, &links.video);
    let media = rel(&package, slide, &links.media);
    let poster = rel(&package, slide, &links.poster);
    assert!(video.relationship_type.ends_with("/video"));
    assert!(media.relationship_type.ends_with("/media"));
    assert!(poster.relationship_type.ends_with("/image"));
    for r in [video, media, poster] {
        let target = r.resolved_target.as_deref().expect("internal target");
        assert!(package.part(target).is_some(), "{target} missing");
    }
    assert_eq!(video.resolved_target, media.resolved_target);
    assert!(
        text(&package, "[Content_Types].xml")
            .contains(r#"Extension="mp4" ContentType="video/mp4""#)
    );
    assert!(errors(&package).is_empty(), "{:?}", errors(&package));
}

#[test]
fn add_video_rejects_unknown_extension() {
    let mut d = deck(1);
    assert!(
        d.add_video("ppt/slides/slide1.xml", vec![1, 2, 3], "flv", PNG.to_vec())
            .is_err()
    );
}

#[test]
fn hyperlink_is_external_and_slide_link_targets_slide_part() {
    let mut d = deck(2);
    let url = d
        .add_hyperlink("ppt/slides/slide1.xml", "https://example.com/a?b=1&c=2")
        .unwrap();
    let jump = d
        .add_slide_link("ppt/slides/slide1.xml", "ppt/slides/slide2.xml")
        .unwrap();
    let package = d.into_package();
    let link = rel(&package, "ppt/slides/slide1.xml", &url);
    assert_eq!(link.target_mode, "External");
    assert_eq!(link.target, "https://example.com/a?b=1&c=2");
    assert_eq!(link.resolved_target, None);
    assert!(link.relationship_type.ends_with("/hyperlink"));

    let slide_link = rel(&package, "ppt/slides/slide1.xml", &jump);
    assert!(slide_link.relationship_type.ends_with("/slide"));
    assert_eq!(
        slide_link.resolved_target.as_deref(),
        Some("ppt/slides/slide2.xml")
    );
    assert!(errors(&package).is_empty(), "{:?}", errors(&package));
}

#[test]
fn set_notes_on_two_slides_shares_one_master() {
    let mut d = deck(2);
    d.set_notes("ppt/slides/slide1.xml", "First line\nSecond & <third>")
        .unwrap();
    d.set_notes("ppt/slides/slide2.xml", "Other slide").unwrap();
    // Replacing notes on a slide that already has them keeps the single notes slide.
    d.set_notes("ppt/slides/slide1.xml", "Rewritten").unwrap();
    let package = d.into_package();

    assert_eq!(
        parts_under(&package, "ppt/notesMasters/"),
        vec!["ppt/notesMasters/notesMaster1.xml"]
    );
    assert_eq!(
        parts_under(&package, "ppt/notesSlides/"),
        vec![
            "ppt/notesSlides/notesSlide1.xml",
            "ppt/notesSlides/notesSlide2.xml"
        ]
    );
    assert!(text(&package, "ppt/_rels/presentation.xml.rels").contains("/notesMaster"));
    assert!(text(&package, "ppt/presentation.xml").contains("notesMasterIdLst"));

    let master_theme: Vec<_> = rels_of(&package, "ppt/notesMasters/notesMaster1.xml");
    assert_eq!(master_theme.len(), 1);
    assert!(master_theme[0].relationship_type.ends_with("/theme"));
    let theme = master_theme[0].resolved_target.clone().unwrap();
    assert!(theme.starts_with("ppt/theme/"));
    assert!(package.part(&theme).is_some());

    let notes1 = rel(&package, "ppt/slides/slide1.xml", "rId2");
    assert!(notes1.relationship_type.ends_with("/notesSlide"));
    assert_eq!(
        notes1.resolved_target.as_deref(),
        Some("ppt/notesSlides/notesSlide1.xml")
    );
    let back: Vec<_> = rels_of(&package, "ppt/notesSlides/notesSlide1.xml");
    assert!(back.iter().any(|r| r.relationship_type.ends_with("/slide")
        && r.resolved_target.as_deref() == Some("ppt/slides/slide1.xml")));
    assert!(
        back.iter()
            .any(|r| r.relationship_type.ends_with("/notesMaster")
                && r.resolved_target.as_deref() == Some("ppt/notesMasters/notesMaster1.xml"))
    );

    let notes = text(&package, "ppt/notesSlides/notesSlide2.xml");
    assert!(notes.contains(r#"<a:t>Other slide</a:t>"#));
    let notes1 = text(&package, "ppt/notesSlides/notesSlide1.xml");
    assert!(notes1.contains("Rewritten"));
    assert!(!notes1.contains("First line"));
    assert!(errors(&package).is_empty(), "{:?}", errors(&package));
}

#[test]
fn notes_text_is_one_paragraph_per_line_and_escaped() {
    let mut d = deck(1);
    d.set_notes("ppt/slides/slide1.xml", "a & b\n<c>\n\"d\"")
        .unwrap();
    let package = d.into_package();
    let notes = text(&package, "ppt/notesSlides/notesSlide1.xml");
    assert_eq!(notes.matches("<a:p>").count(), 3);
    assert!(notes.contains("<a:t>a &amp; b</a:t>"));
    assert!(notes.contains("<a:t>&lt;c&gt;</a:t>"));
    assert!(errors(&package).is_empty(), "{:?}", errors(&package));
}

#[test]
fn set_notes_rejects_control_characters() {
    let mut d = deck(1);
    assert!(
        d.set_notes("ppt/slides/slide1.xml", "bad\u{1}char")
            .is_err()
    );
}

#[test]
fn set_core_properties_writes_values() {
    let mut d = deck(1);
    d.set_core_properties(&CoreProperties {
        title: Some("Quarterly & Review".into()),
        subject: Some("Subject".into()),
        creator: Some("Ana".into()),
        keywords: Some("alpha, beta".into()),
        description: Some("Desc <x>".into()),
    })
    .unwrap();
    let package = d.into_package();
    let core = text(&package, "docProps/core.xml");
    assert!(
        core.contains("<dc:title>Quarterly &amp; Review</dc:title>"),
        "{core}"
    );
    assert!(core.contains("<dc:subject>Subject</dc:subject>"));
    assert!(core.contains("<dc:creator>Ana</dc:creator>"));
    assert!(core.contains("<cp:keywords>alpha, beta</cp:keywords>"));
    assert!(core.contains("<dc:description>Desc &lt;x&gt;</dc:description>"));
    assert!(!core.contains(">Blank<"), "old title survived: {core}");
    assert!(errors(&package).is_empty(), "{:?}", errors(&package));
}

#[test]
fn set_slide_accepts_known_relationships_and_rejects_unknown() {
    let mut d = deck(1);
    let rid = d
        .add_image("ppt/slides/slide1.xml", PNG.to_vec(), "png")
        .unwrap();
    d.set_slide("ppt/slides/slide1.xml", pic_slide(&rid))
        .unwrap();
    assert!(
        d.set_slide("ppt/slides/slide1.xml", pic_slide("rId99"))
            .is_err()
    );
    assert!(
        d.set_slide("ppt/slides/slide9.xml", pic_slide(&rid))
            .is_err()
    );
    let package = d.into_package();
    let session = QuerySession::from_package(&package, Limits::default()).unwrap();
    assert_eq!(session.slide_count(), 1);
    assert!(errors(&package).is_empty(), "{:?}", errors(&package));
}

fn full_sequence() -> Package {
    let mut d = deck(2);
    let rid = d
        .add_image("ppt/slides/slide1.xml", PNG.to_vec(), "png")
        .unwrap();
    d.set_slide("ppt/slides/slide1.xml", pic_slide(&rid))
        .unwrap();
    d.add_slide(LAYOUT).unwrap();
    d.add_hyperlink("ppt/slides/slide1.xml", "https://example.com")
        .unwrap();
    d.add_slide_link("ppt/slides/slide2.xml", "ppt/slides/slide1.xml")
        .unwrap();
    d.add_video("ppt/slides/slide2.xml", vec![7; 64], "mp4", PNG.to_vec())
        .unwrap();
    d.set_notes("ppt/slides/slide1.xml", "Hello\nWorld")
        .unwrap();
    d.set_notes("ppt/slides/slide3.xml", "Third").unwrap();
    d.set_core_properties(&CoreProperties {
        title: Some("Deterministic".into()),
        ..Default::default()
    })
    .unwrap();
    d.into_package()
}

#[test]
fn identical_sequences_give_identical_bytes() {
    let a = full_sequence().to_bytes().unwrap();
    let b = full_sequence().to_bytes().unwrap();
    assert_eq!(a, b);
    let package = full_sequence();
    assert!(errors(&package).is_empty(), "{:?}", errors(&package));
}

#[test]
fn write_then_open_round_trips() {
    let dir = std::env::temp_dir().join(format!("dw-deck-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("round.pptx");
    let _ = std::fs::remove_file(&path);
    let mut d = deck(1);
    d.set_notes("ppt/slides/slide1.xml", "kept").unwrap();
    d.write(&path).unwrap();
    let reopened = Deck::open(&path, Limits::default()).unwrap();
    assert_eq!(reopened.slides().len(), 1);
    let package = reopened.into_package();
    assert!(text(&package, "ppt/notesSlides/notesSlide1.xml").contains("kept"));
    std::fs::remove_file(&path).unwrap();
}
