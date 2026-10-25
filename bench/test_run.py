import json
import pathlib
import sys
import subprocess
import tempfile
import unittest

import run

class RunnerTests(unittest.TestCase):
    def test_percentile(self):
        self.assertEqual(run.percentile(list(range(1, 51)), .95), 48)
        self.assertEqual(run.percentile([4], .95), 4)
        with self.assertRaises(ValueError): run.percentile([], .95)

    def test_stdout_and_provenance(self):
        with tempfile.TemporaryDirectory() as d:
            result = run.measure([sys.executable, '-c', 'print("hello")'], pathlib.Path(d), 5)
            self.assertEqual(pathlib.Path(d, 'stdout.bin').read_bytes(), b'hello\n')
            self.assertGreater(result['wall_ms'], 0)
            self.assertNotIn('runner', result['rss_source'])
            self.assertEqual(result['returncode'], 0)

    def test_failure(self):
        with tempfile.TemporaryDirectory() as d:
            result = run.measure([sys.executable, '-c', 'import sys; print("bad", file=sys.stderr); sys.exit(7)'], pathlib.Path(d), 5)
            self.assertEqual(result['returncode'], 7)
            self.assertIn(b'bad', pathlib.Path(d, 'stderr.bin').read_bytes())

    def test_output_placeholder_and_literal_argv(self):
        with tempfile.TemporaryDirectory() as d:
            out = pathlib.Path(d)
            result = run.measure([sys.executable, '-c', 'import pathlib,sys; pathlib.Path(sys.argv[1]).write_text(sys.argv[2])', '{output}', 'a;$(touch nope)'], out, 5)
            self.assertEqual((out/'artifact.pptx').read_text(), 'a;$(touch nope)')
            self.assertEqual(result['artifact_bytes'], 15)

    def test_timeout(self):
        with tempfile.TemporaryDirectory() as d:
            result = run.measure([sys.executable, '-c', 'import time; time.sleep(3)'], pathlib.Path(d), .01)
            self.assertTrue(result['timed_out'])
            self.assertNotEqual(result['returncode'], 0)

    def test_cli_failure_manifest_and_no_overwrite(self):
        with tempfile.TemporaryDirectory() as d:
            out = pathlib.Path(d)/'result'
            argv = [sys.executable, str(pathlib.Path(run.__file__)), '--result-dir', str(out),
                    '--warmups', '0', '--samples', '2', '--cache-state', 'uncontrolled',
                    '--filesystem-label', 'test', '--', sys.executable, '-c', 'raise SystemExit(4)']
            self.assertEqual(subprocess.run(argv, capture_output=True).returncode, 1)
            report = json.loads((out/'manifest.json').read_text())
            self.assertNotIn('summary', report)
            self.assertEqual(len(report['runs']), 1)
            before = (out/'manifest.json').read_bytes()
            self.assertNotEqual(subprocess.run(argv, capture_output=True).returncode, 0)
            self.assertEqual((out/'manifest.json').read_bytes(), before)

if __name__ == '__main__': unittest.main()
