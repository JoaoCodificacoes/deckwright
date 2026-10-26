import json
import pathlib
import sys
import subprocess
import tempfile
import unittest
from unittest import mock

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

    def test_timeout_with_zero_exit_never_summarized(self):
        with tempfile.TemporaryDirectory() as d:
            out = pathlib.Path(d)/'result'
            argv = ['run.py', '--result-dir', str(out), '--warmups', '0', '--samples', '1',
                    '--cache-state', 'uncontrolled', '--filesystem-label', 'test', '--', 'neutral']
            row = {'returncode': 0, 'timed_out': True, 'wall_ms': 102.9,
                   'peak_rss_bytes': None, 'artifact_bytes': None, 'stdout_bytes': 0}
            with mock.patch.object(sys, 'argv', argv), mock.patch.object(run, 'host', return_value={}), mock.patch.object(run, 'measure', return_value=row):
                self.assertEqual(run.main(), 1)
            report = json.loads((out/'manifest.json').read_text())
            self.assertIn('failure', report)
            self.assertNotIn('summary', report)
            self.assertTrue(report['runs'][0]['timed_out'])

if __name__ == '__main__': unittest.main()
