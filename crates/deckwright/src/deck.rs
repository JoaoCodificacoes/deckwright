//! Deck API over a `Package`: slides, media, hyperlinks, notes and core properties.
//!
//! Each mutating method stages its changes in a private overlay and applies them with
//! `Package::edited`, which revalidates the whole package through `Package::from_bytes`.
//! A failed call leaves the deck unchanged. Part names, relationship IDs and content
//! types are derived from the current state only, so identical call sequences give
//! identical bytes. Media is deduplicated by SHA-256 with an exact byte comparison.
//! Relationship targets are written relative to their source part, except external
//! hyperlinks, which carry `TargetMode="External"`.
//!
//! Choices: repeated `add_image` of identical bytes on one slide returns the existing
//! relationship ID. `set_notes` on a slide that already has notes rewrites that notes
//! part. `set_core_properties` replaces only the fields given as `Some`.

use crate::{
    Error, Limits, Package, Result,
    chart::{self, Chart},
    xlsx,
    xml::{self, Node},
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

const HEADER: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;
const NS: &str = concat!(
    r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
    r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#,
    r#"xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#
);
const ROOT_TREE: &str = r#"<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>"#;
const PKG_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const PML_NS: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const REL_NS_PREFIX: &str = "{http://schemas.openxmlformats.org/officeDocument/2006/relationships}";
const REL_ID: &str = "{http://schemas.openxmlformats.org/officeDocument/2006/relationships}id";
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

const PRES: &str = "ppt/presentation.xml";
const CONTENT_TYPES: &str = "[Content_Types].xml";
const CORE_PART: &str = "docProps/core.xml";

const LAYOUT_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout";
const SLIDE_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";
const IMAGE_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const VIDEO_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/video";
const MEDIA_REL: &str = "http://schemas.microsoft.com/office/2007/relationships/media";
const HYPERLINK_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";
const NOTES_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide";
const NOTES_MASTER_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster";
const THEME_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";
const CORE_REL: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";

const SLIDE_CT: &str = "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";
const NOTES_SLIDE_CT: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml";
const NOTES_MASTER_CT: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml";
const THEME_CT: &str = "application/vnd.openxmlformats-officedocument.theme+xml";
const CORE_CT: &str = "application/vnd.openxmlformats-package.core-properties+xml";

/// Relationship IDs for one video: the video, its media alias and its poster image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VideoLinks {
    pub video: String,
    pub media: String,
    pub poster: String,
}

/// Core document properties. `None` leaves the existing element untouched.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CoreProperties {
    pub title: Option<String>,
    pub subject: Option<String>,
    pub creator: Option<String>,
    pub keywords: Option<String>,
    pub description: Option<String>,
}

/// A deck being authored. Holds the package and applies each call atomically.
pub struct Deck {
    package: Package,
    limits: Limits,
}

impl Deck {
    pub fn from_package(package: Package) -> Self {
        Self {
            package,
            limits: Limits::default(),
        }
    }

    pub fn open(path: impl AsRef<Path>, limits: Limits) -> Result<Self> {
        Ok(Self {
            package: Package::open(path, limits)?,
            limits,
        })
    }

    pub fn into_package(self) -> Package {
        self.package
    }

    pub fn write(&self, path: impl AsRef<Path>) -> Result<()> {
        self.package.write(path)
    }

    /// Slide parts in `sldIdLst` order.
    pub fn slides(&self) -> Vec<String> {
        Tx::new(&self.package, self.limits.max_xml_depth)
            .slide_list()
            .unwrap_or_default()
    }

    /// `(layout part, cSld name)` for every layout reachable from the slide masters.
    pub fn layouts(&self) -> Vec<(String, String)> {
        Tx::new(&self.package, self.limits.max_xml_depth)
            .layout_list()
            .unwrap_or_default()
    }

    /// Appends an empty slide on `layout_part` and returns its part name.
    pub fn add_slide(&mut self, layout_part: &str) -> Result<String> {
        self.mutate(|tx| {
            if !tx
                .layout_list()?
                .iter()
                .any(|(part, _)| part == layout_part)
            {
                return Err(err(layout_part, "not a slide layout of this deck"));
            }
            let part = tx.fresh("ppt/slides", "slide", "xml");
            tx.put(&part, empty_slide());
            tx.ensure_override(&part, SLIDE_CT)?;
            tx.link(&part, LAYOUT_REL, layout_part, false)?;
            tx.append_slide(&part)?;
            Ok(part)
        })
    }

