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

## Wave B (after wave A merges)

- **B1. Charts** (`chart.rs`, `ObjectKind::Chart`, `Deck::add_chart`):
  - Bar, column, line, pie and scatter charts, with categories and series held
    as cached values in a `chartN.xml` part.
  - An embedded minimal xlsx with the same data, so the chart can be edited in
    PowerPoint.
  - A graphicFrame referencing the chart part, plus a title and a legend.
- **B2. OLE objects** (`ole.rs`, `Deck::add_ole_object`): an embedded file
  with a progId and preview image, placed in a graphicFrame `p:oleObj`.
- **B3. Parity demo** (`examples/parity_demo`): one public deck that exercises
  every feature above. It is read back by python-pptx and rendered by
  LibreOffice.
