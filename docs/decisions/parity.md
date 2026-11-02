# General authoring parity (python-pptx feature set)

Status: wave A is being implemented and wave B is specified here. Deckwright
already exceeds python-pptx on animation, transitions, queries and checks. This
document adds the general authoring features python-pptx has and deckwright
lacks. Every new serde field is `#[serde(default)]`, and absent values emit
nothing, so all existing output stays byte-identical. Every enum string is
validated against the listed ECMA-376 values, and errors name the object key.

Independent verification: each feature is read back by python-pptx (where it
supports the feature) and rendered by LibreOffice. Both run locally as
evidence; neither is a CI gate.

## Wave A

### A1. Bullets, levels, indents and hyperlinks (model.rs and builder.rs text paths)

**`ParagraphProperties` gains:**
- `level: Option<u8>` (0..=8), emitted as `lvl`;
- `margin_left: Option<Emu>`, emitted as `marL`;
- `indent: Option<Emu>`, emitted as `indent` (may be negative);
- `bullet: Option<Bullet>`.

`Bullet` is `#[serde(tag = "type", rename_all = "snake_case")]`:
- `None` emits `<a:buNone/>`;
- `Char { char: String, font: Option<String>, color: Option<Color>,
  size_percent: Option<u32> }` emits `buClr`, `buSzPct val=size_percent*1000`,
  `buFont typeface`, then `buChar char`. `size_percent` is 25..=400;
- `AutoNumber { scheme: String, start_at: Option<u32> }` emits
  `<a:buAutoNum type=scheme startAt/>`, with `scheme` an
  ST_TextAutonumberScheme value.

`a:pPr` children follow schema order: `lnSpc`, `spcBef`, `spcAft`,
`buClr`, `buSzPct`, `buFont`, then the bullet element.

**Hyperlinks.**
- `Hyperlink { relationship: String, action: Option<String>, tooltip:
  Option<String> }` emits `<a:hlinkClick r:id action tooltip/>`.
- `TextRun` gains `hyperlink: Option<Hyperlink>`, emitted inside `a:rPr` after
  `latin`. An `rPr` is emitted even when `style` is None.
- `Object` gains `hyperlink: Option<Hyperlink>`, emitted as the first child of
  `p:cNvPr`. Combining it with a video picture's media link is an error.
- Jump-to-slide links use `action: "ppaction://hlinksldjump"`. URL links have
  no action.
- Relationships are created by `Deck` (A2), like picture relationships today.

### A2. Deck API (new `deck.rs`)

`Deck` wraps a `Package` and owns relationship, content-type and
presentation-list bookkeeping.

```rust
Deck::from_package(Package) -> Deck
Deck::open(path, Limits) -> Result<Deck>
deck.into_package() -> Package
deck.write(path) -> Result<()>
deck.slides() -> Vec<String>                      // slide parts in sldIdLst order
deck.layouts() -> Vec<(String, String)>           // (part, cSld@name)
deck.add_slide(layout_part) -> Result<String>     // appends an empty slide
deck.set_slide(slide_part, xml: Vec<u8>) -> Result<()>
deck.add_image(slide_part, bytes, ext) -> Result<String>
deck.add_video(slide_part, bytes, ext, poster_png) -> Result<VideoLinks>
deck.add_hyperlink(slide_part, url) -> Result<String>
deck.add_slide_link(slide_part, target_slide_part) -> Result<String>
deck.set_notes(slide_part, text) -> Result<()>
deck.set_core_properties(&CoreProperties) -> Result<()>
```

- **`add_image`:** `ext` is png, jpeg/jpg, gif, bmp, tiff or svg. Identical
  bytes share one media part. Returns the relationship ID.
- **`add_video`:** `ext` is mp4, m4v, mov, wmv or avi.
  `VideoLinks { video, media, poster }` are the three relationship IDs.
- **`add_hyperlink`:** creates an external relationship and returns its ID.
- **`set_notes`:** creates a notes master (and its theme) when absent, then the
  slide's notesSlide with one paragraph per line of `text`.
