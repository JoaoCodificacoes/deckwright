//! Neutral blank PresentationML template, specified in `docs/decisions/template.md`.
//! Every byte comes from the constants below and the requested size, so two builds
//! with the same arguments are identical. Nothing external is read.
use crate::{Error, Limits, Package, Result, units::Emu};
use std::{
    collections::BTreeMap,
    io::{Cursor, Write},
    ops::RangeInclusive,
};

const MAX_SLIDES: usize = 1000;
const SIZE_BOUNDS: RangeInclusive<i64> = 914_400..=51_206_400;
const HEADER: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;
const NS: &str = concat!(
    r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
    r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#,
    r#"xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#
);
const OFFICE_RELS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PKG_RELS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const PML: &str = "application/vnd.openxmlformats-officedocument.presentationml";
const ROOT_TREE: &str = r#"<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>"#;
const FILL: &str = r#"<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>"#;
const PRES_PROPS: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<p:presentationPr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>"#
);
const VIEW_PROPS: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<p:viewPr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">"#,
    r#"<p:normalViewPr><p:restoredLeft sz="15620"/><p:restoredTop sz="94660"/></p:normalViewPr>"#,
    r#"<p:gridSpacing cx="72008" cy="72008"/></p:viewPr>"#
);
const TABLE_STYLES: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<a:tblStyleLst xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" def="{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}"/>"#
);
const CORE: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">"#,
    r#"<dc:title>Blank</dc:title><dc:creator>Deckwright</dc:creator></cp:coreProperties>"#
);

/// Build a neutral blank deck with `slides` empty slides on the `blank` layout.
/// `slides` must be 1..=1000; each size must be 914400..=51206400 EMU.
pub fn blank(slides: usize, width: Emu, height: Emu) -> Result<Package> {
    if !(1..=MAX_SLIDES).contains(&slides) {
        return Err(invalid(format!(
            "slides must be 1..={MAX_SLIDES}, got {slides}"
        )));
    }
    for (label, value) in [("width", width.0), ("height", height.0)] {
        if !SIZE_BOUNDS.contains(&value) {
            return Err(invalid(format!(
                "{label} must be {}..={} EMU, got {value}",
                SIZE_BOUNDS.start(),
                SIZE_BOUNDS.end()
            )));
        }
    }
    Package::from_bytes(
        encode(&parts(slides, width.0, height.0))?,
        Limits::default(),
    )
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidPart {
        part: "template".into(),
        message: message.into(),
    }
}

fn office(kind: &str) -> String {
    format!("{OFFICE_RELS}/{kind}")
}

