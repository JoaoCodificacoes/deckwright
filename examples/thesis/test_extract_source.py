"""Public neutral parser and package regressions; no private deck content or assets.

Every package is generated here as a minimal synthetic .pptx, and the dw executable
is a tiny fake script written by the test.
"""
import contextlib
import io
import json
import os
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

from extract_source import A, P, R, EXPECTED, digest, extract, object_source, properties, slide_name, text
from verify_static import verify

CONTENT_TYPES = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/></Types>'
ROOT_RELS = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/></Relationships>'
SLIDE_TYPE = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide"
MEDIA_EXT = "{DAA4B4D4-6D54-4E2E-8F54-EE73A2AFCE79}"
P14 = "http://schemas.microsoft.com/office/powerpoint/2010/main"


def slide_xml(*objects, after_csld=""):
    return (f'<p:sld xmlns:p="{P}" xmlns:a="{A}" xmlns:r="{R}"><p:cSld><p:spTree>'
            '<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>'
            + "".join(objects) + f'</p:spTree></p:cSld>{after_csld}</p:sld>')


def textbox(object_id, name, text_value, run_attrs="", end=""):
    return (f'<p:sp><p:nvSpPr><p:cNvPr id="{object_id}" name="{name}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr>'
            '<p:spPr><a:xfrm><a:off x="10" y="20"/><a:ext cx="30" cy="40"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>'
            f'<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr sz="1200" {run_attrs}/><a:t>{text_value}</a:t></a:r>{end}</a:p></p:txBody></p:sp>')


def write_package(path, slides, order):
    """slides: {part name: slide xml}; order: part names in sldIdLst order."""
    rid = {part: "rId" + "".join(ch for ch in part if ch.isdigit()) for part in slides}
    ids = "".join(f'<p:sldId id="{256 + i}" r:id="{rid[part]}"/>' for i, part in enumerate(order))
    presentation = f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:presentation xmlns:p="{P}" xmlns:r="{R}" xmlns:a="{A}"><p:sldIdLst>{ids}</p:sldIdLst><p:sldSz cx="9144000" cy="5143500"/></p:presentation>'
    rels = "".join(f'<Relationship Id="{rid[part]}" Type="{SLIDE_TYPE}" Target="{part[len("ppt/"):]}"/>' for part in slides)
    parts = {
        "[Content_Types].xml": CONTENT_TYPES,
        "_rels/.rels": ROOT_RELS,
        "ppt/presentation.xml": presentation,
        "ppt/_rels/presentation.xml.rels": f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rels}</Relationships>',
    }
    parts.update(slides)
    with zipfile.ZipFile(path, "w") as z:
        for name, data in parts.items():
            z.writestr(name, data)
    return digest(path.read_bytes())


def first_slide_objects(source_dir, name="s01"):
    return json.loads((source_dir / "slides" / f"{name}.json").read_text(encoding="utf-8"))["objects"]


