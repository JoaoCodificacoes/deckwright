#!/usr/bin/env python3
"""One-time private transcription to typed data, not an authoring-time XML copier.

Every supported authored element/attribute is checked explicitly. Unknown source
features fail closed. Template/layout/master/media bytes are kept opaque. Every
slide is extracted in presentation sldIdLst order. Slide timing and transitions
are never copied from XML: they come only from `dw query timeline` when --dw is
given, and are otherwise omitted (the static build).
"""
import argparse
import hashlib
import json
import posixpath
import re
import subprocess
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path
from urllib.parse import unquote

A = "http://schemas.openxmlformats.org/drawingml/2006/main"
P = "http://schemas.openxmlformats.org/presentationml/2006/main"
R = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
MEDIA_NS = "http://schemas.microsoft.com/office/powerpoint/2010/main"
MEDIA_EXT = "{DAA4B4D4-6D71-4841-9C94-3DE7FCFB9230}"
MEDIA_ACTION = "ppaction://media"
EXPECTED = "ee8cce03cf0201150fdd6f9c75f5d551538ed90d6ffae1c417e82b939e027e2b"
NS = {"a": A, "p": P, "r": R}
OBJECT_TAGS = ("sp", "pic", "cxnSp", "grpSp")
SLIDE_ROOT_CHILDREN = {"cSld", "clrMapOvr", "AlternateContent", "timing", "transition"}
PRESET_DASHES = frozenset({"solid", "dot", "dash", "lgDash", "dashDot", "lgDashDot", "lgDashDotDot", "sysDash", "sysDot", "sysDashDot", "sysDashDotDot"})
LINE_END_KINDS = frozenset({"none", "triangle", "stealth", "diamond", "oval", "arrow"})
LINE_END_SIZES = frozenset({"sm", "med", "lg"})
FILL_MODES = frozenset({"none", "norm", "lighten", "lightenLess", "darken", "darkenLess"})
UNDERLINES = frozenset({"none", "words", "sng", "dbl", "heavy", "dotted", "dottedHeavy", "dash", "dashHeavy", "dashLong", "dashLongHeavy", "dotDash", "dotDashHeavy", "dotDotDash", "dotDotDashHeavy", "wavy", "wavyHeavy", "wavyDbl"})
LINE_ORDER = {"solidFill": 0, "noFill": 0, "prstDash": 1, "headEnd": 2, "tailEnd": 3}
BLANK_SLIDE = f'<p:sld xmlns:p="{P}" xmlns:a="{A}"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld></p:sld>'.encode()


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


def required(n, attr):
    if attr not in n.attrib:
        raise ValueError(f"missing {attr} on {local(n.tag)}")
    return n.attrib[attr]


def strict_int(text):
    if not re.fullmatch(r"-?\d+", text):
        raise ValueError("nonliteral integer value")
    return int(text)


def integer(n, source, target, out):
    if source in n.attrib:
        out[target] = int(n.attrib[source])


def boolean(value):
    if value not in ("0", "1"):
        raise ValueError("unsupported boolean value")
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


def line_end(n):
    checked(n, ("type", "w", "len"))
    kind = required(n, "type")
    if kind not in LINE_END_KINDS:
        raise ValueError("unsupported line end kind")
    out = {"kind": kind}
    for attr, target in [("w", "width"), ("len", "length")]:
        if attr in n.attrib:
            if n.attrib[attr] not in LINE_END_SIZES:
                raise ValueError("unsupported line end size")
            out[target] = n.attrib[attr]
    return out


