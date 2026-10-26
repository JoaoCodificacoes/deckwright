#!/usr/bin/env python3
"""Compare authored native objects and retained template/media bytes exactly.

This checks semantic XML after excluding only the documented source-key
extension and explicit xml:space preservation. It is independent of the Rust
compiler and complements, rather than replaces, PowerPoint rendering.
"""
import argparse
import hashlib
import json
from pathlib import Path
import xml.etree.ElementTree as ET
import zipfile
from extract_source import A, P, EXPECTED


def canonical(n):
    attrs = dict(n.attrib)
    if n.tag == "{" + A + "}t":
        attrs.pop("{http://www.w3.org/XML/1998/namespace}space", None)
    children = []
    for c in n:
        if c.tag == "{" + A + "}extLst" and n.tag == "{" + P + "}cNvPr":
            # Only the explicit Deckwright carrier is excluded, never other extensions.
            rest = [e for e in c if e.attrib.get("uri") != "urn:deckwright:source:v1"]
            if not rest:
                continue
            c = ET.fromstring(ET.tostring(c))
            for e in list(c):
                if e.attrib.get("uri") == "urn:deckwright:source:v1":
                    c.remove(e)
        children.append(canonical(c))
    return (n.tag, tuple(sorted(attrs.items())), n.text or "", tuple(children))


def verify(reference, generated):
    if hashlib.sha256(reference.read_bytes()).hexdigest() != EXPECTED:
        raise ValueError("reference is not the frozen authorized source")
    with zipfile.ZipFile(reference) as z:
        source = {n: z.read(n) for n in z.namelist()}
    with zipfile.ZipFile(generated) as z:
        output = {n: z.read(n) for n in z.namelist()}
    ns = {"p": P, "a": A}
    reports = []
    generated_presentation = ET.fromstring(output["ppt/presentation.xml"])
    if len(generated_presentation.findall("p:sldIdLst/p:sldId", ns)) != 3:
        raise ValueError("generated presentation is not three slides")
    for i in range(1, 4):
        name = f"ppt/slides/slide{i}.xml"
        old = ET.fromstring(source[name])
        new = ET.fromstring(output[name])
        old_tree, new_tree = old.find("p:cSld/p:spTree", ns), new.find("p:cSld/p:spTree", ns)
        if len(old_tree) != len(new_tree):
            raise ValueError(f"slide {i}: native root child count changed")
        for original, authored in zip(old_tree, new_tree):
            if canonical(original) != canonical(authored):
                original_id = next((n.attrib.get("id") for n in original.iter() if n.tag == "{" + P + "}cNvPr"), "root")
                raise ValueError(f"slide {i}, object {original_id}: native semantic XML mismatch")
        if new.find("p:timing", ns) is not None:
            raise ValueError("static output unexpectedly has timing")
        counts = {kind: len(new_tree.findall(".//p:" + kind, ns)) for kind in ("sp", "pic", "cxnSp", "grpSp")}
        reports.append({"slide": i, "exact_native_semantics": True, "counts": counts})
    intentionally_edited = {"[Content_Types].xml", "ppt/presentation.xml", "ppt/_rels/presentation.xml.rels"} | {f"ppt/slides/slide{i}.xml" for i in range(1, 4)}
    preserved = []
    for name, data in output.items():
        if name not in intentionally_edited:
            if source.get(name) != data:
                raise ValueError(f"unexpected imported part mutation: {name}")
            preserved.append(name)
    report = {"reference_sha256": EXPECTED, "generated_sha256": hashlib.sha256(generated.read_bytes()).hexdigest(), "slides": reports, "byte_preserved_imported_parts": len(preserved), "preserved_media_parts": sum(n.startswith("ppt/media/") for n in preserved), "intentional_static_omissions": ["transitions", "timing"], "powerpoint_visual_comparison": "separate required oracle gate"}
    print(json.dumps(report, indent=2))
    return report


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reference", type=Path)
    parser.add_argument("generated", type=Path)
    args = parser.parse_args()
    verify(args.reference, args.generated)
