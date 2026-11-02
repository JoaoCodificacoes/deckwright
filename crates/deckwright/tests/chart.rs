use deckwright::{
    Limits, Package,
    builder::compile_slide,
    chart::{Chart, ChartKind, Series},
    check::check_package,
    deck::Deck,
    model::{Color, Object, ObjectKind, Slide, Transform},
    query::{Geometry, QuerySession},
    template,
    units::Emu,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read},
};

const W: i64 = 12_192_000;
const H: i64 = 6_858_000;
const CHART_CT: &str = "application/vnd.openxmlformats-officedocument.drawingml.chart+xml";
const XLSX_CT: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
const CHART_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart";
const PACKAGE_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/package";
const CHART_PART: &str = "ppt/charts/chart1.xml";
const WORKBOOK_PART: &str = "ppt/embeddings/Microsoft_Excel_Worksheet1.xlsx";

fn deck() -> Deck {
    Deck::from_package(template::blank(1, Emu(W), Emu(H)).unwrap())
}

fn series(name: &str, values: &[f64]) -> Series {
    Series {
        name: name.into(),
        values: values.to_vec(),
        x_values: None,
        color: None,
    }
}

fn scatter_series(name: &str, xs: &[f64], ys: &[f64]) -> Series {
    Series {
        name: name.into(),
        values: ys.to_vec(),
        x_values: Some(xs.to_vec()),
        color: None,
    }
}

fn categories() -> Vec<String> {
    vec!["Q1".into(), "Q2".into(), "Q3".into()]
}

/// Clustered column: 3 categories by 2 series, title, legend on the right, data labels.
fn column_chart() -> Chart {
    Chart {
        kind: ChartKind::Column,
        title: Some("Sales".into()),
        categories: categories(),
        series: vec![
            series("North", &[1.0, 2.5, 3.0]),
            series("South", &[4.0, 5.0, 6.0]),
        ],
        legend: Some("r".into()),
        data_labels: true,
        stacked: false,
    }
}

fn stacked_bar_chart() -> Chart {
    Chart {
        kind: ChartKind::Bar,
        title: None,
        categories: categories(),
        series: vec![
            series("North", &[1.0, 2.0, 3.0]),
            series("South", &[4.0, 5.0, 6.0]),
        ],
        legend: None,
        data_labels: false,
        stacked: true,
    }
}

fn line_chart() -> Chart {
    Chart {
        kind: ChartKind::Line,
        title: None,
        categories: categories(),
        series: vec![series("Trend", &[1.0, 3.0, 2.0])],
        legend: None,
        data_labels: false,
        stacked: false,
    }
}

fn pie_chart() -> Chart {
    Chart {
        kind: ChartKind::Pie,
        title: None,
        categories: categories(),
        series: vec![series("Share", &[1.0, 2.0, 3.0])],
        legend: None,
        data_labels: true,
        stacked: false,
    }
}

fn scatter_chart() -> Chart {
    Chart {
        kind: ChartKind::Scatter,
        title: None,
        categories: vec![],
        series: vec![
            scatter_series("A", &[1.0, 2.0, 3.0], &[10.0, 20.0, 15.0]),
            scatter_series("B", &[0.5, 1.5], &[7.0, 8.5]),
        ],
        legend: None,
        data_labels: false,
        stacked: false,
    }
}

fn part_text(package: &Package, part: &str) -> String {
    String::from_utf8(package.part(part).expect(part).to_vec()).unwrap()
}

/// Adds `chart` to a fresh slide and returns the package holding its parts.
fn package_with(chart: &Chart) -> Package {
    let mut d = deck();
    let slide = d.slides()[0].clone();
    d.add_chart(&slide, chart).unwrap();
    d.into_package()
}

fn chart_xml(chart: &Chart) -> String {
    part_text(&package_with(chart), CHART_PART)
}

/// Asserts that every needle occurs, in the order given.
fn in_order(xml: &str, needles: &[&str]) {
    let mut at = 0;
    for needle in needles {
        match xml[at..].find(needle) {
            Some(found) => at += found + needle.len(),
            None => panic!("{needle} missing, or out of order, after byte {at}:\n{xml}"),
        }
    }
}

