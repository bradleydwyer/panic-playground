"""Local importer boundaries and preservation checks; no original code executes."""
import hashlib
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
from import_panic import EXPECTED_SHA256, import_data


class ImportTests(unittest.TestCase):
    source = Path(os.environ.get('PANIC_EXE', ROOT / 'artifacts/original/release/PANIC.EXE'))

    def test_wrong_input_is_rejected_before_creating_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / 'wrong.exe'
            source.write_bytes(b'MZ' + bytes(100))
            destination = root / 'missing-parent/data'
            with self.assertRaisesRegex(ValueError, 'Unsupported PANIC.EXE'):
                import_data(source, destination, assets_only=True)
            self.assertFalse(destination.parent.exists())

    def test_existing_destination_and_dangling_symlink_are_preserved(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            destination = root / 'data'
            destination.mkdir()
            (destination / 'keep').write_bytes(b'unchanged')
            with self.assertRaisesRegex(ValueError, 'Destination already exists'):
                import_data(Path(__file__), destination, assets_only=True)
            self.assertEqual((destination / 'keep').read_bytes(), b'unchanged')
            link = root / 'link'
            try:
                link.symlink_to(root / 'absent')
            except OSError:
                self.skipTest('Symlink creation unavailable on this platform')
            with self.assertRaisesRegex(ValueError, 'Destination already exists'):
                import_data(Path(__file__), link, assets_only=True)
            self.assertTrue(link.is_symlink())

    @unittest.skipUnless(os.environ.get('PANIC_EXE'), 'Set PANIC_EXE to test your local original')
    def test_resources_match_preserved_inventory_and_input_stays_unchanged(self):
        import json
        catalogue = json.loads((ROOT / 'metadata/panic-release.json').read_text())
        expected = catalogue['entries']
        self.assertEqual(len(expected), 31)
        with tempfile.TemporaryDirectory() as tmp:
            destination = Path(tmp) / 'data'
            result = import_data(self.source, destination, assets_only=True)
            self.assertEqual((result['resource_count'], result['unpacked_count']), (23, 0))
            for entry in expected:
                name = entry['name']
                if name.endswith('.EXE'):
                    self.assertFalse((destination / 'files' / name).exists())
                else:
                    blob = (destination / 'files' / name).read_bytes()
                    self.assertEqual(hashlib.sha256(blob).hexdigest(), entry['sha256'])
            self.assertTrue((destination / 'files/README.TXT').is_file())
            self.assertEqual((destination / '.gitignore').read_text(), '*\n!.gitignore\n')
            self.assertEqual(json.loads((destination / 'import.json').read_text()), result)
        self.assertEqual(hashlib.sha256(self.source.read_bytes()).hexdigest(), EXPECTED_SHA256)

    @unittest.skipUnless(os.environ.get('PANIC_EXE'), 'Set PANIC_EXE to test your local original')
    def test_missing_decoder_does_not_leave_partial_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            destination = Path(tmp) / 'data'
            with self.assertRaisesRegex(ValueError, 'Deark not found'):
                import_data(self.source, destination, deark=str(Path(tmp) / 'missing-deark'))
            self.assertFalse(destination.exists())

    @unittest.skipUnless(os.environ.get('PANIC_EXE'), 'Set PANIC_EXE to test your local original')
    def test_changed_bytes_with_correct_length_are_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / 'changed.exe'
            blob = bytearray(self.source.read_bytes())
            blob[-1] ^= 1
            source.write_bytes(blob)
            with self.assertRaisesRegex(ValueError, 'Unsupported PANIC.EXE'):
                import_data(source, root / 'data', assets_only=True)
            self.assertFalse((root / 'data').exists())

    @unittest.skipUnless(os.environ.get('PANIC_EXE'), 'Set PANIC_EXE to test your local original')
    def test_helper_failure_removes_scratch_and_partial_outputs(self):
        def fail_after_extraction(command, work, log):
            self.assertTrue((work / 'data/files/P.P').is_file())
            self.assertTrue((work / 'packed/1.EXE').is_file())
            raise ValueError('simulated helper failure')

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            with patch('import_panic.run_helper', side_effect=fail_after_extraction), \
                    patch('import_panic.importlib.util.find_spec', return_value=object()):
                with self.assertRaisesRegex(ValueError, 'simulated helper failure'):
                    import_data(self.source, root / 'data', deark=sys.executable)
            self.assertEqual(list(root.iterdir()), [])


if __name__ == '__main__':
    unittest.main()
