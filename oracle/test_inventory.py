import importlib.util
import pathlib
import subprocess
import sys
import tempfile
import unittest
import zipfile

class InventoryTest(unittest.TestCase):
    def test_relationship_order_nested_identity_and_raw_evidence(self):
        spec = importlib.util.spec_from_file_location('inventory', pathlib.Path(__file__).with_name('inventory.py'))
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        with tempfile.TemporaryDirectory() as folder:
            path = pathlib.Path(folder) / 'neutral.pptx'
            with zipfile.ZipFile(path, 'w') as z:
                z.writestr('ppt/presentation.xml', '<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:sldIdLst><p:sldId id="256" r:id="rId9"/></p:sldIdLst><p:sldSz cx="100" cy="50"/></p:presentation>')
                z.writestr('ppt/_rels/presentation.xml.rels', '<Relationships><Relationship Id="rId9" Target="slides/slide7.xml" Type="slide"/></Relationships>')
                z.writestr('ppt/slides/slide7.xml', '<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" show="0"><p:cSld><p:spTree><p:grpSp><p:sp><p:nvSpPr><p:cNvPr id="4" name="!!Morph"/></p:nvSpPr><p:spPr><a:xfrm><a:off x="2" y="3"/></a:xfrm></p:spPr><p:txBody><a:p><a:r><a:t>Neutral text</a:t></a:r></a:p></p:txBody></p:sp></p:grpSp></p:spTree></p:cSld><p:timing><p:cTn id="1" repeatCount="indefinite"/></p:timing></p:sld>')
            result = module.inventory(path)
            slide = result['slides'][0]
            self.assertEqual(slide['part'], 'ppt/slides/slide7.xml')
            self.assertEqual(slide['hidden'], True)
            self.assertEqual(slide['shapes'][1]['id'], '4')
            self.assertEqual(slide['shapes'][1]['text'], ['Neutral text'])
            self.assertIn('repeatCount="indefinite"', slide['timing_xml'])
            self.assertIn('x="2"', slide['shapes'][1]['xml'])

class InventoryCliTest(unittest.TestCase):
    def fixture(self, path):
        with zipfile.ZipFile(path, 'w') as archive:
            archive.writestr('ppt/presentation.xml', '<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:sldIdLst/><p:sldSz cx="100" cy="50"/></p:presentation>')

    def run_cli(self, source, output):
        return subprocess.run([sys.executable, '-B', str(pathlib.Path(__file__).with_name('inventory.py')), str(source), '--output', str(output)], capture_output=True, text=True)

    def test_refuses_existing_outputs_and_source_aliases_without_changing_bytes(self):
        for kind in ('same', 'existing', 'symlink', 'hardlink'):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as folder:
                source = pathlib.Path(folder) / 'neutral.pptx'
                self.fixture(source)
                before = source.read_bytes()
                output = pathlib.Path(folder) / 'inventory.json'
                if kind == 'same':
                    output = source
                elif kind == 'existing':
                    output.write_bytes(b'keep existing artifact')
                elif kind == 'symlink':
                    output.symlink_to(source)
                else:
                    output.hardlink_to(source)
                output_before = output.read_bytes()
                result = self.run_cli(source, output)
                self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual(source.read_bytes(), before)
                self.assertEqual(output.read_bytes(), output_before)
                self.assertTrue(zipfile.is_zipfile(source))

    def test_creates_new_output_without_changing_source(self):
        with tempfile.TemporaryDirectory() as folder:
            source = pathlib.Path(folder) / 'neutral.pptx'
            self.fixture(source)
            before = source.read_bytes()
            output = pathlib.Path(folder) / 'new' / 'inventory.json'
            result = self.run_cli(source, output)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(source.read_bytes(), before)
            self.assertTrue(output.read_text().startswith('{'))

if __name__ == '__main__':
    unittest.main()
