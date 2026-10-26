#!/usr/bin/env python3
"""One-time private transcription to typed data, not an authoring-time XML copier.

Every supported authored element/attribute is checked explicitly. Unknown source
features fail closed. Template/layout/master/media bytes are kept opaque. Timing
and transitions are intentionally excluded from this STATIC milestone.
"""
import argparse
import hashlib
import json
import posixpath
import re
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

A = "http://schemas.openxmlformats.org/drawingml/2006/main"
P = "http://schemas.openxmlformats.org/presentationml/2006/main"
R = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
EXPECTED = "ee8cce03cf0201150fdd6f9c75f5d551538ed90d6ffae1c417e82b939e027e2b"
NS = {"a": A, "p": P, "r": R}


def local(tag):
    return tag.rsplit("}", 1)[-1]


def checked(n, attrs=(), children=()):
    if n is None:
        raise ValueError("missing required source element")
    unknown_attrs = set(n.attrib) - set(attrs)
    unknown_children = {local(c.tag) for c in n} - set(children)
    if unknown_attrs or unknown_children:
        raise ValueError(f"unsupported {local(n.tag)}: attrs={unknown_attrs}, children={unknown_children}")
    if not n.tag.startswith(("{" + A + "}", "{" + P + "}")):
        raise ValueError(f"unexpected authored namespace: {n.tag}")
    return n


def one(n, name):
    found = [c for c in n if local(c.tag) == name]
    if len(found) > 1:
        raise ValueError(f"multiple {name} under {local(n.tag)}")
    return found[0] if found else None


def integer(n, source, target, out):
    if source in n.attrib:
        out[target] = int(n.attrib[source])


def boolean(value):
    if value not in ("0", "1"):
        raise ValueError(f"unsupported boolean {value}")
    return value == "1"


def color(n):
    checked(n, ("val",), ("alpha",))
    kind = {"srgbClr": "rgb", "schemeClr": "scheme"}.get(local(n.tag))
    if kind is None:
        raise ValueError(f"unsupported color {n.tag}")
    out = {"type": kind, "value": n.attrib["val"]}
    alpha = one(n, "alpha")
    if alpha is not None:
        checked(alpha, ("val",))
        out["alpha"] = int(alpha.attrib["val"])
    return out


def fill(n):
    if local(n.tag) == "noFill":
        checked(n)
        return {"type": "none"}
    checked(n, (), ("srgbClr", "schemeClr"))
    if len(n) != 1:
        raise ValueError("solidFill requires one color")
    return {"type": "solid", "color": color(n[0])}


def transform(n):
    checked(n, ("rot", "flipH", "flipV"), ("off", "ext", "chOff", "chExt"))
    out = {}
    for child, attrs, targets in [("off", ("x", "y"), ("x", "y")), ("ext", ("cx", "cy"), ("width", "height"))]:
        c = checked(one(n, child), attrs)
        out.update({t: int(c.attrib[a]) for a, t in zip(attrs, targets)})
    integer(n, "rot", "rotation", out)
    for a, target in [("flipH", "flip_h"), ("flipV", "flip_v")]:
        if a in n.attrib:
            out[target] = boolean(n.attrib[a])
    if one(n, "chOff") is not None or one(n, "chExt") is not None:
        out["child"] = {}
        for child, attrs, targets in [("chOff", ("x", "y"), ("x", "y")), ("chExt", ("cx", "cy"), ("width", "height"))]:
            c = checked(one(n, child), attrs)
            out["child"].update({t: int(c.attrib[a]) for a, t in zip(attrs, targets)})
    return out


def properties(n):
    checked(n, (), ("xfrm", "prstGeom", "solidFill", "noFill", "ln", "effectLst"))
    out = {}
    for c in n:
        tag = local(c.tag)
        if tag == "xfrm":
            out["transform"] = transform(c)
        elif tag == "prstGeom":
            checked(c, ("prst",), ("avLst",))
            av = checked(one(c, "avLst"), (), ("gd",))
            adjustments = []
            for gd in av:
                checked(gd, ("name", "fmla"))
                match = re.fullmatch(r"val (-?\d+)", gd.attrib["fmla"])
                if not match:
                    raise ValueError("nonliteral geometry adjustment")
                adjustments.append({"name": gd.attrib["name"], "value": int(match[1])})
            out["geometry"] = {"preset": c.attrib["prst"], "adjustments": adjustments}
        elif tag in ("solidFill", "noFill"):
            out["fill"] = fill(c)
        elif tag == "ln":
            checked(c, ("w",), ("solidFill", "noFill"))
            line = {}
            integer(c, "w", "width", line)
            if len(c) == 1:
                line["fill"] = fill(c[0])
            elif len(c) > 1:
                raise ValueError("multiple line fills")
            out["line"] = line
        elif tag == "effectLst":
            checked(c, (), ("outerShdw",))
            shadows = []
            for s in c:
                checked(s, ("blurRad", "dist", "dir", "algn", "rotWithShape"), ("srgbClr", "schemeClr"))
                if len(s) != 1:
                    raise ValueError("shadow requires one color")
                shadows.append({"blur": int(s.attrib["blurRad"]), "distance": int(s.attrib["dist"]), "direction": int(s.attrib["dir"]), "alignment": s.attrib["algn"], "rotate_with_shape": boolean(s.attrib["rotWithShape"]), "color": color(s[0])})
            out["effects"] = shadows
    return out


