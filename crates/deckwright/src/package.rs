use crate::{
    Error, Result,
    xml::{self, Node},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read, Write},
    path::Path,
};
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_input_bytes: u64,
    pub max_uncompressed_bytes: u64,
    pub max_part_bytes: u64,
    pub max_parts: usize,
    pub max_xml_depth: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_bytes: 128 * 1024 * 1024,
            max_uncompressed_bytes: 512 * 1024 * 1024,
            max_part_bytes: 128 * 1024 * 1024,
            max_parts: 10000,
            max_xml_depth: 256,
        }
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct Relationship {
    pub source: String,
    pub id: String,
    pub relationship_type: String,
    pub target: String,
    pub target_mode: String,
    pub resolved_target: Option<String>,
}
/// Owns opaque uncompressed bytes. No-op export never reserializes XML or media.
pub struct Package {
    parts: BTreeMap<String, Vec<u8>>,
    xml: BTreeMap<String, Node>,
    relationships: Vec<Relationship>,
    content_types: BTreeMap<String, String>,
    input_bytes: u64,
    input_sha256: String,
}
fn invalid(part: &str, message: impl Into<String>) -> Error {
    Error::InvalidPart {
        part: part.into(),
        message: message.into(),
    }
}
fn hash(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
fn valid_name(n: &str) -> bool {
    !n.is_empty()
        && !n.starts_with('/')
        && !n.contains(['\\', '\0', ':', '?', '#', '%'])
        && n.split('/').all(|s| !s.is_empty() && s != "." && s != "..")
}
fn resolve(source: &str, target: &str) -> Result<String> {
    let target = target.split('#').next().unwrap_or_default();
    let mut decoded = Vec::new();
    let mut bytes = target.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            let h = [
                bytes
                    .next()
                    .ok_or_else(|| invalid(source, "bad target escape"))?,
                bytes
                    .next()
                    .ok_or_else(|| invalid(source, "bad target escape"))?,
            ];
            let s = std::str::from_utf8(&h).map_err(|_| invalid(source, "bad target escape"))?;
            decoded
                .push(u8::from_str_radix(s, 16).map_err(|_| invalid(source, "bad target escape"))?);
        } else {
            decoded.push(b);
        }
    }
    let decoded = String::from_utf8(decoded).map_err(|_| invalid(source, "target is not UTF-8"))?;
    if decoded.contains(['\\', ':', '\0', '?']) {
        return Err(invalid(source, "invalid internal target"));
    }
    let mut segments: Vec<&str> = if decoded.starts_with('/') {
        Vec::new()
    } else {
        source
            .rsplit_once('/')
            .map(|(d, _)| d.split('/').collect())
            .unwrap_or_default()
    };
    for s in decoded.split('/') {
        match s {
            "" | "." => {}
            ".." => {
                if segments.pop().is_none() {
                    return Err(invalid(source, "target escapes package"));
                }
            }
            _ => segments.push(s),
        }
    }
    Ok(segments.join("/"))
}
impl Package {
    pub fn open(path: impl AsRef<Path>, limits: Limits) -> Result<Self> {
        let path = path.as_ref();
        let f = std::fs::File::open(path).map_err(|source| Error::Io {
            path: path.into(),
            source,
        })?;
        let mut bytes = Vec::new();
        f.take(limits.max_input_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|source| Error::Io {
                path: path.into(),
                source,
            })?;
        Self::from_bytes(bytes, limits)
    }
    pub fn from_bytes(bytes: Vec<u8>, limits: Limits) -> Result<Self> {
        if bytes.len() as u64 > limits.max_input_bytes {
            return Err(Error::Limit("input bytes".into()));
        }
        validate_directory(&bytes, limits.max_parts)?;
        let input_bytes = bytes.len() as u64;
        let input_sha256 = hash(&bytes);
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
        if archive.len() > limits.max_parts {
            return Err(Error::Limit("part count".into()));
        }
        let mut parts = BTreeMap::new();
        let mut xml_parts = BTreeMap::new();
        let mut total = 0u64;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let name = entry.name().to_owned();
            if entry.is_dir() {
                continue;
            }
            if !valid_name(&name) {
                return Err(invalid(&name, "invalid canonical part name"));
            }
            if parts.contains_key(&name) {
                return Err(invalid(&name, "duplicate part"));
            }
            if entry.size() > limits.max_part_bytes {
                return Err(Error::Limit(format!("part bytes: {name}")));
            }
            total = total
                .checked_add(entry.size())
                .ok_or_else(|| Error::Limit("uncompressed bytes overflow".into()))?;
            if total > limits.max_uncompressed_bytes {
                return Err(Error::Limit("uncompressed bytes".into()));
            }
            let mut data = Vec::new();
            entry
                .by_ref()
                .take(limits.max_part_bytes.saturating_add(1))
                .read_to_end(&mut data)
                .map_err(|e| invalid(&name, e.to_string()))?;
            if data.len() as u64 != entry.size() {
                return Err(invalid(&name, "declared size mismatch"));
            }
            if name.ends_with(".xml") || name.ends_with(".rels") {
                xml_parts.insert(
                    name.clone(),
                    xml::parse(&data, &name, limits.max_xml_depth)?,
                );
            }
            parts.insert(name, data);
        }
        let types = xml_parts
            .get("[Content_Types].xml")
            .ok_or_else(|| invalid("[Content_Types].xml", "missing content types"))?;
        let mut defaults = BTreeMap::new();
        let mut overrides = BTreeMap::new();
        for n in &types.children {
            match n.name.as_str() {
                "Default" => {
                    defaults.insert(
                        attr(n, "Extension").to_ascii_lowercase(),
                        attr(n, "ContentType"),
                    );
                }
                "Override" => {
                    overrides.insert(
                        attr(n, "PartName").trim_start_matches('/').to_string(),
                        attr(n, "ContentType"),
                    );
                }
                _ => {}
            }
        }
        let mut content_types = BTreeMap::new();
        for name in parts.keys() {
            if name == "[Content_Types].xml" {
                continue;
            }
            let kind = overrides
                .get(name)
                .or_else(|| {
                    name.rsplit_once('.')
                        .and_then(|(_, e)| defaults.get(&e.to_ascii_lowercase()))
                })
                .ok_or_else(|| invalid(name, "missing content type"))?;
            content_types.insert(name.clone(), kind.clone());
        }
        let mut relationships = Vec::new();
        for (name, root) in &xml_parts {
            if !name.ends_with(".rels") {
                continue;
            }
            let source = if name == "_rels/.rels" {
                String::new()
            } else {
                let (base, file) = name
                    .rsplit_once("/_rels/")
                    .or_else(|| name.strip_prefix("_rels/").map(|file| ("", file)))
                    .ok_or_else(|| invalid(name, "invalid relationship part path"))?;
                let file = file
                    .strip_suffix(".rels")
                    .ok_or_else(|| invalid(name, "invalid relationship part suffix"))?;
                if file.is_empty() || file.contains('/') {
                    return Err(invalid(name, "invalid relationship source filename"));
                }
                if base.is_empty() {
                    file.to_owned()
                } else {
                    format!("{base}/{file}")
                }
            };
            if !source.is_empty() && !parts.contains_key(&source) {
                return Err(invalid(name, "relationship source missing"));
            }
            let mut ids = BTreeSet::new();
            for n in root.descendants("Relationship") {
                let id = attr(n, "Id");
                let target = attr(n, "Target");
                let relationship_type = attr(n, "Type");
                let mode = n
                    .attrs
                    .get("TargetMode")
                    .cloned()
                    .unwrap_or_else(|| "Internal".into());
                let err = |message: &str| Error::Relationship {
                    part: name.clone(),
                    id: id.clone(),
                    message: message.into(),
                };
                if id.is_empty() || target.is_empty() || relationship_type.is_empty() {
                    return Err(err("missing required attribute"));
                }
                if !ids.insert(id.clone()) {
                    return Err(err("duplicate relationship ID"));
                }
                let resolved_target = match mode.as_str() {
                    "External" => None,
                    "Internal" => {
                        let resolved = resolve(&source, &target)?;
                        if !parts.contains_key(&resolved) {
                            return Err(err(&format!("missing target {resolved}")));
                        }
                        Some(resolved)
                    }
                    _ => return Err(err("invalid TargetMode")),
                };
                relationships.push(Relationship {
                    source: source.clone(),
                    id,
                    relationship_type,
                    target,
                    target_mode: mode,
                    resolved_target,
                });
            }
        }
        Ok(Self {
            parts,
            xml: xml_parts,
            relationships,
            content_types,
            input_bytes,
            input_sha256,
        })
    }
    pub fn part(&self, name: &str) -> Option<&[u8]> {
        self.parts.get(name).map(Vec::as_slice)
    }
    pub fn part_names(&self) -> impl Iterator<Item = &str> {
        self.parts.keys().map(String::as_str)
    }
    pub fn relationships(&self) -> &[Relationship] {
        &self.relationships
    }
    /// Atomically apply explicit part replacements/removals. The returned package
    /// is fully revalidated; this package and all untouched bytes stay unchanged.
    /// Required relationship and content-type edits are the caller's responsibility.
    pub fn edited(
        &self,
        replacements: BTreeMap<String, Vec<u8>>,
        removals: &BTreeSet<String>,
        limits: Limits,
    ) -> Result<Self> {
        let mut parts = self.parts.clone();
        for name in removals {
            if !valid_name(name) || parts.remove(name).is_none() {
                return Err(invalid(name, "cannot remove missing or invalid part"));
            }
        }
        for (name, bytes) in replacements {
            if !valid_name(&name) || removals.contains(&name) {
                return Err(invalid(&name, "invalid or conflicting replacement"));
            }
            parts.insert(name, bytes);
        }
        Self::from_bytes(encode_parts(&parts)?, limits)
    }
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        encode_parts(&self.parts)
    }
    pub fn write(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let bytes = self.to_bytes()?;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|source| Error::Io {
                path: path.into(),
                source,
            })?;
        f.write_all(&bytes)
            .and_then(|_| f.sync_all())
            .map_err(|source| Error::Io {
                path: path.into(),
                source,
            })
    }
    pub fn inventory(&self) -> serde_json::Value {
        let mut totals = BTreeMap::<String, usize>::new();
        let mut slides = Vec::new();
        let order:Vec<String>=self.xml.get("ppt/presentation.xml").map(|root|root.descendants("sldId").filter_map(|n|n.attrs.iter().find(|(k,_)|k.as_str()=="{http://schemas.openxmlformats.org/officeDocument/2006/relationships}id").and_then(|(_,id)|self.relationships.iter().find(|r|r.source=="ppt/presentation.xml"&&r.id==*id)).and_then(|r|r.resolved_target.clone())).collect()).unwrap_or_default();
        for (index, name) in order.iter().enumerate() {
            if let Some(root) = self.xml.get(name) {
                let mut counts = BTreeMap::new();
                for k in [
                    "sp",
                    "pic",
                    "grpSp",
                    "cxnSp",
                    "graphicFrame",
                    "tbl",
                    "videoFile",
                    "audioFile",
                    "timing",
                    "custGeom",
                ] {
                    let c = root.descendants(k).count();
                    counts.insert(k, c);
                    *totals.entry(k.into()).or_default() += c;
                }
                let effects: Vec<_> = root
                    .descendants("cTn")
                    .filter(|n| n.attrs.contains_key("presetClass"))
                    .map(|n| &n.attrs)
                    .collect();
                *totals.entry("effects".into()).or_default() += effects.len();
                let transitions:Vec<_>=root.descendants("transition").map(|n|serde_json::json!({"attributes":n.attrs,"children":n.children.iter().map(|c|&c.name).collect::<Vec<_>>()})).collect();
                let text: Vec<_> = root.descendants("t").map(|n| &n.text).collect();
                let timing_nodes: Vec<_> = root.descendants("cTn").map(|n| &n.attrs).collect();
                let targets: Vec<_> = root.descendants("spTgt").map(|n| &n.attrs).collect();
                slides.push(serde_json::json!({"slide":index+1,"part":name,"counts":counts,"effects":effects,"timing_nodes":timing_nodes,"timing_targets":targets,"timing":root.descendants("timing").collect::<Vec<_>>(),"morph_names":root.descendants("cNvPr").filter_map(|n|n.attrs.get("name")).filter(|n|n.starts_with("!!")).collect::<Vec<_>>(),"transitions":transitions,"text":text}));
            }
        }
        let parts:Vec<_>=self.parts.iter().map(|(n,b)|serde_json::json!({"name":n,"bytes":b.len(),"sha256":hash(b),"content_type":self.content_types.get(n)})).collect();
        serde_json::json!({"schema_version":1,"package":{"bytes":self.input_bytes,"uncompressed_bytes":self.parts.values().map(Vec::len).sum::<usize>(),"part_count":self.parts.len(),"sha256":self.input_sha256},"parts":parts,"relationships":self.relationships,"slides":slides,"totals":totals,"slide_size":self.xml.get("ppt/presentation.xml").and_then(|n|n.descendants("sldSz").next()).map(|n|&n.attrs)})
    }
}
fn encode_parts(parts: &BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(6))
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644);
    for (name, bytes) in parts {
        w.start_file(name, options)?;
        w.write_all(bytes)
            .map_err(|e| invalid(name, e.to_string()))?;
    }
    Ok(w.finish()?.into_inner())
}
fn attr(n: &Node, name: &str) -> String {
    n.attrs.get(name).cloned().unwrap_or_default()
}

