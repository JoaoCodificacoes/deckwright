//! Rebuild three editable static slides from private typed source and template.
use deckwright::{Error, Limits, Package, Result, builder::compile_slide, model::Slide};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::{Path, PathBuf},
};
mod slides {
    pub mod s01;
    pub mod s02;
    pub mod s03;
}

fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_owned(),
        source,
    }
}
fn read_slide(root: &Path, filename: &str) -> Result<Slide> {
    let path = root.join("slides").join(filename);
    let data = std::fs::read(&path).map_err(|e| io(&path, e))?;
    serde_json::from_slice(&data).map_err(|e| Error::InvalidPart {
        part: path.display().to_string(),
        message: e.to_string(),
    })
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
    let slides = [
        slides::s01::build(root)?,
        slides::s02::build(root)?,
        slides::s03::build(root)?,
    ];
    let mut replacements = BTreeMap::new();
    for (index, slide) in slides.iter().enumerate() {
        replacements.insert(
            format!("ppt/slides/slide{}.xml", index + 1),
            compile_slide(slide)?,
        );
    }
    let authored = template.edited(replacements, &BTreeSet::new(), Limits::default())?;
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
    }
    // This sidecar is authoritative until the custom key extension's save/move
    // survival is tested in the later sync task. It includes all nested objects.
    let sidecar = output.with_extension("manifest.json");
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
