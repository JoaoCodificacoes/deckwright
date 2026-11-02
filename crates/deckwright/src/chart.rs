//! Native editable charts: the typed model, its validation, the `c:chartSpace` part and
//! the graphic frame that shows one. `Deck::add_chart` stores the part and its embedded
//! workbook; `builder` wraps `frame_content` around the relationship that call returns.
//!
//! Sheet layout, shared with `xlsx` and used by every formula: category charts keep the
//! series name in row 1 of columns B onward, the categories in column A from row 2, and
//! the values below each name. Scatter charts give series k the columns 2k (X, headed
//! "<name> X") and 2k+1 (Y, headed "<name>"), with values from row 2.
use crate::{
    Error, Result,
    builder::{attr, check_transform, element, empty, fill},
    model::{Color, Fill, Transform, authored_error},
};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

pub(crate) const CHART_URI: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const HEADER: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;
const SHEET: &str = "Sheet1";
const CAT_AX: &str = "500000001";
const VAL_AX: &str = "500000002";
const LEGEND_POSITIONS: &[&str] = &["r", "l", "t", "b"];
/// ST_SchemeColor names accepted by `Color::Scheme`, as in the builder's color check.
const SCHEME_COLORS: &[&str] = &[
    "bg1", "tx1", "bg2", "tx2", "dk1", "dk2", "lt1", "lt2", "accent1", "accent2", "accent3",
    "accent4", "accent5", "accent6", "hlink", "folHlink", "phClr",
];
/// Line and scatter stroke width, 2.25 pt in EMU.
const LINE_WIDTH: i64 = 28575;
const MARKER_SIZE: u32 = 7;