class SourceMappingTests(unittest.TestCase):
    def shape(self, extra="", run_extra="", end=""):
        return ET.fromstring(f'''<p:sp xmlns:p="{P}" xmlns:a="{A}">
          <p:nvSpPr><p:cNvPr id="2" name="!!label"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr>
          <p:spPr>{extra}</p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p>
          <a:r><a:rPr sz="1400" {run_extra}><a:latin typeface="Aptos"/></a:rPr><a:t>Neutral &amp; editable</a:t></a:r>{end}
          </a:p></p:txBody></p:sp>''')

    def test_native_properties_become_typed_data(self):
        value = object_source(self.shape('<a:xfrm><a:off x="10" y="20"/><a:ext cx="30" cy="40"/></a:xfrm>'), 1)
        self.assertEqual(value["key"], "s01/object-2")
        self.assertEqual(value["morph_name"], "!!label")
        self.assertEqual(value["kind"]["properties"]["transform"], {"x": 10, "y": 20, "width": 30, "height": 40})
        run = value["kind"]["text"]["paragraphs"][0]["runs"][0]
        self.assertEqual(run["text"], "Neutral & editable")
        self.assertEqual(run["style"]["font"], "Aptos")
        self.assertNotIn("xml", str(value))

    def test_key_width_follows_slide_count(self):
        self.assertEqual(slide_name(1, 3), "s01")
        self.assertEqual(slide_name(7, 120), "s007")
        self.assertEqual(object_source(self.shape(), 7, 3)["key"], "s007/object-2")

    def test_line_dash_and_tail_end_are_typed(self):
        spec = self.shape('<a:ln w="12700"><a:solidFill><a:srgbClr val="112233"/></a:solidFill><a:prstDash val="dash"/><a:tailEnd type="triangle" w="med" len="sm"/></a:ln>')
        line = object_source(spec, 1)["kind"]["properties"]["line"]
        self.assertEqual(line["width"], 12700)
        self.assertEqual(line["fill"], {"type": "solid", "color": {"type": "rgb", "value": "112233"}})
        self.assertEqual(line["dash"], "dash")
        self.assertEqual(line["tail_end"], {"kind": "triangle", "width": "med", "length": "sm"})
        self.assertNotIn("head_end", line)

    def test_line_end_outside_schema_order_or_domain_fails_closed(self):
        with self.assertRaisesRegex(ValueError, "unsupported"):
            properties(ET.fromstring(f'<p:spPr xmlns:p="{P}" xmlns:a="{A}"><a:ln><a:tailEnd type="triangle"/><a:prstDash val="dash"/></a:ln></p:spPr>'))
        with self.assertRaisesRegex(ValueError, "unsupported"):
            properties(ET.fromstring(f'<p:spPr xmlns:p="{P}" xmlns:a="{A}"><a:ln><a:tailEnd type="bogus"/></a:ln></p:spPr>'))
        with self.assertRaisesRegex(ValueError, "unsupported"):
            properties(ET.fromstring(f'<p:spPr xmlns:p="{P}" xmlns:a="{A}"><a:ln><a:prstDash val="zigzag"/></a:ln></p:spPr>'))

    def test_end_paragraph_properties_empty_attributes_and_absent(self):
        empty = text(ET.fromstring(f'<p:txBody xmlns:p="{P}" xmlns:a="{A}"><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr sz="1200"/><a:t>x</a:t></a:r><a:endParaRPr/></a:p></p:txBody>'))
        self.assertEqual(empty["paragraphs"][0]["end_style"], {})
        self.assertEqual(empty["paragraphs"][0]["runs"][0]["style"], {"size": 1200})
        filled = text(ET.fromstring(f'<p:txBody xmlns:p="{P}" xmlns:a="{A}"><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr sz="900" b="1"><a:latin typeface="Aptos"/></a:endParaRPr></a:p></p:txBody>'))
        self.assertEqual(filled["paragraphs"][0]["end_style"], {"size": 900, "bold": True, "font": "Aptos"})
        absent = text(ET.fromstring(f'<p:txBody xmlns:p="{P}" xmlns:a="{A}"><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr/><a:t>x</a:t></a:r></a:p></p:txBody>'))
        self.assertNotIn("end_style", absent["paragraphs"][0])

    def test_end_paragraph_properties_must_be_last(self):
        with self.assertRaisesRegex(ValueError, "last"):
            text(ET.fromstring(f'<p:txBody xmlns:p="{P}" xmlns:a="{A}"><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr/><a:r><a:rPr/><a:t>x</a:t></a:r></a:p></p:txBody>'))

    def test_underline_maps_and_unknown_value_fails_closed(self):
        value = object_source(self.shape(run_extra='u="sng"'), 1)
        self.assertEqual(value["kind"]["text"]["paragraphs"][0]["runs"][0]["style"]["underline"], "sng")
        with self.assertRaisesRegex(ValueError, "unsupported"):
            object_source(self.shape(run_extra='u="bogus"'), 1)
        with self.assertRaisesRegex(ValueError, "unsupported"):
            object_source(self.shape(run_extra='baseline="30000"'), 1)

    def test_custom_geometry_maps_every_path_command(self):
        geometry = f'''<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect l="l" t="t" r="r" b="b"/>
          <a:pathLst><a:path w="100" h="200" fill="none" stroke="0">
            <a:moveTo><a:pt x="0" y="0"/></a:moveTo>
            <a:lnTo><a:pt x="100" y="0"/></a:lnTo>
            <a:quadBezTo><a:pt x="100" y="100"/><a:pt x="50" y="150"/></a:quadBezTo>
            <a:cubicBezTo><a:pt x="10" y="10"/><a:pt x="20" y="20"/><a:pt x="30" y="30"/></a:cubicBezTo>
            <a:arcTo wR="10" hR="20" stAng="0" swAng="5400000"/>
            <a:close/>
          </a:path></a:pathLst></a:custGeom>'''
        custom = properties(ET.fromstring(f'<p:spPr xmlns:p="{P}" xmlns:a="{A}">{geometry}</p:spPr>'))["custom"]
        self.assertEqual(custom, {"paths": [{"width": 100, "height": 200, "fill": "none", "stroke": False, "commands": [
            {"type": "move_to", "x": 0, "y": 0},
            {"type": "line_to", "x": 100, "y": 0},
            {"type": "quad_to", "x1": 100, "y1": 100, "x": 50, "y": 150},
            {"type": "cubic_to", "x1": 10, "y1": 10, "x2": 20, "y2": 20, "x": 30, "y": 30},
            {"type": "arc_to", "width_radius": 10, "height_radius": 20, "start_angle": 0, "swing_angle": 5400000},
            {"type": "close"},
        ]}]})

    def test_custom_geometry_with_guides_fails_closed(self):
        geometry = f'''<a:custGeom><a:avLst/><a:gdLst><a:gd name="g" fmla="val 5"/></a:gdLst><a:ahLst/><a:cxnLst/><a:rect l="l" t="t" r="r" b="b"/>
          <a:pathLst><a:path w="10" h="10"><a:moveTo><a:pt x="0" y="0"/></a:moveTo></a:path></a:pathLst></a:custGeom>'''
        with self.assertRaisesRegex(ValueError, "unsupported"):
            properties(ET.fromstring(f'<p:spPr xmlns:p="{P}" xmlns:a="{A}">{geometry}</p:spPr>'))

    def test_custom_geometry_with_nonliteral_coordinate_fails_closed(self):
        geometry = f'''<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/><a:rect l="l" t="t" r="r" b="b"/>
          <a:pathLst><a:path w="10" h="10"><a:moveTo><a:pt x="w" y="0"/></a:moveTo></a:path></a:pathLst></a:custGeom>'''
        with self.assertRaisesRegex(ValueError, "nonliteral"):
            properties(ET.fromstring(f'<p:spPr xmlns:p="{P}" xmlns:a="{A}">{geometry}</p:spPr>'))

    def test_preset_and_custom_geometry_together_fail_closed(self):
        with self.assertRaisesRegex(ValueError, "multiple geometries"):
            properties(ET.fromstring(f'<p:spPr xmlns:p="{P}" xmlns:a="{A}"><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:custGeom/></p:spPr>'))

    def picture_xml(self, link=True):
        hyperlink = '<a:hlinkClick r:id="" action="ppaction://media"/>' if link else ""
        media = '<a:videoFile r:link="rId2"/><p:extLst><p:ext uri="' + MEDIA_EXT + '"><p14:media xmlns:p14="' + P14 + '" r:embed="rId3"/></p:ext></p:extLst>'
        return (f'<p:pic xmlns:p="{P}" xmlns:a="{A}" xmlns:r="{R}">'
                f'<p:nvPicPr><p:cNvPr id="4" name="Clip">{hyperlink}</p:cNvPr><p:cNvPicPr/><p:nvPr>{media}</p:nvPr></p:nvPicPr>'
                '<p:blipFill><a:blip r:embed="rId4"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>'
                '<p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="10" cy="10"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>')

    def test_video_picture_maps_to_media(self):
        value = object_source(ET.fromstring(self.picture_xml()), 1)
        self.assertEqual(value["kind"]["type"], "picture")
        self.assertEqual(value["kind"]["relationship"], "rId4")
        self.assertEqual(value["kind"]["media"], {"video_relationship": "rId2", "media_relationship": "rId3"})

    def test_video_without_media_hyperlink_fails_closed(self):
        with self.assertRaisesRegex(ValueError, "incomplete video"):
            object_source(ET.fromstring(self.picture_xml(link=False)), 1)

    def test_nonliteral_geometry_adjustment_fails_closed(self):
        node = ET.fromstring(f'<p:spPr xmlns:p="{P}" xmlns:a="{A}"><a:prstGeom prst="roundRect"><a:avLst><a:gd name="adj" fmla="*/ w 1 2"/></a:avLst></a:prstGeom></p:spPr>')
        with self.assertRaisesRegex(ValueError, "nonliteral"):
            properties(node)


class PackageExtractionTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)

    def tearDown(self):
        self.temp.cleanup()

    def package(self, slides, order, name="source.pptx"):
        path = self.root / name
        return path, write_package(path, slides, order)

    def run_extract(self, source, output, **kwargs):
        with contextlib.redirect_stdout(io.StringIO()) as out:
            extract(source, output, **kwargs)
        return json.loads(out.getvalue())

    def test_default_hash_refuses_a_different_synthetic_source(self):
        source, digest_value = self.package({"ppt/slides/slide1.xml": slide_xml(textbox(2, "Label", "Neutral"))}, ["ppt/slides/slide1.xml"])
        self.assertNotEqual(digest_value, EXPECTED)
        with self.assertRaisesRegex(ValueError, "authorized"):
            extract(source, self.root / "out")

    def test_sha256_mismatch_is_refused(self):
        source, _ = self.package({"ppt/slides/slide1.xml": slide_xml(textbox(2, "Label", "Neutral"))}, ["ppt/slides/slide1.xml"])
        with self.assertRaisesRegex(ValueError, "authorized"):
            extract(source, self.root / "out", sha256="0" * 64)
        self.assertFalse((self.root / "out").exists())

    def test_explicit_sha256_authorizes_source_and_is_recorded(self):
        source, digest_value = self.package({"ppt/slides/slide1.xml": slide_xml(textbox(2, "Label", "Neutral"))}, ["ppt/slides/slide1.xml"])
        self.run_extract(source, self.root / "out", sha256=digest_value.upper())
        manifest = json.loads((self.root / "out" / "manifest.json").read_text())
        self.assertEqual(manifest["source_sha256"], digest_value)

    def test_slides_follow_slide_list_order_not_file_names(self):
        slides = {
            "ppt/slides/slide1.xml": slide_xml(textbox(2, "Label one", "Neutral one")),
            "ppt/slides/slide3.xml": slide_xml(textbox(2, "Label three", "Neutral three")),
            "ppt/slides/slide2.xml": slide_xml(textbox(2, "Label two", "Neutral two")),
        }
        # sldIdLst puts slide3 first, then slide1, then slide2.
        source, _ = self.package(slides, ["ppt/slides/slide3.xml", "ppt/slides/slide1.xml", "ppt/slides/slide2.xml"])
        self.run_extract(source, self.root / "out", sha256=digest(source.read_bytes()))
        manifest = json.loads((self.root / "out" / "manifest.json").read_text())
        self.assertEqual([s["part"] for s in manifest["slides"]], ["ppt/slides/slide3.xml", "ppt/slides/slide1.xml", "ppt/slides/slide2.xml"])
        self.assertEqual(manifest["slide_count"], 3)
        names = [first_slide_objects(self.root / "out", n)[0]["name"] for n in ("s01", "s02", "s03")]
        self.assertEqual(names, ["Label three", "Label one", "Label two"])

    def test_template_keeps_every_slide_blank_and_drops_timing_and_transition(self):
        slides = {
            "ppt/slides/slide1.xml": slide_xml(textbox(2, "Label", "Neutral"), after_csld='<p:timing/>'),
            "ppt/slides/slide2.xml": slide_xml(textbox(2, "Other", "Neutral two"), after_csld='<p:transition><p:fade/></p:transition>'),
        }
        source, digest_value = self.package(slides, ["ppt/slides/slide1.xml", "ppt/slides/slide2.xml"])
        self.run_extract(source, self.root / "out", sha256=digest_value)
        with zipfile.ZipFile(self.root / "out" / "template.pptx") as z:
            presentation = ET.fromstring(z.read("ppt/presentation.xml"))
            self.assertEqual(len(presentation.findall(f"{{{P}}}sldIdLst/{{{P}}}sldId")), 2)
            for part in ("ppt/slides/slide1.xml", "ppt/slides/slide2.xml"):
                root = ET.fromstring(z.read(part))
                self.assertIsNone(root.find(f"{{{P}}}timing"))
                self.assertIsNone(root.find(f"{{{P}}}transition"))
                tree = root.find(f"{{{P}}}cSld/{{{P}}}spTree")
                self.assertEqual([c.tag.rsplit("}", 1)[-1] for c in tree], ["nvGrpSpPr", "grpSpPr"])
        manifest = json.loads((self.root / "out" / "manifest.json").read_text())
        self.assertTrue(manifest["static_only"])
        self.assertEqual(manifest["omissions"], ["timing", "transitions"])
        self.assertFalse((self.root / "out" / "slides" / "s01.timing.json").exists())

    def test_dw_timing_and_opaque_entries_land_in_manifest(self):
        slides = {
            "ppt/slides/slide1.xml": slide_xml(textbox(2, "Label", "Neutral")),
            "ppt/slides/slide2.xml": slide_xml(textbox(2, "Other", "Neutral two")),
        }
        source, digest_value = self.package(slides, ["ppt/slides/slide1.xml", "ppt/slides/slide2.xml"])
        fake = self.root / "fake-dw"
        seen = self.root / "seen.json"
        fake.write_text(FAKE_DW.format(python=sys.executable, seen=str(seen)), encoding="utf-8")
        os.chmod(fake, 0o755)
        self.run_extract(source, self.root / "out", sha256=digest_value, dw=fake)
        calls = json.loads(seen.read_text())
        self.assertEqual(calls, [[str(source), "1"], [str(source), "2"]])
        manifest = json.loads((self.root / "out" / "manifest.json").read_text())
        self.assertFalse(manifest["static_only"])
        self.assertEqual(manifest["omissions"], [])
        self.assertEqual(manifest["unsupported_authored_features"], [{"slide": 2, "targets": [7, 8], "reason": "synthetic opaque effect"}])
        for index in (1, 2):
            timing = json.loads((self.root / "out" / "slides" / f"s0{index}.timing.json").read_text())
            self.assertEqual(set(timing), {"timeline", "transition", "opaque"})
        self.assertEqual(json.loads((self.root / "out" / "slides" / "s02.timing.json").read_text())["opaque"], [{"targets": [7, 8], "reason": "synthetic opaque effect"}])

    def test_dw_failure_fails_closed_without_output(self):
        source, digest_value = self.package({"ppt/slides/slide1.xml": slide_xml(textbox(2, "Label", "Neutral"))}, ["ppt/slides/slide1.xml"])
        fake = self.root / "failing-dw"
        fake.write_text(f"#!{sys.executable}\nimport sys\nsys.exit(3)\n", encoding="utf-8")
        os.chmod(fake, 0o755)
        with self.assertRaisesRegex(ValueError, "exit status 3"):
            extract(source, self.root / "out", sha256=digest_value, dw=fake)
        self.assertFalse((self.root / "out").exists())

    def test_verify_static_accepts_identical_and_rejects_changed_text_for_any_slide_count(self):
        slides = {f"ppt/slides/slide{i}.xml": slide_xml(textbox(2 + i, f"Label {i}", f"Neutral {i}", end="<a:endParaRPr sz=\"900\"/>")) for i in range(1, 6)}
        order = [f"ppt/slides/slide{i}.xml" for i in range(1, 6)]
        reference, digest_value = self.package(slides, order, "reference.pptx")
        same, _ = self.package(slides, order, "same.pptx")
        with contextlib.redirect_stdout(io.StringIO()):
            report = verify(reference, same, sha256=digest_value)
        self.assertEqual(report["slide_count"], 5)
        self.assertEqual(report["slides"][0]["counts"]["endParaRPr"], 1)
        changed_slides = dict(slides)
        changed_slides["ppt/slides/slide4.xml"] = slide_xml(textbox(6, "Label 4", "Neutral 4 changed", end="<a:endParaRPr sz=\"900\"/>"))
        changed, _ = self.package(changed_slides, order, "changed.pptx")
        with contextlib.redirect_stdout(io.StringIO()), self.assertRaisesRegex(ValueError, "slide 4"):
            verify(reference, changed, sha256=digest_value)


FAKE_DW = """#!{python}
import json, pathlib, sys
args = sys.argv[1:]
assert args[:2] == ["query", "timeline"] and args[3] == "--slide" and args[5] == "--json", args
index = int(args[4])
log = pathlib.Path({seen!r})
calls = json.loads(log.read_text()) if log.exists() else []
calls.append([args[2], args[4]])
log.write_text(json.dumps(calls))
opaque = [{{"targets": [7, 8], "reason": "synthetic opaque effect"}}] if index == 2 else []
report = {{"schema": "dw.query.timeline/1", "revision": 1, "schema_version": 1, "slide": index,
          "timing": {{"timeline": {{"stages": []}}, "transition": {{"type": "none"}}, "opaque": opaque}}}}
print(json.dumps(report))
"""


if __name__ == "__main__":
    unittest.main()
