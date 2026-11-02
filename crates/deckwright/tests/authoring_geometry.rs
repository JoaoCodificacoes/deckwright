use deckwright::{builder::compile_slide, model::*};
use serde_json::{Value, json};

fn slide_with(properties: Value) -> Slide {
    serde_json::from_value(
        json!({"objects":[{"key":"shape","id":2,"name":"Shape","kind":{
            "type":"shape","properties":properties
        }}]}),
    )
    .unwrap()
}

fn xml(slide: &Slide) -> String {
    String::from_utf8(compile_slide(slide).unwrap()).unwrap()
}

fn error(slide: &Slide) -> String {
    compile_slide(slide)
        .expect_err("expected rejection")
        .to_string()
}

#[test]
fn preset_geometry_accepts_full_st_shape_type_names() {
    for preset in [
        "ellipse",
        "flowChartProcess",
        "plus",
        "flowChartOfflineStorage",
        "star32",
        "actionButtonMovie",
        "line",
        "straightConnector1",
    ] {
        let out = xml(&slide_with(json!({"geometry":{"preset":preset}})));
        assert!(
            out.contains(&format!(
                "<a:prstGeom prst=\"{preset}\"><a:avLst/></a:prstGeom>"
            )),
            "{preset}"
        );
    }
    let out = xml(&slide_with(
        json!({"geometry":{"preset":"rightArrow","adjustments":[
            {"name":"adj1","value":50000},{"name":"adj2","value":25000}
        ]}}),
    ));
    assert!(out.contains(
        "<a:prstGeom prst=\"rightArrow\"><a:avLst><a:gd name=\"adj1\" fmla=\"val 50000\"/><a:gd name=\"adj2\" fmla=\"val 25000\"/></a:avLst></a:prstGeom>"
    ));
}

#[test]
fn unknown_and_empty_preset_names_are_rejected() {
    for preset in ["notAShape", "", "Ellipse", "rect "] {
        let slide = slide_with(json!({"geometry":{"preset":preset}}));
        assert!(
            error(&slide).contains("unsupported geometry preset"),
            "{preset:?}"
        );
    }
}

#[test]
fn custom_geometry_emits_exact_schema_ordered_xml() {
    let out = xml(&slide_with(json!({
        "transform":{"x":10,"y":20,"width":100,"height":100},
        "custom":{"paths":[
            {"width":100,"height":100,"fill":"none","commands":[
                {"type":"move_to","x":0,"y":0},
                {"type":"line_to","x":100,"y":0},
                {"type":"quad_to","x1":100,"y1":50,"x":50,"y":100},
                {"type":"cubic_to","x1":0,"y1":100,"x2":0,"y2":50,"x":0,"y":0},
                {"type":"arc_to","width_radius":50,"height_radius":50,"start_angle":0,"swing_angle":5400000},
                {"type":"close"}
            ]},
            {"width":10,"height":20,"stroke":true,"commands":[
                {"type":"move_to","x":0,"y":0},
                {"type":"close"}
            ]}
        ]}
    })));
    let expected = concat!(
        "<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/>",
        "<a:rect l=\"l\" t=\"t\" r=\"r\" b=\"b\"/><a:pathLst>",
        "<a:path w=\"100\" h=\"100\" fill=\"none\" stroke=\"0\">",
        "<a:moveTo><a:pt x=\"0\" y=\"0\"/></a:moveTo>",
        "<a:lnTo><a:pt x=\"100\" y=\"0\"/></a:lnTo>",
        "<a:quadBezTo><a:pt x=\"100\" y=\"50\"/><a:pt x=\"50\" y=\"100\"/></a:quadBezTo>",
        "<a:cubicBezTo><a:pt x=\"0\" y=\"100\"/><a:pt x=\"0\" y=\"50\"/><a:pt x=\"0\" y=\"0\"/></a:cubicBezTo>",
        "<a:arcTo wR=\"50\" hR=\"50\" stAng=\"0\" swAng=\"5400000\"/><a:close/></a:path>",
        "<a:path w=\"10\" h=\"20\"><a:moveTo><a:pt x=\"0\" y=\"0\"/></a:moveTo><a:close/></a:path>",
        "</a:pathLst></a:custGeom>"
    );
    assert!(out.contains(expected), "{out}");
    // In spPr the geometry follows the transform and replaces prstGeom.
    assert!(out.contains("</a:xfrm><a:custGeom>"));
    assert!(!out.contains("prstGeom"));
}

