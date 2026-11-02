use deckwright::{
    builder::compile_slide,
    model::{Background, Fill, Slide},
};
use serde_json::{Value, json};

fn shape_with_fill(fill: Value) -> Slide {
    serde_json::from_value(
        json!({"objects":[{"key":"probe","id":2,"name":"Probe","kind":{
            "type":"shape",
            "properties":{"geometry":{"preset":"rect"},"fill":fill}
        }}]}),
    )
    .unwrap()
}

fn shape_with_line(line: Value) -> Slide {
    serde_json::from_value(
        json!({"objects":[{"key":"probe","id":2,"name":"Probe","kind":{
            "type":"shape",
            "properties":{"geometry":{"preset":"line"},"line":line}
        }}]}),
    )
    .unwrap()
}

fn with_background(background: Value) -> Slide {
    serde_json::from_value(json!({
        "objects":[{"key":"probe","id":2,"name":"Probe","kind":{
            "type":"shape",
            "properties":{"geometry":{"preset":"rect"},"fill":{"type":"none"}}
        }}],
        "background":background
    }))
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

fn stops_two() -> Value {
    json!([
        {"position":0,"color":{"type":"rgb","value":"FF0000"}},
        {"position":100000,"color":{"type":"rgb","value":"0000FF"}}
    ])
}

const TWO_STOPS_XML: &str = concat!(
    "<a:gsLst>",
    "<a:gs pos=\"0\"><a:srgbClr val=\"FF0000\"/></a:gs>",
    "<a:gs pos=\"100000\"><a:srgbClr val=\"0000FF\"/></a:gs>",
    "</a:gsLst>",
);

#[test]
fn linear_gradient_with_three_stops_emits_exact_fill() {
    let out = xml(&shape_with_fill(json!({
        "type":"gradient",
        "stops":[
            {"position":0,"color":{"type":"rgb","value":"FF0000"}},
            {"position":50000,"color":{"type":"rgb","value":"00FF00"}},
            {"position":100000,"color":{"type":"rgb","value":"0000FF"}}
        ],
        "angle":5400000,
        "rotate_with_shape":true
    })));
    assert!(
        out.contains(concat!(
            "<p:spPr><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>",
            "<a:gradFill rotWithShape=\"1\"><a:gsLst>",
            "<a:gs pos=\"0\"><a:srgbClr val=\"FF0000\"/></a:gs>",
            "<a:gs pos=\"50000\"><a:srgbClr val=\"00FF00\"/></a:gs>",
            "<a:gs pos=\"100000\"><a:srgbClr val=\"0000FF\"/></a:gs>",
            "</a:gsLst><a:lin ang=\"5400000\" scaled=\"0\"/></a:gradFill></p:spPr>",
        )),
        "{out}"
    );
}

#[test]
fn path_gradient_emits_fill_to_rect() {
    let out = xml(&shape_with_fill(json!({
        "type":"gradient",
        "stops":stops_two(),
        "path":"circle",
        "rotate_with_shape":false
    })));
    assert!(
        out.contains(&format!(
            "<a:gradFill rotWithShape=\"0\">{TWO_STOPS_XML}\
             <a:path path=\"circle\"><a:fillToRect l=\"50000\" t=\"50000\" r=\"50000\" b=\"50000\"/></a:path>\
             </a:gradFill>"
        )),
        "{out}"
    );
}

#[test]
fn pattern_fill_emits_foreground_and_background() {
    let out = xml(&shape_with_fill(json!({
        "type":"pattern",
        "preset":"dkDnDiag",
        "foreground":{"type":"rgb","value":"112233"},
        "background":{"type":"scheme","value":"bg1"}
    })));
    assert!(
        out.contains(concat!(
            "<a:pattFill prst=\"dkDnDiag\">",
            "<a:fgClr><a:srgbClr val=\"112233\"/></a:fgClr>",
            "<a:bgClr><a:schemeClr val=\"bg1\"/></a:bgClr>",
            "</a:pattFill>",
        )),
        "{out}"
    );
}

#[test]
fn picture_fill_emits_stretched_blip() {
    let out = xml(&shape_with_fill(
        json!({"type":"picture","relationship":"rId2"}),
    ));
    assert!(
        out.contains(concat!(
            "<a:blipFill><a:blip r:embed=\"rId2\"/>",
            "<a:stretch><a:fillRect/></a:stretch></a:blipFill>",
        )),
        "{out}"
    );
}

#[test]
fn gradient_line_emits_grad_fill_inside_a_ln() {
    let out = xml(&shape_with_line(json!({
        "width":12700,
        "fill":{"type":"gradient","stops":stops_two(),"angle":0,"rotate_with_shape":true}
    })));
    assert!(
        out.contains(&format!(
            "<a:ln w=\"12700\"><a:gradFill rotWithShape=\"1\">{TWO_STOPS_XML}\
             <a:lin ang=\"0\" scaled=\"0\"/></a:gradFill></a:ln>"
        )),
        "{out}"
    );
}

#[test]
fn pattern_line_emits_patt_fill_inside_a_ln() {
    let out = xml(&shape_with_line(json!({
        "fill":{"type":"pattern","preset":"pct50",
            "foreground":{"type":"rgb","value":"000000"},
            "background":{"type":"rgb","value":"FFFFFF"}}
    })));
    assert!(
        out.contains(concat!(
            "<a:ln><a:pattFill prst=\"pct50\">",
            "<a:fgClr><a:srgbClr val=\"000000\"/></a:fgClr>",
            "<a:bgClr><a:srgbClr val=\"FFFFFF\"/></a:bgClr>",
            "</a:pattFill></a:ln>",
        )),
        "{out}"
    );
}

#[test]
fn picture_fill_on_a_line_is_rejected_naming_the_key() {
    let message = error(&shape_with_line(json!({
        "fill":{"type":"picture","relationship":"rId2"}
    })));
    assert!(message.contains("probe"), "{message}");
    assert!(message.contains("picture"), "{message}");
}

#[test]
fn gradient_rejects_fewer_than_two_stops() {
    let message = error(&shape_with_fill(json!({
        "type":"gradient",
        "stops":[{"position":0,"color":{"type":"rgb","value":"FF0000"}}],
        "angle":0,"rotate_with_shape":true
    })));
    assert!(message.contains("probe"), "{message}");
    assert!(message.contains("at least two"), "{message}");
}

#[test]
fn gradient_rejects_descending_positions() {
    let message = error(&shape_with_fill(json!({
        "type":"gradient",
        "stops":[
            {"position":60000,"color":{"type":"rgb","value":"FF0000"}},
            {"position":10000,"color":{"type":"rgb","value":"0000FF"}}
        ],
        "angle":0,"rotate_with_shape":true
    })));
    assert!(message.contains("ascending"), "{message}");
}

#[test]
fn gradient_rejects_position_above_100000() {
    let message = error(&shape_with_fill(json!({
        "type":"gradient",
        "stops":[
            {"position":0,"color":{"type":"rgb","value":"FF0000"}},
            {"position":100001,"color":{"type":"rgb","value":"0000FF"}}
        ],
        "angle":0,"rotate_with_shape":true
    })));
    assert!(message.contains("100000"), "{message}");
}

#[test]
fn gradient_rejects_neither_angle_nor_path() {
    let message = error(&shape_with_fill(json!({
        "type":"gradient","stops":stops_two(),"rotate_with_shape":true
    })));
    assert!(message.contains("exactly one"), "{message}");
}

#[test]
fn gradient_rejects_angle_and_path_together() {
    let message = error(&shape_with_fill(json!({
        "type":"gradient","stops":stops_two(),"angle":0,"path":"rect",
        "rotate_with_shape":true
    })));
    assert!(message.contains("exactly one"), "{message}");
}

#[test]
fn gradient_rejects_unknown_path_shape() {
    let message = error(&shape_with_fill(json!({
        "type":"gradient","stops":stops_two(),"path":"ellipse","rotate_with_shape":true
    })));
    assert!(message.contains("ellipse"), "{message}");
}

#[test]
fn gradient_rejects_angle_at_full_turn() {
    let message = error(&shape_with_fill(json!({
        "type":"gradient","stops":stops_two(),"angle":21600000,"rotate_with_shape":true
    })));
    assert!(message.contains("21599999"), "{message}");
}

#[test]
fn pattern_rejects_unknown_preset() {
    let message = error(&shape_with_fill(json!({
        "type":"pattern","preset":"pct55",
        "foreground":{"type":"rgb","value":"000000"},
        "background":{"type":"rgb","value":"FFFFFF"}
    })));
    assert!(message.contains("pct55"), "{message}");
}

#[test]
fn picture_fill_rejects_empty_relationship() {
    let message = error(&shape_with_fill(
        json!({"type":"picture","relationship":""}),
    ));
    assert!(message.contains("relationship"), "{message}");
}

#[test]
fn background_fill_is_first_child_of_csld() {
    let out = xml(&with_background(json!({
        "type":"fill",
        "fill":{"type":"solid","color":{"type":"rgb","value":"102030"}}
    })));
    assert!(
        out.contains(concat!(
            "<p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val=\"102030\"/></a:solidFill>",
            "<a:effectLst/></p:bgPr></p:bg><p:spTree>",
        )),
        "{out}"
    );
}

#[test]
fn background_gradient_fill_emits_grad_fill_in_bg_pr() {
    let out = xml(&with_background(json!({
        "type":"fill",
        "fill":{"type":"gradient","stops":stops_two(),"angle":2700000,"rotate_with_shape":true}
    })));
    assert!(
        out.contains(&format!(
            "<p:cSld><p:bg><p:bgPr><a:gradFill rotWithShape=\"1\">{TWO_STOPS_XML}\
             <a:lin ang=\"2700000\" scaled=\"0\"/></a:gradFill><a:effectLst/></p:bgPr></p:bg><p:spTree>"
        )),
        "{out}"
    );
}

#[test]
fn background_reference_is_first_child_of_csld() {
    let out = xml(&with_background(json!({
        "type":"reference","index":1001,
        "color":{"type":"scheme","value":"bg1"}
    })));
    assert!(
        out.contains(concat!(
            "<p:cSld><p:bg><p:bgRef idx=\"1001\"><a:schemeClr val=\"bg1\"/></p:bgRef></p:bg>",
            "<p:spTree>",
        )),
        "{out}"
    );
}

#[test]
fn background_reference_accepts_fill_style_indices() {
    for index in [1, 2, 3, 1002, 1003] {
        let out = xml(&with_background(json!({
            "type":"reference","index":index,
            "color":{"type":"scheme","value":"bg1"}
        })));
        assert!(out.contains(&format!("<p:bgRef idx=\"{index}\">")), "{out}");
    }
}

#[test]
fn background_reference_rejects_other_indices() {
    for index in [0, 4, 1000, 1004] {
        let message = error(&with_background(json!({
            "type":"reference","index":index,
            "color":{"type":"scheme","value":"bg1"}
        })));
        assert!(message.contains("bgRef"), "{index}: {message}");
    }
}

#[test]
fn background_picture_fill_requires_relationship() {
    let message = error(&with_background(json!({
        "type":"fill","fill":{"type":"picture","relationship":""}
    })));
    assert!(message.contains("relationship"), "{message}");
}

#[test]
fn slide_without_background_emits_no_p_bg() {
    let out = xml(&shape_with_fill(json!({"type":"none"})));
    assert!(!out.contains("<p:bg>"), "{out}");
    assert!(out.contains("<p:cSld><p:spTree>"), "{out}");
}

#[test]
fn fill_round_trips_through_serde() {
    for value in [
        json!({"type":"gradient","stops":stops_two(),"path":"shape","rotate_with_shape":false}),
        json!({"type":"pattern","preset":"smCheck",
            "foreground":{"type":"rgb","value":"000000"},
            "background":{"type":"rgb","value":"FFFFFF"}}),
        json!({"type":"picture","relationship":"rId9"}),
    ] {
        let fill: Fill = serde_json::from_value(value.clone()).unwrap();
        // Absent optionals serialize as null, so compare a second, stable round trip.
        let once = serde_json::to_value(&fill).unwrap();
        assert_eq!(once["type"], value["type"]);
        let again: Fill = serde_json::from_value(once.clone()).unwrap();
        assert_eq!(serde_json::to_value(&again).unwrap(), once);
    }
}

#[test]
fn background_round_trips_through_serde() {
    for value in [
        json!({"type":"fill","fill":{"type":"picture","relationship":"rId3"}}),
        json!({"type":"reference","index":1003,"color":{"type":"scheme","value":"bg2"}}),
    ] {
        let background: Background = serde_json::from_value(value.clone()).unwrap();
        let once = serde_json::to_value(&background).unwrap();
        assert_eq!(once["type"], value["type"]);
        let again: Background = serde_json::from_value(once.clone()).unwrap();
        assert_eq!(serde_json::to_value(&again).unwrap(), once);
    }
}

#[test]
fn slide_round_trips_background_and_defaults_it_when_absent() {
    let slide: Slide = serde_json::from_value(json!({
        "objects":[],
        "background":{"type":"reference","index":1001,"color":{"type":"scheme","value":"bg1"}}
    }))
    .unwrap();
    let value = serde_json::to_value(&slide).unwrap();
    let again: Slide = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(&again).unwrap(), value);

    let bare: Slide = serde_json::from_value(json!({"objects":[]})).unwrap();
    assert!(bare.background.is_none());
}