- **`set_core_properties`:** `CoreProperties { title, subject, creator,
  keywords, description }`, all `Option<String>`, written to
  `docProps/core.xml`.
- Relationship IDs are unique per part. Content-type Defaults and Overrides
  are added as needed. Results revalidate through `Package::from_bytes`, and
  identical inputs give identical bytes.

### A3. Tables (new `table.rs` plus an `ObjectKind::Table` hook)

`ObjectKind::Table { transform: Transform, table: Table }`.

**Types.**
- `Table { columns: Vec<Emu>, rows: Vec<TableRow>, style_id: Option<String>,
  first_row: bool, band_row: bool }`;
- `TableRow { height: Emu, cells: Vec<TableCell> }`;
- `TableCell { text: TextBody, fill: Option<Fill>, grid_span: Option<u32>,
  row_span: Option<u32>, h_merge: bool, v_merge: bool, margin_left,
  margin_right, margin_top, margin_bottom: Option<Emu>, anchor:
  Option<String>, borders: Option<CellBorders> }`;
- `CellBorders { left, right, top, bottom: Option<Line> }`.

**XML.** A `p:graphicFrame` holds `nvGraphicFramePr` (with `a:graphicFrameLocks
noGrp="1"`), `p:xfrm`, and `a:graphic/a:graphicData
uri=".../drawingml/2006/table"`. Inside is `a:tbl`, containing:
- `tblPr`, with an optional `tableStyleId` child;
- `tblGrid/gridCol`;
- `a:tr`/`a:tc` cells holding `txBody` and a `tcPr` (borders `lnL`, `lnR`,
  `lnT`, `lnB`, then the fill).

**Validation.** Every row's cells, after spans, match the column count; merged
continuation cells carry `h_merge`/`v_merge`.

**Queries.** `QuerySession` snapshots a graphicFrame's `p:xfrm` geometry, so
state, bounds and Morph queries see tables.

### A4. Gradient, pattern and picture fills; slide backgrounds

**`Fill` gains:**
- `Gradient { stops: Vec<GradientStop>, angle: Option<i32>, path:
  Option<String>, rotate_with_shape: bool }`, with `GradientStop { position:
  u32, color: Color }`. `position` is 0..=100000, with at least 2 stops in
  ascending order. `angle` (60000ths of a degree) gives a linear gradient;
  `path` (circle, rect or shape) gives a path gradient. Set exactly one.
- `Pattern { preset: String, foreground: Color, background: Color }`, where
  `preset` is an ST_PresetPatternVal.
- `Picture { relationship: String }`, emitted as `<a:blipFill><a:blip
  r:embed/><a:stretch><a:fillRect/></a:stretch></a:blipFill>`.

**`Slide` gains** `background: Option<Background>`, emitted as `p:bg` inside
`p:cSld` before `spTree`:
- `Background::Fill { fill }` emits `<p:bgPr>{fill}<a:effectLst/></p:bgPr>`;
- `Background::Reference { index: u32, color: Color }` emits `<p:bgRef idx>`.

## Wave B

### B1. Charts (`chart.rs`, `ObjectKind::Chart`, `Deck::add_chart`)

**Types.**
- `Chart { kind: ChartKind, title: Option<String>, categories: Vec<String>,
  series: Vec<Series>, legend: Option<String>, data_labels: bool, stacked:
  bool }`.
- `ChartKind` is snake_case: bar, column, line, pie, scatter.
- `Series { name: String, values: Vec<f64>, x_values: Option<Vec<f64>>, color:
  Option<Color> }`.
- `legend` is r, l, t or b.

**Validation.**
- At least one series, and finite values.
- Category charts: every `values` length equals `categories.len()`, and
  `x_values` is None.
- Scatter: each series' `x_values` length equals its `values` length, and
  `categories` is empty.
- Pie: exactly one series.
- `stacked` is allowed only for bar and column.

**Chart part (`ppt/charts/chartN.xml`).**
- The root is `c:chartSpace`, with `c:roundedCorners val="0"`, then
  `c:chart`. Inside `c:chart` come an optional `c:title` (rich text), then
  `c:autoTitleDeleted`, then `c:plotArea`.