    /// Replaces the slide XML. Every `r:` reference must name a relationship of the slide.
    pub fn set_slide(&mut self, slide_part: &str, xml: Vec<u8>) -> Result<()> {
        self.mutate(|tx| {
            tx.require_slide(slide_part)?;
            let root = xml::parse(&xml, slide_part, tx.depth)?;
            if root.name != "sld" || root.namespace != PML_NS {
                return Err(err(slide_part, "root element must be p:sld"));
            }
            let rels = tx.rels(slide_part)?;
            let mut refs = Vec::new();
            relationship_refs(&root, &mut refs);
            for (key, id) in refs {
                if !rels.iter().any(|r| r.id == id) {
                    return Err(err(
                        slide_part,
                        format!("{key} names unknown relationship {id}"),
                    ));
                }
            }
            tx.put(slide_part, xml);
            Ok(())
        })
    }

    /// Stores an image on the slide and returns its relationship ID.
    /// `ext` is png, jpeg, jpg, gif, bmp, tiff or svg (case-insensitive).
    pub fn add_image(&mut self, slide_part: &str, bytes: Vec<u8>, ext: &str) -> Result<String> {
        let (ext, content_type) = image_kind(slide_part, ext)?;
        self.mutate(|tx| tx.image(slide_part, &bytes, ext, content_type))
    }

    /// Stores a video with its PNG poster. `ext` is mp4, m4v, mov, wmv or avi.
    pub fn add_video(
        &mut self,
        slide_part: &str,
        bytes: Vec<u8>,
        ext: &str,
        poster_png: Vec<u8>,
    ) -> Result<VideoLinks> {
        let (ext, content_type) = video_kind(slide_part, ext)?;
        if !poster_png.starts_with(PNG_SIGNATURE) {
            return Err(err(slide_part, "poster must be PNG bytes"));
        }
        self.mutate(|tx| {
            tx.require_slide(slide_part)?;
            let media = tx.media_part(&bytes, "media", ext, content_type)?;
            let video = tx.link(slide_part, VIDEO_REL, &media, false)?;
            let media_id = tx.link(slide_part, MEDIA_REL, &media, false)?;
            let poster = tx.image(slide_part, &poster_png, "png", "image/png")?;
            Ok(VideoLinks {
                video,
                media: media_id,
                poster,
            })
        })
    }

    /// External hyperlink relationship. Returns its ID.
    pub fn add_hyperlink(&mut self, slide_part: &str, url: &str) -> Result<String> {
        if url.is_empty() {
            return Err(err(slide_part, "hyperlink target is empty"));
        }
        reject_controls(slide_part, url)?;
        self.mutate(|tx| {
            tx.require_slide(slide_part)?;
            tx.link(slide_part, HYPERLINK_REL, url, true)
        })
    }

    /// Internal relationship from `slide_part` to `target_slide_part`. Returns its ID.
    pub fn add_slide_link(&mut self, slide_part: &str, target_slide_part: &str) -> Result<String> {
        self.mutate(|tx| {
            tx.require_slide(slide_part)?;
            tx.require_slide(target_slide_part)?;
            tx.link(slide_part, SLIDE_REL, target_slide_part, false)
        })
    }

    /// Sets the notes text of a slide, one paragraph per line. Creates the notes
    /// master (with its own theme) and the notes slide when they are absent.
    pub fn set_notes(&mut self, slide_part: &str, text: &str) -> Result<()> {
        let paragraphs = notes_paragraphs(slide_part, text)?;
        self.mutate(|tx| {
            tx.require_slide(slide_part)?;
            let master = tx.notes_master()?;
            match tx.existing_target(slide_part, NOTES_REL)? {
                Some(notes) => tx.put(&notes, notes_slide_xml(&paragraphs)),
                None => {
                    let notes = tx.fresh("ppt/notesSlides", "notesSlide", "xml");
                    tx.put(&notes, notes_slide_xml(&paragraphs));
                    tx.ensure_override(&notes, NOTES_SLIDE_CT)?;
                    tx.link(&notes, NOTES_MASTER_REL, &master, false)?;
                    tx.link(&notes, SLIDE_REL, slide_part, false)?;
                    tx.link(slide_part, NOTES_REL, &notes, false)?;
                }
            }
            Ok(())
        })
    }

