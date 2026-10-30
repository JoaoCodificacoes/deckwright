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