fn between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
    let from = text.find(start).expect(start) + start.len();
    let to = text[from..].find(end).expect(end) + from;
    &text[from..to]
}

/// Each `<c:{tag}>` block that holds a reference: (formula, ptCount, cached values).
fn refs(xml: &str, tag: &str) -> Vec<(String, usize, Vec<String>)> {
    let open = format!("<c:{tag}>");
    let close = format!("</c:{tag}>");
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find(&open) {
        let body = start + open.len();
        let end = rest[body..].find(&close).expect("closing tag") + body;
        let block = &rest[body..end];
        if block.contains("<c:f>") {
            let formula = between(block, "<c:f>", "</c:f>").to_owned();
            let count = between(block, "<c:ptCount val=\"", "\"/>").parse().unwrap();
            let values = block
                .split("<c:v>")
                .skip(1)
                .map(|s| s.split("</c:v>").next().unwrap().to_owned())
                .collect();
            out.push((formula, count, values));
        }
        rest = &rest[end..];
    }
    out
}

/// Parses the first worksheet's cells into reference -> text (strings or numbers).
fn sheet_cells(sheet: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for chunk in sheet.split("<c r=\"").skip(1) {
        let reference = chunk.split('"').next().unwrap().to_owned();
        let value = if chunk.contains("<t xml:space=\"preserve\">") {
            between(chunk, "<t xml:space=\"preserve\">", "</t>")
        } else {
            between(chunk, "<v>", "</v>")
        };
        out.insert(reference, value.to_owned());
    }
    out
}

/// Expands `Sheet1!$B$2:$B$4` into the cell references it names (single row or column).
fn expand(formula: &str) -> Vec<String> {
    let range = formula
        .strip_prefix("Sheet1!")
        .expect("sheet-qualified formula");
    let cleaned = range.replace('$', "");
    let (first, last) = match cleaned.split_once(':') {
        Some(pair) => pair,
        None => (cleaned.as_str(), cleaned.as_str()),
    };
    let split = |cell: &str| {
        let at = cell.find(|c: char| c.is_ascii_digit()).unwrap();
        (cell[..at].to_owned(), cell[at..].parse::<usize>().unwrap())
    };
    let (col, row0) = split(first);
    let (col1, row1) = split(last);
    assert_eq!(col, col1, "test ranges are single-column");
    (row0..=row1).map(|row| format!("{col}{row}")).collect()
}

fn workbook_of(package: &Package) -> Vec<u8> {
    package.part(WORKBOOK_PART).expect("workbook part").to_vec()
}

fn sheet_of(package: &Package) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(workbook_of(package))).unwrap();
    let mut sheet = String::new();
    archive
        .by_name("xl/worksheets/sheet1.xml")
        .unwrap()
        .read_to_string(&mut sheet)
        .unwrap();
    sheet
}

fn invalid(chart: Chart, message: &str) {
    let mut d = deck();
    let slide = d.slides()[0].clone();
    let error = d.add_chart(&slide, &chart).expect_err(message).to_string();
    assert!(
        error.contains(message),
        "{error:?} does not contain {message:?}"
    );
    assert!(
        error.contains(&slide),
        "error must name the slide part: {error:?}"
    );
}

// --- XML structure, in schema order ---

#[test]
fn column_chart_elements_follow_schema_order() {
    let xml = chart_xml(&column_chart());
    assert!(xml.starts_with("<?xml version=\"1.0\""));
    in_order(
        &xml,
        &[
            "<c:chartSpace",
            "<c:roundedCorners val=\"0\"/>",
            "<c:chart>",
            "<c:title>",
            "<a:t>Sales</a:t>",
            "<c:autoTitleDeleted val=\"0\"/>",
            "<c:plotArea>",
            "<c:barChart>",
            "<c:barDir val=\"col\"/>",
            "<c:grouping val=\"clustered\"/>",
            "<c:varyColors val=\"0\"/>",
            "<c:ser>",
            "</c:ser>",
            "<c:dLbls>",
            "<c:showLegendKey val=\"0\"/>",
            "<c:showVal val=\"1\"/>",
            "<c:showCatName val=\"0\"/>",
            "<c:gapWidth val=\"150\"/>",
            "<c:axId val=\"500000001\"/>",
            "<c:axId val=\"500000002\"/>",
            "</c:barChart>",
            "<c:catAx>",
            "<c:axPos val=\"b\"/>",
            "<c:valAx>",
            "<c:axPos val=\"l\"/>",
            "</c:plotArea>",
            "<c:legend>",
            "<c:legendPos val=\"r\"/>",
            "<c:overlay val=\"0\"/>",
            "</c:legend>",
            "<c:plotVisOnly val=\"1\"/>",
            "<c:dispBlanksAs val=\"gap\"/>",
            "</c:chart>",
            "<c:externalData r:id=\"rId1\"><c:autoUpdate val=\"0\"/></c:externalData>",
            "</c:chartSpace>",
        ],
    );
    assert!(
        !xml.contains("<c:overlap"),
        "clustered charts carry no overlap"
    );
    assert_eq!(xml.matches("<c:ser>").count(), 2);
}