    /// Writes the given core properties to `docProps/core.xml`.
    pub fn set_core_properties(&mut self, props: &CoreProperties) -> Result<()> {
        for value in [
            &props.title,
            &props.subject,
            &props.creator,
            &props.keywords,
            &props.description,
        ]
        .into_iter()
        .flatten()
        {
            reject_controls(CORE_PART, value)?;
        }
        self.mutate(|tx| tx.core(props))
    }

    /// Stores a native chart and its embedded workbook. Returns the slide relationship ID
    /// that `ObjectKind::Chart` takes. The chart is validated before anything is written.
    pub fn add_chart(&mut self, slide_part: &str, chart: &Chart) -> Result<String> {
        chart::validate(chart, slide_part)?;
        let workbook_bytes = xlsx::workbook(chart)?;
        self.mutate(|tx| {
            tx.require_slide(slide_part)?;
            let part = tx.fresh("ppt/charts", "chart", "xml");
            let workbook = tx.fresh("ppt/embeddings", "Microsoft_Excel_Worksheet", "xlsx");
            tx.put(&workbook, workbook_bytes);
            tx.ensure_default("xlsx", CHART_XLSX_CT, &workbook)?;
            tx.ensure_override(&part, CHART_CT)?;
            let data = tx.link(&part, CHART_PACKAGE_REL, &workbook, false)?;
            tx.put(&part, chart::chart_xml(chart, &data));
            tx.link(slide_part, CHART_REL, &part, false)
        })
    }

    /// Applies one edit. The overlay is consumed by `Package::edited`, which revalidates
    /// the whole package; on error the deck is left as it was.
    fn mutate<T>(&mut self, edit: impl FnOnce(&mut Tx<'_>) -> Result<T>) -> Result<T> {
        let (value, changes) = {
            let mut tx = Tx::new(&self.package, self.limits.max_xml_depth);
            let value = edit(&mut tx)?;
            (value, tx.out)
        };
        self.package = self
            .package
            .edited(changes, &BTreeSet::new(), self.limits)?;
        Ok(value)
    }
}

/// Read-through overlay: reads see staged writes, writes stay staged until committed.
struct Tx<'a> {
    package: &'a Package,
    depth: usize,
    out: BTreeMap<String, Vec<u8>>,
}

struct Rel {
    id: String,
    ty: String,
    target: String,
    external: bool,
}

impl<'a> Tx<'a> {
    fn new(package: &'a Package, depth: usize) -> Self {
        Self {
            package,
            depth,
            out: BTreeMap::new(),
        }
    }

    fn bytes_ref(&self, name: &str) -> Option<&[u8]> {
        self.out
            .get(name)
            .map(Vec::as_slice)
            .or_else(|| self.package.part(name))
    }

    fn exists(&self, name: &str) -> bool {
        self.bytes_ref(name).is_some()
    }

    fn put(&mut self, name: &str, bytes: impl Into<Vec<u8>>) {
        self.out.insert(name.to_owned(), bytes.into());
    }

    fn text(&self, part: &str) -> Result<Option<String>> {
        self.bytes_ref(part)
            .map(|b| String::from_utf8(b.to_vec()).map_err(|_| err(part, "not UTF-8")))
            .transpose()
    }

    fn required_text(&self, part: &str) -> Result<String> {
        self.text(part)?
            .ok_or_else(|| err(part, "missing required part"))
    }

    fn xml(&self, part: &str) -> Result<Node> {
        let bytes = self
            .bytes_ref(part)
            .ok_or_else(|| err(part, "missing required part"))?;
        xml::parse(bytes, part, self.depth)
    }

    fn opt_xml(&self, part: &str) -> Result<Option<Node>> {
        match self.bytes_ref(part) {
            Some(bytes) => xml::parse(bytes, part, self.depth).map(Some),
            None => Ok(None),
        }
    }

    /// A fresh `dir/stemN.ext` with the lowest unused N.
    fn fresh(&self, dir: &str, stem: &str, ext: &str) -> String {
        let mut n = 1;
        loop {
            let name = format!("{dir}/{stem}{n}.{ext}");
            if !self.exists(&name) {
                return name;
            }
            n += 1;
        }
    }

