use deckwright::{
    Limits, Package,
    builder::compile_slide,
    check::check_package,
    deck::{Deck, OleLinks},
    model::{Object, ObjectKind, Slide, Transform},
    package::Relationship,
    template,
    units::Emu,
};
use serde_json::{Value, json};
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;

const W: i64 = 12_192_000;
const H: i64 = 6_858_000;
const SLIDE: &str = "ppt/slides/slide1.xml";
const PNG: &[u8] = b"\x89PNG\r\n\x1a\nole-preview";
const JPEG: &[u8] = b"\xff\xd8\xff\xe0ole-preview";
const PACKAGE_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/package";
const IMAGE_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const OLE_URI: &str = "http://schemas.openxmlformats.org/presentationml/2006/ole";
const XML_DECL: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;
const CT_NS: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
const RELS_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const OFFICE_DOC: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
const XLSX_CT: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
const DOCX_CT: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
const PPTX_CT: &str = "application/vnd.openxmlformats-officedocument.presentationml.presentation";

fn deck() -> Deck {
    Deck::from_package(template::blank(1, Emu(W), Emu(H)).unwrap())
}

fn blank_bytes() -> Vec<u8> {
    template::blank(1, Emu(W), Emu(H))
        .unwrap()
        .to_bytes()
        .unwrap()
}

fn zip_of(entries: &[(&str, String)]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, body) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(body.as_bytes()).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

/// A tiny OOXML package: content types, package rels, and one main part.
fn ooxml(
    main_part: &str,
    main_type: &str,
    main_xml: String,
    extra: Vec<(&str, String)>,
) -> Vec<u8> {
    let mut entries = vec![
        (
            "[Content_Types].xml",
            format!(
                r#"{XML_DECL}<Types xmlns="{CT_NS}"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/{main_part}" ContentType="{main_type}"/></Types>"#
            ),
        ),
        (
            "_rels/.rels",
            format!(
                r#"{XML_DECL}<Relationships xmlns="{RELS_NS}"><Relationship Id="rId1" Type="{OFFICE_DOC}" Target="{main_part}"/></Relationships>"#
            ),
        ),
        (main_part, main_xml),
    ];
    entries.extend(extra);
    zip_of(&entries)
}

fn xlsx() -> Vec<u8> {
    ooxml(
        "xl/workbook.xml",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml",
        format!(
            r#"{XML_DECL}<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>"#
        ),
        vec![(
            "xl/_rels/workbook.xml.rels",
            format!(
                r#"{XML_DECL}<Relationships xmlns="{RELS_NS}"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#
            ),
        )],
    )
}

fn docx() -> Vec<u8> {
    ooxml(
        "word/document.xml",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
        format!(
            r#"{XML_DECL}<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>embedded</w:t></w:r></w:p></w:body></w:document>"#
        ),
        vec![],
    )
}

