"""Public tests use invented payloads, with no original demo material."""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
from panic_resources import extract_resources, parse_directory, read_release


def synthetic_container(members=None):
    if members is None:
        members = [('README.TXT', b'An invented notice for testing.\n'),
                   ('DATA.BIN', bytes(range(16))),
                   ('PART.EXE', b'A placeholder, not executable code.')]
    data = bytearray(33)
    struct.pack_into('<14H', data, 0, 0x5a4d, 32, 1, 0, 2, *([0] * 9))
    records = []
    for name, payload in members:
        records.append(struct.pack('<16sII', name.encode('ascii'), len(data), len(payload)))
        data.extend(payload)
    directory = len(data)
    data.extend(struct.pack('<III', 0x123456, len(records), 33))
    data.extend(b''.join(records))
    data.extend(struct.pack('<I', directory))
    return bytes(data)


class ResourceTests(unittest.TestCase):
    def test_inventory_records_every_payload(self):
        data = synthetic_container()
        result = parse_directory(data)
        self.assertEqual([e['name'] for e in result['entries']],
                         ['README.TXT', 'DATA.BIN', 'PART.EXE'])
        self.assertEqual(result['source']['sha256'], hashlib.sha256(data).hexdigest())
        self.assertEqual(result['entries'][1]['sha256'], hashlib.sha256(bytes(range(16))).hexdigest())
        self.assertEqual(result['directory']['marker'], 0x123456)

    def test_default_extraction_preserves_resources_and_omits_programs(self):
        with tempfile.TemporaryDirectory() as tmp:
            dest = Path(tmp) / 'data'
            report = extract_resources(synthetic_container(), dest)
            self.assertEqual((dest / 'files/DATA.BIN').read_bytes(), bytes(range(16)))
            self.assertEqual((dest / 'files/README.TXT').read_bytes(), b'An invented notice for testing.\n')
            self.assertFalse((dest / 'files/PART.EXE').exists())
            self.assertEqual(report['extracted'], ['README.TXT', 'DATA.BIN'])
            self.assertEqual(json.loads((dest / 'inventory.json').read_text()), report)
            self.assertEqual((dest / '.gitignore').read_text(), '*\n!.gitignore\n')

    def test_program_opt_in_copies_exact_payload(self):
        with tempfile.TemporaryDirectory() as tmp:
            dest = Path(tmp) / 'all'
            report = extract_resources(synthetic_container(), dest, include_programs=True)
            self.assertEqual(len(report['extracted']), 3)
            self.assertEqual((dest / 'files/PART.EXE').read_bytes(), b'A placeholder, not executable code.')

    def test_rejects_path_names_and_windows_devices(self):
        for name in ['../escape', '/absolute', 'A/B', 'A\\B', '.', '..', 'CON',
                     'con.txt', 'LPT9.DAT', 'A.', 'A:B']:
            with self.subTest(name=name), self.assertRaises(ValueError):
                parse_directory(synthetic_container([(name, b'invented')]))

    def test_rejects_duplicate_names_ignoring_case(self):
        with self.assertRaisesRegex(ValueError, 'Duplicate'):
            parse_directory(synthetic_container([('ONE.DAT', b'1'), ('one.dat', b'2')]))

    def test_rejects_unterminated_name_and_nonzero_padding(self):
        for name in [b'1234567890123456', b'A\0B' + bytes(13)]:
            data = bytearray(synthetic_container())
            directory = struct.unpack_from('<I', data, len(data) - 4)[0]
            data[directory + 12:directory + 28] = name
            with self.assertRaises(ValueError):
                parse_directory(data)

    def test_rejects_truncation_and_invalid_mz(self):
        source = synthetic_container()
        for size in [0, 2, 27, 43, len(source) - 1]:
            with self.subTest(size=size), self.assertRaises(ValueError):
                parse_directory(source[:size])
        for offset, value in [(2, 512), (4, 0), (8, 1), (8, 65535)]:
            data = bytearray(source)
            struct.pack_into('<H', data, offset, value)
            with self.subTest(offset=offset, value=value), self.assertRaises(ValueError):
                parse_directory(data)

    def test_rejects_bad_directory_count_pointer_and_resource_ranges(self):
        source = synthetic_container()
        directory = struct.unpack_from('<I', source, len(source) - 4)[0]
        changes = [(len(source) - 4, 0xffffffff), (directory + 4, 0xffffffff),
                   (directory + 4, 0), (directory + 8, 0),
                   (directory + 12 + 16, 0), (directory + 12 + 20, 0xffffffff),
                   (directory + 12 + 24 + 16, 33)]
        for offset, value in changes:
            data = bytearray(source)
            struct.pack_into('<I', data, offset, value)
            with self.subTest(offset=offset), self.assertRaises(ValueError):
                parse_directory(data)

    def test_validation_failure_leaves_no_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            dest = Path(tmp) / 'absent/data'
            with self.assertRaises(ValueError):
                extract_resources(b'invalid', dest)
            self.assertFalse(dest.parent.exists())

    def test_existing_destination_is_never_replaced(self):
        with tempfile.TemporaryDirectory() as tmp:
            dest = Path(tmp) / 'data'
            dest.mkdir()
            (dest / 'keep').write_text('unchanged')
            with self.assertRaisesRegex(ValueError, 'already exists'):
                extract_resources(synthetic_container(), dest)
            self.assertEqual((dest / 'keep').read_text(), 'unchanged')

    def test_dangling_destination_symlink_is_never_followed(self):
        with tempfile.TemporaryDirectory() as tmp:
            dest = Path(tmp) / 'link'
            try:
                dest.symlink_to(Path(tmp) / 'missing')
            except OSError:
                self.skipTest('Symlink creation unavailable on this platform')
            with self.assertRaisesRegex(ValueError, 'already exists'):
                extract_resources(synthetic_container(), dest)
            self.assertTrue(dest.is_symlink())

    def test_copy_failure_removes_partial_output_and_scratch(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            import shutil
            original_move = shutil.move
            calls = 0

            def fail_on_second_move(source, destination):
                nonlocal calls
                calls += 1
                if calls == 2:
                    raise OSError('simulated storage failure')
                return original_move(source, destination)

            with patch('panic_resources.shutil.move', side_effect=fail_on_second_move):
                with self.assertRaisesRegex(OSError, 'simulated storage failure'):
                    extract_resources(synthetic_container(), root / 'data')
            self.assertEqual(calls, 2)
            self.assertEqual(list(root.iterdir()), [])

    def test_cli_rejects_unknown_release_without_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = root / 'synthetic.exe'
            source.write_bytes(synthetic_container())
            dest = root / 'data'
            result = subprocess.run([sys.executable, str(ROOT / 'tools/panic_resources.py'),
                                     'extract', str(source), '--out', str(dest)],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 1)
            self.assertIn('Unsupported release', result.stderr)
            self.assertFalse(dest.exists())

    def test_identity_check_rejects_same_size_modified_synthetic_carrier(self):
        data = synthetic_container()
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp) / 'synthetic.exe'
            source.write_bytes(data)
            with patch('panic_resources.RELEASE_BYTES', len(data)), \
                    patch('panic_resources.RELEASE_SHA256', hashlib.sha256(data).hexdigest()):
                self.assertEqual(read_release(source), data)
                changed = bytearray(data)
                changed[33] ^= 1
                source.write_bytes(changed)
                with self.assertRaisesRegex(ValueError, 'Unsupported release'):
                    read_release(source)


if __name__ == '__main__':
    unittest.main()
