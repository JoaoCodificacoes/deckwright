#!/usr/bin/env python3
"""Generate neutral/public or local/private OPC stress decks, with graph checks."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import posixpath as pp
import struct
import xml.etree.ElementTree as ET
import zipfile
import zlib

P = 'http://schemas.openxmlformats.org/presentationml/2006/main'
R = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships'
PKG = 'http://schemas.openxmlformats.org/package/2006/relationships'
CT = 'http://schemas.openxmlformats.org/package/2006/content-types'

def source_of(rels):
    return '' if rels == '_rels/.rels' else pp.join(pp.dirname(pp.dirname(rels)), pp.basename(rels)[:-5])

def resolve(source, target):
    return pp.normpath(target.lstrip('/') if target.startswith('/') else pp.join(pp.dirname(source), target))

def xml(data):
    return ET.tostring(data, encoding='utf-8', xml_declaration=True)

def write(path, parts):
    # Fixed metadata and ordering make identical generator inputs reproducible.
    with zipfile.ZipFile(path, 'x', compression=zipfile.ZIP_DEFLATED) as z:
        for name, data in sorted(parts.items()):
            info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            z.writestr(info, data)

def validate(path):
    with zipfile.ZipFile(path) as z:
        names = z.namelist()
        if len(names) != len(set(names)): raise ValueError('duplicate part names')
        parts = {n: z.read(n) for n in names}
    slides = 0
    for name, data in parts.items():
        if name.endswith(('.xml', '.rels')):
            root = ET.fromstring(data)
            slides += root.tag == f'{{{P}}}sld'
            if root.tag == f'{{{P}}}sld':
                shape_ids = [e.get('id') for e in root.iter(f'{{{P}}}cNvPr')]
                if len(shape_ids) != len(set(shape_ids)):
                    raise ValueError(f'duplicate slide-local shape IDs in {name}')
                rel_name = pp.join(pp.dirname(name), '_rels', pp.basename(name) + '.rels')
                bound = {e.get('Id') for e in ET.fromstring(parts[rel_name])} if rel_name in parts else set()
                # The reference contains empty embed fallback attributes; these
                # carry no relationship ID and are retained, not invented links.
                referenced = {v for e in root.iter() for k, v in e.attrib.items() if v and k.startswith('{' + R + '}')}
                if referenced - bound:
                    raise ValueError(f'unbound slide relationship IDs in {name}: {referenced - bound}')
            if name.endswith('.rels'):
                ids = [e.get('Id') for e in root]
                if len(ids) != len(set(ids)): raise ValueError(f'duplicate relationship IDs in {name}')
                for rel in root:
                    if rel.get('TargetMode') != 'External' and resolve(source_of(name), rel.get('Target')) not in parts:
                        raise ValueError(f'missing target in {name}: {rel.get("Target")}')
    pres = ET.fromstring(parts['ppt/presentation.xml']) if 'ppt/presentation.xml' in parts else None
    if pres is not None:
        ids = [e.get('id') for e in pres.findall(f'./{{{P}}}sldIdLst/{{{P}}}sldId')]
        if len(ids) != len(set(ids)): raise ValueError('duplicate presentation slide IDs')
        if len(ids) != slides: raise ValueError('presentation/slide count mismatch')
    media = [v for k, v in parts.items() if '/media/' in k]
    return {'slides': slides, 'parts': len(parts), 'bytes': Path(path).stat().st_size,
            'uncompressed_bytes': sum(map(len, parts.values())), 'media_parts': len(media),
            'media_bytes': sum(map(len, media)), 'distinct_media_hashes': len({hashlib.sha256(b).digest() for b in media}),
            'validation': 'XML well-formed; unique package/presentation/relationship IDs; all internal relationship targets exist; not Office playback validation'}

def unique_png(data, replica):
    # Insert a valid ancillary text chunk before IEND; compressed pixels stay intact.
    if not data.startswith(b'\x89PNG\r\n\x1a\n') or data[-8:-4] != b'IEND':
        return data
    chunk = b'tEXt' + b'deckwright-workload\x00' + str(replica).encode()
    return data[:-12] + struct.pack('>I', len(chunk)-4) + chunk + struct.pack('>I', zlib.crc32(chunk)) + data[-12:]

def expand(source, output, factor=10, unique_media=False):
    if factor < 1: raise ValueError('factor must be positive')
    validate(source)
    with zipfile.ZipFile(source) as z: original = {n: z.read(n) for n in z.namelist()}
    parts = dict(original)
    presentation = ET.fromstring(parts['ppt/presentation.xml'])
    slide_list = presentation.find(f'{{{P}}}sldIdLst')
    pres_rels = ET.fromstring(parts['ppt/_rels/presentation.xml.rels'])
    content = ET.fromstring(parts['[Content_Types].xml'])
    old_slides = list(slide_list)
    by_id = {e.get('Id'): e for e in pres_rels}
    next_id = max(int(e.get('id')) for e in old_slides) + 1
    used_rids = set(by_id)
    for replica in range(1, factor):
        # Replicate the entire ppt dependency graph, retaining shared media only.
        mapping = {n: f'ppt/bench{replica}/' + n[4:] for n in original
                   if n.startswith('ppt/') and n not in ('ppt/presentation.xml', 'ppt/_rels/presentation.xml.rels')
                   and (unique_media or not n.startswith('ppt/media/'))}
        for old, new in mapping.items():
            data = original[old]
            if old.endswith('.rels'):
                root = ET.fromstring(data)
                for rel in root:
                    if rel.get('TargetMode') == 'External': continue
                    target = resolve(source_of(old), rel.get('Target'))
                    rel.set('Target', pp.relpath(mapping.get(target, target), pp.dirname(source_of(new))))
                data = xml(root)
            elif unique_media and old.startswith('ppt/media/'):
                data = unique_png(data, replica)
            parts[new] = data
        for override in list(content):
            old = override.get('PartName', '').lstrip('/')
            if old in mapping:
                ET.SubElement(content, f'{{{CT}}}Override', {'PartName': '/' + mapping[old], 'ContentType': override.get('ContentType')})
        for slide in old_slides:
            rel = by_id[slide.get(f'{{{R}}}id')]
            target = resolve('ppt/presentation.xml', rel.get('Target'))
            rid = f'bench{replica}_{slide.get("id")}'
            while rid in used_rids: rid += '_'
            used_rids.add(rid)
            ET.SubElement(pres_rels, f'{{{PKG}}}Relationship', {'Id': rid, 'Type': rel.get('Type'), 'Target': pp.relpath(mapping[target], 'ppt')})
            ET.SubElement(slide_list, f'{{{P}}}sldId', {'id': str(next_id), f'{{{R}}}id': rid})
            next_id += 1
    parts['ppt/presentation.xml'] = xml(presentation)
    parts['ppt/_rels/presentation.xml.rels'] = xml(pres_rels)
    parts['[Content_Types].xml'] = xml(content)
    write(output, parts)
    result = validate(output)
    result.update(factor=factor, profile='growing-png-media' if unique_media else 'shared-media', source_sha256=hashlib.sha256(Path(source).read_bytes()).hexdigest())
    return result

def neutral(path):
    png = base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a9WQAAAAASUVORK5CYII=')
    parts = {'[Content_Types].xml': f'<Types xmlns="{CT}"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/></Types>'.encode(),
      '_rels/.rels': f'<Relationships xmlns="{PKG}"><Relationship Id="rId1" Type="{R}/officeDocument" Target="ppt/presentation.xml"/></Relationships>'.encode(),
      'ppt/presentation.xml': f'<p:presentation xmlns:p="{P}" xmlns:r="{R}"><p:sldIdLst><p:sldId id="256" r:id="rId1"/></p:sldIdLst><p:sldSz cx="12192000" cy="6858000"/><p:notesSz cx="6858000" cy="9144000"/></p:presentation>'.encode(),
      'ppt/_rels/presentation.xml.rels': f'<Relationships xmlns="{PKG}"><Relationship Id="rId1" Type="{R}/slide" Target="slides/slide1.xml"/></Relationships>'.encode(),
      'ppt/slides/slide1.xml': f'<p:sld xmlns:p="{P}"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld><p:clrMapOvr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:masterClrMapping/></p:clrMapOvr></p:sld>'.encode(),
      'ppt/slides/_rels/slide1.xml.rels': f'<Relationships xmlns="{PKG}"><Relationship Id="rId1" Type="{R}/image" Target="../media/image.png"/></Relationships>'.encode(),
      'ppt/media/image.png': png}
    write(path, parts)
    return validate(path)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, help='omit for neutral fixture')
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--factor', type=int, default=10)
    parser.add_argument('--unique-media', action='store_true')
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    result = expand(args.source, args.output, args.factor, args.unique_media) if args.source else neutral(args.output)
    args.output.with_suffix('.manifest.json').write_text(json.dumps(result, indent=2)+'\n')
    print(json.dumps(result))

if __name__ == '__main__': main()