def line(c):
    checked(c, ("w",), ("solidFill", "noFill", "prstDash", "headEnd", "tailEnd"))
    if [LINE_ORDER[local(x.tag)] for x in c] != sorted(LINE_ORDER[local(x.tag)] for x in c):
        raise ValueError("unsupported line child order")
    out = {}
    integer(c, "w", "width", out)
    fills = [x for x in c if local(x.tag) in ("solidFill", "noFill")]
    if len(fills) > 1:
        raise ValueError("multiple line fills")
    if fills:
        out["fill"] = fill(fills[0])
    dash = one(c, "prstDash")
    if dash is not None:
        checked(dash, ("val",))
        if required(dash, "val") not in PRESET_DASHES:
            raise ValueError("unsupported line dash")
        out["dash"] = dash.attrib["val"]
    for tag, target in [("headEnd", "head_end"), ("tailEnd", "tail_end")]:
        end = one(c, tag)
        if end is not None:
            out[target] = line_end(end)
    return out


def geometry_command(c):
    name = local(c.tag)
    if name == "close":
        checked(c)
        return {"type": "close"}
    if name == "arcTo":
        checked(c, ("wR", "hR", "stAng", "swAng"))
        return {
            "type": "arc_to",
            "width_radius": strict_int(required(c, "wR")),
            "height_radius": strict_int(required(c, "hR")),
            "start_angle": strict_int(required(c, "stAng")),
            "swing_angle": strict_int(required(c, "swAng")),
        }
    kinds = {"moveTo": (1, "move_to", ("x", "y")), "lnTo": (1, "line_to", ("x", "y")), "quadBezTo": (2, "quad_to", ("x1", "y1", "x", "y")), "cubicBezTo": (3, "cubic_to", ("x1", "y1", "x2", "y2", "x", "y"))}
    if name not in kinds:
        raise ValueError("unsupported path command")
    count, kind, keys = kinds[name]
    checked(c, (), ("pt",))
    points = list(c)
    if len(points) != count:
        raise ValueError("unsupported path point count")
    values = []
    for p in points:
        checked(p, ("x", "y"))
        values += [strict_int(required(p, "x")), strict_int(required(p, "y"))]
    return {"type": kind, **dict(zip(keys, values))}


def geometry_path(p):
    checked(p, ("w", "h", "fill", "stroke"), ("moveTo", "lnTo", "quadBezTo", "cubicBezTo", "arcTo", "close"))
    out = {"width": strict_int(required(p, "w")), "height": strict_int(required(p, "h"))}
    if "fill" in p.attrib:
        if p.attrib["fill"] not in FILL_MODES:
            raise ValueError("unsupported path fill mode")
        out["fill"] = p.attrib["fill"]
    out["stroke"] = boolean(p.attrib["stroke"]) if "stroke" in p.attrib else True
    out["commands"] = [geometry_command(c) for c in p]
    return out


def custom_geometry(n):
    checked(n, (), ("avLst", "gdLst", "ahLst", "cxnLst", "rect", "pathLst"))
    if [local(c.tag) for c in n] != ["avLst", "gdLst", "ahLst", "cxnLst", "rect", "pathLst"]:
        raise ValueError("unsupported custGeom child order")
    for name in ("avLst", "gdLst", "ahLst", "cxnLst"):
        checked(one(n, name))  # present and empty; any guide or adjustment fails closed
    rect = checked(one(n, "rect"), ("l", "t", "r", "b"))
    if rect.attrib != {"l": "l", "t": "t", "r": "r", "b": "b"}:
        raise ValueError("unsupported custGeom text rectangle")
    paths = checked(one(n, "pathLst"), (), ("path",))
    if len(paths) == 0:
        raise ValueError("custGeom requires at least one path")
    return {"paths": [geometry_path(p) for p in paths]}


def properties(n):
    checked(n, (), ("xfrm", "prstGeom", "custGeom", "solidFill", "noFill", "ln", "effectLst"))
    if sum(local(c.tag) in ("prstGeom", "custGeom") for c in n) > 1:
        raise ValueError("multiple geometries")
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
        elif tag == "custGeom":
            out["custom"] = custom_geometry(c)
        elif tag in ("solidFill", "noFill"):
            out["fill"] = fill(c)
        elif tag == "ln":
            out["line"] = line(c)
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