#[test]
fn stacked_bar_uses_stacked_grouping_overlap_and_horizontal_axes() {
    let xml = chart_xml(&stacked_bar_chart());
    in_order(
        &xml,
        &[
            "<c:barChart>",
            "<c:barDir val=\"bar\"/>",
            "<c:grouping val=\"stacked\"/>",
            "<c:varyColors val=\"0\"/>",
            "<c:ser>",
            "</c:ser>",
            "<c:gapWidth val=\"150\"/>",
            "<c:overlap val=\"100\"/>",
            "<c:axId val=\"500000001\"/>",
            "<c:axId val=\"500000002\"/>",
            "</c:barChart>",
            "<c:catAx>",
            "<c:axPos val=\"l\"/>",
            "<c:valAx>",
            "<c:axPos val=\"b\"/>",
            "</c:plotArea>",
        ],
    );
    assert!(!xml.contains("<c:title>"));
    assert!(xml.contains("<c:autoTitleDeleted val=\"1\"/>"));
    assert!(!xml.contains("<c:legend>"));
    assert!(!xml.contains("<c:dLbls>"));
}

#[test]
fn line_chart_has_markers_and_two_axes() {
    let xml = chart_xml(&line_chart());
    in_order(
        &xml,
        &[
            "<c:lineChart>",
            "<c:grouping val=\"standard\"/>",
            "<c:varyColors val=\"0\"/>",
            "<c:ser>",
            "<c:marker>",
            "<c:symbol val=\"circle\"/>",
            "</c:marker>",
            "<c:cat>",
            "<c:val>",
            "<c:smooth val=\"0\"/>",
            "</c:ser>",
            "<c:marker val=\"1\"/>",
            "<c:axId val=\"500000001\"/>",
            "<c:axId val=\"500000002\"/>",
            "</c:lineChart>",
            "<c:catAx>",
            "<c:valAx>",
        ],
    );
    assert!(!xml.contains("<c:barChart>"));
}

#[test]
fn pie_chart_has_one_series_and_no_axes() {
    let xml = chart_xml(&pie_chart());
    in_order(
        &xml,
        &[
            "<c:pieChart>",
            "<c:varyColors val=\"1\"/>",
            "<c:ser>",
            "<c:cat>",
            "<c:val>",
            "</c:ser>",
            "<c:dLbls>",
            "<c:showVal val=\"1\"/>",
            "<c:firstSliceAng val=\"0\"/>",
            "</c:pieChart>",
            "</c:plotArea>",
        ],
    );
    assert_eq!(xml.matches("<c:ser>").count(), 1);
    assert!(!xml.contains("axId"));
    assert!(!xml.contains("<c:catAx>"));
    assert!(!xml.contains("<c:valAx>"));
}

