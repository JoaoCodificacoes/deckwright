"""Compare corresponding static Office exports; differences are evidence, not acceptance scores."""
import argparse
import json
from collections import Counter
from pathlib import Path


def compare(reference, authored):
    reports = []
    for index, actual in enumerate(authored['slides']):
        expected = reference['slides'][index]
        expected_shapes = {shape['id']: shape for shape in expected['shapes']}
        actual_shapes = {shape['id']: shape for shape in actual['shapes']}
        differences = []
        for key in sorted(expected_shapes.keys() | actual_shapes.keys()):
            left, right = expected_shapes.get(key), actual_shapes.get(key)
            if left is None or right is None:
                differences.append({'id': key, 'field': 'shape-presence', 'reference': left is not None, 'authored': right is not None})
                continue
            for field in ('name', 'type', 'text', 'font', 'left', 'top', 'width', 'height', 'rotation'):
                if left.get(field) != right.get(field):
                    differences.append({'id': key, 'field': field, 'reference': left.get(field), 'authored': right.get(field)})
        reports.append({'slide': index + 1, 'reference_shape_count': len(expected['shapes']), 'authored_shape_count': len(actual['shapes']),
                        'reference_native_types': dict(Counter(str(shape['type']) for shape in expected['shapes'])),
                        'authored_native_types': dict(Counter(str(shape['type']) for shape in actual['shapes'])),
                        'png_sha256_equal': expected['png_sha256'] == actual['png_sha256'], 'shape_differences': differences})
    return {'schema_version': 1, 'scope': 'corresponding leading slides; top-level COM shapes and static PNG hashes only; nested XML/assets require package evidence',
            'same_office_build': reference['version'] == authored['version'] and reference['build'] == authored['build'],
            'slide_size_equal': all(reference[k] == authored[k] for k in ('slide_width_points', 'slide_height_points')),
            'reference_slides': len(reference['slides']), 'authored_slides': len(authored['slides']), 'slides': reports}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('reference', type=Path)
    parser.add_argument('authored', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = compare(json.loads(args.reference.read_text(encoding='utf-8-sig')), json.loads(args.authored.read_text(encoding='utf-8-sig')))
    with args.output.open('x', encoding='utf-8') as output:
        json.dump(result, output, indent=2)