def style(n):
    checked(n, (), ("lnRef", "fillRef", "effectRef", "fontRef"))
    out = {}
    for tag, target in [("lnRef", "line"), ("fillRef", "fill"), ("effectRef", "effect"), ("fontRef", "font")]:
        c = checked(one(n, tag), ("idx",), ("schemeClr", "srgbClr"))
        if len(c) != 1:
            raise ValueError("style reference requires one color")
        out[target] = {"index": c.attrib["idx"], "color": color(c[0])}
    return out


def text(n):
    checked(n, (), ("bodyPr", "lstStyle", "p"))
    b = checked(one(n, "bodyPr"), ("wrap", "anchor", "lIns", "rIns", "tIns", "bIns", "rtlCol"), ("spAutoFit",))
    body = {a: b.attrib[a] for a in ("wrap", "anchor") if a in b.attrib}
    for a, target in [("lIns", "left"), ("rIns", "right"), ("tIns", "top"), ("bIns", "bottom")]:
        integer(b, a, target, body)
    if "rtlCol" in b.attrib:
        body["rtl_columns"] = boolean(b.attrib["rtlCol"])
    if one(b, "spAutoFit") is not None:
        checked(one(b, "spAutoFit"))
        body["shape_autofit"] = True
    out = {"body": body, "list_style": one(n, "lstStyle") is not None, "paragraphs": []}
    if one(n, "lstStyle") is not None:
        checked(one(n, "lstStyle"))
    for p in [c for c in n if local(c.tag) == "p"]:
        checked(p, (), ("pPr", "r", "fld"))
        paragraph = {"runs": []}
        pp = one(p, "pPr")
        if pp is not None:
            checked(pp, ("algn",), ("spcBef", "spcAft"))
            paragraph["properties"] = {}
            if "algn" in pp.attrib:
                paragraph["properties"]["alignment"] = pp.attrib["algn"]
            for tag, target in [("spcBef", "space_before"), ("spcAft", "space_after")]:
                spacing = one(pp, tag)
                if spacing is not None:
                    checked(spacing, (), ("spcPts",))
                    points = checked(one(spacing, "spcPts"), ("val",))
                    paragraph["properties"][target] = int(points.attrib["val"])
        for r in [c for c in p if local(c.tag) in ("r", "fld")]:
            checked(r, ("id", "type") if local(r.tag) == "fld" else (), ("rPr", "t"))
            t = checked(one(r, "t"))
            run = {"text": t.text or ""}
            if local(r.tag) == "fld":
                run["field"] = {"id": r.attrib["id"], "kind": r.attrib["type"]}
            rp = one(r, "rPr")
            if rp is not None:
                checked(rp, ("sz", "b", "i"), ("solidFill", "latin"))
                rs = {}
                integer(rp, "sz", "size", rs)
                for a, target in [("b", "bold"), ("i", "italic")]:
                    if a in rp.attrib:
                        rs[target] = boolean(rp.attrib[a])
                latin = one(rp, "latin")
                if latin is not None:
                    checked(latin, ("typeface",))
                    rs["font"] = latin.attrib["typeface"]
                solid = one(rp, "solidFill")
                if solid is not None:
                    rs["color"] = fill(solid)["color"]
                run["style"] = rs
            paragraph["runs"].append(run)
        out["paragraphs"].append(paragraph)
    return out