#[test]
fn unknown_fields_are_rejected_in_fills_stops_and_backgrounds() {
    let rejected = [
        serde_json::from_value::<Fill>(json!({
            "type":"gradient","stops":stops_two(),"angle":0,
            "rotate_with_shape":true,"extra":1
        }))
        .err()
        .map(|e| e.to_string()),
        serde_json::from_value::<Fill>(json!({
            "type":"pattern","preset":"pct5","extra":1,
            "foreground":{"type":"rgb","value":"000000"},
            "background":{"type":"rgb","value":"FFFFFF"}
        }))
        .err()
        .map(|e| e.to_string()),
        serde_json::from_value::<Fill>(json!({"type":"picture","relationship":"rId1","extra":1}))
            .err()
            .map(|e| e.to_string()),
        serde_json::from_value::<Fill>(json!({
            "type":"gradient",
            "stops":[{"position":0,"color":{"type":"rgb","value":"FF0000"},"extra":1},
                     {"position":100000,"color":{"type":"rgb","value":"0000FF"}}],
            "angle":0,"rotate_with_shape":true
        }))
        .map(|_| ())
        .err()
        .map(|e| e.to_string()),
        serde_json::from_value::<Background>(json!({
            "type":"fill","fill":{"type":"none"},"extra":1
        }))
        .err()
        .map(|e| e.to_string()),
        serde_json::from_value::<Background>(json!({
            "type":"reference","index":1,"color":{"type":"scheme","value":"bg1"},"extra":1
        }))
        .err()
        .map(|e| e.to_string()),
        serde_json::from_value::<Slide>(json!({"objects":[],"extra":1}))
            .err()
            .map(|e| e.to_string()),
    ];
    for (i, result) in rejected.iter().enumerate() {
        assert!(result.is_some(), "case {i} accepted an unknown field");
    }
}