#[test]
fn scatter_chart_uses_xy_values_and_two_value_axes() {
    let xml = chart_xml(&scatter_chart());
    in_order(
        &xml,
        &[
            "<c:scatterChart>",
            "<c:scatterStyle val=\"lineMarker\"/>",
            "<c:varyColors val=\"0\"/>",
            "<c:ser>",
            "<c:xVal>",
            "<c:yVal>",
            "<c:smooth val=\"0\"/>",
            "</c:ser>",
            "<c:ser>",
            "<c:xVal>",
            "<c:yVal>",
            "</c:ser>",
            "<c:axId val=\"500000001\"/>",
            "<c:axId val=\"500000002\"/>",
            "</c:scatterChart>",
            "<c:valAx>",
            "<c:axPos val=\"b\"/>",
            "<c:crossBetween val=\"midCat\"/>",
            "<c:valAx>",
            "<c:axPos val=\"l\"/>",
            "</c:plotArea>",
        ],
    );
    assert!(!xml.contains("<c:catAx>"));
    assert!(!xml.contains("<c:cat>"));
}

#[test]
fn series_colors_use_fill_for_bars_and_line_for_lines() {
    let mut chart = column_chart();
    chart.series[0].color = Some(Color::Rgb {
        value: "C0FFEE".into(),
        alpha: None,
    });
    let xml = chart_xml(&chart);
    assert!(
        xml.contains("<c:spPr><a:solidFill><a:srgbClr val=\"C0FFEE\"/></a:solidFill></c:spPr>")
    );

    let mut line = line_chart();
    line.series[0].color = Some(Color::Scheme {
        value: "accent1".into(),
        alpha: None,
    });
    let xml = chart_xml(&line);
    assert!(xml.contains(
        "<c:spPr><a:ln w=\"28575\"><a:solidFill><a:schemeClr val=\"accent1\"/></a:solidFill></a:ln></c:spPr>"
    ));
    assert!(xml.contains("<c:marker><c:symbol val=\"circle\"/><c:size val=\"7\"/><c:spPr>"));
}

#[test]
fn title_text_is_escaped() {
    let mut chart = column_chart();
    chart.title = Some("Q&A <draft>".into());
    let xml = chart_xml(&chart);
    assert!(xml.contains("<a:t>Q&amp;A &lt;draft&gt;</a:t>"));
}

// --- series caches and formulas ---

#[test]
fn column_caches_carry_point_counts_and_values() {
    let xml = chart_xml(&column_chart());
    assert_eq!(
        refs(&xml, "tx"),
        vec![
            ("Sheet1!$B$1".into(), 1, vec!["North".into()]),
            ("Sheet1!$C$1".into(), 1, vec!["South".into()]),
        ]
    );
    let cats = vec!["Q1".to_string(), "Q2".into(), "Q3".into()];
    assert_eq!(
        refs(&xml, "cat"),
        vec![
            ("Sheet1!$A$2:$A$4".into(), 3, cats.clone()),
            ("Sheet1!$A$2:$A$4".into(), 3, cats),
        ]
    );
    assert_eq!(
        refs(&xml, "val"),
        vec![
            (
                "Sheet1!$B$2:$B$4".into(),
                3,
                vec!["1".into(), "2.5".into(), "3".into()]
            ),
            (
                "Sheet1!$C$2:$C$4".into(),
                3,
                vec!["4".into(), "5".into(), "6".into()]
            ),
        ]
    );
    assert!(xml.contains("<c:formatCode>General</c:formatCode>"));
}

#[test]
fn scatter_caches_name_each_series_column_pair() {
    let xml = chart_xml(&scatter_chart());
    assert_eq!(
        refs(&xml, "xVal"),
        vec![
            (
                "Sheet1!$A$2:$A$4".into(),
                3,
                vec!["1".into(), "2".into(), "3".into()]
            ),
            (
                "Sheet1!$C$2:$C$3".into(),
                2,
                vec!["0.5".into(), "1.5".into()]
            ),
        ]
    );
    assert_eq!(
        refs(&xml, "yVal"),
        vec![
            (
                "Sheet1!$B$2:$B$4".into(),
                3,
                vec!["10".into(), "20".into(), "15".into()]
            ),
            ("Sheet1!$D$2:$D$3".into(), 2, vec!["7".into(), "8.5".into()]),
        ]
    );
    assert_eq!(
        refs(&xml, "tx"),
        vec![
            ("Sheet1!$B$1".into(), 1, vec!["A".into()]),
            ("Sheet1!$D$1".into(), 1, vec!["B".into()]),
        ]
    );
}