def object_source(n, slide):
    tag = local(n.tag)
    names = {"sp": ("shape", "nvSpPr", "cNvSpPr"), "pic": ("picture", "nvPicPr", "cNvPicPr"), "cxnSp": ("connector", "nvCxnSpPr", "cNvCxnSpPr"), "grpSp": ("group", "nvGrpSpPr", "cNvGrpSpPr")}
    kind, nvtag, cvtag = names[tag]
    checked(n, (), (nvtag, "spPr", "grpSpPr", "style", "txBody", "blipFill", "sp", "pic", "cxnSp", "grpSp"))
    nv = checked(one(n, nvtag), (), ("cNvPr", cvtag, "nvPr"))
    cn = checked(one(nv, "cNvPr"), ("id", "name", "descr"))
    native_id = int(cn.attrib["id"])
    name = cn.attrib["name"]
    out = {"key": f"s{slide:02}/object-{native_id}", "id": native_id, "name": name, "kind": {"type": kind}}
    if name.startswith("!!"):
        out["morph_name"] = name
    if "descr" in cn.attrib:
        out["description"] = cn.attrib["descr"]
    k = out["kind"]
    cv = checked(one(nv, cvtag), ("txBox",), ("spLocks", "picLocks", "stCxn", "endCxn"))
    nvpr = checked(one(nv, "nvPr"), (), ("ph",))
    if kind == "shape":
        if "txBox" in cv.attrib:
            k["textbox"] = boolean(cv.attrib["txBox"])
        locks = one(cv, "spLocks")
        if locks is not None:
            checked(locks, ("noGrp",))
            k["lock_no_group"] = boolean(locks.attrib["noGrp"])
        ph = one(nvpr, "ph")
        if ph is not None:
            checked(ph, ("type", "idx", "sz"))
            k["placeholder"] = {"kind": ph.attrib["type"]}
            integer(ph, "idx", "index", k["placeholder"])
            if "sz" in ph.attrib:
                k["placeholder"]["size"] = ph.attrib["sz"]
        if one(n, "txBody") is not None:
            k["text"] = text(one(n, "txBody"))
    elif kind == "picture":
        locks = one(cv, "picLocks")
        if locks is not None:
            checked(locks, ("noChangeAspect",))
            k["lock_aspect"] = boolean(locks.attrib["noChangeAspect"])
        blip = checked(one(n, "blipFill"), (), ("blip", "srcRect", "stretch"))
        image = checked(one(blip, "blip"), ("{" + R + "}embed",))
        k["relationship"] = image.attrib["{" + R + "}embed"]
        stretch = checked(one(blip, "stretch"), (), ("fillRect",))
        checked(one(stretch, "fillRect"))
        crop = one(blip, "srcRect")
        if crop is not None:
            checked(crop, ("l", "t", "r", "b"))
            k["crop"] = {t: int(crop.attrib.get(a, 0)) for a, t in [("l", "left"), ("t", "top"), ("r", "right"), ("b", "bottom")]}
    elif kind == "connector":
        for tag, target in [("stCxn", "start"), ("endCxn", "end")]:
            endpoint = one(cv, tag)
            if endpoint is not None:
                checked(endpoint, ("id", "idx"))
                k[target] = {"object_id": int(endpoint.attrib["id"]), "index": int(endpoint.attrib["idx"])}
    if kind == "group":
        gp = checked(one(n, "grpSpPr"), (), ("xfrm",))
        k["transform"] = transform(one(gp, "xfrm"))
        k["children"] = [object_source(c, slide) for c in n if local(c.tag) in names]
    else:
        k["properties"] = properties(one(n, "spPr"))
        if one(n, "style") is not None:
            k["style"] = style(one(n, "style"))
    return out


def digest(data):
    return hashlib.sha256(data).hexdigest()


