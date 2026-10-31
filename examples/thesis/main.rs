//! Rebuild every editable slide listed by the manifest from private typed source and template.
use deckwright::{
    Error, Limits, Package, Result,
    builder::compile_slide,
    model::{Object, ObjectKind, Slide},
    timing::{ParsedTiming, compile_animated_slide},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::{Component, Path, PathBuf},
};

fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        source,
    }
}
/// Manifest paths are relative to the prepared source directory and cannot escape it.
fn source_path(root: &Path, relative: &str) -> Result<PathBuf> {
    let path = Path::new(relative);
    if relative.is_empty() || !path.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err(Error::InvalidPart {
            part: "authored manifest".into(),
            message: "slide source must be a relative path inside the source directory".into(),
        });
    }
    Ok(root.join(path))
}
fn read_slide(root: &Path, relative: &str) -> Result<Slide> {
    let path = source_path(root, relative)?;
    let data = std::fs::read(&path).map_err(|e| io(&path, e))?;
    serde_json::from_slice(&data).map_err(|e| Error::InvalidPart {
        part: path.display().to_string(),
        message: e.to_string(),
    })
}
/// Timing exists only for slides extracted with `--dw`, as the sibling `sNN.timing.json`.
fn read_timing(root: &Path, relative: &str) -> Result<Option<ParsedTiming>> {
    let timing_path = source_path(root, relative)?.with_extension("timing.json");
    if !timing_path.exists() {
        return Ok(None);
    }
    let data = std::fs::read(&timing_path).map_err(|e| io(&timing_path, e))?;
    serde_json::from_slice(&data)
        .map(Some)
        .map_err(|e| Error::InvalidPart {
            part: timing_path.display().to_string(),
            message: e.to_string(),
        })
}
/// Current placements are regenerated from the same typed objects compiled into
/// the deck. Extraction-time placements are retained separately as provenance.
fn update_mappings(manifest: &mut serde_json::Value, slides: &[Slide]) -> Result<()> {
    fn bad(message: &str) -> Error {
        Error::InvalidPart {
            part: "authored manifest".into(),
            message: message.into(),
        }
    }
    fn current(
        objects: &[Object],
        sources: &BTreeMap<String, serde_json::Value>,
        out: &mut Vec<serde_json::Value>,
    ) -> Result<()> {
        for object in objects {
            let (kind, transform, relationship, crop) = match &object.kind {
                ObjectKind::Shape { properties, .. } => {
                    ("shape", properties.transform.as_ref(), None, None)
                }
                ObjectKind::Picture {
                    properties,
                    relationship,
                    crop,
                    ..
                } => (
                    "picture",
                    properties.transform.as_ref(),
                    Some(relationship),
                    crop.as_ref(),
                ),
                ObjectKind::Connector { properties, .. } => {
                    ("connector", properties.transform.as_ref(), None, None)
                }
                ObjectKind::Group { transform, .. } => ("group", Some(transform), None, None),
            };
            // Source provenance uses its own stable-key map, never native IDs.
            let source_id = sources
                .get(&object.key)
                .and_then(|s| s.get("source_id"))
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let mut placement = serde_json::json!({"key":object.key,"source_id":source_id,"generated_id":object.id,"kind":kind,"morph_name":object.morph_name,"relationship":relationship,"crop":crop});
            placement["transform"] =
                serde_json::to_value(transform).map_err(|e| bad(&e.to_string()))?;
            out.push(placement);
            if let ObjectKind::Group { children, .. } = &object.kind {
                current(children, sources, out)?;
            }
        }
        Ok(())
    }
    let entries = manifest
        .get_mut("slides")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| bad("slides must be an array"))?;
    if entries.len() != slides.len() {
        return Err(bad(
            "source slide mapping count differs from authored slides",
        ));
    }
    for (entry, slide) in entries.iter_mut().zip(slides) {
        let mut source = entry
            .get("source_placements")
            .or_else(|| entry.get("placements"))
            .and_then(|v| v.as_array())
            .cloned()
            .ok_or_else(|| bad("source placements must be an array"))?;
        let mut sources = BTreeMap::new();
        for placement in &mut source {
            let placement = placement
                .as_object_mut()
                .ok_or_else(|| bad("source placement must be an object"))?;
            placement.remove("generated_id"); // extraction does not establish current IDs
            let key = placement
                .get("key")
                .and_then(|v| v.as_str())
                .ok_or_else(|| bad("source placement requires key"))?
                .to_owned();
            if sources
                .insert(key, serde_json::Value::Object(placement.clone()))
                .is_some()
            {
                return Err(bad("duplicate source placement key"));
            }
        }
        let mut placements = Vec::new();
        current(&slide.objects, &sources, &mut placements)?;
        entry["source_placements"] = serde_json::Value::Array(source);
        entry["placements"] = serde_json::Value::Array(placements);
    }
    Ok(())
}
fn build(root: &Path, output: &Path) -> Result<()> {
    let template = Package::open(root.join("template.pptx"), Limits::default())?;
    let manifest = root.join("manifest.json");
    let bytes = std::fs::read(&manifest).map_err(|e| io(&manifest, e))?;
    let mut source_manifest: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| Error::InvalidPart {
            part: manifest.display().to_string(),
            message: e.to_string(),
        })?;
    if source_manifest["schema_version"] != 1
        || source_manifest["template_sha256"] != template.inventory()["package"]["sha256"]
    {
        return Err(Error::InvalidPart {
            part: manifest.display().to_string(),
            message: "manifest version or template SHA-256 does not match".into(),
        });
    }
    // Manifest order is presentation order; each slide targets its own template part.
    let entries: Vec<(String, String)> = source_manifest
        .get("slides")
        .and_then(|v| v.as_array())
        .ok_or_else(|| Error::InvalidPart {
            part: manifest.display().to_string(),
            message: "slides must be an array".into(),
        })?
        .iter()
        .map(|entry| -> Result<(String, String)> {
            let field = |name: &str| {
                entry
                    .get(name)
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
                    .ok_or_else(|| Error::InvalidPart {
                        part: manifest.display().to_string(),
                        message: format!("slide entry requires {name}"),
                    })
            };
            Ok((field("source")?, field("part")?))
        })
        .collect::<Result<_>>()?;
    let mut slides = Vec::with_capacity(entries.len());
    let mut replacements = BTreeMap::new();
    for (source, part) in entries {
        let slide = read_slide(root, &source)?;
        let xml = match read_timing(root, &source)? {
            Some(timing) => compile_animated_slide(&slide, &timing.timeline, &timing.transition)?,
            None => compile_slide(&slide)?,
        };
        replacements.insert(part, xml);
        slides.push(slide);
    }
    let authored = template.edited(replacements, &BTreeSet::new(), Limits::default())?;
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
    }
    // This sidecar is authoritative until the custom key extension's save/move
    // survival is tested in the later sync task. It includes all nested objects.
    let sidecar = output.with_extension("manifest.json");
    update_mappings(&mut source_manifest, &slides)?;
    source_manifest["authored_static_source"] =
        serde_json::to_value(&slides).map_err(|e| Error::InvalidPart {
            part: "authored manifest".into(),
            message: e.to_string(),
        })?;
    let bytes = serde_json::to_vec_pretty(&source_manifest).map_err(|e| Error::InvalidPart {
        part: "authored manifest".into(),
        message: e.to_string(),
    })?;
    if output.exists() || sidecar.exists() {
        return Err(Error::InvalidPart {
            part: output.display().to_string(),
            message: "output or sidecar already exists; choose a fresh output".into(),
        });
    }
    authored.write(output)?;
    let mut sidecar_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&sidecar)
        .map_err(|e| io(&sidecar, e))?;
    sidecar_file
        .write_all(&bytes)
        .and_then(|_| sidecar_file.sync_all())
        .map_err(|e| io(&sidecar, e))?;
    println!("{}", output.display());
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 2 {
        return Err(Error::InvalidPart {
            part: "thesis example arguments".into(),
            message: "usage: thesis <private-source-directory> <new-output.pptx>".into(),
        });
    }
    build(&args[0], &args[1])
}