#[test]
fn every_formula_matches_the_embedded_sheet_cell_for_cell() {
    for chart in [
        column_chart(),
        stacked_bar_chart(),
        line_chart(),
        pie_chart(),
        scatter_chart(),
    ] {
        let package = package_with(&chart);
        let xml = part_text(&package, CHART_PART);
        let cells = sheet_cells(&sheet_of(&package));
        let mut checked = 0;
        for tag in ["tx", "cat", "val", "xVal", "yVal"] {
            for (formula, count, cached) in refs(&xml, tag) {
                let expected = expand(&formula);
                assert_eq!(expected.len(), count, "{formula} point count");
                assert_eq!(cached.len(), count, "{formula} cache length");
                for (cell, value) in expected.iter().zip(&cached) {
                    assert_eq!(
                        cells.get(cell),
                        Some(value),
                        "{formula}: cell {cell} disagrees with the cache"
                    );
                }
                checked += 1;
            }
        }
        assert!(checked > 0, "no references checked for {:?}", chart.kind);
    }
}

// --- embedded workbook ---

#[test]
fn workbook_opens_as_zip_with_five_parts_and_the_right_cells() {
    let package = package_with(&column_chart());
    let mut archive = zip::ZipArchive::new(Cursor::new(workbook_of(&package))).unwrap();
    let names: BTreeSet<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_owned())
        .collect();
    let expected: BTreeSet<String> = [
        "[Content_Types].xml",
        "_rels/.rels",
        "xl/workbook.xml",
        "xl/_rels/workbook.xml.rels",
        "xl/worksheets/sheet1.xml",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(names, expected);

    let workbook = {
        let mut s = String::new();
        archive
            .by_name("xl/workbook.xml")
            .unwrap()
            .read_to_string(&mut s)
            .unwrap();
        s
    };
    assert!(workbook.contains("<sheet name=\"Sheet1\" sheetId=\"1\" r:id=\"rId1\"/>"));

    let cells = sheet_cells(&sheet_of(&package));
    assert_eq!(cells.get("B1").map(String::as_str), Some("North"));
    assert_eq!(cells.get("C1").map(String::as_str), Some("South"));
    assert_eq!(cells.get("A2").map(String::as_str), Some("Q1"));
    assert_eq!(cells.get("A4").map(String::as_str), Some("Q3"));
    assert_eq!(cells.get("B2").map(String::as_str), Some("1"));
    assert_eq!(cells.get("B3").map(String::as_str), Some("2.5"));
    assert_eq!(cells.get("C4").map(String::as_str), Some("6"));
    assert!(
        !cells.contains_key("A1"),
        "A1 stays empty for category charts"
    );
}

#[test]
fn scatter_workbook_places_x_and_y_in_column_pairs() {
    let package = package_with(&scatter_chart());
    let cells = sheet_cells(&sheet_of(&package));
    assert_eq!(cells.get("A1").map(String::as_str), Some("A X"));
    assert_eq!(cells.get("B1").map(String::as_str), Some("A"));
    assert_eq!(cells.get("C3").map(String::as_str), Some("1.5"));
    assert_eq!(cells.get("D3").map(String::as_str), Some("8.5"));
    assert!(!cells.contains_key("C4"), "series B has two points");
}

// --- validation ---