def run_style(rp):
    """Shared by a:rPr and a:endParaRPr (CT_TextCharacterProperties)."""
    checked(rp, ("sz", "b", "i", "u"), ("solidFill", "latin"))
    rs = {}
    integer(rp, "sz", "size", rs)
    for a, target in [("b", "bold"), ("i", "italic")]:
        if a in rp.attrib:
            rs[target] = boolean(rp.attrib[a])
    if "u" in rp.attrib:
        if rp.attrib["u"] not in UNDERLINES:
            raise ValueError("unsupported underline value")
        rs["underline"] = rp.attrib["u"]
    latin = one(rp, "latin")
    if latin is not None:
        checked(latin, ("typeface",))
        rs["font"] = required(latin, "typeface")
    solid = one(rp, "solidFill")
    if solid is not None:
        rs["color"] = fill(solid)["color"]
    return rs


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
        checked(p, (), ("pPr", "r", "fld", "endParaRPr"))
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
                run["style"] = run_style(rp)
            paragraph["runs"].append(run)
        end = one(p, "endParaRPr")
        if end is not None:
            if p[-1] is not end:
                raise ValueError("endParaRPr must be the last paragraph child")
            paragraph["end_style"] = run_style(end)
        out["paragraphs"].append(paragraph)
    return out


def picture_media(cn, nvpr):
    """Video pictures: a:videoFile, p14:media in p:extLst, and the media hlinkClick."""
    video = one(nvpr, "videoFile")
    ext = one(nvpr, "extLst")
    link = one(cn, "hlinkClick")
    if video is None and ext is None and link is None:
        return None
    if video is None or ext is None or link is None:
        raise ValueError("unsupported picture media: incomplete video link")
    checked(video, ("{" + R + "}link",))
    checked(link, ("{" + R + "}id", "action"))
    if link.attrib != {"{" + R + "}id": "", "action": MEDIA_ACTION}:
        raise ValueError("unsupported picture media action")
    items = checked(ext, (), ("ext",))
    if len(items) != 1 or items[0].attrib != {"uri": MEDIA_EXT}:
        raise ValueError("unsupported picture extension")
    media = list(items[0])
    if len(media) != 1 or media[0].tag != "{" + MEDIA_NS + "}media" or len(media[0]) or set(media[0].attrib) != {"{" + R + "}embed"}:
        raise ValueError("unsupported picture media element")
    return {"video_relationship": required(video, "{" + R + "}link"), "media_relationship": media[0].attrib["{" + R + "}embed"]}


def object_source(n, slide, width=2):
    tag = local(n.tag)
    names = {"sp": ("shape", "nvSpPr", "cNvSpPr"), "pic": ("picture", "nvPicPr", "cNvPicPr"), "cxnSp": ("connector", "nvCxnSpPr", "cNvCxnSpPr"), "grpSp": ("group", "nvGrpSpPr", "cNvGrpSpPr")}
    kind, nvtag, cvtag = names[tag]
    checked(n, (), (nvtag, "spPr", "grpSpPr", "style", "txBody", "blipFill", "sp", "pic", "cxnSp", "grpSp"))
    nv = checked(one(n, nvtag), (), ("cNvPr", cvtag, "nvPr"))
    cn = checked(one(nv, "cNvPr"), ("id", "name", "descr"), ("hlinkClick",) if kind == "picture" else ())
    native_id = int(cn.attrib["id"])
    name = cn.attrib["name"]
    out = {"key": f"s{slide:0{width}}/object-{native_id}", "id": native_id, "name": name, "kind": {"type": kind}}
    if name.startswith("!!"):
        out["morph_name"] = name
    if "descr" in cn.attrib:
        out["description"] = cn.attrib["descr"]
    k = out["kind"]
    cv = checked(one(nv, cvtag), ("txBox",), ("spLocks", "picLocks", "stCxn", "endCxn"))
    nvpr = checked(one(nv, "nvPr"), (), ("ph", "videoFile", "extLst") if kind == "picture" else ("ph",))
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
        media = picture_media(cn, nvpr)
        if media is not None:
            k["media"] = media
    elif kind == "connector":
        for tag, target in [("stCxn", "start"), ("endCxn", "end")]:
            endpoint = one(cv, tag)
            if endpoint is not None:
                checked(endpoint, ("id", "idx"))
                k[target] = {"object_id": int(endpoint.attrib["id"]), "index": int(endpoint.attrib["idx"])}
    if kind == "group":
        gp = checked(one(n, "grpSpPr"), (), ("xfrm",))
        k["transform"] = transform(one(gp, "xfrm"))
        k["children"] = [object_source(c, slide, width) for c in n if local(c.tag) in names]
    else:
        k["properties"] = properties(one(n, "spPr"))
        if one(n, "style") is not None:
            k["style"] = style(one(n, "style"))
    return out


