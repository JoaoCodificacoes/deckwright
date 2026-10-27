"""Public neutral parser regressions; no private deck content or assets."""
import unittest
import xml.etree.ElementTree as ET
from extract_source import A, P, object_source, properties


class SourceTranscriptionTests(unittest.TestCase):
    def shape(self, extra="", run_extra=""):
        return ET.fromstring(f'''<p:sp xmlns:p="{P}" xmlns:a="{A}">
          <p:nvSpPr><p:cNvPr id="2" name="!!label"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr>
          <p:spPr>{extra}</p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p>
          <a:r><a:rPr sz="1400" {run_extra}><a:latin typeface="Aptos"/></a:rPr><a:t>Neutral &amp; editable</a:t></a:r>
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

    def test_unknown_geometry_or_text_features_fail_closed(self):
        for node in [self.shape('<a:custGeom/>'), self.shape(run_extra='u="sng"')]:
            with self.assertRaisesRegex(ValueError, "unsupported"):
                object_source(node, 1)

    def test_nonliteral_geometry_adjustment_fails_closed(self):
        node = ET.fromstring(f'<p:spPr xmlns:p="{P}" xmlns:a="{A}"><a:prstGeom prst="roundRect"><a:avLst><a:gd name="adj" fmla="*/ w 1 2"/></a:avLst></a:prstGeom></p:spPr>')
        with self.assertRaisesRegex(ValueError, "nonliteral"):
            properties(node)


if __name__ == "__main__":
    unittest.main()
