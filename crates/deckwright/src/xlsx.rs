//! The embedded workbook behind a chart: a minimal SpreadsheetML package with one sheet,
//! `Sheet1`, holding the chart's data where the chart formulas look (see `chart`).
//! Text is written inline, so the package has no shared-string or style parts. Zip
//! entries carry a fixed timestamp, so identical charts give identical bytes.
use crate::{
    Error, Result,
    chart::{Chart, ChartKind, column, esc},
};
use std::{collections::BTreeMap, io::Write};

const HEADER: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;
const MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PKG: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const CONTENT_TYPES: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">"#,
    r#"<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>"#,
    r#"<Default Extension="xml" ContentType="application/xml"/>"#,
    r#"<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>"#,
    r#"<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#,
    r#"</Types>"#,
);

enum Cell {
    Text(String),
    Number(f64),
}

/// The workbook bytes for `chart`. Only the five parts a minimal workbook needs.
pub(crate) fn workbook(chart: &Chart) -> Result<Vec<u8>> {
    let parts = [
        ("[Content_Types].xml", CONTENT_TYPES.to_owned()),
        ("_rels/.rels", root_rels()),
        ("xl/workbook.xml", workbook_xml()),
        ("xl/_rels/workbook.xml.rels", workbook_rels()),
        ("xl/worksheets/sheet1.xml", sheet_xml(&cells(chart))),
    ];
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(6))
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644);
    for (name, body) in parts {
        writer.start_file(name, options)?;
        writer
            .write_all(body.as_bytes())
            .map_err(|e| Error::InvalidPart {
                part: name.into(),
                message: e.to_string(),
            })?;
    }
    Ok(writer.finish()?.into_inner())
}

/// Cells keyed by (row, zero-based column); row 1 is the first row.
fn cells(chart: &Chart) -> BTreeMap<(usize, usize), Cell> {
    let mut out = BTreeMap::new();
    match chart.kind {
        ChartKind::Scatter => {
            for (k, s) in chart.series.iter().enumerate() {
                let (x_col, y_col) = (2 * k, 2 * k + 1);
                out.insert((1, x_col), Cell::Text(format!("{} X", s.name)));
                out.insert((1, y_col), Cell::Text(s.name.clone()));
                let xs = s.x_values.as_deref().unwrap_or(&[]);
                for (i, (x, y)) in xs.iter().zip(&s.values).enumerate() {
                    out.insert((i + 2, x_col), Cell::Number(*x));
                    out.insert((i + 2, y_col), Cell::Number(*y));
                }
            }
        }
        _ => {
            for (i, category) in chart.categories.iter().enumerate() {
                out.insert((i + 2, 0), Cell::Text(category.clone()));
            }
            for (k, s) in chart.series.iter().enumerate() {
                out.insert((1, k + 1), Cell::Text(s.name.clone()));
                for (i, value) in s.values.iter().enumerate() {
                    out.insert((i + 2, k + 1), Cell::Number(*value));
                }
            }
        }
    }
    out
}

fn sheet_xml(cells: &BTreeMap<(usize, usize), Cell>) -> String {
    let mut rows = String::new();
    let mut open_row: Option<usize> = None;
    for (&(row, col), cell) in cells {
        if open_row != Some(row) {
            if open_row.is_some() {
                rows.push_str("</row>");
            }
            rows.push_str(&format!(r#"<row r="{row}">"#));
            open_row = Some(row);
        }
        let reference = format!("{}{row}", column(col));
        match cell {
            Cell::Text(text) => rows.push_str(&format!(
                r#"<c r="{reference}" t="inlineStr"><is><t xml:space="preserve">{}</t></is></c>"#,
                esc(text)
            )),
            Cell::Number(value) => {
                rows.push_str(&format!(r#"<c r="{reference}"><v>{value}</v></c>"#))
            }
        }
    }
    if open_row.is_some() {
        rows.push_str("</row>");
    }
    format!(r#"{HEADER}<worksheet xmlns="{MAIN}"><sheetData>{rows}</sheetData></worksheet>"#)
}

fn root_rels() -> String {
    format!(
        r#"{HEADER}<Relationships xmlns="{PKG}"><Relationship Id="rId1" Type="{R}/officeDocument" Target="xl/workbook.xml"/></Relationships>"#
    )
}

fn workbook_xml() -> String {
    format!(
        r#"{HEADER}<workbook xmlns="{MAIN}" xmlns:r="{R}"><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>"#
    )
}

fn workbook_rels() -> String {
    format!(
        r#"{HEADER}<Relationships xmlns="{PKG}"><Relationship Id="rId1" Type="{R}/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#
    )
}