#[test]
fn custom_geometry_validation_rejects_malformed_paths() {
    let path = |width: i64, height: i64, commands: Value| json!({"paths":[{"width":width,"height":height,"commands":commands}]});
    let move0 = json!({"type":"move_to","x":0,"y":0});
    let cases = [
        (json!({"paths":[]}), "at least one path"),
        (
            path(10, 10, json!([{"type":"line_to","x":1,"y":1}])),
            "must start with moveTo",
        ),
        (path(10, 10, json!([])), "must start with moveTo"),
        (
            path(
                10,
                10,
                json!([move0.clone(), {"type":"line_to","x":11,"y":0}]),
            ),
            "path x coordinate",
        ),
        (
            path(
                10,
                10,
                json!([move0.clone(), {"type":"cubic_to","x1":0,"y1":0,"x2":0,"y2":-1,"x":0,"y":0}]),
            ),
            "path y coordinate",
        ),
        (
            path(
                10,
                10,
                json!([move0.clone(), {"type":"arc_to","width_radius":11,"height_radius":1,"start_angle":0,"swing_angle":0}]),
            ),
            "arc width radius",
        ),
        (path(0, 10, json!([move0.clone()])), "path width"),
        (path(10, 0, json!([move0.clone()])), "path height"),
        (
            json!({"paths":[{"width":10,"height":10,"fill":"banana","commands":[move0.clone()]}]}),
            "path fill mode",
        ),
    ];
    for (custom, message) in cases {
        let slide = slide_with(json!({"custom": custom}));
        let found = error(&slide);
        assert!(found.contains(message), "{message}: {found}");
    }
    let mixed = slide_with(json!({
        "geometry":{"preset":"rect"},
        "custom":{"paths":[{"width":10,"height":10,"commands":[{"type":"move_to","x":0,"y":0}]}]}
    }));
    assert!(error(&mixed).contains("mutually exclusive"));
}

fn video_picture(media: Value) -> Slide {
    serde_json::from_value(
        json!({"objects":[{"key":"clip","id":2,"name":"Clip","kind":{
            "type":"picture","relationship":"rId3","media":media,
            "properties":{"transform":{"x":1,"y":2,"width":3,"height":4}}
        }}]}),
    )
    .unwrap()
}

#[test]
fn video_picture_emits_media_link_extension_and_click_action() {
    let out = xml(&video_picture(json!({
        "video_relationship":"rId4","media_relationship":"rId5"
    })));
    assert!(out.contains(
        "<p:cNvPr id=\"2\" name=\"Clip\"><a:hlinkClick r:id=\"\" action=\"ppaction://media\"/><a:extLst>"
    ));
    assert!(out.contains(concat!(
        "<p:nvPr><a:videoFile r:link=\"rId4\"/><p:extLst>",
        "<p:ext uri=\"{DAA4B4D4-6D71-4841-9C94-3DE7FCFB9230}\">",
        "<p14:media xmlns:p14=\"http://schemas.microsoft.com/office/powerpoint/2010/main\" r:embed=\"rId5\"/>",
        "</p:ext></p:extLst></p:nvPr>"
    )));
    // The poster image stays the blip; the media parts are the caller's relationships.
    assert!(out.contains("<a:blip r:embed=\"rId3\"/>"));
    // Schema order inside cNvPr: hlinkClick precedes the source extLst.
    assert!(out.find("<a:hlinkClick").unwrap() < out.find("<a:extLst>").unwrap());
}

#[test]
fn video_picture_rejects_missing_relationship_ids() {
    for media in [
        json!({"video_relationship":"","media_relationship":"rId5"}),
        json!({"video_relationship":"rId4","media_relationship":""}),
    ] {
        assert!(error(&video_picture(media)).contains("video picture requires"));
    }
}

