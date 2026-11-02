use deckwright::{
    Limits,
    builder::compile_slide,
    model::{
        Color, Fill, Line, Object, ObjectKind, Paragraph, Slide, TextBody, TextRun, Transform,
    },
    query::{Geometry, QuerySession},
    table::{CellBorders, Table, TableCell, TableRow},
    template,
    units::Emu,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

const STYLE: &str = "{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}";
const EMPTY_CELL: &str =
    "<a:tc><a:txBody><a:bodyPr/><a:lstStyle/><a:p/></a:txBody><a:tcPr/></a:tc>";

fn text(value: &str) -> TextBody {
    TextBody {
        paragraphs: vec![Paragraph {
            runs: vec![TextRun {
                text: value.into(),
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn empty_text() -> TextBody {
    TextBody {
        paragraphs: vec![Paragraph::default()],
        ..Default::default()
    }
}

fn cell(value: &str) -> TableCell {
    TableCell {
        text: text(value),
        ..Default::default()
    }
}

fn empty_cell() -> TableCell {
    TableCell {
        text: empty_text(),
        ..Default::default()
    }
}

fn row(height: i64, cells: Vec<TableCell>) -> TableRow {
    TableRow {
        height: Emu(height),
        cells,
    }
}

fn table(columns: &[i64], rows: Vec<TableRow>) -> Table {
    Table {
        columns: columns.iter().map(|w| Emu(*w)).collect(),
        rows,
        ..Default::default()
    }
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

fn slide_with(table: Table, transform: Transform) -> Slide {
    Slide {
        objects: vec![Object {
            key: "tbl".into(),
            id: 2,
            name: "Table 1".into(),
            morph_name: None,
            description: None,
            hyperlink: None,
            kind: ObjectKind::Table { transform, table },
        }],
        ..Default::default()
    }
}

fn slide_of(table: Table) -> Slide {
    slide_with(table, transform())
}

fn xml(slide: &Slide) -> String {
    String::from_utf8(compile_slide(slide).unwrap()).unwrap()
}

fn frame(xml: &str) -> &str {
    let start = xml.find("<p:graphicFrame>").expect("graphicFrame");
    let end = xml.find("</p:graphicFrame>").expect("graphicFrame end") + "</p:graphicFrame>".len();
    &xml[start..end]
}

fn rejected(table: Table, message: &str) {
    let error = compile_slide(&slide_of(table))
        .expect_err(message)
        .to_string();
    assert!(
        error.contains(message),
        "{error:?} does not contain {message:?}"
    );
    assert!(error.contains("tbl"), "error must name the key: {error:?}");
}

fn valid_two_by_two() -> Table {
    table(
        &[1000, 1000],
        vec![
            row(200, vec![cell("a"), cell("b")]),
            row(200, vec![cell("c"), cell("d")]),
        ],
    )
}

#[test]
fn three_by_two_graphic_frame_skeleton_is_exact() {
    let t = table(
        &[1200, 1800],
        (0..3)
            .map(|_| row(200, vec![empty_cell(), empty_cell()]))
            .collect(),
    );
    let out = xml(&slide_of(t));
    let row_xml = format!("<a:tr h=\"200\">{EMPTY_CELL}{EMPTY_CELL}</a:tr>");
    let expected = format!(
        concat!(
            "<p:graphicFrame><p:nvGraphicFramePr>",
            "<p:cNvPr id=\"2\" name=\"Table 1\"><a:extLst><a:ext uri=\"urn:deckwright:source:v1\">",
            "<dw:source xmlns:dw=\"urn:deckwright:source:v1\" key=\"tbl\"/></a:ext></a:extLst></p:cNvPr>",
            "<p:cNvGraphicFramePr><a:graphicFrameLocks noGrp=\"1\"/></p:cNvGraphicFramePr>",
            "<p:nvPr/></p:nvGraphicFramePr>",
            "<p:xfrm><a:off x=\"1000\" y=\"2000\"/><a:ext cx=\"3000\" cy=\"600\"/></p:xfrm>",
            "<a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/table\">",
            "<a:tbl><a:tblPr/>",
            "<a:tblGrid><a:gridCol w=\"1200\"/><a:gridCol w=\"1800\"/></a:tblGrid>",
            "{rows}",
            "</a:tbl></a:graphicData></a:graphic></p:graphicFrame>"
        ),
        rows = row_xml.repeat(3)
    );
    assert_eq!(frame(&out), expected);
}

#[test]
fn header_row_uses_first_row_and_style_id() {
    let mut t = valid_two_by_two();
    t.first_row = true;
    t.style_id = Some(STYLE.into());
    let out = xml(&slide_of(t));
    assert!(out.contains(&format!(
        "<a:tblPr firstRow=\"1\"><a:tableStyleId>{STYLE}</a:tableStyleId></a:tblPr>"
    )));
    let mut banded = valid_two_by_two();
    banded.band_row = true;
    assert!(xml(&slide_of(banded)).contains("<a:tblPr bandRow=\"1\"/>"));
}

#[test]
fn cell_fill_borders_margins_and_anchor_follow_schema_order() {
    let mut c = cell("x");
    c.fill = Some(Fill::Solid {
        color: Color::Rgb {
            value: "112233".into(),
            alpha: None,
        },
    });
    c.margin_left = Some(Emu(91440));
    c.margin_right = Some(Emu(91440));
    c.margin_top = Some(Emu(45720));
    c.margin_bottom = Some(Emu(45720));
    c.anchor = Some("ctr".into());
    let solid = |value: &str| Fill::Solid {
        color: Color::Rgb {
            value: value.into(),
            alpha: None,
        },
    };
    c.borders = Some(CellBorders {
        left: Some(Line {
            width: Some(Emu(12700)),
            fill: Some(solid("FF0000")),
            ..Default::default()
        }),
        bottom: Some(Line {
            width: Some(Emu(25400)),
            fill: Some(solid("0000FF")),
            ..Default::default()
        }),
        ..Default::default()
    });
    let out = xml(&slide_of(table(&[2000], vec![row(300, vec![c])])));
    let expected = concat!(
        "<a:tc><a:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t xml:space=\"preserve\">x</a:t></a:r></a:p></a:txBody>",
        "<a:tcPr marL=\"91440\" marR=\"91440\" marT=\"45720\" marB=\"45720\" anchor=\"ctr\">",
        "<a:lnL w=\"12700\"><a:solidFill><a:srgbClr val=\"FF0000\"/></a:solidFill></a:lnL>",
        "<a:lnB w=\"25400\"><a:solidFill><a:srgbClr val=\"0000FF\"/></a:solidFill></a:lnB>",
        "<a:solidFill><a:srgbClr val=\"112233\"/></a:solidFill>",
        "</a:tcPr></a:tc>"
    );
    assert!(out.contains(expected), "{out}");
}

#[test]
fn grid_span_with_hmerge_continuations_fills_the_header_row() {
    let mut header = cell("Merged header");
    header.grid_span = Some(3);
    let mut continuation = empty_cell();
    continuation.h_merge = true;
    let t = table(
        &[1000, 1000, 1000],
        vec![
            row(
                300,
                vec![header, continuation.clone(), continuation.clone()],
            ),
            row(300, vec![cell("a"), cell("b"), cell("c")]),
            row(300, vec![cell("d"), cell("e"), cell("f")]),
            row(300, vec![cell("g"), cell("h"), cell("i")]),
        ],
    );
    let out = xml(&slide_of(t));
    assert!(out.contains("<a:tc gridSpan=\"3\">"));
    assert_eq!(out.matches("<a:tc hMerge=\"1\">").count(), 2);
    // Every row keeps one a:tc per column, so 4 rows of 3 columns give 12 cells.
    assert_eq!(out.matches("</a:tc>").count(), 12);
}

#[test]
fn row_span_with_vmerge_continuation() {
    let mut top = cell("tall");
    top.row_span = Some(2);
    let mut below = empty_cell();
    below.v_merge = true;
    let t = table(
        &[1000, 1000],
        vec![
            row(200, vec![top, cell("b")]),
            row(200, vec![below, cell("d")]),
        ],
    );
    let out = xml(&slide_of(t));
    assert!(out.contains("<a:tc rowSpan=\"2\">"));
    assert!(out.contains("<a:tc vMerge=\"1\">"));
}

#[test]
fn two_way_span_flags_interior_cells_with_both_merges() {
    let mut origin = cell("block");
    origin.grid_span = Some(2);
    origin.row_span = Some(2);
    let h = TableCell {
        h_merge: true,
        ..empty_cell()
    };
    let v = TableCell {
        v_merge: true,
        ..empty_cell()
    };
    let hv = TableCell {
        h_merge: true,
        v_merge: true,
        ..empty_cell()
    };
    let t = table(
        &[1000, 1000, 1000],
        vec![
            row(200, vec![origin, h, cell("c")]),
            row(200, vec![v, hv, cell("f")]),
            row(200, vec![cell("g"), cell("h"), cell("i")]),
        ],
    );
    let out = xml(&slide_of(t));
    assert!(out.contains("<a:tc gridSpan=\"2\" rowSpan=\"2\">"));
    assert!(out.contains("<a:tc hMerge=\"1\">"));
    assert!(out.contains("<a:tc vMerge=\"1\">"));
    assert!(out.contains("<a:tc hMerge=\"1\" vMerge=\"1\">"));
}

#[test]
fn validation_rejects_bad_grids_spans_and_values() {
    let mut no_columns = valid_two_by_two();
    no_columns.columns.clear();
    rejected(no_columns, "at least one column");

    let mut zero_width = valid_two_by_two();
    zero_width.columns[1] = Emu(0);
    rejected(zero_width, "column width outside");

    let mut no_rows = valid_two_by_two();
    no_rows.rows.clear();
    rejected(no_rows, "at least one row");

    let mut zero_height = valid_two_by_two();
    zero_height.rows[0].height = Emu(0);
    rejected(zero_height, "row height outside");

    let mut short = valid_two_by_two();
    short.rows[1].cells.pop();
    rejected(short, "row has 1 cells; table has 2 columns");

    let mut too_wide = valid_two_by_two();
    too_wide.rows[0].cells[0].grid_span = Some(3);
    rejected(too_wide, "gridSpan exceeds the table columns");

    let mut too_tall = valid_two_by_two();
    too_tall.rows[0].cells[0].row_span = Some(3);
    rejected(too_tall, "rowSpan exceeds the table rows");

    let mut trivial = valid_two_by_two();
    trivial.rows[0].cells[0].grid_span = Some(1);
    rejected(trivial, "must be at least 2");

    let mut orphan = valid_two_by_two();
    orphan.rows[0].cells[1].h_merge = true;
    rejected(orphan, "disagree with the span");

    let mut missing = valid_two_by_two();
    missing.rows[0].cells[0].grid_span = Some(2);
    rejected(missing, "disagree with the span");

    let mut wrong_direction = valid_two_by_two();
    wrong_direction.rows[0].cells[0].row_span = Some(2);
    wrong_direction.rows[1].cells[0].v_merge = true;
    wrong_direction.rows[1].cells[1].h_merge = true;
    rejected(wrong_direction, "disagree with the span");

    let mut overlap = table(
        &[1000, 1000, 1000],
        vec![row(200, vec![empty_cell(), empty_cell(), empty_cell()])],
    );
    overlap.rows[0].cells[0].grid_span = Some(2);
    overlap.rows[0].cells[1].grid_span = Some(2);
    rejected(overlap, "merged cell spans overlap");

    let mut origin_flagged = valid_two_by_two();
    origin_flagged.rows[0].cells[0].grid_span = Some(2);
    origin_flagged.rows[0].cells[0].h_merge = true;
    origin_flagged.rows[0].cells[1].h_merge = true;
    rejected(origin_flagged, "cannot also be a merge continuation");

    let mut anchor = valid_two_by_two();
    anchor.rows[0].cells[0].anchor = Some("banana".into());
    rejected(anchor, "cell anchor");

    let mut style = valid_two_by_two();
    style.style_id = Some("{abc}".into());
    rejected(style, "braced uppercase GUID");

    let mut margin = valid_two_by_two();
    margin.rows[0].cells[0].margin_left = Some(Emu(-1));
    rejected(margin, "cell margin outside");

    let mut no_paragraphs = valid_two_by_two();
    no_paragraphs.rows[0].cells[0].text.paragraphs.clear();
    rejected(no_paragraphs, "text body requires at least one paragraph");

    let mut border = valid_two_by_two();
    border.rows[0].cells[0].borders = Some(CellBorders {
        top: Some(Line {
            width: Some(Emu(-1)),
            ..Default::default()
        }),
        ..Default::default()
    });
    rejected(border, "line width outside");

    let rotated = Transform {
        rotation: Some(90),
        ..transform()
    };
    let error = compile_slide(&slide_with(valid_two_by_two(), rotated))
        .expect_err("rotation")
        .to_string();
    assert!(
        error.contains("supports only x, y, width and height"),
        "{error}"
    );

    let flat = Transform {
        height: Emu(0),
        ..transform()
    };
    let error = compile_slide(&slide_with(valid_two_by_two(), flat))
        .expect_err("extent")
        .to_string();
    assert!(error.contains("frame extent must be nonzero"), "{error}");

    assert!(compile_slide(&slide_of(valid_two_by_two())).is_ok());
}

#[test]
fn query_session_reports_table_geometry_and_text() {
    let mut t = valid_two_by_two();
    t.rows[0].cells[0] = cell("A1");
    let base = template::blank(1, Emu(12192000), Emu(6858000)).unwrap();
    let bytes = compile_slide(&slide_of(t)).unwrap();
    let package = base
        .edited(
            BTreeMap::from([("ppt/slides/slide1.xml".to_string(), bytes)]),
            &BTreeSet::new(),
            Limits::default(),
        )
        .unwrap();
    let session = QuerySession::from_package(&package, Limits::default()).unwrap();
    let state = session.state(1, &[], 0).unwrap();
    let shape = state.shapes.iter().find(|s| s.id == 2).expect("table");
    assert_eq!(shape.name, "Table 1");
    assert_eq!(
        shape.geometry,
        Some(Geometry {
            x: 1000.0,
            y: 2000.0,
            width: 3000.0,
            height: 600.0,
        })
    );
    assert!(shape.text.contains("A1"));
    assert!(shape.warnings.is_empty(), "{:?}", shape.warnings);
}

fn table_json() -> Value {
    let c = json!({
        "text": {"paragraphs": [{"runs": [{"text": "A1"}]}]},
        "fill": {"type": "solid", "color": {"type": "rgb", "value": "112233"}},
        "grid_span": 2,
        "anchor": "ctr",
        "margin_left": 91440,
        "borders": {"bottom": {"width": 12700}}
    });
    json!({
        "objects": [{
            "key": "tbl",
            "id": 2,
            "name": "Table 1",
            "kind": {
                "type": "table",
                "transform": {"x": 1000, "y": 2000, "width": 3000, "height": 600},
                "table": {
                    "columns": [1500, 1500],
                    "first_row": true,
                    "style_id": STYLE,
                    "rows": [
                        {"height": 300, "cells": [c, {"text": {"paragraphs": [{}]}, "h_merge": true}]},
                        {"height": 300, "cells": [
                            {"text": {"paragraphs": [{}]}},
                            {"text": {"paragraphs": [{}]}}
                        ]}
                    ]
                }
            }
        }]
    })
}

#[test]
fn serde_round_trip_is_stable_and_minimal_tables_parse() {
    let value = table_json();
    let slide: Slide = serde_json::from_value(value.clone()).unwrap();
    let back = serde_json::to_value(&slide).unwrap();
    let again: Slide = serde_json::from_value(back.clone()).unwrap();
    assert_eq!(serde_json::to_value(&again).unwrap(), back);
    assert_eq!(xml(&slide), xml(&again));
    // Optional fields may be absent; only columns, rows and cell text are required.
    let minimal: Slide = serde_json::from_value(json!({
        "objects": [{"key": "t", "id": 2, "name": "T", "kind": {
            "type": "table",
            "transform": {"x": 0, "y": 0, "width": 10, "height": 10},
            "table": {"columns": [10], "rows": [{"height": 10, "cells": [
                {"text": {"paragraphs": [{}]}}
            ]}]}
        }}]
    }))
    .unwrap();
    assert!(compile_slide(&minimal).is_ok());
}

#[test]
fn serde_rejects_unknown_fields_at_every_table_level() {
    let base = table_json();
    let pointers = [
        "/objects/0/kind",
        "/objects/0/kind/transform",
        "/objects/0/kind/table",
        "/objects/0/kind/table/rows/0",
        "/objects/0/kind/table/rows/0/cells/0",
        "/objects/0/kind/table/rows/0/cells/0/borders",
        "/objects/0/kind/table/rows/0/cells/0/text",
    ];
    for pointer in pointers {
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
