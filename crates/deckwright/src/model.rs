//! Editable static source model. Optional values mean inheritance, not defaults.
//! Coordinates inside groups remain in their explicit child coordinate system.
use crate::{Error, Result, units::Emu};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slide {
    pub objects: Vec<Object>,
    /// Emitted as `p:bg` before the shape tree; None inherits the layout or master background.
    #[serde(default)]
    pub background: Option<Background>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Object {
    pub key: String,
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub morph_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// Emitted as the first child of `p:cNvPr`. Mutually exclusive with a video's media link.
    #[serde(default)]
    pub hyperlink: Option<Hyperlink>,
    pub kind: ObjectKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ObjectKind {
    Shape {
        #[serde(default)]
        textbox: bool,
        #[serde(default)]
        lock_no_group: bool,
        #[serde(default)]
        placeholder: Option<Placeholder>,
        properties: Properties,
        #[serde(default)]
        style: Option<StyleReferences>,
        #[serde(default)]
        text: Option<TextBody>,
    },
    Picture {
        relationship: String,
        #[serde(default)]
        lock_aspect: bool,
        #[serde(default)]
        crop: Option<Crop>,
        properties: Properties,
        /// Makes a video picture: `relationship` stays the poster image.
        #[serde(default)]
        media: Option<MediaLink>,
    },
    Connector {
        properties: Properties,
        #[serde(default)]
        style: Option<StyleReferences>,
        #[serde(default)]
        start: Option<Connection>,
        #[serde(default)]
        end: Option<Connection>,
    },
    Group {
        transform: Transform,
        children: Vec<Object>,
    },
    /// Native table in a `p:graphicFrame`; the frame transform is position and extent only.
    Table {
        transform: Transform,
        table: crate::table::Table,
    },
    /// OLE object in a `p:graphicFrame`. `relationship` names the embedded OOXML package
    /// and `preview_relationship` the PNG shown in its place. The frame transform is
    /// position and extent only.
    Ole {
        transform: Transform,
        prog_id: String,
        relationship: String,
        preview_relationship: String,
        #[serde(default)]
        show_as_icon: bool,
    },
    /// Native chart in a `p:graphicFrame`; `relationship` is the ID `Deck::add_chart` returned.
    Chart {
        transform: Transform,
        relationship: String,
    },
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transform {
    pub x: Emu,
    pub y: Emu,
    pub width: Emu,
    pub height: Emu,
    #[serde(default)]
    pub rotation: Option<i32>,
    #[serde(default)]
    pub flip_h: bool,
    #[serde(default)]
    pub flip_v: bool,
    #[serde(default)]
    pub child: Option<ChildSpace>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChildSpace {
    pub x: Emu,
    pub y: Emu,
    pub width: Emu,
    pub height: Emu,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Properties {
    #[serde(default)]
    pub transform: Option<Transform>,
    #[serde(default)]
    pub geometry: Option<Geometry>,
    /// Custom geometry; mutually exclusive with `geometry`.
    #[serde(default)]
    pub custom: Option<CustomGeometry>,
    #[serde(default)]
    pub fill: Option<Fill>,
    #[serde(default)]
    pub line: Option<Line>,
    /// None inherits effects; Some(empty) explicitly suppresses inherited effects.
    #[serde(default)]
    pub effects: Option<Vec<Shadow>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Geometry {
    pub preset: String,
    #[serde(default)]
    pub adjustments: Vec<Adjustment>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Adjustment {
    pub name: String,
    pub value: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomGeometry {
    pub paths: Vec<GeometryPath>,
}
/// A path in its own coordinate space `0..=width` by `0..=height`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeometryPath {
    pub width: Emu,
    pub height: Emu,
    /// ST_PathFillMode, such as "none" or "norm"; None inherits the default.
    #[serde(default)]
    pub fill: Option<String>,
    /// false emits `stroke="0"`; true is the DrawingML default and is omitted.
    #[serde(default)]
    pub stroke: bool,
    pub commands: Vec<PathCommand>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PathCommand {
    MoveTo {
        x: Emu,
        y: Emu,
    },
    LineTo {
        x: Emu,
        y: Emu,
    },
    QuadTo {
        x1: Emu,
        y1: Emu,
        x: Emu,
        y: Emu,
    },
    CubicTo {
        x1: Emu,
        y1: Emu,
        x2: Emu,
        y2: Emu,
        x: Emu,
        y: Emu,
    },
    /// Angles are in 60000ths of a degree.
    ArcTo {
        width_radius: Emu,
        height_radius: Emu,
        start_angle: i32,
        swing_angle: i32,
    },
    /// A struct variant: serde ignores deny_unknown_fields on unit variants of tagged enums.
    Close {},
}
/// Relationship IDs for a video picture's video part and its media extension part.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaLink {
    pub video_relationship: String,
    pub media_relationship: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Color {
    Rgb {
        value: String,
        #[serde(default)]
        alpha: Option<u32>,
    },
    Scheme {
        value: String,
        #[serde(default)]
        alpha: Option<u32>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Fill {
    None,
    Solid {
        color: Color,
    },
    /// Exactly one of `angle` (linear, 60000ths of a degree) or `path` (circle, rect or shape).
    Gradient {
        stops: Vec<GradientStop>,
        #[serde(default)]
        angle: Option<i32>,
        #[serde(default)]
        path: Option<String>,
        rotate_with_shape: bool,
    },
    /// `preset` is ST_PresetPatternVal, such as pct50 or dkDnDiag.
    Pattern {
        preset: String,
        foreground: Color,
        background: Color,
    },
    /// `relationship` is an image relationship ID on the slide.
    Picture {
        relationship: String,
    },
}
/// `position` is in 1000ths of a percent: 0..=100000.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GradientStop {
    pub position: u32,
    pub color: Color,
}
/// Slide background: a fill (`p:bgPr`) or a theme background style reference (`p:bgRef`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Background {
    Fill {
        fill: Fill,
    },
    /// `index` is 1..=3 (fill styles) or 1001..=1003 (background fill styles).
    Reference {
        index: u32,
        color: Color,
    },
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Line {
    #[serde(default)]
    pub width: Option<Emu>,
    #[serde(default)]
    pub fill: Option<Fill>,
    /// ST_PresetLineDashVal, such as solid, dash or sysDot.
    #[serde(default)]
    pub dash: Option<String>,
    #[serde(default)]
    pub head_end: Option<LineEnd>,
    #[serde(default)]
    pub tail_end: Option<LineEnd>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LineEnd {
    /// ST_LineEndType: none, triangle, stealth, diamond, oval or arrow.
    pub kind: String,
    /// ST_LineEndWidth: sm, med or lg.
    #[serde(default)]
    pub width: Option<String>,
    /// ST_LineEndLength: sm, med or lg.
    #[serde(default)]
    pub length: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shadow {
    pub blur: Emu,
    pub distance: Emu,
    pub direction: i32,
    pub alignment: String,
    pub rotate_with_shape: bool,
    pub color: Color,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placeholder {
    pub kind: String,
    #[serde(default)]
    pub index: Option<u32>,
    #[serde(default)]
    pub size: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connection {
    pub object_id: u32,
    pub index: u32,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Crop {
    #[serde(default)]
    pub left: i32,
    #[serde(default)]
    pub top: i32,
    #[serde(default)]
    pub right: i32,
    #[serde(default)]
    pub bottom: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StyleReferences {
    pub line: Reference,
    pub fill: Reference,
    pub effect: Reference,
    pub font: Reference,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub index: String,
    pub color: Color,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextBody {
    #[serde(default)]
    pub body: BodyProperties,
    #[serde(default)]
    pub list_style: bool,
    pub paragraphs: Vec<Paragraph>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyProperties {
    #[serde(default)]
    pub wrap: Option<String>,
    #[serde(default)]
    pub anchor: Option<String>,
    #[serde(default)]
    pub left: Option<Emu>,
    #[serde(default)]
    pub right: Option<Emu>,
    #[serde(default)]
    pub top: Option<Emu>,
    #[serde(default)]
    pub bottom: Option<Emu>,
    #[serde(default)]
    pub rtl_columns: Option<bool>,
    #[serde(default)]
    pub shape_autofit: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Paragraph {
    #[serde(default)]
    pub properties: Option<ParagraphProperties>,
    #[serde(default)]
    pub runs: Vec<TextRun>,
    /// Emitted as `a:endParaRPr` after the runs.
    #[serde(default)]
    pub end_style: Option<RunStyle>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParagraphProperties {
    #[serde(default)]
    pub alignment: Option<String>,
    /// Spacing in hundredths of a point, as in DrawingML.
    #[serde(default)]
    pub space_before: Option<i32>,
    #[serde(default)]
    pub space_after: Option<i32>,
    /// Outline level, 0..=8, emitted as `lvl`.
    #[serde(default)]
    pub level: Option<u8>,
    /// Left margin, emitted as `marL`.
    #[serde(default)]
    pub margin_left: Option<Emu>,
    /// First-line indent, emitted as `indent`; may be negative for hanging indents.
    #[serde(default)]
    pub indent: Option<Emu>,
    #[serde(default)]
    pub bullet: Option<Bullet>,
}
/// Paragraph bullet. `None` emits `a:buNone`, which suppresses inherited bullets.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Bullet {
    None,
    Char {
        /// The bullet character, emitted as `a:buChar`.
        char: String,
        #[serde(default)]
        font: Option<String>,
        #[serde(default)]
        color: Option<Color>,
        /// Bullet size as a percentage of text size, 25..=400.
        #[serde(default)]
        size_percent: Option<u32>,
    },
    AutoNumber {
        /// ST_TextAutonumberScheme, such as arabicPeriod or romanLcParenR.
        scheme: String,
        /// First number, 1..=32767.
        #[serde(default)]
        start_at: Option<u32>,
    },
}
/// Hyperlink target. URL links carry no action; jump-to-slide uses `ppaction://hlinksldjump`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hyperlink {
    /// Relationship ID created for the target, such as an external URL or a slide.
    pub relationship: String,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub tooltip: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextRun {
    pub text: String,
    #[serde(default)]
    pub style: Option<RunStyle>,
    #[serde(default)]
    pub field: Option<Field>,
    /// Emitted as `a:hlinkClick` inside `a:rPr`, after `latin`.
    #[serde(default)]
    pub hyperlink: Option<Hyperlink>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunStyle {
    #[serde(default)]
    pub font: Option<String>,
    #[serde(default)]
    pub size: Option<u32>,
    #[serde(default)]
    pub bold: Option<bool>,
    #[serde(default)]
    pub italic: Option<bool>,
    /// ST_TextUnderlineType, such as sng, dbl or none.
    #[serde(default)]
    pub underline: Option<String>,
    #[serde(default)]
    pub color: Option<Color>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Field {
    pub id: String,
    pub kind: String,
}

impl Slide {
    pub fn object_mut(&mut self, key: &str) -> Result<&mut Object> {
        fn find<'a>(objects: &'a mut [Object], key: &str) -> Option<&'a mut Object> {
            for object in objects {
                if object.key == key {
                    return Some(object);
                }
                if let ObjectKind::Group { children, .. } = &mut object.kind
                    && let Some(found) = find(children, key)
                {
                    return Some(found);
                }
            }
            None
        }
        find(&mut self.objects, key).ok_or_else(|| authored_error(key, "source key not found"))
    }
    /// Replace a single-run label. Rich text requires editing its runs explicitly.
    pub fn set_text(&mut self, key: &str, value: impl Into<String>) -> Result<()> {
        let object = self.object_mut(key)?;
        if let ObjectKind::Shape {
            text: Some(text), ..
        } = &mut object.kind
            && text.paragraphs.len() == 1
            && text.paragraphs[0].runs.len() == 1
        {
            text.paragraphs[0].runs[0].text = value.into();
            return Ok(());
        }
        Err(authored_error(
            key,
            "set_text requires a single-run text shape; edit rich runs explicitly",
        ))
    }
    /// Translate in the containing group's coordinates; overflow leaves the object unchanged.
    pub fn translate(&mut self, key: &str, dx: Emu, dy: Emu) -> Result<()> {
        let object = self.object_mut(key)?;
        let transform = match &mut object.kind {
            ObjectKind::Shape { properties, .. }
            | ObjectKind::Picture { properties, .. }
            | ObjectKind::Connector { properties, .. } => properties.transform.as_mut(),
            ObjectKind::Group { transform, .. }
            | ObjectKind::Table { transform, .. }
            | ObjectKind::Ole { transform, .. }
            | ObjectKind::Chart { transform, .. } => Some(transform),
        }
        .ok_or_else(|| {
            authored_error(
                key,
                "cannot translate inherited geometry; supply an explicit transform",
            )
        })?;
        let x = transform
            .x
            .0
            .checked_add(dx.0)
            .ok_or_else(|| authored_error(key, "x translation overflow"))?;
        let y = transform
            .y
            .0
            .checked_add(dy.0)
            .ok_or_else(|| authored_error(key, "y translation overflow"))?;
        transform.x = Emu(x);
        transform.y = Emu(y);
        Ok(())
    }
}
pub(crate) fn authored_error(key: &str, message: &str) -> Error {
    Error::InvalidPart {
        part: format!("authored object {key}"),
        message: message.into(),
    }
}
