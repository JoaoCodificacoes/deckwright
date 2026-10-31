#!/usr/bin/env python3
"""Compare authored native objects and retained template/media bytes exactly.

This checks semantic XML for every slide, in sldIdLst order, after excluding only
the documented source-key extension and explicit xml:space preservation. The
comparison has no element allowlist, so new authored elements (line dashes and
ends, endParaRPr, custom geometry, video media, underline) are compared too.
Timing is not compared here. It is independent of the Rust compiler and
complements, rather than replaces, PowerPoint rendering.
"""
import argparse
import hashlib
import json
from pathlib import Path
import xml.etree.ElementTree as ET
import zipfile
from extract_source import A, P, EXPECTED, digest, normalize_sha256, slide_parts


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


def parts_of(path):
    with zipfile.ZipFile(path) as z:
        return {n: z.read(n) for n in z.namelist()}


COUNTED = {
    "sp": ".//p:sp",
    "pic": ".//p:pic",
    "cxnSp": ".//p:cxnSp",
    "grpSp": ".//p:grpSp",
    "custGeom": ".//a:custGeom",
    "videoFile": ".//a:videoFile",
    "endParaRPr": ".//a:endParaRPr",
    "prstDash": ".//a:prstDash",
    "headEnd": ".//a:headEnd",
    "tailEnd": ".//a:tailEnd",
    "underlinedRuns": ".//a:rPr[@u]",
}


def verify(reference, generated, sha256=None):
    authorized = EXPECTED if sha256 is None else normalize_sha256(sha256)
    if digest(reference.read_bytes()) != authorized:
        raise ValueError("reference is not the authorized source")
    source, output = parts_of(reference), parts_of(generated)
    ns = {"p": P, "a": A}
    order = slide_parts(source)
    if slide_parts(output) != order:
        raise ValueError("generated slide list or part order differs from the source")
    reports = []
    for i, name in enumerate(order, 1):
        old = ET.fromstring(source[name])
        new = ET.fromstring(output[name])
        old_tree, new_tree = old.find("p:cSld/p:spTree", ns), new.find("p:cSld/p:spTree", ns)
        if len(old_tree) != len(new_tree):
            raise ValueError(f"slide {i}: native root child count changed")
        for original, authored in zip(old_tree, new_tree):
            if canonical(original) != canonical(authored):
                original_id = next((n.attrib.get("id") for n in original.iter() if n.tag == "{" + P + "}cNvPr"), "root")
                raise ValueError(f"slide {i}, object {original_id}: native semantic XML mismatch")
        counts = {label: len(new_tree.findall(path, ns)) for label, path in COUNTED.items()}
        reports.append({"slide": i, "part": name, "exact_native_semantics": True, "timing_present": new.find("p:timing", ns) is not None, "counts": counts})
    intentionally_edited = {"[Content_Types].xml"} | set(order)
    preserved = []
    for name, data in output.items():
        if name not in intentionally_edited:
            if source.get(name) != data:
                raise ValueError(f"unexpected imported part mutation: {name}")
            preserved.append(name)
    report = {
        "reference_sha256": authorized,
        "generated_sha256": hashlib.sha256(generated.read_bytes()).hexdigest(),
        "slide_count": len(order),
        "slides": reports,
        "timing_compared": False,
        "byte_preserved_imported_parts": len(preserved),
        "preserved_media_parts": sum(n.startswith("ppt/media/") for n in preserved),
        "powerpoint_visual_comparison": "separate required oracle gate",
    }
    print(json.dumps(report, indent=2))
    return report


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reference", type=Path)
    parser.add_argument("generated", type=Path)
    parser.add_argument("--sha256", help="authorize this explicitly named reference instead of the frozen one")
    args = parser.parse_args()
    verify(args.reference, args.generated, args.sha256)
