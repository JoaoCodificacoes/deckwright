use deckwright::{
    builder::compile_slide,
    model::{Bullet, Hyperlink, ParagraphProperties, Slide, TextRun},
};
use serde_json::{Value, json};

/// ST_TextAutonumberScheme, spelled as ECMA-376 lists it.
const AUTONUMBER_SCHEMES: &[&str] = &[
    "alphaLcParenBoth",
    "alphaLcParenR",
    "alphaLcPeriod",
    "alphaUcParenBoth",
    "alphaUcParenR",
    "alphaUcPeriod",
    "arabic1Minus",
    "arabic2Minus",
    "arabicDBPeriod",
    "arabicDBPlain",
    "arabicParenBoth",
    "arabicParenR",
    "arabicPeriod",
    "arabicPlain",
    "circleNumDBPlain",
    "circleNumWdBlackPlain",
    "circleNumWdWhitePlain",
    "ea1ChsPeriod",
    "ea1ChsPlain",
    "ea1ChtPeriod",
    "ea1ChtPlain",
    "ea1JpnChsDBPeriod",
    "ea1JpnKorPeriod",
    "ea1JpnKorPlain",
    "hebrew2Minus",
    "hindiAlpha1Period",
    "hindiAlphaPeriod",
    "hindiNumParenR",
    "hindiNumPeriod",
    "romanLcParenBoth",
    "romanLcParenR",
    "romanLcPeriod",
    "romanUcParenBoth",
    "romanUcParenR",
    "romanUcPeriod",
    "thaiAlphaParenBoth",
    "thaiAlphaParenR",
    "thaiAlphaPeriod",
    "thaiNumParenBoth",
    "thaiNumParenR",
    "thaiNumPeriod",
];