    fn slide_list(&self) -> Result<Vec<String>> {
        let root = self.xml(PRES)?;
        let rels = self.rels(PRES)?;
        root.descendants("sldId")
            .map(|n| target_for(PRES, &rels, &attr(n, REL_ID)))
            .collect()
    }

    fn layout_list(&self) -> Result<Vec<(String, String)>> {
        let root = self.xml(PRES)?;
        let rels = self.rels(PRES)?;
        let mut out = Vec::new();
        for master in root.descendants("sldMasterId") {
            let master_part = target_for(PRES, &rels, &attr(master, REL_ID))?;
            let master_root = self.xml(&master_part)?;
            let master_rels = self.rels(&master_part)?;
            for layout in master_root.descendants("sldLayoutId") {
                let part = target_for(&master_part, &master_rels, &attr(layout, REL_ID))?;
                let name = self
                    .xml(&part)?
                    .descendants("cSld")
                    .next()
                    .map(|c| attr(c, "name"))
                    .unwrap_or_default();
                out.push((part, name));
            }
        }
        Ok(out)
    }

    fn require_slide(&self, part: &str) -> Result<()> {
        if self.slide_list()?.iter().any(|s| s == part) {
            Ok(())
        } else {
            Err(err(part, "not a slide of this deck"))
        }
    }

    fn rels(&self, source: &str) -> Result<Vec<Rel>> {
        let Some(root) = self.opt_xml(&rels_part(source))? else {
            return Ok(Vec::new());
        };
        Ok(root
            .descendants("Relationship")
            .map(|n| Rel {
                id: attr(n, "Id"),
                ty: attr(n, "Type"),
                target: attr(n, "Target"),
                external: attr(n, "TargetMode") == "External",
            })
            .collect())
    }

    /// Target part of the first internal relationship of `ty`, if any.
    fn existing_target(&self, source: &str, ty: &str) -> Result<Option<String>> {
        Ok(self
            .rels(source)?
            .into_iter()
            .find(|r| r.ty == ty && !r.external)
            .map(|r| resolve(source, &r.target)))
    }