fn pptx() -> Vec<u8> {
    blank_bytes()
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

fn errors(package: &Package) -> Vec<String> {
    check_package(package)
        .findings
        .iter()
        .filter(|f| f.severity == "error")
        .map(|f| format!("{}: {}", f.rule, f.message))
        .collect()
}

fn transform() -> Transform {
    Transform {
        x: Emu(1000),
        y: Emu(2000),
        width: Emu(3000),
        height: Emu(600),
        ..Default::default()
    }
}

fn ole_kind(prog_id: &str, show_as_icon: bool) -> ObjectKind {
    ObjectKind::Ole {
        transform: transform(),
        prog_id: prog_id.into(),
        relationship: "rId2".into(),
        preview_relationship: "rId3".into(),
        show_as_icon,
    }
}

fn slide_of(kind: ObjectKind) -> Slide {
    Slide {
        objects: vec![Object {
            key: "ole".into(),
            id: 2,
            name: "Workbook".into(),
            morph_name: None,
            description: None,
            hyperlink: None,
            kind,
        }],
        ..Default::default()
    }
}

fn xml(slide: &Slide) -> String {
    String::from_utf8(compile_slide(slide).unwrap()).unwrap()
}

fn frame(xml: &str) -> &str {
    let start = xml.find("<p:graphicFrame>").expect("graphicFrame");
    let end = xml.find("</p:graphicFrame>").expect("graphicFrame end") + "</p:graphicFrame>".len();
    &xml[start..end]
}

/// The python-pptx `new_ole_object_graphicFrame` shape with the DeckWright frame
/// (cNvPr source extension, noGrp lock). imgW and imgH are the frame extent.
fn expected_frame(show_as_icon: bool) -> String {
    let icon = if show_as_icon {
        r#" showAsIcon="1""#
    } else {
        ""
    };
    format!(
        concat!(
            "<p:graphicFrame><p:nvGraphicFramePr>",
            "<p:cNvPr id=\"2\" name=\"Workbook\"><a:extLst><a:ext uri=\"urn:deckwright:source:v1\">",
            "<dw:source xmlns:dw=\"urn:deckwright:source:v1\" key=\"ole\"/></a:ext></a:extLst></p:cNvPr>",
            "<p:cNvGraphicFramePr><a:graphicFrameLocks noGrp=\"1\"/></p:cNvGraphicFramePr>",
            "<p:nvPr/></p:nvGraphicFramePr>",
            "<p:xfrm><a:off x=\"1000\" y=\"2000\"/><a:ext cx=\"3000\" cy=\"600\"/></p:xfrm>",
            "<a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/presentationml/2006/ole\">",
            "<p:oleObj progId=\"Excel.Sheet.12\" r:id=\"rId2\"{icon} imgW=\"3000\" imgH=\"600\">",
            "<p:embed/>",
            "<p:pic><p:nvPicPr><p:cNvPr id=\"0\" name=\"\"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr>",
            "<p:blipFill><a:blip r:embed=\"rId3\"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>",
            "<p:spPr><a:xfrm><a:off x=\"1000\" y=\"2000\"/><a:ext cx=\"3000\" cy=\"600\"/></a:xfrm>",
            "<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr></p:pic>",
            "</p:oleObj></a:graphicData></a:graphic></p:graphicFrame>"
        ),
        icon = icon
    )
}

/// A slide with one OLE frame bound to the links that `add_ole_object` returned.
fn ole_slide(prog_id: &str, links: &OleLinks) -> Slide {
    slide_of(ObjectKind::Ole {
        transform: transform(),
        prog_id: prog_id.into(),
        relationship: links.object.clone(),
        preview_relationship: links.preview.clone(),
        show_as_icon: true,
    })
}

#[test]
fn graphic_frame_skeleton_is_exact_with_show_as_icon() {
    let out = xml(&slide_of(ole_kind("Excel.Sheet.12", true)));
    assert_eq!(frame(&out), expected_frame(true));
}

#[test]
fn graphic_frame_skeleton_is_exact_without_show_as_icon() {
    let out = xml(&slide_of(ole_kind("Excel.Sheet.12", false)));
    assert_eq!(frame(&out), expected_frame(false));
    assert!(!out.contains("showAsIcon"));
}

#[test]
fn embedded_parts_follow_python_pptx_names_types_and_prog_ids() {
    let cases = [
        (
            "xlsx",
            xlsx(),
            "Excel.Sheet.12",
            "Microsoft_Excel_Sheet1.xlsx",
            XLSX_CT,
        ),
        (
            "docx",
            docx(),
            "Word.Document.12",
            "Microsoft_Word_Document1.docx",
            DOCX_CT,
        ),
        (
            "pptx",
            pptx(),
            "PowerPoint.Show.12",
            "Microsoft_PowerPoint_Presentation1.pptx",
            PPTX_CT,
        ),
    ];
    for (ext, bytes, prog_id, part, content_type) in cases {
        let mut deck = deck();
        let links = deck
            .add_ole_object(SLIDE, bytes.clone(), ext, PNG.to_vec())
            .unwrap();
        assert_ne!(links.object, links.preview, "{ext}: distinct relationships");
        deck.set_slide(SLIDE, xml(&ole_slide(prog_id, &links)).into_bytes())
            .unwrap();
        let package = deck.into_package();

        let object_part = format!("ppt/embeddings/{part}");
        assert_eq!(
            package.part(&object_part),
            Some(bytes.as_slice()),
            "{ext} bytes"
        );
        let content_types = text(&package, "[Content_Types].xml");
        assert!(
            content_types.contains(&format!(
                r#"<Default Extension="{ext}" ContentType="{content_type}"/>"#
            )),
            "{ext} default content type missing: {content_types}"
        );

        let object = rel(&package, SLIDE, &links.object);
        assert_eq!(object.relationship_type, PACKAGE_REL);
        assert_eq!(
            object.resolved_target.as_deref(),
            Some(object_part.as_str())
        );
        let preview = rel(&package, SLIDE, &links.preview);
        assert_eq!(preview.relationship_type, IMAGE_REL);
        assert_eq!(
            preview.resolved_target.as_deref(),
            Some("ppt/media/image1.png")
        );
        assert_eq!(package.part("ppt/media/image1.png"), Some(PNG));

        let slide = text(&package, SLIDE);
        assert!(
            slide.contains(&format!(r#"progId="{prog_id}""#)),
            "{ext} progId"
        );
        assert!(slide.contains(&format!(r#"r:id="{}""#, links.object)));
        assert!(slide.contains(&format!(r#"r:embed="{}""#, links.preview)));
        assert!(slide.contains(OLE_URI));
        assert_eq!(errors(&package), Vec::<String>::new(), "{ext} findings");
    }
}

#[test]
fn repeated_embeds_take_the_lowest_unused_number() {
    let mut deck = deck();
    let first = deck
        .add_ole_object(SLIDE, xlsx(), "xlsx", PNG.to_vec())
        .unwrap();
    let second = deck
        .add_ole_object(SLIDE, xlsx(), "xlsx", PNG.to_vec())
        .unwrap();
    assert_ne!(first.object, second.object);
    let package = deck.into_package();
    assert!(
        package
            .part("ppt/embeddings/Microsoft_Excel_Sheet1.xlsx")
            .is_some()
    );
    assert!(
        package
            .part("ppt/embeddings/Microsoft_Excel_Sheet2.xlsx")
            .is_some()
    );
    assert_eq!(
        rel(&package, SLIDE, &second.object)
            .resolved_target
            .as_deref(),
        Some("ppt/embeddings/Microsoft_Excel_Sheet2.xlsx")
    );
}

#[test]
fn only_ooxml_packages_are_embedded() {
    let mut deck = deck();
    let before = blank_bytes();

    let error = deck
        .add_ole_object(SLIDE, xlsx(), "bin", PNG.to_vec())
        .unwrap_err()
        .to_string();
    assert!(error.contains("xlsx, docx or pptx"), "{error:?}");
    assert!(error.contains("bin"), "{error:?}");

    let error = deck
        .add_ole_object(SLIDE, b"not a zip archive".to_vec(), "xlsx", PNG.to_vec())
        .unwrap_err()
        .to_string();
    assert!(error.contains("zip"), "{error:?}");

    let error = deck
        .add_ole_object(SLIDE, xlsx(), "xlsx", JPEG.to_vec())
        .unwrap_err()
        .to_string();
    assert!(error.contains("PNG"), "{error:?}");

    let error = deck
        .add_ole_object("ppt/slides/slide9.xml", xlsx(), "xlsx", PNG.to_vec())
        .unwrap_err()
        .to_string();
    assert!(error.contains("not a slide"), "{error:?}");

    // Rejected calls stage nothing.
    assert_eq!(deck.into_package().to_bytes().unwrap(), before);
}

#[test]
fn embedded_objects_revalidate_and_check_clean_after_round_trip() {
    // One OLE object per slide: python-pptx and PowerPoint both write id="0" for the
    // preview picture, and check_package reports duplicate IDs across a slide.
    let mut deck = Deck::from_package(template::blank(3, Emu(W), Emu(H)).unwrap());
    let slides = deck.slides();
    let cases = [
        (xlsx(), "xlsx", "Excel.Sheet.12", false),
        (docx(), "docx", "Word.Document.12", true),
        (pptx(), "pptx", "PowerPoint.Show.12", true),
    ];
    for (slide, (bytes, ext, prog_id, show_as_icon)) in slides.iter().zip(cases.clone()) {
        let links = deck
            .add_ole_object(slide, bytes, ext, PNG.to_vec())
            .unwrap();
        let object = Slide {
            objects: vec![Object {
                key: "ole".into(),
                id: 2,
                name: "Embedded".into(),
                morph_name: None,
                description: None,
                hyperlink: None,
                kind: ObjectKind::Ole {
                    transform: transform(),
                    prog_id: prog_id.into(),
                    relationship: links.object,
                    preview_relationship: links.preview,
                    show_as_icon,
                },
            }],
            ..Default::default()
        };
        deck.set_slide(slide, xml(&object).into_bytes()).unwrap();
    }

    let bytes = deck.into_package().to_bytes().unwrap();
    let package = Package::from_bytes(bytes, Limits::default()).unwrap();
    assert_eq!(errors(&package), Vec::<String>::new());
    for (slide, (_, _, prog_id, _)) in slides.iter().zip(cases) {
        let out = text(&package, slide);
        assert_eq!(out.matches(OLE_URI).count(), 1);
        assert!(out.contains(&format!(r#"progId="{prog_id}""#)));
    }
    assert!(
        package
            .part("ppt/embeddings/Microsoft_Excel_Sheet1.xlsx")
            .is_some()
    );
    assert!(
        package
            .part("ppt/embeddings/Microsoft_Word_Document1.docx")
            .is_some()
    );
    assert!(
        package
            .part("ppt/embeddings/Microsoft_PowerPoint_Presentation1.pptx")
            .is_some()
    );
}

fn ole_with(prog_id: &str, relationship: &str, preview: &str, transform: Transform) -> ObjectKind {
    ObjectKind::Ole {
        transform,
        prog_id: prog_id.into(),
        relationship: relationship.into(),
        preview_relationship: preview.into(),
        show_as_icon: false,
    }
}

#[test]
fn ole_validation_names_the_key_and_rejects_bad_fields() {
    let cases: Vec<(ObjectKind, &str)> = vec![
        (ole_with("", "rId2", "rId3", transform()), "progId"),
        (
            ole_with("Excel.Sheet.12", "", "rId3", transform()),
            "relationship",
        ),
        (
            ole_with("Excel.Sheet.12", "rId2", "", transform()),
            "preview",
        ),
        (
            ole_with(
                "Excel.Sheet.12",
                "rId2",
                "rId3",
                Transform {
                    width: Emu(0),
                    ..transform()
                },
            ),
            "extent",
        ),
        (
            ole_with(
                "Excel.Sheet.12",
                "rId2",
                "rId3",
                Transform {
                    rotation: Some(90),
                    ..transform()
                },
            ),
            "OLE frame supports only",
        ),
    ];
    for (kind, message) in cases {
        let error = compile_slide(&slide_of(kind))
            .expect_err(message)
            .to_string();
        assert!(
            error.contains(message),
            "{error:?} does not contain {message:?}"
        );
        assert!(error.contains("ole"), "error must name the key: {error:?}");
    }
    assert!(compile_slide(&slide_of(ole_kind("Excel.Sheet.12", true))).is_ok());
}

fn ole_json(show_as_icon: Option<bool>) -> Value {
    let mut kind = json!({
        "type": "ole",
        "transform": {"x": 1000, "y": 2000, "width": 3000, "height": 600},
        "prog_id": "Excel.Sheet.12",
        "relationship": "rId2",
        "preview_relationship": "rId3"
    });
    if let Some(value) = show_as_icon {
        kind["show_as_icon"] = json!(value);
    }
    json!({"objects": [{"key": "ole", "id": 2, "name": "Workbook", "kind": kind}]})
}

#[test]
fn serde_round_trip_is_stable_and_show_as_icon_defaults_off() {
    let value = ole_json(Some(true));
    let slide: Slide = serde_json::from_value(value.clone()).unwrap();
    let back = serde_json::to_value(&slide).unwrap();
    let again: Slide = serde_json::from_value(back.clone()).unwrap();
    assert_eq!(serde_json::to_value(&again).unwrap(), back);
    assert_eq!(xml(&slide), xml(&again));

    let minimal: Slide = serde_json::from_value(ole_json(None)).unwrap();
    assert!(!xml(&minimal).contains("showAsIcon"));
}

#[test]
fn serde_rejects_unknown_fields_at_kind_and_transform_levels() {
    let base = ole_json(Some(false));
    for pointer in ["/objects/0/kind", "/objects/0/kind/transform"] {
        let mut value = base.clone();
        value
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("bogus".into(), json!(1));
        assert!(
            serde_json::from_value::<Slide>(value).is_err(),
            "unknown field accepted at {pointer}"
        );
    }
    assert!(serde_json::from_value::<Slide>(base).is_ok());
}
