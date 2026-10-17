"""Opt-in verification; original bytes are supplied locally and never committed."""
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
from panic_resources import extract_resources, parse_directory, read_release


@unittest.skipUnless(os.environ.get('PANIC_EXE'), 'Set PANIC_EXE to test your local original')
class LocalReleaseTests(unittest.TestCase):
    def test_original_inventory_and_extracted_members(self):
        source = Path(os.environ['PANIC_EXE'])
        data = read_release(source)
        expected = json.loads((ROOT / 'metadata/panic-release.json').read_text())
        actual = parse_directory(data)
        self.assertEqual(actual['source'], expected['source'])
        self.assertEqual(actual['directory'], expected['directory'])
        self.assertEqual(actual['entries'], expected['entries'])
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for include_programs, count in [(False, 23), (True, 31)]:
                dest = root / str(count)
                report = extract_resources(data, dest, include_programs=include_programs)
                self.assertEqual(len(report['extracted']), count)
                self.assertIn('README.TXT', report['extracted'])
                for entry in expected['entries']:
                    target = dest / 'files' / entry['name']
                    if entry['name'] in report['extracted']:
                        self.assertEqual(target.stat().st_size, entry['bytes'])
                        self.assertEqual(hashlib.sha256(target.read_bytes()).hexdigest(), entry['sha256'])
                    else:
                        self.assertFalse(target.exists())
        self.assertEqual(hashlib.sha256(source.read_bytes()).hexdigest(), expected['source']['sha256'])


if __name__ == '__main__':
    unittest.main()
