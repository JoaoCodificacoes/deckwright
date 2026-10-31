use deckwright::{
    builder::compile_slide,
    model::{Line, LineEnd, Slide},
};
use serde_json::{Value, json};

fn shape_with_line(line: Value) -> Slide {
    serde_json::from_value(
        json!({"objects":[{"key":"probe","id":2,"name":"Probe","kind":{
            "type":"shape",
            "properties":{"geometry":{"preset":"line"},"line":line}
        }}]}),
    )
    .unwrap()
}

fn shape_with_text(paragraphs: Value) -> Slide {
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

#[test]
fn line_dash_only_emits_prst_dash_child() {
    let out = xml(&shape_with_line(json!({"dash":"sysDot"})));
    assert!(
        out.contains("<a:ln><a:prstDash val=\"sysDot\"/></a:ln>"),
        "{out}"
    );
}

#[test]
fn line_tail_end_only_emits_tail_end_with_attributes() {
    let out = xml(&shape_with_line(json!({
        "tail_end":{"kind":"triangle","width":"med","length":"lg"}
    })));
    assert!(
        out.contains("<a:ln><a:tailEnd type=\"triangle\" w=\"med\" len=\"lg\"/></a:ln>"),
        "{out}"
    );
}

#[test]
fn line_dash_head_and_tail_follow_schema_order() {
    let out = xml(&shape_with_line(json!({
        "width":12700,
        "fill":{"type":"solid","color":{"type":"rgb","value":"112233"}},
        "dash":"dash",
        "head_end":{"kind":"oval","width":"sm","length":"sm"},
        "tail_end":{"kind":"arrow","width":"lg","length":"lg"}
    })));
    assert!(
        out.contains(concat!(
            "<a:ln w=\"12700\"><a:solidFill><a:srgbClr val=\"112233\"/></a:solidFill>",
            "<a:prstDash val=\"dash\"/>",
            "<a:headEnd type=\"oval\" w=\"sm\" len=\"sm\"/>",
            "<a:tailEnd type=\"arrow\" w=\"lg\" len=\"lg\"/></a:ln>",
        )),
        "{out}"
    );
}

#[test]
fn line_end_omits_absent_width_and_length() {
    let out = xml(&shape_with_line(json!({"head_end":{"kind":"stealth"}})));
    assert!(
        out.contains("<a:ln><a:headEnd type=\"stealth\"/></a:ln>"),
        "{out}"
    );
}

#[test]
fn invalid_line_values_are_rejected_with_object_key() {
    let cases = [
        (json!({"dash":"wiggle"}), "line dash"),
        (json!({"tail_end":{"kind":"banana"}}), "line end kind"),
        (
            json!({"head_end":{"kind":"oval","width":"huge"}}),
            "line end width",
        ),
        (
            json!({"tail_end":{"kind":"oval","length":"tiny"}}),
            "line end length",
        ),
    ];
    for (line, field) in cases {
        let message = error(&shape_with_line(line));
        assert!(message.contains("probe"), "{message}");
        assert!(message.contains(field), "{message}");
    }
}

#[test]
fn paragraph_end_style_follows_runs_with_rpr_form() {
    let style = json!({
        "size":2400,"bold":true,
        "color":{"type":"rgb","value":"112233"},
        "font":"Calibri"
    });
    let out = xml(&shape_with_text(json!([{
        "runs":[{"text":"A","style":style}],
        "end_style":style
    }])));
    let form =
        "<a:solidFill><a:srgbClr val=\"112233\"/></a:solidFill><a:latin typeface=\"Calibri\"/>";
    assert!(
        out.contains(&format!(
            "<a:r><a:rPr sz=\"2400\" b=\"1\">{form}</a:rPr><a:t xml:space=\"preserve\">A</a:t></a:r>\
             <a:endParaRPr sz=\"2400\" b=\"1\">{form}</a:endParaRPr></a:p>"
        )),
        "{out}"
    );
}

#[test]
fn empty_paragraph_end_style_emits_only_when_present() {
    let with = xml(&shape_with_text(json!([{"runs":[],"end_style":{}}])));
    assert!(with.contains("<a:p><a:endParaRPr/></a:p>"), "{with}");

    let without = xml(&shape_with_text(json!([{"runs":[]}])));
    assert!(!without.contains("endParaRPr"), "{without}");
    assert!(without.contains("<a:p/>"), "{without}");
}

#[test]
fn underline_emits_u_attribute_and_rejects_unknown_values() {
    let out = xml(&shape_with_text(json!([{
        "runs":[{"text":"U","style":{"underline":"sng"}}]
    }])));
    assert!(out.contains("<a:rPr u=\"sng\"/>"), "{out}");

    let bad = shape_with_text(json!([{
        "runs":[{"text":"U","style":{"underline":"zigzag"}}]
    }]));
    let message = error(&bad);
    assert!(message.contains("probe"), "{message}");
    assert!(message.contains("underline"), "{message}");

    let bad_end = shape_with_text(json!([{
        "runs":[],"end_style":{"underline":"zigzag"}
    }]));
    assert!(error(&bad_end).contains("underline"));
}

#[test]
fn serde_round_trips_new_field_names() {
    let input = json!({
        "dash":"lgDashDot",
        "head_end":{"kind":"diamond","width":"med","length":"sm"},
        "tail_end":{"kind":"arrow","width":"lg","length":"lg"}
    });
    let line: Line = serde_json::from_value(input.clone()).unwrap();
    let out = serde_json::to_value(&line).unwrap();
    assert_eq!(out["dash"], input["dash"]);
    assert_eq!(out["head_end"], input["head_end"]);
    assert_eq!(out["tail_end"], input["tail_end"]);

    let again: Line = serde_json::from_value(out).unwrap();
    assert_eq!(again.dash.as_deref(), Some("lgDashDot"));
    let head: LineEnd = again.head_end.unwrap();
    assert_eq!(head.kind, "diamond");
    assert_eq!(head.width.as_deref(), Some("med"));
    assert_eq!(head.length.as_deref(), Some("sm"));

    let paragraph: deckwright::model::Paragraph = serde_json::from_value(json!({
        "runs":[],
        "end_style":{"underline":"dbl","size":1800}
    }))
    .unwrap();
    let end = serde_json::to_value(&paragraph).unwrap();
    assert_eq!(end["end_style"]["underline"], json!("dbl"));
    assert_eq!(end["end_style"]["size"], json!(1800));
}

#[test]
fn unknown_fields_are_rejected_in_new_structs() {
    assert!(serde_json::from_value::<LineEnd>(json!({"kind":"oval","colour":"red"})).is_err());
    assert!(serde_json::from_value::<Line>(json!({"dash":"dash","arrow":true})).is_err());
    assert!(
        serde_json::from_value::<deckwright::model::RunStyle>(json!({"underlined":true})).is_err()
    );
}