fn rel(id: &str, ty: &str, target: &str) -> String {
    format!(r#"<Relationship Id="{id}" Type="{ty}" Target="{target}"/>"#)
}

fn rels(entries: &[String]) -> String {
    format!(
        r#"{HEADER}<Relationships xmlns="{PKG_RELS}">{}</Relationships>"#,
        entries.concat()
    )
}

fn parts(slides: usize, width: i64, height: i64) -> BTreeMap<String, String> {
    let mut p = BTreeMap::new();
    let fixed = [
        (
            "/ppt/presentation.xml",
            format!("{PML}.presentation.main+xml"),
        ),
        ("/ppt/presProps.xml", format!("{PML}.presProps+xml")),
        ("/ppt/viewProps.xml", format!("{PML}.viewProps+xml")),
        ("/ppt/tableStyles.xml", format!("{PML}.tableStyles+xml")),
        (
            "/ppt/theme/theme1.xml",
            "application/vnd.openxmlformats-officedocument.theme+xml".into(),
        ),
        (
            "/ppt/slideMasters/slideMaster1.xml",
            format!("{PML}.slideMaster+xml"),
        ),
        (
            "/ppt/slideLayouts/slideLayout1.xml",
            format!("{PML}.slideLayout+xml"),
        ),
        (
            "/docProps/core.xml",
            "application/vnd.openxmlformats-package.core-properties+xml".into(),
        ),
        (
            "/docProps/app.xml",
            "application/vnd.openxmlformats-officedocument.extended-properties+xml".into(),
        ),
    ];
    let overrides: String = fixed
        .into_iter()
        .map(|(name, ty)| format!(r#"<Override PartName="{name}" ContentType="{ty}"/>"#))
        .chain((1..=slides).map(|n| {
            format!(
                r#"<Override PartName="/ppt/slides/slide{n}.xml" ContentType="{PML}.slide+xml"/>"#
            )
        }))
        .collect();
    p.insert(
        "[Content_Types].xml".into(),
        format!(
            r#"{HEADER}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>{overrides}</Types>"#
        ),
    );
    p.insert(
        "_rels/.rels".into(),
        rels(&[
            rel("rId1", &office("officeDocument"), "ppt/presentation.xml"),
            rel(
                "rId2",
                &format!("{PKG_RELS}/metadata/core-properties"),
                "docProps/core.xml",
            ),
            rel("rId3", &office("extended-properties"), "docProps/app.xml"),
        ]),
    );
    p.insert("docProps/core.xml".into(), CORE.into());
    p.insert(
        "docProps/app.xml".into(),
        format!(
            r#"{HEADER}<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Application>Deckwright</Application><Slides>{slides}</Slides></Properties>"#
        ),
    );

    let ids: String = (1..=slides)
        .map(|n| format!(r#"<p:sldId id="{}" r:id="rId{}"/>"#, 255 + n, 5 + n))
        .collect();
    p.insert(
        "ppt/presentation.xml".into(),
        format!(
            r#"{HEADER}<p:presentation {NS} saveSubsetFonts="1"><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst><p:sldIdLst>{ids}</p:sldIdLst><p:sldSz cx="{width}" cy="{height}"/><p:notesSz cx="6858000" cy="9144000"/></p:presentation>"#
        ),
    );
    let mut pres = vec![
        rel(
            "rId1",
            &office("slideMaster"),
            "slideMasters/slideMaster1.xml",
        ),
        rel("rId2", &office("presProps"), "presProps.xml"),
        rel("rId3", &office("viewProps"), "viewProps.xml"),
        rel("rId4", &office("theme"), "theme/theme1.xml"),
        rel("rId5", &office("tableStyles"), "tableStyles.xml"),
    ];
    pres.extend((1..=slides).map(|n| {
        rel(
            &format!("rId{}", n + 5),
            &office("slide"),
            &format!("slides/slide{n}.xml"),
        )
    }));
    p.insert("ppt/_rels/presentation.xml.rels".into(), rels(&pres));
    p.insert("ppt/presProps.xml".into(), PRES_PROPS.into());
    p.insert("ppt/viewProps.xml".into(), VIEW_PROPS.into());
    p.insert("ppt/tableStyles.xml".into(), TABLE_STYLES.into());
    p.insert("ppt/theme/theme1.xml".into(), theme());
    p.insert(
        "ppt/slideMasters/slideMaster1.xml".into(),
        master(width, height),
    );
    p.insert(
        "ppt/slideMasters/_rels/slideMaster1.xml.rels".into(),
        rels(&[
            rel(
                "rId1",
                &office("slideLayout"),
                "../slideLayouts/slideLayout1.xml",
            ),
            rel("rId2", &office("theme"), "../theme/theme1.xml"),
        ]),
    );
    p.insert(
        "ppt/slideLayouts/slideLayout1.xml".into(),
        format!(
            r#"{HEADER}<p:sldLayout {NS} type="blank" preserve="1"><p:cSld name="Blank"><p:spTree>{ROOT_TREE}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#
        ),
    );
    p.insert(
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels".into(),
        rels(&[rel(
            "rId1",
            &office("slideMaster"),
            "../slideMasters/slideMaster1.xml",
        )]),
    );
    for n in 1..=slides {
        p.insert(
            format!("ppt/slides/slide{n}.xml"),
            format!(
                r#"{HEADER}<p:sld {NS}><p:cSld><p:spTree>{ROOT_TREE}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#
            ),
        );
        p.insert(
            format!("ppt/slides/_rels/slide{n}.xml.rels"),
            rels(&[rel(
                "rId1",
                &office("slideLayout"),
                "../slideLayouts/slideLayout1.xml",
            )]),
        );
    }
    p
}

fn theme() -> String {
    let colors: String = [
        ("dk1", "000000"),
        ("lt1", "FFFFFF"),
        ("dk2", "1F2937"),
        ("lt2", "F3F4F6"),
        ("accent1", "2563EB"),
        ("accent2", "F97316"),
        ("accent3", "10B981"),
        ("accent4", "7C3AED"),
        ("accent5", "DC2626"),
        ("accent6", "6B7280"),
        ("hlink", "2563EB"),
        ("folHlink", "7C3AED"),
    ]
    .map(|(name, value)| format!(r#"<a:{name}><a:srgbClr val="{value}"/></a:{name}>"#))
    .concat();
    let font = r#"<a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/>"#;
    let fills = FILL.repeat(3);
    let lines: String = [6350, 12700, 19050]
        .map(|w| format!(r#"<a:ln w="{w}">{FILL}</a:ln>"#))
        .concat();
    let effects = r#"<a:effectStyle><a:effectLst/></a:effectStyle>"#.repeat(3);
    format!(
        r#"{HEADER}<a:theme {NS} name="Deckwright"><a:themeElements><a:clrScheme name="Deckwright">{colors}</a:clrScheme><a:fontScheme name="Deckwright"><a:majorFont>{font}</a:majorFont><a:minorFont>{font}</a:minorFont></a:fontScheme><a:fmtScheme name="Deckwright"><a:fillStyleLst>{fills}</a:fillStyleLst><a:lnStyleLst>{lines}</a:lnStyleLst><a:effectStyleLst>{effects}</a:effectStyleLst><a:bgFillStyleLst>{fills}</a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>"#
    )
}

fn master(width: i64, height: i64) -> String {
    let (x, w) = (width * 7 / 100, width * 86 / 100);
    let (title_y, title_h) = (height * 5 / 100, height * 18 / 100);
    let (body_y, body_h) = (height * 30 / 100, height * 60 / 100);
    format!(
        r#"{HEADER}<p:sldMaster {NS}><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree>{ROOT_TREE}<p:sp><p:nvSpPr><p:cNvPr id="2" name="Title Placeholder 1"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{title_y}"/><a:ext cx="{w}" cy="{title_h}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr anchor="ctr"/><a:lstStyle/><a:p><a:endParaRPr lang="en-US"/></a:p></p:txBody></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Text Placeholder 2"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{body_y}"/><a:ext cx="{w}" cy="{body_h}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr lang="en-US"/></a:p></p:txBody></p:sp></p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/><p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst><p:txStyles><p:titleStyle><a:lvl1pPr algn="l"><a:defRPr sz="4400" kern="1200"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mj-lt"/><a:ea typeface="+mj-ea"/><a:cs typeface="+mj-cs"/></a:defRPr></a:lvl1pPr></p:titleStyle><p:bodyStyle><a:lvl1pPr marL="0" indent="0" algn="l"><a:buNone/><a:defRPr sz="2800" kern="1200"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mn-lt"/><a:ea typeface="+mn-ea"/><a:cs typeface="+mn-cs"/></a:defRPr></a:lvl1pPr></p:bodyStyle><p:otherStyle><a:defPPr><a:defRPr lang="en-US"/></a:defPPr><a:lvl1pPr marL="0" algn="l" defTabSz="914400"><a:defRPr sz="1800" kern="1200"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mn-lt"/><a:ea typeface="+mn-ea"/><a:cs typeface="+mn-cs"/></a:defRPr></a:lvl1pPr></p:otherStyle></p:txStyles></p:sldMaster>"#
    )
}

/// Fixed-metadata ZIP writer. Part order is the BTreeMap order, so bytes are stable.
fn encode(parts: &BTreeMap<String, String>) -> Result<Vec<u8>> {
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(6))
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644);
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, body) in parts {
        writer.start_file(name, options)?;
        writer
            .write_all(body.as_bytes())
            .map_err(|e| invalid(format!("{name}: {e}")))?;
    }
    Ok(writer.finish()?.into_inner())
}