#[test]
fn validation_rejects_each_invalid_chart() {
    let base = column_chart;

    invalid(
        Chart {
            series: vec![],
            ..base()
        },
        "at least one series",
    );
    invalid(
        Chart {
            series: vec![series("N", &[1.0, f64::NAN, 3.0])],
            ..base()
        },
        "non-finite",
    );
    invalid(
        Chart {
            series: vec![series("N", &[1.0, f64::INFINITY, 3.0])],
            ..base()
        },
        "non-finite",
    );
    invalid(
        Chart {
            series: vec![series("N", &[1.0, 2.0])],
            ..base()
        },
        "series N has 2 values for 3 categories",
    );
    invalid(
        Chart {
            categories: vec![],
            series: vec![series("N", &[])],
            ..base()
        },
        "category charts require at least one category",
    );
    invalid(
        Chart {
            series: vec![Series {
                x_values: Some(vec![1.0, 2.0, 3.0]),
                ..series("N", &[1.0, 2.0, 3.0])
            }],
            ..base()
        },
        "only scatter charts take them",
    );
    invalid(
        Chart {
            kind: ChartKind::Scatter,
            categories: vec![],
            series: vec![series("N", &[1.0, 2.0])],
            ..base()
        },
        "scatter series N requires x_values",
    );
    invalid(
        Chart {
            kind: ChartKind::Scatter,
            categories: vec![],
            series: vec![scatter_series("N", &[1.0], &[1.0, 2.0])],
            ..base()
        },
        "scatter series N has 1 x values for 2 values",
    );
    invalid(
        Chart {
            kind: ChartKind::Scatter,
            series: vec![scatter_series("N", &[1.0, 2.0, 3.0], &[1.0, 2.0, 3.0])],
            ..base()
        },
        "scatter charts take no categories",
    );
    invalid(
        Chart {
            kind: ChartKind::Scatter,
            categories: vec![],
            series: vec![scatter_series("N", &[1.0, f64::NAN], &[1.0, 2.0])],
            ..base()
        },
        "non-finite",
    );
    invalid(
        Chart {
            kind: ChartKind::Pie,
            series: vec![series("A", &[1.0, 2.0, 3.0]), series("B", &[1.0, 2.0, 3.0])],
            ..base()
        },
        "pie charts take exactly one series",
    );
    invalid(
        Chart {
            kind: ChartKind::Line,
            stacked: true,
            ..base()
        },
        "stacked applies only to bar and column charts",
    );
    invalid(
        Chart {
            legend: Some("tr".into()),
            ..base()
        },
        "unsupported legend position: tr",
    );
    invalid(
        Chart {
            title: Some("bad\u{1}title".into()),
            ..base()
        },
        "forbidden XML character",
    );
    invalid(
        Chart {
            categories: vec!["ok".into(), "bad\u{FFFE}".into(), "ok".into()],
            ..base()
        },
        "forbidden XML character",
    );
    invalid(
        Chart {
            series: vec![Series {
                color: Some(Color::Rgb {
                    value: "12345G".into(),
                    alpha: None,
                }),
                ..series("N", &[1.0, 2.0, 3.0])
            }],
            ..base()
        },
        "invalid RGB/scheme color",
    );
    invalid(
        Chart {
            series: vec![Series {
                color: Some(Color::Scheme {
                    value: "accent9".into(),
                    alpha: None,
                }),
                ..series("N", &[1.0, 2.0, 3.0])
            }],
            ..base()
        },
        "invalid RGB/scheme color",
    );
}

#[test]
fn add_chart_rejects_non_slides_and_leaves_the_deck_usable() {
    let mut d = deck();
    let error = d
        .add_chart("ppt/slides/slide99.xml", &column_chart())
        .expect_err("not a slide")
        .to_string();
    assert!(error.contains("not a slide of this deck"), "{error}");
    let slide = d.slides()[0].clone();
    d.add_chart(&slide, &column_chart()).unwrap();
}

// --- Deck::add_chart parts, relationships and content types ---

fn rel_of(package: &Package, source: &str, ty: &str) -> Vec<(String, Option<String>)> {
    package
        .relationships()
        .iter()
        .filter(|r| r.source == source && r.relationship_type == ty)
        .map(|r| (r.id.clone(), r.resolved_target.clone()))
        .collect()
}