    fn add_rel(&mut self, source: &str, ty: &str, target: &str, external: bool) -> Result<String> {
        let part = rels_part(source);
        let next = self
            .rels(source)?
            .iter()
            .filter_map(|r| r.id.strip_prefix("rId")?.parse::<u32>().ok())
            .max()
            .unwrap_or(0)
            + 1;
        let id = format!("rId{next}");
        let (stored, mode) = if external {
            (target.to_owned(), r#" TargetMode="External""#)
        } else {
            (relative(source, target), "")
        };
        let entry = format!(
            r#"<Relationship Id="{id}" Type="{ty}" Target="{}"{mode}/>"#,
            esc(&stored)
        );
        match self.text(&part)? {
            Some(xml) => {
                let updated = insert_into(&part, &xml, "Relationships", &entry)?;
                self.put(&part, updated);
            }
            None => self.put(
                &part,
                format!(r#"{HEADER}<Relationships xmlns="{PKG_REL}">{entry}</Relationships>"#),
            ),
        }
        Ok(id)
    }

    /// Returns the existing matching relationship ID, or adds one.
    fn link(&mut self, source: &str, ty: &str, target: &str, external: bool) -> Result<String> {
        let existing = self.rels(source)?.into_iter().find(|r| {
            let same_target = if external {
                r.target == target
            } else {
                resolve(source, &r.target) == target
            };
            r.ty == ty && r.external == external && same_target
        });
        match existing {
            Some(rel) => Ok(rel.id),
            None => self.add_rel(source, ty, target, external),
        }
    }

    fn ensure_override(&mut self, part: &str, content_type: &str) -> Result<()> {
        let xml = self.required_text(CONTENT_TYPES)?;
        let root = xml::parse(xml.as_bytes(), CONTENT_TYPES, self.depth)?;
        let name = format!("/{part}");
        if let Some(found) = root
            .children
            .iter()
            .find(|n| n.name == "Override" && attr(n, "PartName") == name)
        {
            return if attr(found, "ContentType") == content_type {
                Ok(())
            } else {
                Err(err(part, "conflicting content type override"))
            };
        }
        let entry = format!(
            r#"<Override PartName="{}" ContentType="{}"/>"#,
            esc(&name),
            esc(content_type)
        );
        let updated = insert_into(CONTENT_TYPES, &xml, "Types", &entry)?;
        self.put(CONTENT_TYPES, updated);
        Ok(())
    }

    /// Covers `part` by extension, falling back to an Override when the extension is
    /// already mapped to a different type.
    fn ensure_default(&mut self, ext: &str, content_type: &str, part: &str) -> Result<()> {
        let xml = self.required_text(CONTENT_TYPES)?;
        let root = xml::parse(xml.as_bytes(), CONTENT_TYPES, self.depth)?;
        let existing = root
            .children
            .iter()
            .find(|n| n.name == "Default" && attr(n, "Extension").eq_ignore_ascii_case(ext))
            .map(|n| attr(n, "ContentType"));
        match existing {
            Some(found) if found == content_type => Ok(()),
            Some(_) => self.ensure_override(part, content_type),
            None => {
                let entry = format!(
                    r#"<Default Extension="{ext}" ContentType="{}"/>"#,
                    esc(content_type)
                );
                let updated = insert_into(CONTENT_TYPES, &xml, "Types", &entry)?;
                self.put(CONTENT_TYPES, updated);
                Ok(())
            }
        }
    }

    fn append_slide(&mut self, part: &str) -> Result<()> {
        let rid = self.link(PRES, SLIDE_REL, part, false)?;
        let xml = self.required_text(PRES)?;
        let largest = self
            .xml(PRES)?
            .descendants("sldId")
            .filter_map(|n| attr(n, "id").parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        let id = largest.max(255) + 1;
        let p = prefix_of(&xml, "sldIdLst");
        let entry = format!(r#"<{p}sldId id="{id}" r:id="{rid}"/>"#);
        let updated = insert_into(PRES, &xml, "sldIdLst", &entry)?;
        self.put(PRES, updated);
        Ok(())
    }

    /// Existing notes master, or a new one with its own theme.
    fn notes_master(&mut self) -> Result<String> {
        if let Some(master) = self.existing_target(PRES, NOTES_MASTER_REL)? {
            return Ok(master);
        }
        let master = self.fresh("ppt/notesMasters", "notesMaster", "xml");
        let theme = self.fresh("ppt/theme", "theme", "xml");
        self.put(&theme, theme_xml());
        self.ensure_override(&theme, THEME_CT)?;
        self.put(&master, notes_master_xml());
        self.ensure_override(&master, NOTES_MASTER_CT)?;
        self.link(&master, THEME_REL, &theme, false)?;
        let rid = self.link(PRES, NOTES_MASTER_REL, &master, false)?;
        let xml = self.required_text(PRES)?;
        let p = prefix_of(&xml, "sldIdLst");
        let entry = format!(
            r#"<{p}notesMasterIdLst><{p}notesMasterId r:id="{rid}"/></{p}notesMasterIdLst>"#
        );
        let updated = insert_before(PRES, &xml, "sldIdLst", &entry)?;
        self.put(PRES, updated);
        Ok(master)
    }

    fn image(
        &mut self,
        slide: &str,
        bytes: &[u8],
        ext: &str,
        content_type: &str,
    ) -> Result<String> {
        self.require_slide(slide)?;
        let media = self.media_part(bytes, "image", ext, content_type)?;
        self.link(slide, IMAGE_REL, &media, false)
    }

    /// Existing media part with identical bytes, or a new `ppt/media` part.
    fn media_part(
        &mut self,
        bytes: &[u8],
        stem: &str,
        ext: &str,
        content_type: &str,
    ) -> Result<String> {
        if let Some(name) = self.find_media(bytes) {
            return Ok(name);
        }
        let name = self.fresh("ppt/media", stem, ext);
        self.put(&name, bytes);
        self.ensure_default(ext, content_type, &name)?;
        Ok(name)
    }

    fn find_media(&self, bytes: &[u8]) -> Option<String> {
        let digest = sha256(bytes);
        self.package
            .part_names()
            .chain(self.out.keys().map(String::as_str))
            .filter(|name| name.starts_with("ppt/media/"))
            .find(|name| {
                self.bytes_ref(name)
                    .is_some_and(|b| b.len() == bytes.len() && sha256(b) == digest)
            })
            .map(str::to_owned)
    }

    fn core(&mut self, props: &CoreProperties) -> Result<()> {
        let mut xml = self.text(CORE_PART)?.unwrap_or_else(core_skeleton);
        let fields: [(&str, &str, &Option<String>); 5] = [
            ("title", "dc", &props.title),
            ("subject", "dc", &props.subject),
            ("creator", "dc", &props.creator),
            ("keywords", "cp", &props.keywords),
            ("description", "dc", &props.description),
        ];
        for (local, prefix, value) in fields {
            if let Some(value) = value {
                xml = remove_elements(&xml, local);
                let entry = format!("<{prefix}:{local}>{}</{prefix}:{local}>", esc(value));
                xml = insert_into(CORE_PART, &xml, "coreProperties", &entry)?;
            }
        }
        self.put(CORE_PART, xml);
        self.ensure_override(CORE_PART, CORE_CT)?;
        self.link("", CORE_REL, CORE_PART, false)?;
        Ok(())
    }
}

fn err(part: &str, message: impl Into<String>) -> Error {
    Error::InvalidPart {
        part: part.into(),
        message: message.into(),
    }
}

fn attr(n: &Node, key: &str) -> String {
    n.attrs.get(key).cloned().unwrap_or_default()
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Rejects characters that XML 1.0 cannot carry (tab is allowed).
fn reject_controls(part: &str, s: &str) -> Result<()> {
    if s.chars().any(|c| c < ' ' && c != '\t') {
        return Err(err(part, "text contains a control character"));
    }
    Ok(())
}

fn image_kind(slide: &str, ext: &str) -> Result<(&'static str, &'static str)> {
    Ok(match ext.to_ascii_lowercase().as_str() {
        "png" => ("png", "image/png"),
        "jpeg" | "jpg" => ("jpeg", "image/jpeg"),
        "gif" => ("gif", "image/gif"),
        "bmp" => ("bmp", "image/bmp"),
        "tiff" => ("tiff", "image/tiff"),
        "svg" => ("svg", "image/svg+xml"),
        _ => return Err(err(slide, format!("unsupported image extension {ext:?}"))),
    })
}

fn video_kind(slide: &str, ext: &str) -> Result<(&'static str, &'static str)> {
    Ok(match ext.to_ascii_lowercase().as_str() {
        "mp4" => ("mp4", "video/mp4"),
        "m4v" => ("m4v", "video/mp4"),
        "mov" => ("mov", "video/quicktime"),
        "wmv" => ("wmv", "video/x-ms-wmv"),
        "avi" => ("avi", "video/x-msvideo"),
        _ => return Err(err(slide, format!("unsupported video extension {ext:?}"))),
    })
}

fn rels_part(source: &str) -> String {
    if source.is_empty() {
        return "_rels/.rels".into();
    }
    match source.rsplit_once('/') {
        Some((dir, file)) => format!("{dir}/_rels/{file}.rels"),
        None => format!("_rels/{source}.rels"),
    }
}

/// Relative reference from `source` to the part `target`.
fn relative(source: &str, target: &str) -> String {
    let from: Vec<&str> = source.split('/').collect();
    let from_dir = &from[..from.len() - 1];
    let to: Vec<&str> = target.split('/').collect();
    let common = from_dir
        .iter()
        .zip(&to[..to.len() - 1])
        .take_while(|(a, b)| a == b)
        .count();
    let mut parts = vec![".."; from_dir.len() - common];
    parts.extend_from_slice(&to[common..]);
    parts.join("/")
}

/// Part name an internal relationship target from `source` denotes.
fn resolve(source: &str, target: &str) -> String {
    let target = target.split('#').next().unwrap_or("");
    let mut segments: Vec<&str> = if target.starts_with('/') {
        Vec::new()
    } else {
        source
            .rsplit_once('/')
            .map(|(dir, _)| dir.split('/').collect())
            .unwrap_or_default()
    };
    for s in target.trim_start_matches('/').split('/') {
        match s {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            _ => segments.push(s),
        }
    }
    segments.join("/")
}

fn target_for(source: &str, rels: &[Rel], id: &str) -> Result<String> {
    rels.iter()
        .find(|r| r.id == id && !r.external)
        .map(|r| resolve(source, &r.target))
        .ok_or_else(|| err(source, format!("{id} does not name an internal part")))
}

fn relationship_refs(node: &Node, out: &mut Vec<(String, String)>) {
    for (key, value) in &node.attrs {
        if let Some(local) = key.strip_prefix(REL_NS_PREFIX) {
            out.push((local.to_owned(), value.clone()));
        }
    }
    for child in &node.children {
        relationship_refs(child, out);
    }
}

fn notes_paragraphs(part: &str, text: &str) -> Result<String> {
    let mut out = String::new();
    for line in text.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        reject_controls(part, line)?;
        if line.is_empty() {
            out.push_str(r#"<a:p><a:endParaRPr lang="en-US"/></a:p>"#);
        } else {
            out.push_str(&format!(
                r#"<a:p><a:r><a:rPr lang="en-US"/><a:t>{}</a:t></a:r></a:p>"#,
                esc(line)
            ));
        }
    }
    Ok(out)
}

fn empty_slide() -> String {
    format!(
        r#"{HEADER}<p:sld {NS}><p:cSld><p:spTree>{ROOT_TREE}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#
    )
}

fn notes_slide_xml(paragraphs: &str) -> String {
    format!(
        r#"{HEADER}<p:notes {NS}><p:cSld><p:spTree>{ROOT_TREE}<p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image Placeholder 1"/><p:cNvSpPr><a:spLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvSpPr><p:nvPr><p:ph type="sldImg"/></p:nvPr></p:nvSpPr><p:spPr/></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Notes Placeholder 2"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/>{paragraphs}</p:txBody></p:sp></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:notes>"#
    )
}

fn notes_master_xml() -> String {
    format!(
        r#"{HEADER}<p:notesMaster {NS}><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg><p:spTree>{ROOT_TREE}<p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image Placeholder 1"/><p:cNvSpPr><a:spLocks noGrp="1" noRot="1" noChangeAspect="1"/></p:cNvSpPr><p:nvPr><p:ph type="sldImg" idx="2"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="685800" y="1143000"/><a:ext cx="5486400" cy="3086100"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln w="12700"><a:solidFill><a:prstClr val="black"/></a:solidFill></a:ln></p:spPr></p:sp><p:sp><p:nvSpPr><p:cNvPr id="3" name="Notes Placeholder 2"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="body" sz="quarter" idx="3"/></p:nvPr></p:nvSpPr><p:spPr><a:xfrm><a:off x="685800" y="4400550"/><a:ext cx="5486400" cy="3600450"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr vert="horz" lIns="91440" tIns="45720" rIns="91440" bIns="45720" rtlCol="0"/><a:lstStyle/><a:p><a:endParaRPr lang="en-US"/></a:p></p:txBody></p:sp></p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/><p:notesStyle><a:lvl1pPr marL="0" algn="l" defTabSz="914400" rtl="0" eaLnBrk="1" latinLnBrk="0" hangingPunct="1"><a:defRPr sz="1200" kern="1200"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mn-lt"/><a:ea typeface="+mn-ea"/><a:cs typeface="+mn-cs"/></a:defRPr></a:lvl1pPr></p:notesStyle></p:notesMaster>"#
    )
}

/// Complete theme for the notes master, matching the template's theme.
fn theme_xml() -> String {
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
    let fill = r#"<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>"#;
    let fills = fill.repeat(3);
    let lines: String = [6350, 12700, 19050]
        .map(|w| format!(r#"<a:ln w="{w}">{fill}</a:ln>"#))
        .concat();
    let effects = r#"<a:effectStyle><a:effectLst/></a:effectStyle>"#.repeat(3);
    format!(
        r#"{HEADER}<a:theme {NS} name="Deckwright"><a:themeElements><a:clrScheme name="Deckwright">{colors}</a:clrScheme><a:fontScheme name="Deckwright"><a:majorFont>{font}</a:majorFont><a:minorFont>{font}</a:minorFont></a:fontScheme><a:fmtScheme name="Deckwright"><a:fillStyleLst>{fills}</a:fillStyleLst><a:lnStyleLst>{lines}</a:lnStyleLst><a:effectStyleLst>{effects}</a:effectStyleLst><a:bgFillStyleLst>{fills}</a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>"#
    )
}

fn core_skeleton() -> String {
    format!(
        r#"{HEADER}<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"></cp:coreProperties>"#
    )
}

/// A start or end tag found by a scan of raw XML text.
struct Tag<'a> {
    start: usize,
    end: usize,
    name: &'a str,
    local: &'a str,
    closing: bool,
    self_closing: bool,
}

/// Index just past the `>` that ends the tag starting at `start`, skipping quoted values.
fn tag_end(xml: &str, start: usize) -> Option<usize> {
    let mut quote: Option<u8> = None;
    for (offset, &b) in xml.as_bytes()[start..].iter().enumerate() {
        match quote {
            Some(q) if b == q => quote = None,
            Some(_) => {}
            None => match b {
                b'"' | b'\'' => quote = Some(b),
                b'>' => return Some(start + offset + 1),
                _ => {}
            },
        }
    }
    None
}

fn tags(xml: &str) -> Vec<Tag<'_>> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(offset) = xml[at..].find('<') {
        let start = at + offset;
        let Some(end) = tag_end(xml, start) else {
            break;
        };
        let inner = &xml[start + 1..end - 1];
        at = end;
        if inner.starts_with(['?', '!']) {
            continue;
        }
        let closing = inner.starts_with('/');
        let body = if closing { &inner[1..] } else { inner };
        let name = body
            .split(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .next()
            .unwrap_or("");
        let local = name.rsplit(':').next().unwrap_or(name);
        out.push(Tag {
            start,
            end,
            name,
            local,
            closing,
            self_closing: !closing && inner.ends_with('/'),
        });
    }
    out
}

/// Namespace prefix (with colon, or empty) used by the first `local` element.
fn prefix_of(xml: &str, local: &str) -> String {
    tags(xml)
        .iter()
        .find(|t| !t.closing && t.local == local)
        .and_then(|t| t.name.split_once(':'))
        .map(|(prefix, _)| format!("{prefix}:"))
        .unwrap_or_default()
}

/// Appends `fragment` as the last child of the first `local` element.
fn insert_into(part: &str, xml: &str, local: &str, fragment: &str) -> Result<String> {
    let all = tags(xml);
    let index = all
        .iter()
        .position(|t| !t.closing && t.local == local)
        .ok_or_else(|| err(part, format!("no {local} element")))?;
    let open = &all[index];
    if open.self_closing {
        let raw = &xml[open.start..open.end];
        let head = raw[..raw.len() - 2].trim_end();
        return Ok(format!(
            "{}{head}>{fragment}</{}>{}",
            &xml[..open.start],
            open.name,
            &xml[open.end..]
        ));
    }
    let close = all[index + 1..]
        .iter()
        .find(|t| t.closing && t.local == local)
        .ok_or_else(|| err(part, format!("unclosed {local} element")))?;
    Ok(format!(
        "{}{fragment}{}",
        &xml[..close.start],
        &xml[close.start..]
    ))
}

/// Inserts `fragment` immediately before the first `local` element.
fn insert_before(part: &str, xml: &str, local: &str, fragment: &str) -> Result<String> {
    let start = tags(xml)
        .iter()
        .find(|t| !t.closing && t.local == local)
        .map(|t| t.start)
        .ok_or_else(|| err(part, format!("no {local} element")))?;
    Ok(format!("{}{fragment}{}", &xml[..start], &xml[start..]))
}

/// Removes every paired or self-closing element with the given local name.
fn remove_elements(xml: &str, local: &str) -> String {
    let all = tags(xml);
    let mut spans = Vec::new();
    for (i, t) in all.iter().enumerate() {
        if t.closing || t.local != local {
            continue;
        }
        if t.self_closing {
            spans.push((t.start, t.end));
        } else if let Some(close) = all[i + 1..].iter().find(|c| c.closing && c.local == local) {
            spans.push((t.start, close.end));
        }
    }
    let mut out = String::new();
    let mut last = 0;
    for (start, end) in spans {
        out.push_str(&xml[last..start]);
        last = end;
    }
    out.push_str(&xml[last..]);
    out
}

// Charts (`Deck::add_chart`): relationship types and content types.
const CHART_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart";
const CHART_PACKAGE_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/package";
const CHART_CT: &str = "application/vnd.openxmlformats-officedocument.drawingml.chart+xml";
const CHART_XLSX_CT: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
