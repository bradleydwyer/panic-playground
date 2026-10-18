"""Look check tests with invented images and a stand-in renderer; no demo data."""
from contextlib import redirect_stderr, redirect_stdout
import io
import json
import os
from pathlib import Path
import random
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch
import zlib

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
import look_check as L


def noise_image(width, height, seed=1):
    rng = random.Random(seed)
    return L.Image(width, height, [bytes(rng.randrange(256) for _ in range(width * 4))
                                   for _ in range(height)])


def plain_image(width, height, rgb=(60, 120, 180)):
    return L.Image(width, height, [bytes((*rgb, 255)) * width for _ in range(height)])


def edit(image, pixels, delta):
    """A copy with each (x, y) in pixels shifted by delta on R, G and B."""
    rows = [bytearray(r) for r in image.rows]
    for x, y in pixels:
        for c in range(3):
            rows[y][x * 4 + c] = max(0, min(255, rows[y][x * 4 + c] + delta))
    return L.Image(image.width, image.height, [bytes(r) for r in rows])


def paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    return a if pa <= pb and pa <= pc else b if pb <= pc else c


def filtered_png(width, rows, channels, filters, depth=8, interlace=0, color=None):
    """A PNG built with the reference definition of each row filter."""
    raw, prior = bytearray(), bytes(width * channels)
    for y, row in enumerate(rows):
        kind = filters[y % len(filters)]
        raw.append(kind)
        for i, value in enumerate(row):
            a = row[i - channels] if i >= channels else 0
            c = prior[i - channels] if i >= channels else 0
            b = prior[i]
            raw.append((value - [0, a, b, (a + b) // 2, paeth(a, b, c)][kind]) & 255)
        prior = row
    color = {1: 0, 2: 4, 3: 2, 4: 6}[channels] if color is None else color
    data = zlib.compress(bytes(raw))

    def chunk(kind, body):
        return (struct.pack('>I', len(body)) + kind + body
                + struct.pack('>I', zlib.crc32(kind + body)))
    # Two IDAT chunks, as encoders may split the stream.
    header = struct.pack('>IIBBBBB', width, len(rows), depth, color, 0, 0, interlace)
    return (L.PNG_SIGNATURE + chunk(b'IHDR', header)
            + chunk(b'IDAT', data[:5]) + chunk(b'IDAT', data[5:]) + chunk(b'IEND', b''))


class PngTests(unittest.TestCase):
    def test_every_filter_and_channel_layout_decodes_to_rgba(self):
        rng = random.Random(7)
        width = 9
        for channels in (1, 2, 3, 4):
            rows = [bytes(rng.randrange(256) for _ in range(width * channels)) for _ in range(12)]
            image = L.decode_png(filtered_png(width, rows, channels, [0, 1, 2, 3, 4, 4, 3, 1]))
            for row, decoded in zip(rows, image.rows):
                if channels == 4:
                    expected = row
                elif channels == 3:
                    expected = b''.join(row[i:i + 3] + b'\xff' for i in range(0, len(row), 3))
                elif channels == 2:
                    expected = b''.join(bytes((g, g, g, a)) for g, a in zip(row[0::2], row[1::2]))
                else:
                    expected = b''.join(bytes((g, g, g, 255)) for g in row)
                self.assertEqual(decoded, expected, f'{channels} channels')
            self.assertEqual((image.width, image.height), (width, 12))

    def test_written_png_reads_back(self):
        image = noise_image(13, 5)
        rgb = [b''.join(r[i:i + 3] for i in range(0, len(r), 4)) for r in image.rows]
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'x.png'
            L.write_png(path, 13, 5, rgb)
            back = L.read_png(path)
        self.assertEqual([r[3::4] for r in back.rows], [b'\xff' * 13] * 5)
        self.assertEqual(L.encode_png(13, 5, image.rows, 4)[:8], L.PNG_SIGNATURE)
        self.assertEqual(L.decode_png(L.encode_png(13, 5, image.rows, 4)).rows, image.rows)

    def test_unsupported_pngs_are_refused(self):
        rows = [bytes(8)] * 2
        for options in ({'depth': 16}, {'interlace': 1}, {'color': 3}):
            with self.subTest(options), self.assertRaises(ValueError):
                L.decode_png(filtered_png(2, rows, 4, [0], **options))
        with self.assertRaises(ValueError):
            L.decode_png(b'GIF89a')


class CompareTests(unittest.TestCase):
    def setUp(self):
        self.image = noise_image(400, 250)  # 100,000 pixels: noise allows 10 beyond tolerance
        self.pixels = 400 * 250

    def status(self, other, **limits):
        return L.classify(L.diff_images(self.image, other), self.pixels, **limits)

    def test_identical(self):
        diff = L.diff_images(self.image, self.image)
        self.assertEqual((diff.peak, diff.beyond, diff.differing), (0, 0, 0))
        self.assertEqual(self.status(self.image), 'identical')

    def test_a_handful_of_pixels_is_noise(self):
        plain = plain_image(400, 250)
        self.image = plain
        moved = edit(plain, [(5 * i, 3 * i) for i in range(10)], 60)
        diff = L.diff_images(plain, moved)
        self.assertEqual((diff.peak, diff.beyond, diff.differing), (60, 10, 10))
        self.assertEqual(self.status(moved), 'noise')
        self.assertEqual(self.status(edit(moved, [(399, 249)], 60)), 'changed')
        self.assertEqual(self.status(moved, max_changed=0.001), 'changed')

    def test_scattered_steps_within_tolerance_are_noise(self):
        plain = plain_image(400, 250)
        self.image = plain
        moved = edit(plain, [(x, y) for y in range(250) for x in range(0, 400, 10)], 2)
        diff = L.diff_images(plain, moved)
        self.assertEqual((diff.peak, diff.beyond, diff.differing), (2, 0, 10000))
        self.assertEqual(self.status(moved), 'noise')

    def test_uniform_shift_within_tolerance_is_a_change(self):
        plain = plain_image(400, 250)
        self.image = plain
        moved = plain_image(400, 250, (62, 122, 182))
        diff = L.diff_images(plain, moved)
        self.assertEqual((diff.peak, diff.beyond), (2, 0))
        self.assertEqual(self.status(moved), 'changed')

    def test_broad_change(self):
        moved = edit(self.image, [(x, y) for y in range(100) for x in range(400)], 9)
        self.assertEqual(self.status(moved), 'changed')

    def test_sizes_must_match(self):
        with self.assertRaises(ValueError):
            L.diff_images(self.image, noise_image(10, 10))

    def test_shrink_keeps_lone_pixels_in_max_mode(self):
        rows = [bytes((0, 0, 0, 0)), bytes((0, 200, 0, 0)),
                bytes((4, 8, 12, 16)), bytes((8, 0, 0, 0))]
        self.assertEqual(L.shrink(rows, 4, 1, 2, 'max'), ([bytes((200, 0)), bytes((8, 16))], 2))
        self.assertEqual(L.shrink(rows, 4, 1, 2), ([bytes((50, 0)), bytes((5, 7))], 2))
        self.assertEqual(L.shrink(rows, 4, 1, 1), (rows, 4))

    def test_heatmap_colours_only_differences(self):
        after = plain_image(4, 1, (200, 200, 200))
        heat = L.heatmap([bytes((0, 1, 9, 255))], after.rows)[0]
        grey = bytes((200 * 3 // 10,) * 3)
        pixels = [heat[i:i + 3] for i in range(0, 12, 3)]
        self.assertEqual(pixels[0], grey)
        self.assertEqual(pixels[1], bytes((40, 70, 170)))  # within the tolerance
        self.assertEqual(pixels[3], b'\xff\xff\xff')
        self.assertNotIn(pixels[2], (grey, pixels[1], pixels[3]))


class ShotTests(unittest.TestCase):
    def test_names_are_unique(self):
        names = [s.name for s in L.SHOTS]
        self.assertEqual(len(names), len(set(names)))

    def test_shot_flags_exist_in_panic_hd(self):
        source = (ROOT / 'rust/src/bin/panic-hd.rs').read_text()
        flags = {f for s in L.SHOTS for f in s.flags if f.startswith('--')}
        for flag in flags | set(L.CAPTURE_FLAGS):
            self.assertIn(f'"{flag}"', source)

    def test_selection_by_group_name_and_glob(self):
        self.assertEqual([s.name for s in L.select(L.SHOTS, ['logo'])], ['logo-16'])
        self.assertEqual({s.group for s in L.select(L.SHOTS, ['scroller'])}, {'scroller'})
        self.assertEqual([s.name for s in L.select(L.SHOTS, ['intro-7*', 'wall-27'])],
                         ['intro-7.6', 'wall-27'])
        self.assertEqual(len(L.select(L.SHOTS, [])), len(L.SHOTS))
        with self.assertRaises(L.LookError):
            L.select(L.SHOTS, ['nonsense'])


FAKE = '''#!{python}
"""A stand-in for panic-hd that draws an invented gradient."""
import os
import sys
sys.path.insert(0, {tools!r})
import look_check

args = sys.argv[1:]
if args == ['--help']:
    print('Usage: fake [options]')
    for flag in ('--data', '--size', '--seek', '--headless', '--out', '--scroller',
                 '--transition', '--faithful', '--original', '--scroller-backdrop',
                 '--wall-strength'):
        print('  ' + flag)
    sys.exit(0)
value = lambda flag: args[args.index(flag) + 1]
width, height = map(int, value('--size').split('x'))
seek = float(value('--seek'))
mode = os.environ.get('FAKE_MODE', '')
if mode == 'fail' and seek > 20:
    sys.exit('fake: cannot render')
rows = []
for y in range(height):
    row = bytearray()
    for x in range(width):
        v = (x * 3 + y * 5 + int(seek * 10)) & 255
        if mode == 'change' and seek > 20 and x < width // 2:
            v = (v + 40) & 255
        row += bytes((v, (v * 7) & 255, len(args) * 9 & 255, 255))
    rows.append(bytes(row))
if mode == 'noise' and seek > 20:
    rows[3] = rows[3][:8] + bytes(((rows[3][8] + 50) & 255,)) + rows[3][9:]
with open(value('--out'), 'xb') as out:
    out.write(look_check.encode_png(width, height, rows, 4))
print(f'Saved {{width}} x {{height}} at {{seek:.3f}}s to {{value("--out")}} using Fake GPU')
'''


class EndToEndTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        self.fake = root / 'fake-panic-hd'
        self.fake.write_text(FAKE.format(python=sys.executable, tools=str(ROOT / 'tools')))
        self.fake.chmod(0o755)
        (root / 'data').mkdir()
        self.options = ['--dir', str(root / 'look'), '--bin', str(self.fake),
                        '--data', str(root / 'data')]
        self.root = root

    def tearDown(self):
        self.tmp.cleanup()

    def run_tool(self, *args, mode=''):
        with patch.dict(os.environ, {'FAKE_MODE': mode}), \
                redirect_stdout(io.StringIO()) as out, redirect_stderr(io.StringIO()) as err:
            code = L.main(list(args))
        return code, out.getvalue() + err.getvalue()

    def report(self, base, new):
        return json.loads((self.root / 'look' / 'reports' / f'{base}--{new}' / 'report.json')
                          .read_text())

    def test_render_check_and_compare(self):
        only = ['--only', 'intro-5.5,scroller-27,chamber-27']
        code, _ = self.run_tool('render', 'base', *self.options, '--size', '64x36', *only)
        self.assertEqual(code, 0)
        manifest = json.loads((self.root / 'look/sets/base/manifest.json').read_text())
        self.assertEqual([s['name'] for s in manifest['shots']],
                         ['intro-5.5', 'scroller-27', 'chamber-27'])
        self.assertEqual((manifest['size'], manifest['adapter'], manifest['binary']),
                         ([64, 36], 'Fake GPU', str(self.fake.resolve())))
        self.assertIn('--scroller-backdrop', manifest['shots'][2]['command'])

        code, output = self.run_tool('render', 'base', *self.options, *only)
        self.assertEqual(code, 2)
        self.assertIn('--force', output)

        code, _ = self.run_tool('check', 'base', *self.options, '--jobs', '1')
        self.assertEqual(code, 0)
        statuses = {s['name']: s['status'] for s in self.report('base', 'current')['shots']}
        self.assertEqual(set(statuses.values()), {'identical'})

        # One pixel in a 64 x 36 frame is 0.043%.
        code, _ = self.run_tool('check', 'base', *self.options, '--jobs', '1',
                                '--max-changed', '0.05', mode='noise')
        self.assertEqual(code, 0)
        shots = {s['name']: s for s in self.report('base', 'current')['shots']}
        self.assertEqual(shots['scroller-27']['status'], 'noise')
        self.assertEqual((shots['scroller-27']['max'], shots['scroller-27']['differing']), (50, 1))
        self.assertEqual(L.read_png(shots['chamber-27']['heatmap']).width, 64)

        code, output = self.run_tool('check', 'base', *self.options, '--jobs', '2', mode='change')
        self.assertEqual(code, 1)
        statuses = {s['name']: s['status'] for s in self.report('base', 'current')['shots']}
        self.assertEqual(statuses, {'intro-5.5': 'identical', 'scroller-27': 'changed',
                                    'chamber-27': 'changed'})
        sheet = L.read_png(self.root / 'look/reports/base--current/sheet.png')
        self.assertEqual(sheet.width, 2 * 10 + 3 * 64 + 2 * 8)
        self.assertIn('2 changed', output)

        code, _ = self.run_tool('compare', 'base', 'current', '--dir', str(self.root / 'look'),
                                '--only', 'intro', '--jobs', '1')
        self.assertEqual(code, 0)

    def test_failed_and_unsupported_shots(self):
        code, _ = self.run_tool('render', 'base', *self.options, '--size', '64x36',
                                '--only', 'scroller-22,musical-31')
        self.assertEqual(code, 0)
        manifest = json.loads((self.root / 'look/sets/base/manifest.json').read_text())
        self.assertEqual([s['name'] for s in manifest['skipped']], ['musical-31'])
        self.assertIn('--scroller-lighting', manifest['skipped'][0]['reason'])

        code, _ = self.run_tool('check', 'base', *self.options, '--jobs', '1', mode='fail')
        self.assertEqual(code, 1)
        shots = {s['name']: s for s in self.report('base', 'current')['shots']}
        self.assertEqual(shots['scroller-22']['status'], 'failed')
        self.assertIn('cannot render', shots['scroller-22']['detail'])

    def test_extra_flags_reach_every_shot(self):
        code, _ = self.run_tool('render', 'strong', *self.options, '--size', '64x36',
                                '--only', 'logo', "--extra=--wall-strength 0.8")
        self.assertEqual(code, 0)
        manifest = json.loads((self.root / 'look/sets/strong/manifest.json').read_text())
        self.assertEqual(manifest['extra'], ['--wall-strength', '0.8'])
        self.assertIn('0.8', manifest['shots'][0]['command'])

    def test_missing_base_is_explained(self):
        code, output = self.run_tool('check', 'nothing', *self.options)
        self.assertEqual(code, 2)
        self.assertIn('render one first', output)


if __name__ == '__main__':
    unittest.main()