#[test]
fn add_chart_creates_parts_relationships_and_content_types() {
    let mut d = deck();
    let slide = d.slides()[0].clone();
    let rid = d.add_chart(&slide, &column_chart()).unwrap();
    let package = d.into_package();

    assert!(rid.starts_with("rId"));
    assert!(package.part(CHART_PART).is_some());
    assert!(package.part(WORKBOOK_PART).is_some());

    let slide_charts = rel_of(&package, &slide, CHART_REL);
    assert_eq!(
        slide_charts,
        vec![(rid, Some(CHART_PART.to_owned()))],
        "slide names the chart by the returned ID"
    );
    assert_eq!(
        rel_of(&package, CHART_PART, PACKAGE_REL),
        vec![("rId1".to_owned(), Some(WORKBOOK_PART.to_owned()))]
    );
    let chart_rels = String::from_utf8(
        package
            .part("ppt/charts/_rels/chart1.xml.rels")
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(chart_rels.contains("Target=\"../embeddings/Microsoft_Excel_Worksheet1.xlsx\""));

    let types = part_text(&package, "[Content_Types].xml");
    assert!(types.contains(&format!(
        "<Override PartName=\"/{CHART_PART}\" ContentType=\"{CHART_CT}\"/>"
    )));
    assert!(types.contains(&format!(
        "<Default Extension=\"xlsx\" ContentType=\"{XLSX_CT}\"/>"
    )));

    let bytes = package.to_bytes().unwrap();
    Package::from_bytes(bytes, Limits::default()).expect("package revalidates");
}

#[test]
fn second_chart_gets_the_next_part_names_and_identical_sequences_match() {
    let build = || {
        let mut d = deck();
        let slide = d.slides()[0].clone();
        d.add_chart(&slide, &column_chart()).unwrap();
        d.add_chart(&slide, &pie_chart()).unwrap();
        d.into_package()
    };
    let first = build();
    assert!(first.part("ppt/charts/chart2.xml").is_some());
    assert!(
        first
            .part("ppt/embeddings/Microsoft_Excel_Worksheet2.xlsx")
            .is_some()
    );
    assert_eq!(first.to_bytes().unwrap(), build().to_bytes().unwrap());
}

// --- slide frame, check, query and round trip ---

fn slide_with_chart(d: &mut Deck, slide: &str, rid: &str) {
    let object = Object {
        key: "chart".into(),
        id: 2,
        name: "Chart 1".into(),
        morph_name: None,
        description: None,
        hyperlink: None,
        kind: ObjectKind::Chart {
            transform: Transform {
                x: Emu(1000),
                y: Emu(2000),
                width: Emu(3000),
                height: Emu(2000),
                ..Default::default()
            },
            relationship: rid.into(),
        },
    };
    let xml = compile_slide(&Slide {
        objects: vec![object],
        ..Default::default()
    })
    .unwrap();
    d.set_slide(slide, xml).unwrap();
}

#[test]
fn chart_frame_compiles_to_a_graphic_frame_with_the_chart_reference() {
    let slide = Slide {
        objects: vec![Object {
            key: "chart".into(),
            id: 2,
            name: "Chart 1".into(),
            morph_name: None,
            description: None,
            hyperlink: None,
            kind: ObjectKind::Chart {
                transform: Transform {
                    x: Emu(1000),
                    y: Emu(2000),
                    width: Emu(3000),
                    height: Emu(2000),
                    ..Default::default()
                },
                relationship: "rId2".into(),
            },
        }],
        ..Default::default()
    };
    let xml = String::from_utf8(compile_slide(&slide).unwrap()).unwrap();
    in_order(
        &xml,
        &[
            "<p:graphicFrame>",
            "<p:cNvPr id=\"2\" name=\"Chart 1\"",
            "<a:graphicFrameLocks noGrp=\"1\"/>",
            "<p:xfrm><a:off x=\"1000\" y=\"2000\"/><a:ext cx=\"3000\" cy=\"2000\"/></p:xfrm>",
            "<a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/chart\">",
            "<c:chart xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" r:id=\"rId2\"/>",
            "</a:graphicData>",
            "</p:graphicFrame>",
        ],
    );
}

#[test]
fn chart_frame_validation_rejects_bad_frames() {
    let frame = |transform: Transform, relationship: &str| Slide {
        objects: vec![Object {
            key: "chart".into(),
            id: 2,
            name: "Chart 1".into(),
            morph_name: None,
            description: None,
            hyperlink: None,
            kind: ObjectKind::Chart {
                transform,
                relationship: relationship.into(),
            },
        }],
        ..Default::default()
    };
    let good = || Transform {
        width: Emu(3000),
        height: Emu(2000),
        ..Default::default()
    };
    let cases = [
        (
            frame(
                Transform {
                    width: Emu(0),
                    ..good()
                },
                "rId2",
            ),
            "chart frame extent must be nonzero",
        ),
        (
            frame(
                Transform {
                    rotation: Some(90),
                    ..good()
                },
                "rId2",
            ),
            "chart frame supports only x, y, width and height",
        ),
        (frame(good(), ""), "chart requires a relationship ID"),
    ];
    for (slide, message) in cases {
        let error = compile_slide(&slide).expect_err(message).to_string();
        assert!(error.contains(message), "{error}");
        assert!(error.contains("chart"), "must name the key: {error}");
    }
}

#[test]
fn chart_slide_passes_check_and_query_sees_the_frame_geometry() {
    let mut d = deck();
    let slide = d.slides()[0].clone();
    let rid = d.add_chart(&slide, &column_chart()).unwrap();
    slide_with_chart(&mut d, &slide, &rid);
    let package = d.into_package();

    let report = check_package(&package);
    let errors: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.severity == "error")
        .map(|f| format!("{}: {}", f.rule, f.message))
        .collect();
    assert!(errors.is_empty(), "unexpected check errors: {errors:?}");

    let session = QuerySession::from_package(&package, Limits::default()).unwrap();
    let state = session.state(1, &[], 0).unwrap();
    let shape = state
        .shapes
        .iter()
        .find(|s| s.id == 2)
        .expect("chart frame");
    assert_eq!(shape.name, "Chart 1");
    assert_eq!(
        shape.geometry,
        Some(Geometry {
            x: 1000.0,
            y: 2000.0,
            width: 3000.0,
            height: 2000.0,
        })
    );
}

