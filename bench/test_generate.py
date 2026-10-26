import pathlib
import tempfile
import unittest
import zipfile
import xml.etree.ElementTree as ET
import generate

class GeneratorTests(unittest.TestCase):
    def test_structural_shared_and_unique(self):
        with tempfile.TemporaryDirectory() as d:
            root = pathlib.Path(d)
            source = root/'source.pptx'
            generate.neutral(source)
            shared = root/'shared.pptx'
            unique = root/'unique.pptx'
            generate.expand(source, shared, 10, False)
            generate.expand(source, unique, 10, True)
            self.assertEqual(generate.validate(shared)['slides'], 10)
            self.assertEqual(generate.validate(shared)['media_parts'], 1)
            self.assertEqual(generate.validate(unique)['media_parts'], 10)
            self.assertEqual(generate.validate(unique)['distinct_media_hashes'], 10)
            with zipfile.ZipFile(shared) as z:
                self.assertEqual(z.read('ppt/media/image.png'), zipfile.ZipFile(source).read('ppt/media/image.png'))

    def test_missing_relationship_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            p = pathlib.Path(d)/'bad.pptx'
            with zipfile.ZipFile(p, 'w') as z:
                z.writestr('_rels/.rels', '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="r1" Target="missing" Type="test"/></Relationships>')
            with self.assertRaises(ValueError): generate.validate(p)

    def test_notes_backlinks_remapped_and_deterministic(self):
        with tempfile.TemporaryDirectory() as d:
            root = pathlib.Path(d)
            original = root/'original.pptx'
            generate.neutral(original)
            with zipfile.ZipFile(original) as z: parts = {n:z.read(n) for n in z.namelist()}
            parts['ppt/notesSlides/notesSlide1.xml'] = f'<p:notes xmlns:p="{generate.P}"/>'.encode()
            parts['ppt/notesSlides/_rels/notesSlide1.xml.rels'] = f'<Relationships xmlns="{generate.PKG}"><Relationship Id="s1" Type="{generate.R}/slide" Target="../slides/slide1.xml"/></Relationships>'.encode()
            rels = ET.fromstring(parts['ppt/slides/_rels/slide1.xml.rels'])
            ET.SubElement(rels, f'{{{generate.PKG}}}Relationship', Id='notes', Type=generate.R+'/notesSlide', Target='../notesSlides/notesSlide1.xml')
            parts['ppt/slides/_rels/slide1.xml.rels'] = generate.xml(rels)
            source = root/'source.pptx'
            generate.write(source, parts)
            one, two = root/'one.pptx', root/'two.pptx'
            generate.expand(source, one, 2)
            generate.expand(source, two, 2)
            self.assertEqual(one.read_bytes(), two.read_bytes())
            with zipfile.ZipFile(one) as z:
                name = 'ppt/bench1/notesSlides/_rels/notesSlide1.xml.rels'
                rel = ET.fromstring(z.read(name))[0]
                self.assertEqual(generate.resolve(generate.source_of(name), rel.get('Target')), 'ppt/bench1/slides/slide1.xml')

    def test_namespace_values_and_unknown_extensions_preserved(self):
        with tempfile.TemporaryDirectory() as d:
            root = pathlib.Path(d)
            neutral = root/'neutral.pptx'
            generate.neutral(neutral)
            with zipfile.ZipFile(neutral) as z: parts = {n:z.read(n) for n in z.namelist()}
            data = parts['ppt/presentation.xml'].replace(b'<p:presentation ', b'<p:presentation xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:ghost="urn:ghost" mc:Ignorable="ghost" ')
            data = data.replace(b'</p:presentation>', b'<mc:AlternateContent><mc:Choice Requires="ghost"><ghost:unknown value="keep"/></mc:Choice></mc:AlternateContent></p:presentation>')
            parts['ppt/presentation.xml'] = data
            source, output = root/'source.pptx', root/'output.pptx'
            generate.write(source, parts)
            generate.expand(source, output, 2)
            with zipfile.ZipFile(output) as z:
                result = z.read('ppt/presentation.xml')
            # The original byte sequence survives with only one inserted node.
            added = result[len(data.split(b'</p:sldIdLst>')[0]):result.index(b'</p:sldIdLst>')]
            self.assertEqual(result.replace(added, b'', 1), data)
            self.assertIn(b'xmlns:ghost="urn:ghost"', result)
            self.assertIn(b'Requires="ghost"', result)

    def test_registered_layout_master_graph_stays_shared(self):
        with tempfile.TemporaryDirectory() as d:
            root = pathlib.Path(d)
            neutral = root/'neutral.pptx'
            generate.neutral(neutral)
            with zipfile.ZipFile(neutral) as z: parts = {n:z.read(n) for n in z.namelist()}
            parts['ppt/slideMasters/master.xml'] = f'<p:sldMaster xmlns:p="{generate.P}"/>'.encode()
            parts['ppt/slideLayouts/layout.xml'] = f'<p:sldLayout xmlns:p="{generate.P}"/>'.encode()
            parts['ppt/slideLayouts/_rels/layout.xml.rels'] = f'<Relationships xmlns="{generate.PKG}"><Relationship Id="m" Type="{generate.R}/slideMaster" Target="../slideMasters/master.xml"/></Relationships>'.encode()
            rels = ET.fromstring(parts['ppt/slides/_rels/slide1.xml.rels'])
            ET.SubElement(rels, f'{{{generate.PKG}}}Relationship', Id='layout', Type=generate.R+'/slideLayout', Target='../slideLayouts/layout.xml')
            parts['ppt/slides/_rels/slide1.xml.rels'] = generate.xml(rels)
            source, output = root/'source.pptx', root/'output.pptx'
            generate.write(source, parts)
            generate.expand(source, output, 2)
            with zipfile.ZipFile(output) as z:
                self.assertFalse(any('/bench1/slideMasters/' in n or '/bench1/slideLayouts/' in n for n in z.namelist()))
                name = 'ppt/bench1/slides/_rels/slide1.xml.rels'
                rel = next(e for e in ET.fromstring(z.read(name)) if e.get('Id') == 'layout')
                self.assertEqual(generate.resolve(generate.source_of(name), rel.get('Target')), 'ppt/slideLayouts/layout.xml')

if __name__ == '__main__': unittest.main()