def extract(source, output):
    source_resolved, output_resolved = source.resolve(), output.resolve()
    if source_resolved == output_resolved or output_resolved in source_resolved.parents:
        raise ValueError("source/output aliases or source is inside output directory")
    data = source.read_bytes()
    if digest(data) != EXPECTED:
        raise ValueError("source is not the frozen authorized reference")
    if output.exists():
        raise ValueError("source output already exists; choose a fresh directory")
    with zipfile.ZipFile(source) as z:
        parts = {name: z.read(name) for name in z.namelist()}
    slides = []
    for i in range(1, 4):
        root = ET.fromstring(parts[f"ppt/slides/slide{i}.xml"])
        tree = root.find("p:cSld/p:spTree", NS)
        # Slide-level properties beyond static color mapping/timing require review.
        allowed = {"cSld", "clrMapOvr", "AlternateContent", "timing"}
        if root.attrib or {local(c.tag) for c in root} - allowed:
            raise ValueError("unsupported slide root properties")
        cslide = root.find("p:cSld", NS)
        checked(cslide, (), ("spTree",))
        checked(tree, (), ("nvGrpSpPr", "grpSpPr", "sp", "pic", "cxnSp", "grpSp"))
        slides.append({"objects": [object_source(c, i) for c in tree if local(c.tag) in ("sp", "pic", "cxnSp", "grpSp")]})
        # Template contains no source authored content, only an empty slide shell.
        parts[f"ppt/slides/slide{i}.xml"] = (f'<p:sld xmlns:p="{P}" xmlns:a="{A}"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld></p:sld>').encode()
    presentation = parts["ppt/presentation.xml"]
    ids = list(re.finditer(rb"<p:sldId\b[^>]*/>", presentation))
    if len(ids) != 25:
        raise ValueError("unexpected presentation slide list encoding")
    keep_ids = {ET.fromstring(m[0].decode().replace("<p:sldId", f'<p:sldId xmlns:p="{P}" xmlns:r="{R}"')).attrib["{" + R + "}id"] for m in ids[:3]}
    for match in reversed(ids[3:]):
        presentation = presentation[:match.start()] + presentation[match.end():]
    parts["ppt/presentation.xml"] = presentation
    relname = "ppt/_rels/presentation.xml.rels"
    relbytes = parts[relname]
    for match in reversed(list(re.finditer(rb"<Relationship\b[^>]*/>", relbytes))):
        rel = ET.fromstring(match[0])
        if rel.attrib["Type"].endswith("/slide") and rel.attrib["Id"] not in keep_ids:
            relbytes = relbytes[:match.start()] + relbytes[match.end():]
    parts[relname] = relbytes
    # Keep exactly the opaque dependency closure of the three blank template slides.
    seen = set()
    def visit(name):
        if name in seen:
            return
        if name not in parts:
            raise ValueError(f"missing template dependency {name}")
        seen.add(name)
        directory, file = posixpath.split(name)
        relpath = posixpath.join(directory, "_rels", file + ".rels") if name else "_rels/.rels"
        if relpath in parts:
            seen.add(relpath)
            for rel in ET.fromstring(parts[relpath]):
                if rel.attrib.get("TargetMode") != "External":
                    from urllib.parse import unquote
                    target = unquote(rel.attrib["Target"].split("#")[0])
                    target = posixpath.normpath(target.lstrip("/")) if target.startswith("/") else posixpath.normpath(posixpath.join(directory, target))
                    visit(target)
    # Root relationships have the package root as source.
    seen.add("_rels/.rels")
    for rel in ET.fromstring(parts["_rels/.rels"]):
        if rel.attrib.get("TargetMode") != "External":
            visit(rel.attrib["Target"].lstrip("/"))
    ct = parts["[Content_Types].xml"]
    for match in reversed(list(re.finditer(rb"<Override\b[^>]*/>", ct))):
        if ET.fromstring(match[0]).attrib["PartName"].lstrip("/") not in seen:
            ct = ct[:match.start()] + ct[match.end():]
    parts["[Content_Types].xml"] = ct
    seen.add("[Content_Types].xml")
    output.mkdir(parents=True)
    (output / "slides").mkdir()
    for i, slide in enumerate(slides, 1):
        with (output / "slides" / f"s{i:02}.json").open("x", encoding="utf-8") as file:
            file.write(json.dumps(slide, ensure_ascii=False, indent=2))
    with zipfile.ZipFile(output / "template.pptx", "x", compression=zipfile.ZIP_DEFLATED) as z:
        for name in sorted(seen):
            info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            z.writestr(info, parts[name])
    assets = [{"part": n, "sha256": digest(parts[n]), "bytes": len(parts[n])} for n in sorted(seen) if n.startswith("ppt/media/")]
    def placements(objects):
        result = []
        for o in objects:
            k = o["kind"]
            result.append({"key": o["key"], "source_id": o["id"], "generated_id": o["id"], "kind": k["type"], "morph_name": o.get("morph_name"), "transform": k.get("transform", k.get("properties", {}).get("transform")), "relationship": k.get("relationship"), "crop": k.get("crop")})
            result.extend(placements(k.get("children", [])))
        return result
    manifest = {"schema_version": 1, "source_sha256": EXPECTED, "template_sha256": digest((output / "template.pptx").read_bytes()), "static_only": True, "source_key_carrier": "provisional OOXML extension plus authoritative sidecar", "slides": [{"source_slide": i, "generated_slide": i, "source": f"slides/s{i:02}.json", "placements": placements(s["objects"])} for i, s in enumerate(slides, 1)], "assets": assets, "omissions": ["timing", "transitions"], "unsupported_authored_features": []}
    with (output / "manifest.json").open("x", encoding="utf-8") as file:
        file.write(json.dumps(manifest, ensure_ascii=False, indent=2))
    print(json.dumps({"output": str(output), "slides": [len(s["placements"]) for s in manifest["slides"]], "assets": len(assets)}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    extract(args.source, args.output)
