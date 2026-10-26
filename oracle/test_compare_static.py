import copy
import unittest

from compare_static import compare


class StaticComparisonTest(unittest.TestCase):
    def test_detects_text_type_geometry_and_render_differences(self):
        reference = {'version': '16.0', 'build': 'neutral', 'slide_width_points': 960, 'slide_height_points': 540,
                     'slides': [{'png_sha256': 'same', 'shapes': [{'id': 2, 'text': 'original', 'type': 17, 'left': 10}]}]}
        authored = copy.deepcopy(reference)
        equal = compare(reference, authored)
        self.assertTrue(equal['slides'][0]['png_sha256_equal'])
        self.assertEqual(equal['slides'][0]['shape_differences'], [])
        authored['slides'][0]['png_sha256'] = 'different'
        authored['slides'][0]['shapes'][0].update(text='changed', type=13, left=11)
        changed = compare(reference, authored)['slides'][0]
        self.assertFalse(changed['png_sha256_equal'])
        self.assertEqual({item['field'] for item in changed['shape_differences']}, {'text', 'type', 'left'})


if __name__ == '__main__':
    unittest.main()
