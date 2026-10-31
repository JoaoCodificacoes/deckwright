#!/usr/bin/env python3
"""Private integration check: source edit -> intended native object only.

Run against the compiled Rust example and prepared private source directory.
The baseline and mutation must be built by Rust, never by this check script.
The mutated object is the first top-level text shape with a plain first run, in
manifest slide order, so the check works for any slide count.
"""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET
import zipfile
from verify_static import canonical
from extract_source import A, P


def run(binary, source, output):
    return subprocess.run([str(binary), str(source), str(output)], check=True, capture_output=True, text=True)


def parts(path):
    with zipfile.ZipFile(path) as zip:
        return {name: zip.read(name) for name in zip.namelist()}


def choose_target(manifest, source_dir):
    """Return (slide index, manifest entry, slide JSON, object) for the first eligible shape."""
    for index, entry in enumerate(manifest["slides"]):
        slide = json.loads((source_dir / entry["source"]).read_text(encoding="utf-8"))
        for obj in slide["objects"]:
            kind = obj["kind"]
            paragraphs = kind.get("text", {}).get("paragraphs", [])
            transform = kind.get("properties", {}).get("transform")
            if kind["type"] == "shape" and transform and paragraphs and paragraphs[0]["runs"] and "field" not in paragraphs[0]["runs"][0]:
                return index, entry, slide, obj
    raise ValueError("no plain text shape to mutate")


def check(binary, source):
    with tempfile.TemporaryDirectory(prefix="deckwright-authoring-") as temp:
        root = Path(temp)
        run(binary, source, root / "baseline.pptx")
        run(binary, source, root / "repeat.pptx")
        original_bytes = (root / "baseline.pptx").read_bytes()
        if original_bytes != (root / "repeat.pptx").read_bytes():
            raise ValueError("identical builds are not deterministic")
        copied = root / "source"
        shutil.copytree(source, copied)
        source_manifest = json.loads((copied / "manifest.json").read_text(encoding="utf-8"))
        index, entry, slide, target = choose_target(source_manifest, copied)
        path = copied / entry["source"]
        part = entry["part"]
        old_target = copy.deepcopy(target)
        target["id"] = 999
        run_data = target["kind"]["text"]["paragraphs"][0]["runs"][0]
        run_data["text"] += "!"  # a one-character source change
        target["kind"]["properties"]["transform"]["x"] += 12700  # one point
        path.write_text(json.dumps(slide, ensure_ascii=False), encoding="utf-8")
        run(binary, copied, root / "mutated.pptx")
        before, after = parts(root / "baseline.pptx"), parts(root / "mutated.pptx")
        changed_parts = [name for name in before if before[name] != after[name]]
        if changed_parts != [part]:
            raise ValueError(f"unrelated native/template/media parts changed: {changed_parts}")
        ns = {"p": P, "a": A}
        trees = [ET.fromstring(p[part]).find("p:cSld/p:spTree", ns) for p in (before, after)]
        changed_objects = []
        for original, authored in zip(*trees):
            if canonical(original) != canonical(authored):
                object_id = next(n.attrib["id"] for n in authored.iter() if n.tag == "{" + P + "}cNvPr")
                changed_objects.append(object_id)
                text = authored.find("p:txBody/a:p/a:r/a:t", ns).text
                x = int(authored.find("p:spPr/a:xfrm/a:off", ns).attrib["x"])
                if text != run_data["text"] or x != old_target["kind"]["properties"]["transform"]["x"] + 12700:
                    raise ValueError("intended native text/geometry mutation missing")
        if changed_objects != ["999"]:
            raise ValueError(f"mutation changed wrong native objects: {changed_objects}")
        baseline_map = json.loads((root / "baseline.manifest.json").read_text())
        mutated_map = json.loads((root / "mutated.manifest.json").read_text())
        mutated_entry = mutated_map["slides"][index]
        placement = next(p for p in mutated_entry["placements"] if p["key"] == target["key"])
        authored_target = next(o for o in mutated_map["authored_static_source"][index]["objects"] if o["key"] == target["key"])
        if placement["generated_id"] != 999 or placement["transform"] != authored_target["kind"]["properties"]["transform"]:
            raise ValueError("current sidecar identity/placement is stale after typed edits")
        original = next(p for p in mutated_entry["source_placements"] if p["key"] == target["key"])
        if original["source_id"] != old_target["id"] or original["transform"] != old_target["kind"]["properties"]["transform"]:
            raise ValueError("original source provenance was changed by authoring")
        if (baseline_map["assets"] != mutated_map["assets"]
                or baseline_map["slides"][:index] != mutated_map["slides"][:index]
                or baseline_map["slides"][index + 1:] != mutated_map["slides"][index + 1:]):
            raise ValueError("unrelated sidecar source/asset/generated mappings changed")
        # Existing PPTX and sidecar outputs must survive a repeated invocation.
        sidecar = root / "baseline.manifest.json"
        old_sidecar = sidecar.read_bytes()
        repeated = subprocess.run([str(binary), str(source), str(root / "baseline.pptx")], capture_output=True)
        if repeated.returncode == 0 or (root / "baseline.pptx").read_bytes() != original_bytes or sidecar.read_bytes() != old_sidecar:
            raise ValueError("existing output protection failed")
        report = {
            "slide_count": len(source_manifest["slides"]),
            "mutated_slide": index + 1,
            "deterministic": True,
            "baseline_sha256": hashlib.sha256(original_bytes).hexdigest(),
            "changed_parts": changed_parts,
            "changed_native_objects": changed_objects,
            "text_mutation": "one character",
            "layout_mutation_emu": 12700,
            "current_manifest_matches_generated_id_and_geometry": True,
            "source_provenance_preserved": True,
            "unrelated_manifest_data_unchanged": True,
            "all_unrelated_template_media_native_objects_unchanged": True,
            "existing_outputs_protected": True,
        }
        print(json.dumps(report, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("private_source", type=Path)
    args = parser.parse_args()
    check(args.binary.resolve(), args.private_source.resolve())