def source_slide_objects(root, slide, width):
    if root.attrib or {local(c.tag) for c in root} - SLIDE_ROOT_CHILDREN:
        raise ValueError("unsupported slide root properties")
    checked(root.find("p:cSld", NS), (), ("spTree",))
    tree = checked(root.find("p:cSld/p:spTree", NS), (), ("nvGrpSpPr", "grpSpPr") + OBJECT_TAGS)
    return [object_source(c, slide, width) for c in tree if local(c.tag) in OBJECT_TAGS]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def normalize_sha256(value):
    value = value.lower()
    if not re.fullmatch(r"[0-9a-f]{64}", value):
        raise ValueError("--sha256 must be 64 hexadecimal digits")
    return value


def slide_parts(parts):
    """Slide part names in presentation sldIdLst order (file names do not define order)."""
    presentation = ET.fromstring(parts["ppt/presentation.xml"])
    targets = {}
    for rel in ET.fromstring(parts["ppt/_rels/presentation.xml.rels"]):
        if rel.attrib["Type"].endswith("/slide") and rel.attrib.get("TargetMode") != "External":
            target = rel.attrib["Target"]
            name = target.lstrip("/") if target.startswith("/") else posixpath.normpath(posixpath.join("ppt", target))
            targets[rel.attrib["Id"]] = name
    listing = checked(presentation.find("p:sldIdLst", NS), (), ("sldId",))
    order = []
    for entry in listing:
        checked(entry, ("id", "{" + R + "}id"))
        rid = entry.attrib["{" + R + "}id"]
        if rid not in targets or targets[rid] not in parts:
            raise ValueError("slide list entry has no slide part")
        order.append(targets[rid])
    if not order or len(set(order)) != len(order):
        raise ValueError("invalid slide list")
    return order


def slide_name(index, count):
    return f"s{index:0{max(2, len(str(count)))}}"


def dw_timing(dw, source, index):
    """Run `dw query timeline` without a shell and return only its typed timing object."""
    completed = subprocess.run([str(dw), "query", "timeline", str(source), "--slide", str(index), "--json"], capture_output=True, check=False)
    if completed.returncode != 0:
        raise ValueError(f"dw query timeline failed for slide {index} (exit status {completed.returncode})")
    report = json.loads(completed.stdout)
    timing = report.get("timing") if isinstance(report, dict) else None
    if not isinstance(report, dict) or report.get("slide") != index or not isinstance(timing, dict) or set(timing) != {"timeline", "transition", "opaque"}:
        raise ValueError(f"unexpected dw timeline output for slide {index}")
    return timing


def placements(objects):
    result = []
    for o in objects:
        k = o["kind"]
        result.append({"key": o["key"], "source_id": o["id"], "generated_id": o["id"], "kind": k["type"], "morph_name": o.get("morph_name"), "transform": k.get("transform", k.get("properties", {}).get("transform")), "relationship": k.get("relationship"), "crop": k.get("crop")})
        result.extend(placements(k.get("children", [])))
    return result