/// A chart with its data. Category charts take `categories` and one value per category
/// in each series; scatter charts take `x_values` per series and no categories.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chart {
    pub kind: ChartKind,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    pub series: Vec<Series>,
    /// Legend position: r, l, t or b. None omits the legend.
    #[serde(default)]
    pub legend: Option<String>,
    /// Shows each point's value.
    #[serde(default)]
    pub data_labels: bool,
    /// Stacks series. Allowed only for bar and column charts.
    #[serde(default)]
    pub stacked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartKind {
    Bar,
    Column,
    Line,
    Pie,
    Scatter,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Series {
    pub name: String,
    pub values: Vec<f64>,
    /// X values, one per value. Required for scatter series and rejected elsewhere.
    #[serde(default)]
    pub x_values: Option<Vec<f64>>,
    #[serde(default)]
    pub color: Option<Color>,
}

/// Checks a chart before anything is written. Errors name `part`, the slide it goes on.
pub(crate) fn validate(chart: &Chart, part: &str) -> Result<()> {
    let fail = |message: &str| -> Result<()> { Err(invalid(part, message)) };
    if chart.series.is_empty() {
        return fail("chart requires at least one series");
    }
    if let Some(title) = &chart.title {
        xml_text(part, title)?;
    }
    for category in &chart.categories {
        xml_text(part, category)?;
    }
    if let Some(position) = &chart.legend
        && !LEGEND_POSITIONS.contains(&position.as_str())
    {
        return fail(&format!("unsupported legend position: {position}"));
    }
    match chart.kind {
        ChartKind::Scatter => {
            if !chart.categories.is_empty() {
                return fail("scatter charts take no categories");
            }
            for s in &chart.series {
                let Some(xs) = &s.x_values else {
                    return fail(&format!("scatter series {} requires x_values", s.name));
                };
                if xs.len() != s.values.len() {
                    return fail(&format!(
                        "scatter series {} has {} x values for {} values",
                        s.name,
                        xs.len(),
                        s.values.len()
                    ));
                }
            }
        }
        _ => {
            if chart.categories.is_empty() {
                return fail("category charts require at least one category");
            }
            for s in &chart.series {
                if s.x_values.is_some() {
                    return fail(&format!(
                        "series {} has x_values; only scatter charts take them",
                        s.name
                    ));
                }
                if s.values.len() != chart.categories.len() {
                    return fail(&format!(
                        "series {} has {} values for {} categories",
                        s.name,
                        s.values.len(),
                        chart.categories.len()
                    ));
                }
            }
        }
    }
    if chart.kind == ChartKind::Pie && chart.series.len() != 1 {
        return fail("pie charts take exactly one series");
    }
    if chart.stacked && !matches!(chart.kind, ChartKind::Bar | ChartKind::Column) {
        return fail("stacked applies only to bar and column charts");
    }
    for s in &chart.series {
        xml_text(part, &s.name)?;
        let xs = s.x_values.iter().flatten();
        if s.values.iter().chain(xs).any(|v| !v.is_finite()) {
            return fail(&format!("series {} has a non-finite value", s.name));
        }
        if let Some(color) = &s.color {
            check_color(color, part)?;
        }
    }
    Ok(())
}

/// Validates the frame of an `ObjectKind::Chart`. It takes position and extent only.
pub(crate) fn validate_frame(transform: &Transform, relationship: &str, key: &str) -> Result<()> {
    check_transform(transform, key)?;
    if transform.rotation.is_some()
        || transform.flip_h
        || transform.flip_v
        || transform.child.is_some()
    {
        return Err(authored_error(
            key,
            "chart frame supports only x, y, width and height",
        ));
    }
    if transform.width.0 == 0 || transform.height.0 == 0 {
        return Err(authored_error(key, "chart frame extent must be nonzero"));
    }
    if relationship.is_empty() {
        return Err(authored_error(key, "chart requires a relationship ID"));
    }
    Ok(())
}

/// The `p:xfrm` and `a:graphic` children of a chart `p:graphicFrame`.
pub(crate) fn frame_content(transform: &Transform, relationship: &str) -> String {
    let xfrm = element(
        "p:xfrm",
        String::new(),
        element(
            "a:off",
            attr("x", transform.x.0) + &attr("y", transform.y.0),
            String::new(),
        ) + &element(
            "a:ext",
            attr("cx", transform.width.0) + &attr("cy", transform.height.0),
            String::new(),
        ),
    );
    let chart = element(
        "c:chart",
        attr("xmlns:c", CHART_URI) + &attr("r:id", relationship),
        String::new(),
    );
    xfrm + &element(
        "a:graphic",
        String::new(),
        element("a:graphicData", attr("uri", CHART_URI), chart),
    )
}

/// The `c:chartSpace` part. `workbook_relationship` is the chart's relationship to its
/// embedded workbook, which `c:externalData` names.
pub(crate) fn chart_xml(chart: &Chart, workbook_relationship: &str) -> String {
    let title = chart.title.as_deref().map(title_xml).unwrap_or_default();
    let legend = chart
        .legend
        .as_ref()
        .map(|position| {
            element(
                "c:legend",
                String::new(),
                val("c:legendPos", position) + &val("c:overlay", 0),
            )
        })
        .unwrap_or_default();
    let plot = element("c:plotArea", String::new(), plot_body(chart));
    let body = title
        + &val("c:autoTitleDeleted", i32::from(chart.title.is_none()))
        + &plot
        + &legend
        + &val("c:plotVisOnly", 1)
        + &val("c:dispBlanksAs", "gap");
    let external = element(
        "c:externalData",
        attr("r:id", workbook_relationship),
        val("c:autoUpdate", 0),
    );
    let root = attr("xmlns:c", CHART_URI) + &attr("xmlns:a", A) + &attr("xmlns:r", R);
    let space = element(
        "c:chartSpace",
        root,
        val("c:roundedCorners", 0) + &element("c:chart", String::new(), body) + &external,
    );
    format!("{HEADER}{space}")
}

/// Spreadsheet column name for a zero-based index: 0 is A, 25 is Z, 26 is AA.
pub(crate) fn column(index: usize) -> String {
    let mut n = index;
    let mut name = Vec::new();
    loop {
        name.push(b'A' + (n % 26) as u8);
        if n < 26 {
            break;
        }
        n = n / 26 - 1;
    }
    name.reverse();
    String::from_utf8(name).expect("ASCII letters")
}

/// Escapes text for an XML element body.
pub(crate) fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn invalid(part: &str, message: impl Into<String>) -> Error {
    Error::InvalidPart {
        part: part.into(),
        message: message.into(),
    }
}

/// Characters XML 1.0 can carry: tab, line feed, carriage return and the Unicode ranges.
fn xml_text(part: &str, value: &str) -> Result<()> {
    let allowed = value.chars().all(|c| {
        matches!(
            c,
            '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}'
        )
    });
    if allowed {
        Ok(())
    } else {
        Err(invalid(part, "text contains forbidden XML character"))
    }
}

fn check_color(color: &Color, part: &str) -> Result<()> {
    let valid = match color {
        Color::Rgb { value, alpha } => {
            value.len() == 6
                && value.bytes().all(|b| b.is_ascii_hexdigit())
                && alpha.is_none_or(|v| v <= 100000)
        }
        Color::Scheme { value, alpha } => {
            SCHEME_COLORS.contains(&value.as_str()) && alpha.is_none_or(|v| v <= 100000)
        }
    };
    if valid {
        Ok(())
    } else {
        Err(invalid(part, "invalid RGB/scheme color or alpha"))
    }
}

/// `<c:{name} val="..."/>`.
fn val(name: &str, value: impl Display) -> String {
    element(name, attr("val", value), String::new())
}

fn title_xml(title: &str) -> String {
    let run = element(
        "a:r",
        String::new(),
        element("a:t", String::new(), esc(title)),
    );
    let paragraph = element("a:p", String::new(), run);
    let rich = element(
        "c:rich",
        String::new(),
        empty("a:bodyPr") + &empty("a:lstStyle") + &paragraph,
    );
    element(
        "c:title",
        String::new(),
        element("c:tx", String::new(), rich) + &val("c:overlay", 0),
    )
}

fn plot_body(chart: &Chart) -> String {
    match chart.kind {
        ChartKind::Pie => pie_plot(chart),
        ChartKind::Scatter => scatter_plot(chart),
        ChartKind::Bar | ChartKind::Column | ChartKind::Line => category_plot(chart),
    }
}

fn category_plot(chart: &Chart) -> String {
    let horizontal = chart.kind == ChartKind::Bar;
    let series: String = chart
        .series
        .iter()
        .enumerate()
        .map(|(k, s)| category_series(chart.kind, &chart.categories, k, s))
        .collect();
    let labels = data_labels(chart.data_labels);
    let typed = if chart.kind == ChartKind::Line {
        element(
            "c:lineChart",
            String::new(),
            val("c:grouping", "standard")
                + &val("c:varyColors", 0)
                + &series
                + &labels
                + &val("c:marker", 1)
                + &axis_ids(),
        )
    } else {
        let grouping = if chart.stacked {
            "stacked"
        } else {
            "clustered"
        };
        let mut inner = val("c:barDir", if horizontal { "bar" } else { "col" })
            + &val("c:grouping", grouping)
            + &val("c:varyColors", 0)
            + &series
            + &labels
            + &val("c:gapWidth", 150);
        if chart.stacked {
            inner += &val("c:overlap", 100);
        }
        inner += &axis_ids();
        element("c:barChart", String::new(), inner)
    };
    let (category_pos, value_pos) = if horizontal { ("l", "b") } else { ("b", "l") };
    typed + &category_axis(category_pos) + &value_axis(VAL_AX, CAT_AX, value_pos, true, "between")
}

fn pie_plot(chart: &Chart) -> String {
    let series = category_series(chart.kind, &chart.categories, 0, &chart.series[0]);
    element(
        "c:pieChart",
        String::new(),
        val("c:varyColors", 1)
            + &series
            + &data_labels(chart.data_labels)
            + &val("c:firstSliceAng", 0),
    )
}

fn scatter_plot(chart: &Chart) -> String {
    let series: String = chart
        .series
        .iter()
        .enumerate()
        .map(|(k, s)| scatter_series(k, s))
        .collect();
    let typed = element(
        "c:scatterChart",
        String::new(),
        val("c:scatterStyle", "lineMarker")
            + &val("c:varyColors", 0)
            + &series
            + &data_labels(chart.data_labels)
            + &axis_ids(),
    );
    typed
        + &value_axis(CAT_AX, VAL_AX, "b", false, "midCat")
        + &value_axis(VAL_AX, CAT_AX, "l", true, "midCat")
}

/// Series `k` of a category chart: name in row 1, values in the column after categories.
fn category_series(kind: ChartKind, categories: &[String], k: usize, s: &Series) -> String {
    let column = column(k + 1);
    let last = categories.len() + 1;
    let line = kind == ChartKind::Line;
    let mut out = val("c:idx", k)
        + &val("c:order", k)
        + &name_ref(&format!("{SHEET}!${column}$1"), &s.name)
        + &series_props(kind, s);
    if line {
        out += &marker(s);
    }
    out += &element(
        "c:cat",
        String::new(),
        str_ref(&format!("{SHEET}!$A$2:$A${last}"), categories),
    );
    out += &element(
        "c:val",
        String::new(),
        num_ref(&format!("{SHEET}!${column}$2:${column}${last}"), &s.values),
    );
    if line {
        out += &val("c:smooth", 0);
    }
    element("c:ser", String::new(), out)
}

/// Scatter series `k`: X in column 2k, Y in column 2k+1.
fn scatter_series(k: usize, s: &Series) -> String {
    let x_column = column(2 * k);
    let y_column = column(2 * k + 1);
    let last = s.values.len() + 1;
    let xs = s.x_values.as_deref().unwrap_or(&[]);
    let out = val("c:idx", k)
        + &val("c:order", k)
        + &name_ref(&format!("{SHEET}!${y_column}$1"), &s.name)
        + &series_props(ChartKind::Scatter, s)
        + &marker(s)
        + &element(
            "c:xVal",
            String::new(),
            num_ref(&format!("{SHEET}!${x_column}$2:${x_column}${last}"), xs),
        )
        + &element(
            "c:yVal",
            String::new(),
            num_ref(
                &format!("{SHEET}!${y_column}$2:${y_column}${last}"),
                &s.values,
            ),
        )
        + &val("c:smooth", 0);
    element("c:ser", String::new(), out)
}

fn series_props(kind: ChartKind, s: &Series) -> String {
    let Some(color) = &s.color else {
        return String::new();
    };
    let solid = fill(&Fill::Solid {
        color: color.clone(),
    });
    let inner = if matches!(kind, ChartKind::Line | ChartKind::Scatter) {
        element("a:ln", attr("w", LINE_WIDTH), solid)
    } else {
        solid
    };
    element("c:spPr", String::new(), inner)
}

fn marker(s: &Series) -> String {
    let props = match &s.color {
        Some(color) => element(
            "c:spPr",
            String::new(),
            fill(&Fill::Solid {
                color: color.clone(),
            }),
        ),
        None => String::new(),
    };
    element(
        "c:marker",
        String::new(),
        val("c:symbol", "circle") + &val("c:size", MARKER_SIZE) + &props,
    )
}

fn data_labels(on: bool) -> String {
    if !on {
        return String::new();
    }
    element(
        "c:dLbls",
        String::new(),
        val("c:showLegendKey", 0)
            + &val("c:showVal", 1)
            + &val("c:showCatName", 0)
            + &val("c:showSerName", 0)
            + &val("c:showPercent", 0)
            + &val("c:showBubbleSize", 0),
    )
}

fn axis_ids() -> String {
    val("c:axId", CAT_AX) + &val("c:axId", VAL_AX)
}

fn scaling() -> String {
    element("c:scaling", String::new(), val("c:orientation", "minMax"))
}

fn num_format() -> String {
    element(
        "c:numFmt",
        attr("formatCode", "General") + &attr("sourceLinked", 1),
        String::new(),
    )
}

fn category_axis(position: &str) -> String {
    element(
        "c:catAx",
        String::new(),
        val("c:axId", CAT_AX)
            + &scaling()
            + &val("c:delete", 0)
            + &val("c:axPos", position)
            + &num_format()
            + &val("c:majorTickMark", "out")
            + &val("c:minorTickMark", "none")
            + &val("c:tickLblPos", "nextTo")
            + &val("c:crossAx", VAL_AX)
            + &val("c:crosses", "autoZero")
            + &val("c:auto", 1)
            + &val("c:lblAlgn", "ctr")
            + &val("c:lblOffset", 100)
            + &val("c:noMultiLvlLbl", 0),
    )
}

fn value_axis(id: &str, cross: &str, position: &str, gridlines: bool, between: &str) -> String {
    let grid = if gridlines {
        empty("c:majorGridlines")
    } else {
        String::new()
    };
    element(
        "c:valAx",
        String::new(),
        val("c:axId", id)
            + &scaling()
            + &val("c:delete", 0)
            + &val("c:axPos", position)
            + &grid
            + &num_format()
            + &val("c:majorTickMark", "out")
            + &val("c:minorTickMark", "none")
            + &val("c:tickLblPos", "nextTo")
            + &val("c:crossAx", cross)
            + &val("c:crosses", "autoZero")
            + &val("c:crossBetween", between),
    )
}

fn point(index: usize, value: &str) -> String {
    element(
        "c:pt",
        attr("idx", index),
        element("c:v", String::new(), value.to_owned()),
    )
}

fn name_ref(formula: &str, name: &str) -> String {
    element("c:tx", String::new(), str_ref(formula, &[name]))
}

fn str_ref<S: AsRef<str>>(formula: &str, values: &[S]) -> String {
    let points: String = values
        .iter()
        .enumerate()
        .map(|(i, v)| point(i, &esc(v.as_ref())))
        .collect();
    element(
        "c:strRef",
        String::new(),
        element("c:f", String::new(), esc(formula))
            + &element(
                "c:strCache",
                String::new(),
                val("c:ptCount", values.len()) + &points,
            ),
    )
}

fn num_ref(formula: &str, values: &[f64]) -> String {
    let points: String = values
        .iter()
        .enumerate()
        .map(|(i, v)| point(i, &v.to_string()))
        .collect();
    element(
        "c:numRef",
        String::new(),
        element("c:f", String::new(), esc(formula))
            + &element(
                "c:numCache",
                String::new(),
                element("c:formatCode", String::new(), "General".into())
                    + &val("c:ptCount", values.len())
                    + &points,
            ),
    )
}
