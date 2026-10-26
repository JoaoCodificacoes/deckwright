//! Compile bounded typed static content into native editable DrawingML.
//! No source slide or shape XML is accepted by this API.
use crate::{Result, model::*};
use std::collections::BTreeSet;

const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn attr(name: &str, value: impl std::fmt::Display) -> String {
    format!(" {name}=\"{}\"", escape(&value.to_string()))
}
fn opt<T: std::fmt::Display>(name: &str, value: Option<T>) -> String {
    value.map(|v| attr(name, v)).unwrap_or_default()
}
fn element(name: &str, attrs: String, children: String) -> String {
    if children.is_empty() {
        format!("<{name}{attrs}/>")
    } else {
        format!("<{name}{attrs}>{children}</{name}>")
    }
}
fn empty(name: &str) -> String {
    element(name, String::new(), String::new())
}
fn color(value: &Color) -> String {
    let (name, value, alpha) = match value {
        Color::Rgb { value, alpha } => ("a:srgbClr", value, alpha),
        Color::Scheme { value, alpha } => ("a:schemeClr", value, alpha),
    };
    element(
        name,
        attr("val", value),
        alpha
            .map(|v| element("a:alpha", attr("val", v), String::new()))
            .unwrap_or_default(),
    )
}
fn fill(value: &Fill) -> String {
    match value {
        Fill::None => empty("a:noFill"),
        Fill::Solid { color: c } => element("a:solidFill", String::new(), color(c)),
    }
}
fn transform(t: &Transform) -> String {
    let attrs = opt("rot", t.rotation)
        + &if t.flip_h {
            attr("flipH", 1)
        } else {
            String::new()
        }
        + &if t.flip_v {
            attr("flipV", 1)
        } else {
            String::new()
        };
    let mut children = element("a:off", attr("x", t.x.0) + &attr("y", t.y.0), String::new())
        + &element(
            "a:ext",
            attr("cx", t.width.0) + &attr("cy", t.height.0),
            String::new(),
        );
    if let Some(c) = &t.child {
        children += &element(
            "a:chOff",
            attr("x", c.x.0) + &attr("y", c.y.0),
            String::new(),
        );
        children += &element(
            "a:chExt",
            attr("cx", c.width.0) + &attr("cy", c.height.0),
            String::new(),
        );
    }
    element("a:xfrm", attrs, children)
}
fn properties(p: &Properties) -> String {
    let mut content = p.transform.as_ref().map(transform).unwrap_or_default();
    if let Some(g) = &p.geometry {
        let adjustments = g
            .adjustments
            .iter()
            .map(|a| {
                element(
                    "a:gd",
                    attr("name", &a.name) + &attr("fmla", format!("val {}", a.value)),
                    String::new(),
                )
            })
            .collect::<String>();
        content += &element(
            "a:prstGeom",
            attr("prst", &g.preset),
            element("a:avLst", String::new(), adjustments),
        );
    }
    if let Some(f) = &p.fill {
        content += &fill(f);
    }
    if let Some(l) = &p.line {
        content += &element(
            "a:ln",
            opt("w", l.width.map(|v| v.0)),
            l.fill.as_ref().map(fill).unwrap_or_default(),
        );
    }
    if let Some(effects) = &p.effects {
        let effects = effects
            .iter()
            .map(|s| {
                element(
                    "a:outerShdw",
                    attr("blurRad", s.blur.0)
                        + &attr("dist", s.distance.0)
                        + &attr("dir", s.direction)
                        + &attr("algn", &s.alignment)
                        + &attr("rotWithShape", u8::from(s.rotate_with_shape)),
                    color(&s.color),
                )
            })
            .collect::<String>();
        content += &element("a:effectLst", String::new(), effects);
    }
    element("p:spPr", String::new(), content)
}
fn style(value: &Option<StyleReferences>) -> String {
    value
        .as_ref()
        .map(|s| {
            let refs = [
                ("a:lnRef", &s.line),
                ("a:fillRef", &s.fill),
                ("a:effectRef", &s.effect),
                ("a:fontRef", &s.font),
            ];
            element(
                "p:style",
                String::new(),
                refs.iter()
                    .map(|(tag, r)| element(tag, attr("idx", &r.index), color(&r.color)))
                    .collect(),
            )
        })
        .unwrap_or_default()
}
fn text(t: &TextBody) -> String {
    let b = &t.body;
    let attrs = opt("wrap", b.wrap.as_ref())
        + &opt("lIns", b.left.map(|v| v.0))
        + &opt("rIns", b.right.map(|v| v.0))
        + &opt("tIns", b.top.map(|v| v.0))
        + &opt("bIns", b.bottom.map(|v| v.0))
        + &opt("rtlCol", b.rtl_columns.map(u8::from))
        + &opt("anchor", b.anchor.as_ref());
    let mut content = element(
        "a:bodyPr",
        attrs,
        if b.shape_autofit {
            empty("a:spAutoFit")
        } else {
            String::new()
        },
    );
    if t.list_style {
        content += &empty("a:lstStyle");
    }
    for p in &t.paragraphs {
        let mut paragraph = String::new();
        if let Some(pp) = &p.properties {
            let mut spacing = String::new();
            for (tag, val) in [("a:spcBef", pp.space_before), ("a:spcAft", pp.space_after)] {
                if let Some(v) = val {
                    spacing += &element(
                        tag,
                        String::new(),
                        element("a:spcPts", attr("val", v), String::new()),
                    );
                }
            }
            paragraph += &element("a:pPr", opt("algn", pp.alignment.as_ref()), spacing);
        }
        for r in &p.runs {
            let mut run = String::new();
            if let Some(s) = &r.style {
                let attrs = opt("sz", s.size)
                    + &opt("b", s.bold.map(u8::from))
                    + &opt("i", s.italic.map(u8::from));
                let mut run_style = s
                    .color
                    .as_ref()
                    .map(|c| element("a:solidFill", String::new(), color(c)))
                    .unwrap_or_default();
                if let Some(font) = &s.font {
                    run_style += &element("a:latin", attr("typeface", font), String::new());
                }
                run += &element("a:rPr", attrs, run_style);
            }
            // xml:space makes authored leading/trailing and repeated whitespace explicit.
            run += &element("a:t", attr("xml:space", "preserve"), escape(&r.text));
            paragraph += &if let Some(field) = &r.field {
                element(
                    "a:fld",
                    attr("id", &field.id) + &attr("type", &field.kind),
                    run,
                )
            } else {
                element("a:r", String::new(), run)
            };
        }
        content += &element("a:p", String::new(), paragraph);
    }
    element("p:txBody", String::new(), content)
}
fn common(o: &Object) -> String {
    let name = o.morph_name.as_ref().unwrap_or(&o.name);
    let attrs = attr("id", o.id) + &attr("name", name) + &opt("descr", o.description.as_ref());
    let key = element(
        "dw:source",
        attr("xmlns:dw", "urn:deckwright:source:v1") + &attr("key", &o.key),
        String::new(),
    );
    element(
        "p:cNvPr",
        attrs,
        element(
            "a:extLst",
            String::new(),
            element("a:ext", attr("uri", "urn:deckwright:source:v1"), key),
        ),
    )
}
fn object(o: &Object) -> String {
    let nv = common(o);
    let mut content;
    let tag;
    match &o.kind {
        ObjectKind::Shape {
            textbox,
            lock_no_group,
            placeholder,
            properties: p,
            style: s,
            text: t,
        } => {
            tag = "p:sp";
            let locks = if *lock_no_group {
                element("a:spLocks", attr("noGrp", 1), String::new())
            } else {
                String::new()
            };
            let ph = placeholder
                .as_ref()
                .map(|p| {
                    element(
                        "p:ph",
                        attr("type", &p.kind) + &opt("idx", p.index) + &opt("sz", p.size.as_ref()),
                        String::new(),
                    )
                })
                .unwrap_or_default();
            content = element(
                "p:nvSpPr",
                String::new(),
                nv + &element(
                    "p:cNvSpPr",
                    if *textbox {
                        attr("txBox", 1)
                    } else {
                        String::new()
                    },
                    locks,
                ) + &element("p:nvPr", String::new(), ph),
            );
            content += &properties(p);
            content += &style(s);
            if let Some(t) = t {
                content += &text(t);
            }
        }
        ObjectKind::Picture {
            relationship,
            lock_aspect,
            crop,
            properties: p,
        } => {
            tag = "p:pic";
            let locks = if *lock_aspect {
                element("a:picLocks", attr("noChangeAspect", 1), String::new())
            } else {
                String::new()
            };
            content = element(
                "p:nvPicPr",
                String::new(),
                nv + &element("p:cNvPicPr", String::new(), locks) + &empty("p:nvPr"),
            );
            let mut blip = element("a:blip", attr("r:embed", relationship), String::new());
            if let Some(c) = crop {
                blip += &element(
                    "a:srcRect",
                    attr("l", c.left)
                        + &attr("t", c.top)
                        + &attr("r", c.right)
                        + &attr("b", c.bottom),
                    String::new(),
                );
            }
            blip += &element("a:stretch", String::new(), empty("a:fillRect"));
            content += &element("p:blipFill", String::new(), blip);
            content += &properties(p);
        }
        ObjectKind::Connector {
            properties: p,
            style: s,
            start,
            end,
        } => {
            tag = "p:cxnSp";
            let mut endpoints = String::new();
            for (tag, connection) in [("a:stCxn", start), ("a:endCxn", end)] {
                if let Some(c) = connection {
                    endpoints += &element(
                        tag,
                        attr("id", c.object_id) + &attr("idx", c.index),
                        String::new(),
                    );
                }
            }
            content = element(
                "p:nvCxnSpPr",
                String::new(),
                nv + &element("p:cNvCxnSpPr", String::new(), endpoints) + &empty("p:nvPr"),
            );
            content += &properties(p);
            content += &style(s);
        }
        ObjectKind::Group {
            transform: t,
            children,
        } => {
            tag = "p:grpSp";
            content = element(
                "p:nvGrpSpPr",
                String::new(),
                nv + &empty("p:cNvGrpSpPr") + &empty("p:nvPr"),
            );
            content += &element("p:grpSpPr", String::new(), transform(t));
            for child in children {
                content += &object(child);
            }
        }
    }
    element(tag, String::new(), content)
}