#[cfg(test)]
mod tests {
    use super::*;
    use deckwright::{
        model::{ChildSpace, Transform},
        units::Emu,
    };
    use serde_json::json;
    #[test]
    fn placement_map_tracks_nested_current_objects_and_retains_source_provenance() {
        let mut slide: Slide =
            serde_json::from_str(include_str!("../../fixtures/generated/authored-slide.json"))
                .unwrap();
        slide.objects[0].id = 999;
        slide.translate("heading", Emu(12700), Emu(0)).unwrap();
        let child = slide.objects.remove(0);
        slide.objects.push(Object {
            key: "new-group".into(),
            id: 100,
            name: "Group".into(),
            morph_name: None,
            description: None,
            kind: ObjectKind::Group {
                transform: Transform {
                    width: Emu(100),
                    height: Emu(100),
                    child: Some(ChildSpace {
                        x: Emu(0),
                        y: Emu(0),
                        width: Emu(100),
                        height: Emu(100),
                    }),
                    ..Transform::default()
                },
                children: vec![child],
            },
        });
        let original = json!({"key":"heading","source_id":2,"generated_id":2,"transform":{"x":100,"y":200,"width":1000,"height":2000}});
        let mut manifest =
            json!({"assets":[{"sha256":"unchanged"}],"slides":[{"placements":[original]}]});
        update_mappings(&mut manifest, &[slide.clone()]).unwrap();
        assert_eq!(
            manifest["slides"][0]["placements"][0]["source_id"],
            serde_json::Value::Null
        );
        assert_eq!(manifest["slides"][0]["placements"][1]["generated_id"], 999);
        assert_eq!(manifest["slides"][0]["placements"][1]["source_id"], 2);
        assert_eq!(
            manifest["slides"][0]["placements"][1]["transform"]["x"],
            12800
        );
        assert_eq!(
            manifest["slides"][0]["source_placements"][0]["transform"]["x"],
            100
        );
        assert!(
            manifest["slides"][0]["source_placements"][0]
                .get("generated_id")
                .is_none()
        );
        assert_eq!(manifest["assets"], json!([{"sha256":"unchanged"}]));
        let once = manifest.clone();
        update_mappings(&mut manifest, &[slide]).unwrap();
        assert_eq!(once, manifest);
    }
}
