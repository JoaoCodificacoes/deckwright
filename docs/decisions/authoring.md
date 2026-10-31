# Authoring primitives, wave 6 (prepares #5 slides 4–25)

1. **Preset geometry.** `Geometry.preset` accepts any ECMA-376 `ST_ShapeType`
   name, not only rect/roundRect/line. The builder keeps the name list as a
   sorted `const` array and rejects anything else with the existing
   "unsupported geometry preset" error. Adjustments are passed through as
   `<a:gd name fmla="val N"/>`.
2. **Custom geometry.** `Properties` gains `#[serde(default)] custom:
   Option<CustomGeometry>`. Setting both `custom` and `geometry` is an error.
   - `CustomGeometry { paths: Vec<GeometryPath> }`, where `GeometryPath { width:
     Emu, height: Emu, fill: Option<String>, stroke: bool, commands:
     Vec<PathCommand> }`. `fill` is an ST_PathFillMode such as "none" or
     "norm"; None omits the attribute.
   - `PathCommand` is `#[serde(tag = "type", rename_all = "snake_case")]`:
     `MoveTo { x, y }`, `LineTo { x, y }`, `QuadTo { x1, y1, x, y }`,
     `CubicTo { x1, y1, x2, y2, x, y }`, `ArcTo { width_radius,
     height_radius, start_angle, swing_angle }` (angles in 60000ths of a
     degree) and `Close`. Coordinates are `Emu` in path space.
   - The builder emits `<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/>
     <a:rect l="l" t="t" r="r" b="b"/><a:pathLst>...</a:pathLst></a:custGeom>`
     in schema child order.
   - Validation: at least one path; each path starts with MoveTo; coordinates
     lie within 0..=path size; sizes are > 0.
3. **Video pictures.** `ObjectKind::Picture` gains `#[serde(default)] media:
   Option<MediaLink>`, with `MediaLink { video_relationship: String,
   media_relationship: String }`.
   - The builder emits `<a:videoFile r:link="{video}"/>` in `p:nvPr`, followed
     by `<p:extLst><p:ext uri="{DAA4B4D4-6D54-4E2E-8F54-EE73A2AFCE79}">
     <p14:media xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main"
     r:embed="{media}"/></p:ext></p:extLst>`.
   - It also emits `<a:hlinkClick r:id="" action="ppaction://media"/>` inside
     `p:cNvPr`.
   - The picture's `relationship` stays the poster image. Relationship parts
     remain the caller's responsibility, as for existing pictures.

Existing slides 1–3 output must stay byte-identical. The tests prove it with
the existing authoring fixtures.

## Wave 9: line ends, dashes, paragraph end style, underline

The model additions below cover every authored feature the v2 thesis deck uses
beyond wave 6. Every field is `#[serde(default)]`, and `None` emits nothing, so
existing output stays byte-identical.

- **`Line`** derives `Default` and gains:
  - `dash: Option<String>`, an ST_PresetLineDashVal: solid, dot, dash, lgDash,
    dashDot, lgDashDot, lgDashDotDot, sysDash, sysDot, sysDashDot or
    sysDashDotDot;
  - `head_end: Option<LineEnd>` and `tail_end: Option<LineEnd>`.

  `LineEnd { kind: String, width: Option<String>, length: Option<String> }`.
  `kind` is one of none, triangle, stealth, diamond, oval or arrow. `width` and
  `length` are sm, med or lg. Children of `a:ln` follow schema order: fill,
  `<a:prstDash val/>`, `<a:headEnd type w len/>`, then
  `<a:tailEnd type w len/>`. Absent `w` and `len` attributes are omitted.
- **`Paragraph`** gains `end_style: Option<RunStyle>`, emitted as
  `<a:endParaRPr .../>` after the runs. It uses the same attribute and child
  emission as `<a:rPr>`, and `Some(RunStyle::default())` emits
  `<a:endParaRPr/>`.
- **`RunStyle`** gains `underline: Option<String>`, emitted as the `u`
  attribute. It is an ST_TextUnderlineType such as sng, dbl or none.

## Thesis source preparation, v2 (private input, generic code)

- `extract_source.py SOURCE OUTPUT [--sha256 HEX] [--dw PATH]`:
  - It keeps the frozen hash as the default.
  - `--sha256` authorizes a different explicitly named source and is recorded
    as `source_sha256`.
  - It extracts every slide in `sldIdLst` order into `slides/sNN.json`.
  - The template keeps every slide part, blanked to an empty `spTree`, with
    timing and transitions removed. It no longer drops slides.
  - New mappings: line dash and ends, `endParaRPr`, `u`, `custGeom` into
    `Properties.custom`, and video pictures into `Picture.media`. Everything
    else still fails closed.
  - With `--dw`, each slide's `dw query timeline` result is saved as
    `slides/sNN.timing.json`. Its `opaque` entries are listed in the manifest's
    `unsupported_authored_features`.
- `examples/thesis` builds every slide the manifest lists, in order. When
  `sNN.timing.json` exists, the slide is compiled with
  `timing::compile_animated_slide(slide, timeline, transition)`; otherwise it
  uses `compile_slide`. It never reads source slide XML.
- Private inputs and outputs stay under the ignored `fixtures/private/` and
  `build/` directories.
