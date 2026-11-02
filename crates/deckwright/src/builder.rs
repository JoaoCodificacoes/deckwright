//! Compile bounded typed static content into native editable DrawingML.
//! No source slide or shape XML is accepted by this API.
use crate::{Result, model::*, units::Emu};
use std::collections::BTreeSet;

const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const P14: &str = "http://schemas.microsoft.com/office/powerpoint/2010/main";
const PATH_FILL_MODES: &[&str] = &[
    "none",
    "norm",
    "lighten",
    "lightenLess",
    "darken",
    "darkenLess",
];
/// ECMA-376 ST_PresetPatternVal names.
const PATTERN_PRESETS: &[&str] = &[
    "cross",
    "dashDnDiag",
    "dashHorz",
    "dashUpDiag",
    "dashVert",
    "diagBrick",
    "diagCross",
    "divot",
    "dkDnDiag",
    "dkHorz",
    "dkUpDiag",
    "dkVert",
    "dnDiag",
    "dotDmnd",
    "dotGrid",
    "horz",
    "horzBrick",
    "lgCheck",
    "lgConfetti",
    "lgGrid",
    "ltDnDiag",
    "ltHorz",
    "ltUpDiag",
    "ltVert",
    "narHorz",
    "narVert",
    "openDmnd",
    "pct10",
    "pct20",
    "pct25",
    "pct30",
    "pct40",
    "pct5",
    "pct50",
    "pct60",
    "pct70",
    "pct75",
    "pct80",
    "pct90",
    "plaid",
    "shingle",
    "smCheck",
    "smConfetti",
    "smGrid",
    "solidDmnd",
    "sphere",
    "trellis",
    "upDiag",
    "vert",
    "wave",
    "weave",
    "wdDnDiag",
    "wdUpDiag",
    "zigZag",
];
/// ST_TextAutonumberScheme values, in schema order.
const AUTONUMBER_SCHEMES: &[&str] = &[
    "alphaLcParenBoth",
    "alphaLcParenR",
    "alphaLcPeriod",
    "alphaUcParenBoth",
    "alphaUcParenR",
    "alphaUcPeriod",
    "arabic1Minus",
    "arabic2Minus",
    "arabicDbPeriod",
    "arabicDbPlain",
    "arabicParenBoth",
    "arabicParenR",
    "arabicPeriod",
    "arabicPlain",
    "circleNumDbPlain",
    "circleNumWdBlackPlain",
    "circleNumWdWhitePlain",
    "ea1ChsPeriod",
    "ea1ChsPlain",
    "ea1ChtPeriod",
    "ea1ChtPlain",
    "ea1JpnChsDbPeriod",
    "ea1JpnKorPeriod",
    "ea1JpnKorPlain",
    "hebrew2Minus",
    "hindiAlpha1Period",
    "hindiAlphaPeriod",
    "hindiNumParenR",
    "hindiNumPeriod",
    "romanLcParenBoth",
    "romanLcParenR",
    "romanLcPeriod",
    "romanUcParenBoth",
    "romanUcParenR",
    "romanUcPeriod",
    "thaiAlphaParenBoth",
    "thaiAlphaParenR",
    "thaiAlphaPeriod",
    "thaiNumParenBoth",
    "thaiNumParenR",
    "thaiNumPeriod",
];
/// ECMA-376 ST_ShapeType preset names, sorted for binary_search.
#[rustfmt::skip]
const PRESET_GEOMETRIES: &[&str] = &[
    "accentBorderCallout1", "accentBorderCallout2", "accentBorderCallout3", "accentCallout1",
    "accentCallout2", "accentCallout3", "actionButtonBackPrevious", "actionButtonBeginning",
    "actionButtonBlank", "actionButtonDocument", "actionButtonEnd", "actionButtonForwardNext",
    "actionButtonHelp", "actionButtonHome", "actionButtonInformation", "actionButtonMovie",
    "actionButtonReturn", "actionButtonSound", "arc", "bentArrow",
    "bentConnector2", "bentConnector3", "bentConnector4", "bentConnector5",
    "bentUpArrow", "bevel", "blockArc", "borderCallout1",
    "borderCallout2", "borderCallout3", "bracePair", "bracketPair",
    "callout1", "callout2", "callout3", "can",
    "chartPlus", "chartStar", "chartX", "chevron",
    "chord", "circularArrow", "cloud", "cloudCallout",
    "corner", "cornerTabs", "cube", "curvedConnector2",
    "curvedConnector3", "curvedConnector4", "curvedConnector5", "curvedDownArrow",
    "curvedLeftArrow", "curvedRightArrow", "curvedUpArrow", "decagon",
    "diagStripe", "diamond", "dodecagon", "donut",
    "doubleWave", "downArrow", "downArrowCallout", "ellipse",
    "ellipseRibbon", "ellipseRibbon2",
    "flowChartAlternateProcess", "flowChartCollate", "flowChartConnector", "flowChartDecision",
    "flowChartDelay", "flowChartDisplay", "flowChartDocument", "flowChartExtract",
    "flowChartInputOutput", "flowChartInternalStorage", "flowChartMagneticDisk", "flowChartMagneticDrum",
    "flowChartMagneticTape", "flowChartManualInput", "flowChartManualOperation", "flowChartMerge",
    "flowChartMultidocument", "flowChartOfflineStorage", "flowChartOffpageConnector", "flowChartOnlineStorage",
    "flowChartOr", "flowChartPredefinedProcess", "flowChartPreparation", "flowChartProcess",
    "flowChartPunchedCard", "flowChartPunchedTape", "flowChartSort", "flowChartSummingJunction",
    "flowChartTerminator", "foldedCorner", "frame", "funnel",
    "gear6", "gear9", "halfFrame", "heart",
    "heptagon", "hexagon", "homePlate", "horizontalScroll",
    "irregularSeal1", "irregularSeal2", "leftArrow", "leftArrowCallout",
    "leftBrace", "leftBracket", "leftCircularArrow", "leftRightArrow",
    "leftRightArrowCallout", "leftRightCircularArrow", "leftRightRibbon", "leftRightUpArrow",
    "leftUpArrow", "lightningBolt", "line", "lineInv",
    "mathDivide", "mathEqual", "mathMinus", "mathMultiply",
    "mathNotEqual", "mathPlus", "moon", "noSmoking",
    "nonIsoscelesTrapezoid", "notchedRightArrow", "octagon", "parallelogram",
    "pentagon", "pie", "pieWedge", "plaque",
    "plaqueTabs", "plus", "quadArrow", "quadArrowCallout",
    "rect", "ribbon", "ribbon2", "rightArrow",
    "rightArrowCallout", "rightBrace", "rightBracket", "round1Rect",
    "round2DiagRect", "round2SameRect", "roundRect", "rtTriangle",
    "smileyFace", "snip1Rect", "snip2DiagRect", "snip2SameRect",
    "snipRoundRect", "squareTabs", "star10", "star12",
    "star16", "star24", "star32", "star4",
    "star5", "star6", "star7", "star8",
    "straightConnector1", "stripedRightArrow", "sun", "swooshArrow",
    "teardrop", "trapezoid", "triangle", "upArrow",
    "upArrowCallout", "upDownArrow", "upDownArrowCallout", "uturnArrow",
    "verticalScroll", "wave", "wedgeEllipseCallout", "wedgeRectCallout",
    "wedgeRoundRectCallout",
];

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
pub(crate) fn attr(name: &str, value: impl std::fmt::Display) -> String {
    format!(" {name}=\"{}\"", escape(&value.to_string()))
}
pub(crate) fn opt<T: std::fmt::Display>(name: &str, value: Option<T>) -> String {
    value.map(|v| attr(name, v)).unwrap_or_default()
}
pub(crate) fn element(name: &str, attrs: String, children: String) -> String {
    if children.is_empty() {
        format!("<{name}{attrs}/>")
    } else {
        format!("<{name}{attrs}>{children}</{name}>")
    }
}
pub(crate) fn empty(name: &str) -> String {
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
pub(crate) fn fill(value: &Fill) -> String {
    match value {
        Fill::None => empty("a:noFill"),
        Fill::Solid { color: c } => element("a:solidFill", String::new(), color(c)),
        Fill::Gradient {
            stops,
            angle,
            path,
            rotate_with_shape,
        } => {
            let stops = stops
                .iter()
                .map(|s| element("a:gs", attr("pos", s.position), color(&s.color)))
                .collect::<String>();
            // Validation guarantees exactly one of angle or path.
            let direction = match (angle, path) {
                (Some(angle), _) => element(
                    "a:lin",
                    attr("ang", angle) + &attr("scaled", 0),
                    String::new(),
                ),
                (None, Some(path)) => element(
                    "a:path",
                    attr("path", path),
                    element(
                        "a:fillToRect",
                        attr("l", 50000)
                            + &attr("t", 50000)
                            + &attr("r", 50000)
                            + &attr("b", 50000),
                        String::new(),
                    ),
                ),
                (None, None) => String::new(),
            };
            element(
                "a:gradFill",
                attr("rotWithShape", u8::from(*rotate_with_shape)),
                element("a:gsLst", String::new(), stops) + &direction,
            )
        }
        Fill::Pattern {
            preset,
            foreground,
            background,
        } => element(
            "a:pattFill",
            attr("prst", preset),
            element("a:fgClr", String::new(), color(foreground))
                + &element("a:bgClr", String::new(), color(background)),
        ),
        Fill::Picture { relationship } => element(
            "a:blipFill",
            String::new(),
            element("a:blip", attr("r:embed", relationship), String::new())
                + &element("a:stretch", String::new(), empty("a:fillRect")),
        ),
    }
}
fn background(value: &Background) -> String {
    let inner = match value {
        Background::Fill { fill: f } => {
            element("p:bgPr", String::new(), fill(f) + &empty("a:effectLst"))
        }
        Background::Reference { index, color: c } => {
            element("p:bgRef", attr("idx", index), color(c))
        }
    };
    element("p:bg", String::new(), inner)
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
fn point(x: Emu, y: Emu) -> String {
    element("a:pt", attr("x", x.0) + &attr("y", y.0), String::new())
}
fn command(c: &PathCommand) -> String {
    match *c {
        PathCommand::MoveTo { x, y } => element("a:moveTo", String::new(), point(x, y)),
        PathCommand::LineTo { x, y } => element("a:lnTo", String::new(), point(x, y)),
        PathCommand::QuadTo { x1, y1, x, y } => {
            element("a:quadBezTo", String::new(), point(x1, y1) + &point(x, y))
        }
        PathCommand::CubicTo {
            x1,
            y1,
            x2,
            y2,
            x,
            y,
        } => element(
            "a:cubicBezTo",
            String::new(),
            point(x1, y1) + &point(x2, y2) + &point(x, y),
        ),
        PathCommand::ArcTo {
            width_radius,
            height_radius,
            start_angle,
            swing_angle,
        } => element(
            "a:arcTo",
            attr("wR", width_radius.0)
                + &attr("hR", height_radius.0)
                + &attr("stAng", start_angle)
                + &attr("swAng", swing_angle),
            String::new(),
        ),
        PathCommand::Close {} => empty("a:close"),
    }
}
fn custom_geometry(c: &CustomGeometry) -> String {
    let paths = c
        .paths
        .iter()
        .map(|p| {
            let attrs = attr("w", p.width.0)
                + &attr("h", p.height.0)
                + &opt("fill", p.fill.as_ref())
                + &if p.stroke {
                    String::new()
                } else {
                    attr("stroke", 0)
                };
            element("a:path", attrs, p.commands.iter().map(command).collect())
        })
        .collect::<String>();
    let rect = attr("l", "l") + &attr("t", "t") + &attr("r", "r") + &attr("b", "b");
    element(
        "a:custGeom",
        String::new(),
        empty("a:avLst")
            + &empty("a:gdLst")
            + &empty("a:ahLst")
            + &empty("a:cxnLst")
            + &element("a:rect", rect, String::new())
            + &element("a:pathLst", String::new(), paths),
    )
}
fn properties(p: &Properties) -> String {
    let mut content = p.transform.as_ref().map(transform).unwrap_or_default();
    if let Some(c) = &p.custom {
        content += &custom_geometry(c);
    }
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
        content += &line_with_tag("a:ln", l);
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
/// a:ln and the table cell borders (lnL, lnR, lnT, lnB) share one content model.
pub(crate) fn line_with_tag(tag: &str, l: &Line) -> String {
    // Schema order inside a:ln: fill, prstDash, headEnd, tailEnd.
    let mut children = l.fill.as_ref().map(fill).unwrap_or_default();
    if let Some(dash) = &l.dash {
        children += &element("a:prstDash", attr("val", dash), String::new());
    }
    for (end_tag, end) in [("a:headEnd", &l.head_end), ("a:tailEnd", &l.tail_end)] {
        if let Some(end) = end {
            children += &element(
                end_tag,
                attr("type", &end.kind)
                    + &opt("w", end.width.as_ref())
                    + &opt("len", end.length.as_ref()),
                String::new(),
            );
        }
    }
    element(tag, opt("w", l.width.map(|v| v.0)), children)
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
    text_with("p:txBody", t)
}
/// Shared by slide text (`p:txBody`) and table cells (`a:txBody`): same content, different tag.
pub(crate) fn text_with(tag: &str, t: &TextBody) -> String {
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
            if let Some(b) = &pp.bullet {
                spacing += &bullet(b);
            }
            let attrs = opt("algn", pp.alignment.as_ref())
                + &opt("lvl", pp.level)
                + &opt("marL", pp.margin_left.map(|v| v.0))
                + &opt("indent", pp.indent.map(|v| v.0));
            paragraph += &element("a:pPr", attrs, spacing);
        }
        for r in &p.runs {
            let mut run = String::new();
            if r.style.is_some() || r.hyperlink.is_some() {
                let style = r.style.clone().unwrap_or_default();
                run += &run_properties("a:rPr", &style, r.hyperlink.as_ref());
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
        if let Some(end) = &p.end_style {
            paragraph += &run_properties("a:endParaRPr", end, None);
        }
        content += &element("a:p", String::new(), paragraph);
    }
    element(tag, String::new(), content)
}
/// Paragraph bullet children, in schema order: buClr, buSzPct, buFont, then the bullet.
fn bullet(b: &Bullet) -> String {
    match b {
        Bullet::None => empty("a:buNone"),
        Bullet::Char {
            char: glyph,
            font,
            color: bullet_color,
            size_percent,
        } => {
            let mut children = bullet_color
                .as_ref()
                .map(|c| element("a:buClr", String::new(), color(c)))
                .unwrap_or_default();
            if let Some(size) = size_percent {
                // ST_TextBulletSizePercent is in thousandths of a percent.
                children += &element("a:buSzPct", attr("val", size * 1000), String::new());
            }
            if let Some(font) = font {
                children += &element("a:buFont", attr("typeface", font), String::new());
            }
            children + &element("a:buChar", attr("char", glyph), String::new())
        }
        Bullet::AutoNumber { scheme, start_at } => element(
            "a:buAutoNum",
            attr("type", scheme) + &opt("startAt", *start_at),
            String::new(),
        ),
    }
}
fn hyperlink(tag: &str, h: &Hyperlink) -> String {
    element(
        tag,
        attr("r:id", &h.relationship)
            + &opt("action", h.action.as_ref())
            + &opt("tooltip", h.tooltip.as_ref()),
        String::new(),
    )
}
/// Shared by a:rPr and a:endParaRPr: identical attributes and children, different tag.
/// Only a:rPr carries a hyperlink; hlinkClick follows latin in schema order.
fn run_properties(tag: &str, s: &RunStyle, link: Option<&Hyperlink>) -> String {
    let attrs = opt("sz", s.size)
        + &opt("b", s.bold.map(u8::from))
        + &opt("i", s.italic.map(u8::from))
        + &opt("u", s.underline.as_ref());
    let mut children = s
        .color
        .as_ref()
        .map(|c| element("a:solidFill", String::new(), color(c)))
        .unwrap_or_default();
    if let Some(font) = &s.font {
        children += &element("a:latin", attr("typeface", font), String::new());
    }
    if let Some(link) = link {
        children += &hyperlink("a:hlinkClick", link);
    }
    element(tag, attrs, children)
}
fn common(o: &Object) -> String {
    let name = o.morph_name.as_ref().unwrap_or(&o.name);
    let attrs = attr("id", o.id) + &attr("name", name) + &opt("descr", o.description.as_ref());
    let key = element(
        "dw:source",
        attr("xmlns:dw", "urn:deckwright:source:v1") + &attr("key", &o.key),
        String::new(),
    );
    // Object hyperlinks come first in cNvPr. A video's media click action follows them;
    // validation rejects a slide that has both.
    let hyperlink_child = o
        .hyperlink
        .as_ref()
        .map(|h| hyperlink("a:hlinkClick", h))
        .unwrap_or_default();
    // A video picture's click action is the media playback action (before extLst).
    let link = match &o.kind {
        ObjectKind::Picture { media: Some(_), .. } => element(
            "a:hlinkClick",
            attr("r:id", "") + &attr("action", "ppaction://media"),
            String::new(),
        ),
        _ => String::new(),
    };
    element(
        "p:cNvPr",
        attrs,
        hyperlink_child
            + &link
            + &element(
                "a:extLst",
                String::new(),
                element("a:ext", attr("uri", "urn:deckwright:source:v1"), key),
            ),
    )
}
fn picture_nv_pr(media: &Option<MediaLink>) -> String {
    match media {
        None => empty("p:nvPr"),
        Some(m) => {
            let media_ext = element(
                "p14:media",
                attr("xmlns:p14", P14) + &attr("r:embed", &m.media_relationship),
                String::new(),
            );
            let ext = element(
                "p:ext",
                attr("uri", "{DAA4B4D4-6D71-4841-9C94-3DE7FCFB9230}"),
                media_ext,
            );
            element(
                "p:nvPr",
                String::new(),
                element(
                    "a:videoFile",
                    attr("r:link", &m.video_relationship),
                    String::new(),
                ) + &element("p:extLst", String::new(), ext),
            )
        }
    }
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
            media,
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
                nv + &element("p:cNvPicPr", String::new(), locks) + &picture_nv_pr(media),
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
        ObjectKind::Table {
            transform: t,
            table,
        } => {
            tag = "p:graphicFrame";
            content = element(
                "p:nvGraphicFramePr",
                String::new(),
                nv + &element(
                    "p:cNvGraphicFramePr",
                    String::new(),
                    element("a:graphicFrameLocks", attr("noGrp", 1), String::new()),
                ) + &empty("p:nvPr"),
            );
            content += &crate::table::frame_content(t, table);
        }
        ObjectKind::Chart {
            transform: t,
            relationship,
        } => {
            tag = "p:graphicFrame";
            content = element(
                "p:nvGraphicFramePr",
                String::new(),
                nv + &element(
                    "p:cNvGraphicFramePr",
                    String::new(),
                    element("a:graphicFrameLocks", attr("noGrp", 1), String::new()),
                ) + &empty("p:nvPr"),
            );
            content += &crate::chart::frame_content(t, relationship);
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
        slide
            .background
            .as_ref()
            .map(background)
            .unwrap_or_default()
            + &element("p:spTree", String::new(), tree),
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
            if let Some(h) = &o.hyperlink {
                check_hyperlink(h, &o.key)?;
                if matches!(&o.kind, ObjectKind::Picture { media: Some(_), .. }) {
                    return Err(authored_error(
                        &o.key,
                        "video picture media link cannot be combined with a hyperlink",
                    ));
                }
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
                    properties,
                    text,
                    placeholder,
                    style,
                    ..
                } => {
                    check_properties(properties, &o.key)?;
                    if let Some(text) = text {
                        check_text(text, &o.key)?;
                    }
                    if let Some(style) = style {
                        check_style(style, &o.key)?;
                    }
                    if let Some(ph) = placeholder {
                        member(
                            &ph.kind,
                            &[
                                "title", "body", "ctrTitle", "subTitle", "dt", "sldNum", "ftr",
                                "hdr", "obj", "chart", "tbl", "clipArt", "dgm", "media", "sldImg",
                                "pic",
                            ],
                            &o.key,
                            "placeholder type",
                        )?;
                        if let Some(size) = &ph.size {
                            member(
                                size,
                                &["full", "half", "quarter"],
                                &o.key,
                                "placeholder size",
                            )?;
                        }
                    }
                }
                ObjectKind::Picture {
                    properties,
                    relationship,
                    media,
                    ..
                } => {
                    check_properties(properties, &o.key)?;
                    if relationship.is_empty() {
                        return Err(authored_error(&o.key, "picture requires relationship ID"));
                    }
                    if let Some(m) = media
                        && (m.video_relationship.is_empty() || m.media_relationship.is_empty())
                    {
                        return Err(authored_error(
                            &o.key,
                            "video picture requires video and media relationship IDs",
                        ));
                    }
                }
                ObjectKind::Connector {
                    properties, style, ..
                } => {
                    check_properties(properties, &o.key)?;
                    if let Some(style) = style {
                        check_style(style, &o.key)?;
                    }
                }
                ObjectKind::Table { transform, table } => {
                    crate::table::validate(transform, table, &o.key)?;
                }
                ObjectKind::Chart {
                    transform,
                    relationship,
                } => {
                    crate::chart::validate_frame(transform, relationship, &o.key)?;
                }
            }
        }
        Ok(())
    }
    visit(&slide.objects, &mut keys, &mut ids, 0)?;
    if let Some(background) = &slide.background {
        check_background(background)?;
    }
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
pub(crate) fn check_transform(t: &Transform, key: &str) -> Result<()> {
    for value in [t.x.0, t.y.0] {
        range(
            value,
            -27273042329600,
            27273042316900,
            key,
            "transform coordinate",
        )?;
    }
    for value in [t.width.0, t.height.0] {
        range(value, 0, i32::MAX.into(), key, "transform extent")?;
    }
    if let Some(c) = &t.child {
        for value in [c.x.0, c.y.0] {
            range(
                value,
                -27273042329600,
                27273042316900,
                key,
                "child coordinate",
            )?;
        }
        for value in [c.width.0, c.height.0] {
            range(value, 0, i32::MAX.into(), key, "child extent")?;
        }
    }
    Ok(())
}
pub(crate) fn check_properties(p: &Properties, key: &str) -> Result<()> {
    if let Some(t) = &p.transform {
        check_transform(t, key)?;
        if t.child.is_some() {
            return Err(authored_error(
                key,
                "child coordinate space is supported only on groups",
            ));
        }
    }
    if p.geometry.is_some() && p.custom.is_some() {
        return Err(authored_error(
            key,
            "custom and preset geometry are mutually exclusive",
        ));
    }
    if let Some(g) = &p.geometry
        && PRESET_GEOMETRIES.binary_search(&g.preset.as_str()).is_err()
    {
        return Err(authored_error(key, "unsupported geometry preset"));
    }
    if let Some(c) = &p.custom {
        check_custom(c, key)?;
    }
    if let Some(fill) = &p.fill {
        check_fill(fill, key, false)?;
    }
    if let Some(width) = p.line.as_ref().and_then(|l| l.width) {
        range(width.0, 0, 20116800, key, "line width")?;
    }
    if let Some(line) = &p.line {
        // a:ln allows noFill, solidFill, gradFill and pattFill, but not blipFill.
        if let Some(fill) = &line.fill {
            check_fill(fill, key, true)?;
        }
        if let Some(dash) = &line.dash {
            member(
                dash,
                &[
                    "solid",
                    "dot",
                    "dash",
                    "lgDash",
                    "dashDot",
                    "lgDashDot",
                    "lgDashDotDot",
                    "sysDash",
                    "sysDot",
                    "sysDashDot",
                    "sysDashDotDot",
                ],
                key,
                "line dash",
            )?;
        }
        for end in [&line.head_end, &line.tail_end].into_iter().flatten() {
            member(
                &end.kind,
                &["none", "triangle", "stealth", "diamond", "oval", "arrow"],
                key,
                "line end kind",
            )?;
            if let Some(width) = &end.width {
                member(width, &["sm", "med", "lg"], key, "line end width")?;
            }
            if let Some(length) = &end.length {
                member(length, &["sm", "med", "lg"], key, "line end length")?;
            }
        }
    }
    if let Some(effects) = &p.effects {
        for shadow in effects {
            range(shadow.blur.0, 0, i32::MAX.into(), key, "shadow blur")?;
            range(
                shadow.distance.0,
                0,
                i32::MAX.into(),
                key,
                "shadow distance",
            )?;
            range(
                shadow.direction.into(),
                0,
                21599999,
                key,
                "shadow direction",
            )?;
            member(
                &shadow.alignment,
                &["tl", "t", "tr", "l", "ctr", "r", "bl", "b", "br"],
                key,
                "shadow alignment",
            )?;
        }
    }
    Ok(())
}

fn check_fill(fill: &Fill, key: &str, on_line: bool) -> Result<()> {
    match fill {
        Fill::None | Fill::Solid { .. } => Ok(()),
        Fill::Gradient {
            stops, angle, path, ..
        } => {
            if stops.len() < 2 {
                return Err(authored_error(key, "gradient requires at least two stops"));
            }
            for stop in stops {
                range(
                    stop.position.into(),
                    0,
                    100000,
                    key,
                    "gradient stop position",
                )?;
            }
            if stops.windows(2).any(|w| w[0].position > w[1].position) {
                return Err(authored_error(
                    key,
                    "gradient stop positions must be ascending",
                ));
            }
            match (angle, path) {
                (Some(angle), None) => range(i64::from(*angle), 0, 21599999, key, "gradient angle"),
                (None, Some(path)) => {
                    member(path, &["circle", "rect", "shape"], key, "gradient path")
                }
                _ => Err(authored_error(
                    key,
                    "gradient requires exactly one of angle or path",
                )),
            }
        }
        Fill::Pattern { preset, .. } => member(preset, PATTERN_PRESETS, key, "pattern preset"),
        Fill::Picture { relationship } => {
            if on_line {
                return Err(authored_error(key, "picture fill is not allowed on a line"));
            }
            if relationship.is_empty() {
                return Err(authored_error(key, "picture fill requires relationship ID"));
            }
            Ok(())
        }
    }
}
fn check_background(background: &Background) -> Result<()> {
    match background {
        Background::Fill { fill } => check_fill(fill, "background", false),
        Background::Reference { index, .. } => {
            if matches!(index, 1..=3 | 1001..=1003) {
                Ok(())
            } else {
                Err(authored_error(
                    "background",
                    "bgRef idx must be 1..=3 or 1001..=1003",
                ))
            }
        }
    }
}

/// Paths use ST_PositiveCoordinate for their size and ST_Coordinate for points.
fn check_custom(c: &CustomGeometry, key: &str) -> Result<()> {
    if c.paths.is_empty() {
        return Err(authored_error(
            key,
            "custom geometry requires at least one path",
        ));
    }
    for path in &c.paths {
        let (w, h) = (path.width.0, path.height.0);
        range(w, 1, 27273042316900, key, "path width")?;
        range(h, 1, 27273042316900, key, "path height")?;
        if let Some(fill) = &path.fill {
            member(fill, PATH_FILL_MODES, key, "path fill mode")?;
        }
        if !matches!(path.commands.first(), Some(PathCommand::MoveTo { .. })) {
            return Err(authored_error(key, "custom path must start with moveTo"));
        }
        let point = |x: Emu, y: Emu| -> Result<()> {
            range(x.0, 0, w, key, "path x coordinate")?;
            range(y.0, 0, h, key, "path y coordinate")
        };
        for command in &path.commands {
            match *command {
                PathCommand::MoveTo { x, y } | PathCommand::LineTo { x, y } => point(x, y)?,
                PathCommand::QuadTo { x1, y1, x, y } => {
                    point(x1, y1)?;
                    point(x, y)?;
                }
                PathCommand::CubicTo {
                    x1,
                    y1,
                    x2,
                    y2,
                    x,
                    y,
                } => {
                    point(x1, y1)?;
                    point(x2, y2)?;
                    point(x, y)?;
                }
                PathCommand::ArcTo {
                    width_radius,
                    height_radius,
                    ..
                } => {
                    range(width_radius.0, 0, w, key, "arc width radius")?;
                    range(height_radius.0, 0, h, key, "arc height radius")?;
                }
                PathCommand::Close {} => {}
            }
        }
    }
    Ok(())
}

// Attribute domains follow the official Open-XML-SDK DrawingML/PresentationML
// schema validators. Integer model fields already constrain angles, percentages,
// IDs and connection indices to their corresponding signed/unsigned 32-bit types.
pub(crate) fn range(value: i64, min: i64, max: i64, key: &str, field: &str) -> Result<()> {
    if !(min..=max).contains(&value) {
        return Err(authored_error(
            key,
            &format!("{field} outside {min}..={max}: {value}"),
        ));
    }
    Ok(())
}
pub(crate) fn member(value: &str, allowed: &[&str], key: &str, field: &str) -> Result<()> {
    if !allowed.contains(&value) {
        return Err(authored_error(
            key,
            &format!("unsupported {field}: {value}"),
        ));
    }
    Ok(())
}
fn check_style(style: &StyleReferences, key: &str) -> Result<()> {
    for reference in [&style.line, &style.fill, &style.effect] {
        if reference.index.parse::<u32>().is_err() {
            return Err(authored_error(
                key,
                "style matrix index must be an unsigned 32-bit integer",
            ));
        }
    }
    member(
        &style.font.index,
        &["major", "minor", "none"],
        key,
        "font style index",
    )
}
fn check_run_style(style: &RunStyle, key: &str) -> Result<()> {
    if let Some(size) = style.size {
        range(size.into(), 100, 400000, key, "font size")?;
    }
    if let Some(underline) = &style.underline {
        member(
            underline,
            &[
                "none",
                "words",
                "sng",
                "dbl",
                "heavy",
                "dotted",
                "dottedHeavy",
                "dash",
                "dashHeavy",
                "dashLong",
                "dashLongHeavy",
                "dotDash",
                "dotDashHeavy",
                "dotDotDash",
                "dotDotDashHeavy",
                "wavy",
                "wavyHeavy",
                "wavyDbl",
            ],
            key,
            "underline",
        )?;
    }
    Ok(())
}
fn check_hyperlink(link: &Hyperlink, key: &str) -> Result<()> {
    if link.relationship.is_empty() {
        return Err(authored_error(key, "hyperlink requires relationship ID"));
    }
    Ok(())
}
fn check_bullet(bullet: &Bullet, key: &str) -> Result<()> {
    match bullet {
        Bullet::None => {}
        Bullet::Char {
            char: glyph,
            size_percent,
            ..
        } => {
            if glyph.is_empty() {
                return Err(authored_error(key, "bullet character must not be empty"));
            }
            if let Some(size) = size_percent {
                range((*size).into(), 25, 400, key, "bullet size percent")?;
            }
        }
        Bullet::AutoNumber { scheme, start_at } => {
            member(scheme, AUTONUMBER_SCHEMES, key, "auto number scheme")?;
            if let Some(start) = start_at {
                range((*start).into(), 1, 32767, key, "bullet start number")?;
            }
        }
    }
    Ok(())
}
pub(crate) fn check_text(text: &TextBody, key: &str) -> Result<()> {
    if text.paragraphs.is_empty() {
        return Err(authored_error(
            key,
            "text body requires at least one paragraph",
        ));
    }
    let body = &text.body;
    if let Some(wrap) = &body.wrap {
        member(wrap, &["none", "square"], key, "text wrap")?;
    }
    if let Some(anchor) = &body.anchor {
        member(anchor, &["t", "ctr", "b"], key, "text anchor")?;
    }
    for inset in [body.left, body.right, body.top, body.bottom]
        .into_iter()
        .flatten()
    {
        range(inset.0, i32::MIN.into(), i32::MAX.into(), key, "text inset")?;
    }
    for paragraph in &text.paragraphs {
        if let Some(p) = &paragraph.properties {
            if let Some(alignment) = &p.alignment {
                member(
                    alignment,
                    &["l", "ctr", "r", "just", "justLow", "dist", "thaiDist"],
                    key,
                    "paragraph alignment",
                )?;
            }
            for spacing in [p.space_before, p.space_after].into_iter().flatten() {
                range(spacing.into(), 0, 158400, key, "paragraph spacing")?;
            }
            if let Some(level) = p.level {
                range(level.into(), 0, 8, key, "paragraph level")?;
            }
            // ST_TextMargin and ST_TextIndent schema domains.
            if let Some(margin) = p.margin_left {
                range(margin.0, 0, 51206400, key, "paragraph left margin")?;
            }
            if let Some(indent) = p.indent {
                range(indent.0, -51206400, 51206400, key, "paragraph indent")?;
            }
            if let Some(bullet) = &p.bullet {
                check_bullet(bullet, key)?;
            }
        }
        if let Some(end) = &paragraph.end_style {
            check_run_style(end, key)?;
        }
        for run in &paragraph.runs {
            if let Some(style) = &run.style {
                check_run_style(style, key)?;
            }
            if let Some(link) = &run.hyperlink {
                check_hyperlink(link, key)?;
            }
            if let Some(field) = &run.field {
                let bytes = field.id.as_bytes();
                let valid = bytes.len() == 38
                    && bytes[0] == b'{'
                    && bytes[37] == b'}'
                    && bytes.iter().enumerate().skip(1).take(36).all(|(i, b)| {
                        if [9, 14, 19, 24].contains(&i) {
                            *b == b'-'
                        } else {
                            b.is_ascii_digit() || (b'A'..=b'F').contains(b)
                        }
                    });
                if !valid {
                    return Err(authored_error(
                        key,
                        "field id must be a braced uppercase GUID",
                    ));
                }
            }
        }
    }
    Ok(())
}
