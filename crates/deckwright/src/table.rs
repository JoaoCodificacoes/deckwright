//! Native PowerPoint tables: the typed model, its validation and the `a:tbl` emitter.
//! `builder` wraps the output in `p:graphicFrame`. This module owns everything inside it.
//! Every row lists one cell per column, including the continuation cells of merges, so
//! each `a:tr` holds exactly as many `a:tc` elements as the table has columns.
use crate::{
    Result,
    builder::{
        attr, check_properties, check_text, check_transform, element, fill, line_with_tag, member,
        opt, range, text_with,
    },
    model::{Fill, Line, Properties, TextBody, Transform, authored_error},
    units::Emu,
};
use serde::{Deserialize, Serialize};

pub(crate) const TABLE_URI: &str = "http://schemas.openxmlformats.org/drawingml/2006/table";
/// ST_PositiveCoordinate upper bound, used for column, row and margin lengths.
const MAX_LENGTH: i64 = 27273042316900;
const CELL_ANCHORS: &[&str] = &["t", "ctr", "b", "just", "dist"];

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Table {
    /// Column widths in order. Every width must be positive.
    pub columns: Vec<Emu>,
    pub rows: Vec<TableRow>,
    /// Braced uppercase table style GUID, such as the built-in default.
    #[serde(default)]
    pub style_id: Option<String>,
    #[serde(default)]
    pub first_row: bool,
    #[serde(default)]
    pub band_row: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableRow {
    pub height: Emu,
    /// One cell per column. Merge continuation cells are present and flagged.
    pub cells: Vec<TableCell>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableCell {
    pub text: TextBody,
    #[serde(default)]
    pub fill: Option<Fill>,
    /// Columns covered by this cell, starting here. At least 2 when set.
    #[serde(default)]
    pub grid_span: Option<u32>,
    /// Rows covered by this cell, starting here. At least 2 when set.
    #[serde(default)]
    pub row_span: Option<u32>,
    /// Continuation of a horizontal span started in the same row.
    #[serde(default)]
    pub h_merge: bool,
    /// Continuation of a vertical span started in the same column.
    #[serde(default)]
    pub v_merge: bool,
    #[serde(default)]
    pub margin_left: Option<Emu>,
    #[serde(default)]
    pub margin_right: Option<Emu>,
    #[serde(default)]
    pub margin_top: Option<Emu>,
    #[serde(default)]
    pub margin_bottom: Option<Emu>,
    /// ST_TextAnchoringType: t, ctr, b, just or dist.
    #[serde(default)]
    pub anchor: Option<String>,
    #[serde(default)]
    pub borders: Option<CellBorders>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellBorders {
    #[serde(default)]
    pub left: Option<Line>,
    #[serde(default)]
    pub right: Option<Line>,
    #[serde(default)]
    pub top: Option<Line>,
    #[serde(default)]
    pub bottom: Option<Line>,
}

/// Validate a table frame. Errors name the object key, as the builder does everywhere.
pub(crate) fn validate(transform: &Transform, table: &Table, key: &str) -> Result<()> {
    check_transform(transform, key)?;
    if transform.rotation.is_some()
        || transform.flip_h
        || transform.flip_v
        || transform.child.is_some()
    {
        return Err(authored_error(
            key,
            "table frame supports only x, y, width and height",
        ));
    }
    if transform.width.0 == 0 || transform.height.0 == 0 {
        return Err(authored_error(key, "table frame extent must be nonzero"));
    }
    if table.columns.is_empty() {
        return Err(authored_error(key, "table requires at least one column"));
    }
    if table.rows.is_empty() {
        return Err(authored_error(key, "table requires at least one row"));
    }
    for width in &table.columns {
        range(width.0, 1, MAX_LENGTH, key, "column width")?;
    }
    if let Some(id) = &table.style_id
        && !braced_guid(id)
    {
        return Err(authored_error(
            key,
            "table style ID must be a braced uppercase GUID",
        ));
    }
    let columns = table.columns.len();
    for row in &table.rows {
        range(row.height.0, 1, MAX_LENGTH, key, "row height")?;
        if row.cells.len() != columns {
            return Err(authored_error(
                key,
                &format!(
                    "row has {} cells; table has {columns} columns",
                    row.cells.len()
                ),
            ));
        }
        for cell in &row.cells {
            check_cell(cell, key)?;
        }
    }
    check_spans(table, key)
}

/// The `p:xfrm` and `a:graphic` children of a table `p:graphicFrame`.
pub(crate) fn frame_content(transform: &Transform, table: &Table) -> String {
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
    let data = element("a:graphicData", attr("uri", TABLE_URI), tbl(table));
    xfrm + &element("a:graphic", String::new(), data)
}

fn tbl(table: &Table) -> String {
    let flags = flag("firstRow", table.first_row) + &flag("bandRow", table.band_row);
    let style = table
        .style_id
        .as_ref()
        .map(|id| element("a:tableStyleId", String::new(), id.clone()))
        .unwrap_or_default();
    let grid: String = table
        .columns
        .iter()
        .map(|w| element("a:gridCol", attr("w", w.0), String::new()))
        .collect();
    let rows: String = table
        .rows
        .iter()
        .map(|row| {
            let cells: String = row.cells.iter().map(cell).collect();
            element("a:tr", attr("h", row.height.0), cells)
        })
        .collect();
    element(
        "a:tbl",
        String::new(),
        element("a:tblPr", flags, style) + &element("a:tblGrid", String::new(), grid) + &rows,
    )
}

fn cell(c: &TableCell) -> String {
    let spans = opt("gridSpan", c.grid_span)
        + &opt("rowSpan", c.row_span)
        + &flag("hMerge", c.h_merge)
        + &flag("vMerge", c.v_merge);
    // Table cells always carry an explicit a:lstStyle, as PowerPoint writes them.
    let mut text = c.text.clone();
    text.list_style = true;
    let mut borders = String::new();
    if let Some(b) = &c.borders {
        for (tag, line) in [
            ("a:lnL", &b.left),
            ("a:lnR", &b.right),
            ("a:lnT", &b.top),
            ("a:lnB", &b.bottom),
        ] {
            if let Some(line) = line {
                borders += &line_with_tag(tag, line);
            }
        }
    }
    // Schema order inside a:tcPr: borders, then the fill.
    let props_attrs = opt("marL", c.margin_left.map(|v| v.0))
        + &opt("marR", c.margin_right.map(|v| v.0))
        + &opt("marT", c.margin_top.map(|v| v.0))
        + &opt("marB", c.margin_bottom.map(|v| v.0))
        + &opt("anchor", c.anchor.as_ref());
    let props_children = borders + &c.fill.as_ref().map(fill).unwrap_or_default();
    element(
        "a:tc",
        spans,
        text_with("a:txBody", &text) + &element("a:tcPr", props_attrs, props_children),
    )
}

fn flag(name: &str, on: bool) -> String {
    if on { attr(name, 1) } else { String::new() }
}

fn check_cell(cell: &TableCell, key: &str) -> Result<()> {
    check_text(&cell.text, key)?;
    for margin in [
        cell.margin_left,
        cell.margin_right,
        cell.margin_top,
        cell.margin_bottom,
    ]
    .into_iter()
    .flatten()
    {
        range(margin.0, 0, MAX_LENGTH, key, "cell margin")?;
    }
    if let Some(anchor) = &cell.anchor {
        member(anchor, CELL_ANCHORS, key, "cell anchor")?;
    }
    if let Some(borders) = &cell.borders {
        for line in [&borders.left, &borders.right, &borders.top, &borders.bottom]
            .into_iter()
            .flatten()
        {
            // Border lines use the same width and dash rules as shape lines.
            check_properties(
                &Properties {
                    line: Some(line.clone()),
                    ..Default::default()
                },
                key,
            )?;
        }
    }
    Ok(())
}

/// Each cell position is either a span origin, a continuation of one, or plain.
/// A continuation is flagged h_merge when it lies right of the origin and v_merge when it lies
/// below it, so interior cells of a two-way span carry both flags.
fn check_spans(table: &Table, key: &str) -> Result<()> {
    let (columns, rows) = (table.columns.len(), table.rows.len());
    let mut origin = vec![vec![None::<(usize, usize)>; columns]; rows];
    for (r, row) in table.rows.iter().enumerate() {
        for (c, cell) in row.cells.iter().enumerate() {
            if cell.grid_span.is_none() && cell.row_span.is_none() {
                continue;
            }
            if cell.grid_span.is_some_and(|n| n < 2) || cell.row_span.is_some_and(|n| n < 2) {
                return Err(authored_error(
                    key,
                    "gridSpan and rowSpan must be at least 2 when set",
                ));
            }
            if cell.h_merge || cell.v_merge {
                return Err(authored_error(
                    key,
                    "a span origin cannot also be a merge continuation",
                ));
            }
            let across = cell.grid_span.map_or(1, |n| n as usize);
            let down = cell.row_span.map_or(1, |n| n as usize);
            if c + across > columns {
                return Err(authored_error(key, "gridSpan exceeds the table columns"));
            }
            if r + down > rows {
                return Err(authored_error(key, "rowSpan exceeds the table rows"));
            }
            for dr in 0..down {
                for dc in 0..across {
                    let slot = &mut origin[r + dr][c + dc];
                    if slot.is_some() {
                        return Err(authored_error(key, "merged cell spans overlap"));
                    }
                    *slot = Some((r, c));
                }
            }
        }
    }
    for (r, row) in table.rows.iter().enumerate() {
        for (c, cell) in row.cells.iter().enumerate() {
            let expected = match origin[r][c] {
                None => (false, false),
                // Origin flags were rejected above, so there is nothing more to check here.
                Some((r0, c0)) if (r0, c0) == (r, c) => continue,
                Some((r0, c0)) => (c > c0, r > r0),
            };
            if (cell.h_merge, cell.v_merge) != expected {
                return Err(authored_error(
                    key,
                    &format!(
                        "merge flags at row {} column {} disagree with the span",
                        r + 1,
                        c + 1
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// Braced GUID with uppercase hex digits, as PowerPoint writes table style IDs.
fn braced_guid(value: &str) -> bool {
    let b = value.as_bytes();
    b.len() == 38
        && b[0] == b'{'
        && b[37] == b'}'
        && b[1..37].iter().enumerate().all(|(i, byte)| {
            if [8, 13, 18, 23].contains(&i) {
                *byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'A'..=b'F').contains(byte)
            }
        })
}