/// Validate supported authoring constructs and compile in source order (back to front).
pub fn compile_slide(slide: &Slide) -> Result<Vec<u8>> {
    validate(slide)?;
    let root_nv = element(
        "p:nvGrpSpPr",
        String::new(),
        element("p:cNvPr", attr("id", 1) + &attr("name", ""), String::new())
            + &empty("p:cNvGrpSpPr")
            + &empty("p:nvPr"),
    );
    let tree =
        root_nv + &empty("p:grpSpPr") + &slide.objects.iter().map(object).collect::<String>();
    let content = element(
        "p:cSld",
        String::new(),
        element("p:spTree", String::new(), tree),
    ) + &element("p:clrMapOvr", String::new(), empty("a:masterClrMapping"));
    Ok(
        ("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>".to_string()
            + &element(
                "p:sld",
                attr("xmlns:a", A) + &attr("xmlns:p", P) + &attr("xmlns:r", R),
                content,
            ))
            .into_bytes(),
    )
}

fn validate(slide: &Slide) -> Result<()> {
    let mut keys = BTreeSet::new();
    let mut ids = BTreeSet::from([1]);
    fn visit(
        objects: &[Object],
        keys: &mut BTreeSet<String>,
        ids: &mut BTreeSet<u32>,
        depth: usize,
    ) -> Result<()> {
        if depth > 64 {
            return Err(authored_error("slide", "group nesting exceeds 64"));
        }
        for o in objects {
            if o.key.is_empty() || !keys.insert(o.key.clone()) {
                return Err(authored_error(&o.key, "empty or duplicate source key"));
            }
            if o.id == 0 || !ids.insert(o.id) {
                return Err(authored_error(&o.key, "zero or duplicate native object ID"));
            }
            if let Some(m) = &o.morph_name
                && !m.starts_with("!!")
            {
                return Err(authored_error(
                    &o.key,
                    "Morph matching name must start with !!",
                ));
            }
            match &o.kind {
                ObjectKind::Group {
                    transform,
                    children,
                } => {
                    check_transform(transform, &o.key)?;
                    if transform
                        .child
                        .as_ref()
                        .is_none_or(|c| c.width.0 <= 0 || c.height.0 <= 0)
                    {
                        return Err(authored_error(
                            &o.key,
                            "group requires a nonzero child coordinate space",
                        ));
                    }
                    visit(children, keys, ids, depth + 1)?;
                }
                ObjectKind::Shape {
                    properties, text, ..
                } => {
                    check_properties(properties, &o.key)?;
                    if let Some(text) = text
                        && text.paragraphs.is_empty()
                    {
                        return Err(authored_error(
                            &o.key,
                            "text body requires at least one paragraph",
                        ));
                    }
                }
                ObjectKind::Picture {
                    properties,
                    relationship,
                    ..
                } => {
                    check_properties(properties, &o.key)?;
                    if relationship.is_empty() {
                        return Err(authored_error(&o.key, "picture requires relationship ID"));
                    }
                }
                ObjectKind::Connector { properties, .. } => check_properties(properties, &o.key)?,
            }
        }
        Ok(())
    }
    visit(&slide.objects, &mut keys, &mut ids, 0)?;
    fn endpoints(objects: &[Object], ids: &BTreeSet<u32>) -> Result<()> {
        for o in objects {
            if let ObjectKind::Connector { start, end, .. } = &o.kind {
                for c in [start, end].into_iter().flatten() {
                    if !ids.contains(&c.object_id) || c.object_id == 1 {
                        return Err(authored_error(
                            &o.key,
                            "connector endpoint object does not exist",
                        ));
                    }
                }
            }
            if let ObjectKind::Group { children, .. } = &o.kind {
                endpoints(children, ids)?;
            }
        }
        Ok(())
    }
    endpoints(&slide.objects, &ids)?;
    // Validate every string before XML emission, including private source text.
    let value = serde_json::to_value(slide).map_err(|e| authored_error("slide", &e.to_string()))?;
    fn colors(v: &serde_json::Value, key: &str) -> Result<()> {
        match v {
            serde_json::Value::Array(a) => {
                for v in a {
                    colors(v, key)?;
                }
            }
            serde_json::Value::Object(o) => {
                let key = o.get("key").and_then(|v| v.as_str()).unwrap_or(key);
                let kind = o.get("type").and_then(|v| v.as_str());
                if matches!(kind, Some("rgb" | "scheme")) {
                    let value = o.get("value").and_then(|v| v.as_str()).unwrap_or_default();
                    let valid = if kind == Some("rgb") {
                        value.len() == 6 && value.bytes().all(|b| b.is_ascii_hexdigit())
                    } else {
                        [
                            "bg1", "tx1", "bg2", "tx2", "dk1", "dk2", "lt1", "lt2", "accent1",
                            "accent2", "accent3", "accent4", "accent5", "accent6", "hlink",
                            "folHlink", "phClr",
                        ]
                        .contains(&value)
                    };
                    if !valid
                        || o.get("alpha")
                            .and_then(|v| v.as_u64())
                            .is_some_and(|v| v > 100000)
                    {
                        return Err(authored_error(key, "invalid RGB/scheme color or alpha"));
                    }
                }
                for v in o.values() {
                    colors(v, key)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    colors(&value, "slide")?;
    fn strings(v: &serde_json::Value) -> bool {
        match v {
            serde_json::Value::String(s) => s.chars().all(|c| matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}')),
            serde_json::Value::Array(a) => a.iter().all(strings),
            serde_json::Value::Object(o) => o.values().all(strings),
            _ => true,
        }
    }
    if !strings(&value) {
        return Err(authored_error(
            "slide",
            "text contains forbidden XML character",
        ));
    }
    Ok(())
}
fn check_transform(t: &Transform, key: &str) -> Result<()> {
    if t.width.0 < 0 || t.height.0 < 0 {
        return Err(authored_error(key, "negative extent"));
    }
    Ok(())
}
fn check_properties(p: &Properties, key: &str) -> Result<()> {
    if let Some(t) = &p.transform {
        check_transform(t, key)?;
    }
    if let Some(g) = &p.geometry
        && !["rect", "roundRect", "line"].contains(&g.preset.as_str())
    {
        return Err(authored_error(key, "unsupported geometry preset"));
    }
    Ok(())
}