#[test]
fn picture_without_media_keeps_the_existing_output() {
    let slide: Slide = serde_json::from_value(json!({"objects":[{"key":"picture","id":4,"name":"Picture","kind":{
        "type":"picture","relationship":"rId3",
        "crop":{"left":1000,"top":2000,"right":3000,"bottom":4000},
        "lock_aspect":true,
        "properties":{"transform":{"x":180,"y":200,"width":50,"height":70},"geometry":{"preset":"rect"}}
    }}]}))
    .unwrap();
    let out = xml(&slide);
    // Captured from the pre-change compiler at base a02f06c.
    let start = out.find("<p:pic>").unwrap();
    let end = start + out[start..].find("</p:pic>").unwrap() + "</p:pic>".len();
    assert_eq!(
        &out[start..end],
        concat!(
            "<p:pic><p:nvPicPr><p:cNvPr id=\"4\" name=\"Picture\"><a:extLst><a:ext uri=\"urn:deckwright:source:v1\">",
            "<dw:source xmlns:dw=\"urn:deckwright:source:v1\" key=\"picture\"/></a:ext></a:extLst></p:cNvPr>",
            "<p:cNvPicPr><a:picLocks noChangeAspect=\"1\"/></p:cNvPicPr><p:nvPr/></p:nvPicPr>",
            "<p:blipFill><a:blip r:embed=\"rId3\"/><a:srcRect l=\"1000\" t=\"2000\" r=\"3000\" b=\"4000\"/>",
            "<a:stretch><a:fillRect/></a:stretch></p:blipFill>",
            "<p:spPr><a:xfrm><a:off x=\"180\" y=\"200\"/><a:ext cx=\"50\" cy=\"70\"/></a:xfrm>",
            "<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr></p:pic>"
        )
    );
    assert!(!out.contains("videoFile") && !out.contains("hlinkClick"));
}

#[test]
fn new_model_types_round_trip_and_reject_unknown_fields() {
    let json = json!({"objects":[
        {"key":"custom","id":2,"name":"Custom","kind":{"type":"shape","properties":{
            "transform":{"x":0,"y":0,"width":10,"height":10},
            "custom":{"paths":[{"width":10,"height":10,"fill":"norm","commands":[
                {"type":"move_to","x":0,"y":0},{"type":"arc_to","width_radius":5,"height_radius":5,"start_angle":0,"swing_angle":60000},{"type":"close"}
            ]}]}}}},
        {"key":"clip","id":3,"name":"Clip","kind":{"type":"picture","relationship":"rId3",
            "media":{"video_relationship":"rId4","media_relationship":"rId5"},"properties":{}}}
    ]});
    let slide: Slide = serde_json::from_value(json.clone()).unwrap();
    let back = serde_json::to_value(&slide).unwrap();
    assert_eq!(
        back["objects"][0]["kind"]["properties"]["custom"]["paths"][0]["commands"][1]["type"],
        "arc_to"
    );
    assert_eq!(
        back["objects"][1]["kind"]["media"]["media_relationship"],
        "rId5"
    );
    let reloaded: Slide = serde_json::from_value(back).unwrap();
    assert_eq!(xml(&slide), xml(&reloaded));

    let extra = [
        json!({"type":"close","extra":1}),
        json!({"type":"banana"}),
        json!({"type":"move_to","x":0,"y":0,"z":1}),
    ];
    for command in extra {
        assert!(serde_json::from_value::<PathCommand>(command).is_err());
    }
    assert!(serde_json::from_value::<CustomGeometry>(json!({"paths":[],"extra":true})).is_err());
    assert!(
        serde_json::from_value::<MediaLink>(
            json!({"video_relationship":"a","media_relationship":"b","x":1})
        )
        .is_err()
    );
    let mut bad = json.clone();
    bad["objects"][1]["kind"]["media"]["extra"] = json!(1);
    assert!(serde_json::from_value::<Slide>(bad).is_err());
    let mut bad = json;
    bad["objects"][0]["kind"]["properties"]["custom"]["paths"][0]["extra"] = json!(1);
    assert!(serde_json::from_value::<Slide>(bad).is_err());
}

#[test]
fn explosion_names_are_not_preset_geometry() {
    // ST_ShapeType spells these irregularSeal1/irregularSeal2; explosion1/2 are UI names.
    for (name, ok) in [
        ("explosion1", false),
        ("explosion2", false),
        ("irregularSeal1", true),
        ("irregularSeal2", true),
    ] {
        let mut slide = deckwright::model::Slide::default();
        slide.objects.push(deckwright::model::Object {
            key: "s".into(),
            id: 2,
            name: "s".into(),
            morph_name: None,
            description: None,
            hyperlink: None,
            kind: deckwright::model::ObjectKind::Shape {
                textbox: false,
                lock_no_group: false,
                placeholder: None,
                properties: deckwright::model::Properties {
                    geometry: Some(deckwright::model::Geometry {
                        preset: name.into(),
                        adjustments: vec![],
                    }),
                    ..Default::default()
                },
                style: None,
                text: None,
            },
        });
        assert_eq!(
            deckwright::builder::compile_slide(&slide).is_ok(),
            ok,
            "{name}"
        );
    }
}