fn shape_with_paragraphs(paragraphs: Value) -> Slide {
    serde_json::from_value(
        json!({"objects":[{"key":"probe","id":2,"name":"Probe","kind":{
            "type":"shape",
            "properties":{"geometry":{"preset":"rect"},"fill":{"type":"none"}},
            "text":{"paragraphs":paragraphs}
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

fn one_paragraph(properties: Value) -> Slide {
    shape_with_paragraphs(json!([{"properties":properties,"runs":[{"text":"Item"}]}]))
}

const ITEM_RUN: &str = r#"<a:r><a:t xml:space="preserve">Item</a:t></a:r>"#;

#[test]
fn level_margin_and_negative_indent_emit_exact_ppr() {
    let out = xml(&one_paragraph(json!({
        "level": 1, "margin_left": 457200, "indent": -228600
    })));
    assert!(
        out.contains(&format!(
            "<a:p><a:pPr lvl=\"1\" marL=\"457200\" indent=\"-228600\"/>{ITEM_RUN}</a:p>"
        )),
        "{out}"
    );
}

#[test]
fn level_zero_is_emitted_explicitly() {
    let out = xml(&one_paragraph(json!({"level": 0})));
    assert!(
        out.contains(&format!("<a:p><a:pPr lvl=\"0\"/>{ITEM_RUN}</a:p>")),
        "{out}"
    );
}

#[test]
fn char_bullet_with_all_options_follows_schema_order() {
    let out = xml(&one_paragraph(json!({
        "space_before": 200,
        "space_after": 100,
        "bullet": {
            "type": "char",
            "char": "•",
            "font": "Arial",
            "color": {"type": "rgb", "value": "FF0000"},
            "size_percent": 110
        }
    })));
    assert!(
        out.contains(concat!(
            "<a:pPr><a:spcBef><a:spcPts val=\"200\"/></a:spcBef>",
            "<a:spcAft><a:spcPts val=\"100\"/></a:spcAft>",
            "<a:buClr><a:srgbClr val=\"FF0000\"/></a:buClr>",
            "<a:buSzPct val=\"110000\"/>",
            "<a:buFont typeface=\"Arial\"/>",
            "<a:buChar char=\"•\"/></a:pPr>",
        )),
        "{out}"
    );
}

#[test]
fn char_bullet_without_options_emits_only_buchar() {
    let out = xml(&one_paragraph(json!({
        "bullet": {"type": "char", "char": "-"}
    })));
    assert!(
        out.contains(&format!("<a:pPr><a:buChar char=\"-\"/></a:pPr>{ITEM_RUN}")),
        "{out}"
    );
}

#[test]
fn auto_number_with_start_at_emits_exact_ppr() {
    let out = xml(&one_paragraph(json!({
        "level": 2,
        "bullet": {"type": "auto_number", "scheme": "romanLcPeriod", "start_at": 3}
    })));
    assert!(
        out.contains(&format!(
            "<a:pPr lvl=\"2\"><a:buAutoNum type=\"romanLcPeriod\" startAt=\"3\"/></a:pPr>{ITEM_RUN}"
        )),
        "{out}"
    );
}

#[test]
fn auto_number_without_start_at_omits_attribute() {
    let out = xml(&one_paragraph(json!({
        "bullet": {"type": "auto_number", "scheme": "arabicPeriod"}
    })));
    assert!(
        out.contains("<a:pPr><a:buAutoNum type=\"arabicPeriod\"/></a:pPr>"),
        "{out}"
    );
}

#[test]
fn bullet_none_emits_bu_none() {
    let out = xml(&one_paragraph(json!({"bullet": {"type": "none"}})));
    assert!(
        out.contains(&format!("<a:pPr><a:buNone/></a:pPr>{ITEM_RUN}")),
        "{out}"
    );
}

#[test]
fn absent_text_properties_emit_no_new_markup() {
    let out = xml(&one_paragraph(json!({"alignment": "ctr"})));
    assert!(
        out.contains(&format!("<a:p><a:pPr algn=\"ctr\"/>{ITEM_RUN}</a:p>")),
        "{out}"
    );
    assert!(!out.contains("<a:rPr"), "{out}");
    assert!(!out.contains("hlinkClick"), "{out}");
}

#[test]
fn every_autonumber_scheme_is_accepted() {
    for scheme in AUTONUMBER_SCHEMES {
        let slide = one_paragraph(json!({
            "bullet": {"type": "auto_number", "scheme": scheme}
        }));
        assert!(
            compile_slide(&slide).is_ok(),
            "scheme {scheme} rejected: {}",
            error(&slide)
        );
    }
}

#[test]
fn range_boundaries_are_accepted() {
    let cases = [
        json!({"level": 8}),
        json!({"margin_left": 0}),
        json!({"margin_left": 51206400}),
        json!({"indent": -51206400}),
        json!({"indent": 51206400}),
        json!({"bullet": {"type": "char", "char": "•", "size_percent": 25}}),
        json!({"bullet": {"type": "char", "char": "•", "size_percent": 400}}),
        json!({"bullet": {"type": "auto_number", "scheme": "arabicPeriod", "start_at": 1}}),
        json!({"bullet": {"type": "auto_number", "scheme": "arabicPeriod", "start_at": 32767}}),
    ];
    for properties in cases {
        let slide = one_paragraph(properties.clone());
        assert!(
            compile_slide(&slide).is_ok(),
            "{properties} rejected: {}",
            error(&slide)
        );
    }
}

#[test]
fn out_of_range_and_unknown_enum_values_are_rejected_with_key() {
    let cases: [(Value, &str); 11] = [
        (json!({"level": 9}), "paragraph level"),
        (json!({"margin_left": -1}), "paragraph left margin"),
        (json!({"indent": 51206401}), "paragraph indent"),
        (json!({"indent": -51206401}), "paragraph indent"),
        (
            json!({"bullet": {"type": "char", "char": "•", "size_percent": 24}}),
            "bullet size percent",
        ),
        (
            json!({"bullet": {"type": "char", "char": "•", "size_percent": 401}}),
            "bullet size percent",
        ),
        (
            json!({"bullet": {"type": "char", "char": ""}}),
            "bullet character must not be empty",
        ),
        (
            json!({"bullet": {"type": "auto_number", "scheme": "arabicperiod"}}),
            "auto number scheme",
        ),
        (
            json!({"bullet": {"type": "auto_number", "scheme": "bogus"}}),
            "auto number scheme",
        ),
        (
            json!({"bullet": {"type": "auto_number", "scheme": "arabicPeriod", "start_at": 0}}),
            "bullet start number",
        ),
        (
            json!({"bullet": {"type": "auto_number", "scheme": "arabicPeriod", "start_at": 32768}}),
            "bullet start number",
        ),
    ];
    for (properties, message) in cases {
        let slide = one_paragraph(properties.clone());
        let text = error(&slide);
        assert!(text.contains("probe"), "{properties}: {text}");
        assert!(text.contains(message), "{properties}: {text}");
    }
}

#[test]
fn run_hyperlink_without_style_emits_rpr_with_link() {
    let out = xml(&shape_with_paragraphs(json!([{"runs":[
        {"text":"Docs","hyperlink":{"relationship":"rId9"}}
    ]}])));
    assert!(
        out.contains(concat!(
            "<a:r><a:rPr><a:hlinkClick r:id=\"rId9\"/></a:rPr>",
            "<a:t xml:space=\"preserve\">Docs</a:t></a:r>",
        )),
        "{out}"
    );
}

#[test]
fn run_hyperlink_with_style_orders_latin_before_hlink() {
    let out = xml(&shape_with_paragraphs(json!([{"runs":[{
        "text":"Docs",
        "style":{"font":"Calibri","bold":true,"color":{"type":"rgb","value":"0000FF"}},
        "hyperlink":{"relationship":"rId9","tooltip":"Open docs"}
    }]}])));
    assert!(
        out.contains(concat!(
            "<a:rPr b=\"1\"><a:solidFill><a:srgbClr val=\"0000FF\"/></a:solidFill>",
            "<a:latin typeface=\"Calibri\"/>",
            "<a:hlinkClick r:id=\"rId9\" tooltip=\"Open docs\"/></a:rPr>",
        )),
        "{out}"
    );
}

#[test]
fn run_without_hyperlink_or_style_keeps_bare_run() {
    let out = xml(&shape_with_paragraphs(json!([{"runs":[{"text":"plain"}]}])));
    assert!(
        out.contains("<a:r><a:t xml:space=\"preserve\">plain</a:t></a:r>"),
        "{out}"
    );
    assert!(!out.contains("<a:rPr"), "{out}");
}

#[test]
fn jump_to_slide_run_link_carries_action() {
    let out = xml(&shape_with_paragraphs(json!([{"runs":[{
        "text":"Next",
        "hyperlink":{"relationship":"rId4","action":"ppaction://hlinksldjump"}
    }]}])));
    assert!(
        out.contains(
            "<a:rPr><a:hlinkClick r:id=\"rId4\" action=\"ppaction://hlinksldjump\"/></a:rPr>"
        ),
        "{out}"
    );
}

#[test]
fn run_hyperlink_requires_relationship_id() {
    let slide = shape_with_paragraphs(json!([{"runs":[
        {"text":"x","hyperlink":{"relationship":""}}
    ]}]));
    let text = error(&slide);
    assert!(text.contains("probe"), "{text}");
    assert!(
        text.contains("hyperlink requires relationship ID"),
        "{text}"
    );
}

fn shape_with_object_link(hyperlink: Value) -> Slide {
    serde_json::from_value(json!({"objects":[{"key":"probe","id":2,"name":"Probe",
        "hyperlink": hyperlink,
        "kind":{"type":"shape","properties":{"geometry":{"preset":"rect"}}}}]}))
    .unwrap()
}

#[test]
fn shape_hyperlink_is_first_cnvpr_child() {
    let out = xml(&shape_with_object_link(json!({"relationship":"rId7"})));
    assert!(
        out.contains("<p:cNvPr id=\"2\" name=\"Probe\"><a:hlinkClick r:id=\"rId7\"/><a:extLst>"),
        "{out}"
    );
}

#[test]
fn shape_jump_link_carries_action_and_tooltip_first() {
    let out = xml(&shape_with_object_link(json!({
        "relationship":"rId8",
        "action":"ppaction://hlinksldjump",
        "tooltip":"Go to slide 2"
    })));
    assert!(
        out.contains(concat!(
            "<p:cNvPr id=\"2\" name=\"Probe\">",
            "<a:hlinkClick r:id=\"rId8\" action=\"ppaction://hlinksldjump\" tooltip=\"Go to slide 2\"/>",
            "<a:extLst>",
        )),
        "{out}"
    );
}

#[test]
fn shape_without_hyperlink_has_unchanged_cnvpr() {
    let slide: Slide =
        serde_json::from_value(json!({"objects":[{"key":"probe","id":2,"name":"Probe",
        "kind":{"type":"shape","properties":{"geometry":{"preset":"rect"}}}}]}))
        .unwrap();
    assert!(!xml(&slide).contains("hlinkClick"));
}

#[test]
fn video_picture_with_hyperlink_is_rejected_naming_key() {
    let slide: Slide =
        serde_json::from_value(json!({"objects":[{"key":"clip","id":2,"name":"Clip",
        "hyperlink":{"relationship":"rId7"},
        "kind":{"type":"picture","relationship":"rId2","properties":{},
            "media":{"video_relationship":"rId3","media_relationship":"rId4"}}}]}))
        .unwrap();
    let text = error(&slide);
    assert!(text.contains("clip"), "{text}");
    assert!(text.contains("hyperlink"), "{text}");
}

#[test]
fn object_hyperlink_requires_relationship_id() {
    let text = error(&shape_with_object_link(json!({"relationship":""})));
    assert!(text.contains("probe"), "{text}");
    assert!(
        text.contains("hyperlink requires relationship ID"),
        "{text}"
    );
}

#[test]
fn bullet_serde_uses_snake_case_tags_and_round_trips() {
    let variants = [
        json!({"type":"none"}),
        json!({"type":"char","char":"•","font":"Arial","color":{"type":"rgb","value":"112233"},"size_percent":50}),
        json!({"type":"auto_number","scheme":"alphaUcParenR","start_at":4}),
    ];
    for value in variants {
        let bullet: Bullet = serde_json::from_value(value.clone()).unwrap();
        let back = serde_json::to_value(&bullet).unwrap();
        let again: Bullet = serde_json::from_value(back.clone()).unwrap();
        assert_eq!(serde_json::to_value(&again).unwrap(), back);
        assert_eq!(back["type"], value["type"]);
    }
}

#[test]
fn hyperlink_and_paragraph_properties_round_trip_through_json() {
    let hyperlink = Hyperlink {
        relationship: "rId3".into(),
        action: Some("ppaction://hlinksldjump".into()),
        tooltip: None,
    };
    let value = serde_json::to_value(&hyperlink).unwrap();
    let back: Hyperlink = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(&back).unwrap(), value);

    let properties: ParagraphProperties = serde_json::from_value(json!({
        "level": 1,
        "margin_left": 457200,
        "indent": -228600,
        "bullet": {"type": "auto_number", "scheme": "arabicPeriod"}
    }))
    .unwrap();
    let value = serde_json::to_value(&properties).unwrap();
    let back: ParagraphProperties = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(&back).unwrap(), value);
    assert_eq!(back.level, Some(1));
    assert_eq!(back.indent.map(|e| e.0), Some(-228600));
}

#[test]
fn slide_with_links_and_bullets_compiles_identically_after_round_trip() {
    let slide: Slide = serde_json::from_value(json!({"objects":[{"key":"probe","id":2,
        "name":"Probe","hyperlink":{"relationship":"rId7"},
        "kind":{"type":"shape","properties":{"geometry":{"preset":"rect"}},
            "text":{"paragraphs":[{"properties":{"level":1,"bullet":{"type":"char","char":"*"}},
                "runs":[{"text":"a","hyperlink":{"relationship":"rId9"}}]}]}}}]}))
    .unwrap();
    let round: Slide = serde_json::from_value(serde_json::to_value(&slide).unwrap()).unwrap();
    assert_eq!(xml(&slide), xml(&round));
}

#[test]
fn unknown_fields_are_rejected() {
    assert!(serde_json::from_value::<Bullet>(json!({"type":"char","char":"x","bogus":1})).is_err());
    assert!(serde_json::from_value::<Bullet>(json!({"type":"dash"})).is_err());
    assert!(
        serde_json::from_value::<ParagraphProperties>(json!({"level":1,"bogus":true})).is_err()
    );
    assert!(serde_json::from_value::<Hyperlink>(json!({"relationship":"rId1","bogus":1})).is_err());
    assert!(
        serde_json::from_value::<TextRun>(json!({
            "text":"x","hyperlink":{"relationship":"rId1","extra":true}
        }))
        .is_err()
    );
    let nested = json!({"objects":[{"key":"probe","id":2,"name":"Probe",
        "hyperlink":{"relationship":"rId1"},"hyperlnk":{"relationship":"rId1"},
        "kind":{"type":"shape","properties":{}}}]});
    assert!(serde_json::from_value::<Slide>(nested).is_err());
}
