"""Reproducible local reference evidence; XML is evidence, not playback semantics."""
import argparse
import hashlib
import json
import posixpath
from pathlib import Path
from xml.etree import ElementTree as ET
from zipfile import ZipFile

P = 'http://schemas.openxmlformats.org/presentationml/2006/main'
A = 'http://schemas.openxmlformats.org/drawingml/2006/main'
R = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships'

def inventory(path):
    data = Path(path).read_bytes()
    with ZipFile(path) as archive:
        def xml(part):
            return ET.fromstring(archive.read(part))
        def relationships(part):
            relpart = posixpath.join(posixpath.dirname(part), '_rels', posixpath.basename(part) + '.rels')
            if relpart not in archive.namelist():
                return []
            return [dict(node.attrib, resolved_target=(None if node.get('TargetMode') == 'External' else posixpath.normpath(posixpath.join(posixpath.dirname(part), node.get('Target', ''))).lstrip('/'))) for node in xml(relpart)]
        presentation = xml('ppt/presentation.xml')
        mapping = {r['Id']: r for r in relationships('ppt/presentation.xml')}
        slides = []
        for index, entry in enumerate(presentation.findall(f'{{{P}}}sldIdLst/{{{P}}}sldId'), 1):
            part = mapping[entry.get(f'{{{R}}}id')]['resolved_target']
            root = xml(part)
            rels = relationships(part)
            shapes = []
            for node in root.iter():
                kind = node.tag.split('}')[-1]
                if kind not in {'sp', 'pic', 'cxnSp', 'grpSp', 'graphicFrame'}:
                    continue
                identity = node.find(f'.//{{{P}}}cNvPr')
                shapes.append({'kind': kind, 'id': identity.get('id') if identity is not None else None, 'name': identity.get('name') if identity is not None else None, 'text': [n.text or '' for n in node.iter(f'{{{A}}}t')], 'xml': ET.tostring(node, encoding='unicode')})
            notes = []
            for rel in rels:
                if rel['Type'].endswith('/notesSlide') and rel['resolved_target']:
                    notes.append({'part': rel['resolved_target'], 'xml': archive.read(rel['resolved_target']).decode('utf-8')})
            def subtree(name):
                found = root.find(f'{{{P}}}{name}')
                return ET.tostring(found, encoding='unicode') if found is not None else None
            slides.append({'slide': index, 'part': part, 'hidden': root.get('show', '1') == '0', 'shapes': shapes, 'relationships': rels, 'notes': notes, 'timing_xml': subtree('timing'), 'transition_xml': subtree('transition'), 'slide_xml': archive.read(part).decode('utf-8')})
        return {'schema_version': 1, 'evidence': 'package-xml-only', 'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data), 'slide_size': presentation.find(f'{{{P}}}sldSz').attrib, 'parts': [{'part': n.filename, 'bytes': n.file_size, 'sha256': hashlib.sha256(archive.read(n)).hexdigest()} for n in archive.infolist()], 'slides': slides}

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('input', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(inventory(args.input), ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
