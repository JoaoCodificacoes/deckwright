//! Editable static source model. Optional values mean inheritance, not defaults.
//! Coordinates inside groups remain in their explicit child coordinate system.
use crate::{Error, Result, units::Emu};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slide {
    pub objects: Vec<Object>,
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
    Solid { color: Color },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Line {
    #[serde(default)]
    pub width: Option<Emu>,
    #[serde(default)]
    pub fill: Option<Fill>,
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
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextRun {
    pub text: String,
    #[serde(default)]
    pub style: Option<RunStyle>,
    #[serde(default)]
    pub field: Option<Field>,
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
            ObjectKind::Group { transform, .. } => Some(transform),
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