#[test]
fn translate_moves_a_chart_frame() {
    let mut slide = Slide {
        objects: vec![Object {
            key: "chart".into(),
            id: 2,
            name: "Chart 1".into(),
            morph_name: None,
            description: None,
            hyperlink: None,
            kind: ObjectKind::Chart {
                transform: Transform {
                    x: Emu(1000),
                    y: Emu(2000),
                    width: Emu(3000),
                    height: Emu(2000),
                    ..Default::default()
                },
                relationship: "rId2".into(),
            },
        }],
        ..Default::default()
    };
    slide.translate("chart", Emu(5), Emu(-7)).unwrap();
    let ObjectKind::Chart { transform, .. } = &slide.objects[0].kind else {
        panic!("chart expected");
    };
    assert_eq!((transform.x, transform.y), (Emu(1005), Emu(1993)));
}

#[test]
fn chart_serde_round_trips_and_rejects_unknown_fields() {
    let chart = column_chart();
    let json = serde_json::to_value(&chart).unwrap();
    assert_eq!(json["kind"], json!("column"));
    let back: Chart = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(serde_json::to_value(&back).unwrap(), json);

    let minimal: Chart = serde_json::from_value(json!({
        "kind": "pie",
        "series": [{"name": "Share", "values": [1.0, 2.0]}],
        "categories": ["a", "b"]
    }))
    .unwrap();
    assert!(!minimal.data_labels && !minimal.stacked);
    assert!(minimal.title.is_none() && minimal.legend.is_none());

    let mut extra = json.clone();
    extra["surprise"] = json!(1);
    assert!(serde_json::from_value::<Chart>(extra).is_err());

    let mut series_extra = json.clone();
    series_extra["series"][0]["surprise"] = json!(1);
    assert!(serde_json::from_value::<Chart>(series_extra).is_err());

    let mut bad_kind = json.clone();
    bad_kind["kind"] = json!("area");
    assert!(serde_json::from_value::<Chart>(bad_kind).is_err());
}

#[test]
fn chart_object_kind_round_trips_and_rejects_unknown_fields() {
    let value: Value = json!({
        "type": "chart",
        "transform": {"x": 1000, "y": 2000, "width": 3000, "height": 2000},
        "relationship": "rId2"
    });
    let kind: ObjectKind = serde_json::from_value(value.clone()).unwrap();
    let emitted = serde_json::to_value(&kind).unwrap();
    assert_eq!(emitted["type"], json!("chart"));
    assert_eq!(emitted["relationship"], json!("rId2"));
    let back: ObjectKind = serde_json::from_value(emitted.clone()).unwrap();
    assert_eq!(serde_json::to_value(&back).unwrap(), emitted);

    let mut extra = value.clone();
    extra["extra"] = json!(true);
    assert!(serde_json::from_value::<ObjectKind>(extra).is_err());
}