// Validate the raw directory before zip's name index can collapse duplicate entries.
fn validate_directory(bytes: &[u8], max_parts: usize) -> Result<()> {
    let bad = || {
        invalid(
            "ZIP directory",
            "invalid or unsupported ZIP directory (ZIP64 is not supported)",
        )
    };
    let pos = (bytes.len().saturating_sub(65557)..bytes.len().saturating_sub(21))
        .rev()
        .find(|&i| {
            bytes.get(i..i + 4) == Some(b"PK\x05\x06")
                && bytes
                    .get(i + 20..i + 22)
                    .map(|b| i + 22 + u16::from_le_bytes([b[0], b[1]]) as usize == bytes.len())
                    .unwrap_or(false)
        })
        .ok_or_else(bad)?;
    let u16at = |i: usize| -> Result<usize> {
        let b = bytes.get(i..i + 2).ok_or_else(bad)?;
        Ok(u16::from_le_bytes([b[0], b[1]]) as usize)
    };
    let u32at = |i: usize| -> Result<usize> {
        let b = bytes.get(i..i + 4).ok_or_else(bad)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
    };
    let count = u16at(pos + 10)?;
    if count == 65535 || u16at(pos + 4)? != 0 || u16at(pos + 6)? != 0 || u16at(pos + 8)? != count {
        return Err(bad());
    }
    if count > max_parts {
        return Err(Error::Limit("part count".into()));
    }
    let mut offset = u32at(pos + 16)?;
    let end = offset.checked_add(u32at(pos + 12)?).ok_or_else(bad)?;
    if end != pos {
        return Err(bad());
    }
    let mut names = BTreeSet::new();
    for _ in 0..count {
        if bytes.get(offset..offset + 4) != Some(b"PK\x01\x02") {
            return Err(bad());
        }
        let size = u16at(offset + 28)?;
        let name = bytes.get(offset + 46..offset + 46 + size).ok_or_else(bad)?;
        if !names.insert(name) {
            return Err(invalid(&String::from_utf8_lossy(name), "duplicate part"));
        }
        offset = offset
            .checked_add(46 + size + u16at(offset + 30)? + u16at(offset + 32)?)
            .ok_or_else(bad)?;
        if offset > end {
            return Err(bad());
        }
    }
    if offset != end {
        return Err(bad());
    }
    Ok(())
}