def extract(source, output, sha256=None, dw=None):
    source_resolved, output_resolved = source.resolve(), output.resolve()
    if source_resolved == output_resolved or output_resolved in source_resolved.parents:
        raise ValueError("source/output aliases or source is inside output directory")
    data = source.read_bytes()
    actual = digest(data)
    authorized = EXPECTED if sha256 is None else normalize_sha256(sha256)
    if actual != authorized:
        raise ValueError("source SHA-256 does not match the authorized reference")
    if output.exists():
        raise ValueError("source output already exists; choose a fresh directory")
    if dw is not None and not dw.is_file():
        raise ValueError("--dw must name an existing executable file")
    with zipfile.ZipFile(source) as z:
        parts = {name: z.read(name) for name in z.namelist()}
    order = slide_parts(parts)
    names = [slide_name(i, len(order)) for i in range(1, len(order) + 1)]
    width = max(2, len(str(len(order))))
    slides = []
    for index, part in enumerate(order, 1):
        slides.append({"objects": source_slide_objects(ET.fromstring(parts[part]), index, width)})
        # Every slide stays in the template as an empty shell; authored content is rebuilt from typed data.
        parts[part] = BLANK_SLIDE
    # Keep exactly the opaque dependency closure of the package, including every (blank) slide.
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
                    target = unquote(rel.attrib["Target"].split("#")[0])
                    target = posixpath.normpath(target.lstrip("/")) if target.startswith("/") else posixpath.normpath(posixpath.join(directory, target))
                    visit(target)

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
    timings = [dw_timing(dw, source, i) for i in range(1, len(order) + 1)] if dw is not None else None
    unsupported = []
    for index, timing in enumerate(timings or [], 1):
        for entry in timing["opaque"]:
            if set(entry) != {"targets", "reason"}:
                raise ValueError("unexpected opaque timing entry")
            unsupported.append({"slide": index, "targets": entry["targets"], "reason": entry["reason"]})
    output.mkdir(parents=True)
    (output / "slides").mkdir()
    for name, slide in zip(names, slides):
        with (output / "slides" / f"{name}.json").open("x", encoding="utf-8") as file:
            file.write(json.dumps(slide, ensure_ascii=False, indent=2))
    for name, timing in zip(names, timings or []):
        with (output / "slides" / f"{name}.timing.json").open("x", encoding="utf-8") as file:
            file.write(json.dumps(timing, ensure_ascii=False, indent=2))
    with zipfile.ZipFile(output / "template.pptx", "x", compression=zipfile.ZIP_DEFLATED) as z:
        for name in sorted(seen):
            info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            z.writestr(info, parts[name])
    assets = [{"part": n, "sha256": digest(parts[n]), "bytes": len(parts[n])} for n in sorted(seen) if n.startswith("ppt/media/")]
    manifest = {
        "schema_version": 1,
        "source_sha256": actual,
        "template_sha256": digest((output / "template.pptx").read_bytes()),
        "slide_count": len(order),
        "static_only": timings is None,
        "source_key_carrier": "provisional OOXML extension plus authoritative sidecar",
        "slides": [{"source_slide": i, "generated_slide": i, "part": part, "source": f"slides/{name}.json", "placements": placements(s["objects"])} for i, (part, name, s) in enumerate(zip(order, names, slides), 1)],
        "assets": assets,
        "omissions": [] if timings is not None else ["timing", "transitions"],
        "unsupported_authored_features": unsupported,
    }
    with (output / "manifest.json").open("x", encoding="utf-8") as file:
        file.write(json.dumps(manifest, ensure_ascii=False, indent=2))
    summary = {"output": str(output), "slides": [len(s["placements"]) for s in manifest["slides"]], "assets": len(assets), "timing_files": len(timings or []), "unsupported_authored_features": len(unsupported)}
    print(json.dumps(summary))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--sha256", help="authorize this explicitly named source instead of the frozen reference")
    parser.add_argument("--dw", type=Path, help="dw executable used for `query timeline` per slide")
    args = parser.parse_args()
    extract(args.source, args.output, args.sha256, args.dw)