- The plot area holds one chart-type element:
  - bar and column: `c:barChart`, with `barDir` (bar or col), `grouping`
    (clustered or stacked), `varyColors 0`, the series, `gapWidth 150`,
    `overlap 100` when stacked, and two `axId` elements;
  - line: `c:lineChart`, with `grouping standard`, `varyColors 0`, series with
    `c:marker`, and two axes;
  - pie: `c:pieChart`, with `varyColors 1`, one series and no axes;
  - scatter: `c:scatterChart`, with `scatterStyle lineMarker`, `varyColors 0`,
    series using `xVal`/`yVal`, and two `valAx`.
- Category charts also get `c:catAx` and `c:valAx`, with matching IDs.
- Each `c:ser` has: `idx`, `order`, a `tx` strRef with strCache, `spPr`
  (solidFill when a color is set), and `cat` (strRef plus strCache) with
  `val` (numRef plus numCache, formatCode General). Scatter series use
  `xVal`/`yVal` numRef plus numCache instead.
- Formulas point at the embedded sheet: `Sheet1!$B$1` for the first series
  name, and `Sheet1!$A$2:$A$n` for the categories.
- After the plot area come `c:legend` (with `legendPos` and
  `overlay 0`), `c:plotVisOnly 1`, and `c:dispBlanksAs gap`. When
  `data_labels` is set, `c:dLbls` with `showVal 1` goes in each chart-type
  element.
- The root ends with `c:externalData r:id` (`c:autoUpdate val="0"`).

**Embedded workbook (`ppt/embeddings/Microsoft_Excel_Worksheet{N}.xlsx`).** A
minimal SpreadsheetML package:
- `[Content_Types].xml`, `_rels/.rels`, `xl/workbook.xml` (one sheet named
  Sheet1), `xl/_rels/workbook.xml.rels` and `xl/worksheets/sheet1.xml`;
- strings are written as `t="inlineStr"`;
- category charts: A1 is empty, row 1 holds the series names, column A holds
  the categories, and the series values run down columns B and onward;
- scatter: each series takes a column pair holding its X and Y values.
It must open in python-pptx (`chart.part.chart_workbook`) and in
LibreOffice.

**Deck and builder wiring.**
- `Deck::add_chart(slide_part, &Chart) -> Result<String>` writes the chart
  part, the workbook, the chart's rels (type `.../package`), the content-type
  Override for the chart and the Default for xlsx, and the slide relationship
  (type `.../chart`). It returns the slide relationship ID.
- `ObjectKind::Chart { transform: Transform, relationship: String }` emits a
  `p:graphicFrame` with `graphicData uri=".../drawingml/2006/chart"` and
  `<c:chart r:id/>`.

### B2. OLE objects (`ObjectKind::Ole`, `Deck::add_ole_object`)

This follows python-pptx's `add_ole_object` form. Only OOXML packages are
embedded (xlsx, docx, pptx). Arbitrary binaries would need an OLE compound
file and are rejected with a clear error.

- `Deck::add_ole_object(slide_part, bytes, ext, preview_png) ->
  Result<OleLinks { object, preview }>`:
  - writes `ppt/embeddings/{name}N.{ext}`, with the content-type Default for
    the extension;
  - adds the slide relationships: type `.../package` for the object and
    `.../image` for the preview;
  - maps the progId: xlsx to `Excel.Sheet.12`, docx to `Word.Document.12`,
    pptx to `PowerPoint.Show.12`.
- `ObjectKind::Ole { transform: Transform, prog_id: String, relationship:
  String, preview_relationship: String, show_as_icon: bool }` emits a
  `p:graphicFrame` with `graphicData
  uri="http://schemas.openxmlformats.org/presentationml/2006/ole"`. Inside is
  `<p:oleObj progId r:id showAsIcon?>` holding `<p:embed/>` and a `p:pic`.
  That picture has a blipFill, using the preview relationship and a stretch,
  plus spPr with the same transform.

### B3. Parity demo (`examples/parity_demo`)

One public deck exercising every A and B feature, built only through `Deck`
and the model. It is read back by python-pptx and rendered by LibreOffice as
local evidence.
