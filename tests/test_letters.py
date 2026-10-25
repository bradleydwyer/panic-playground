"""Part 7's authored letters: the kit's checks on synthetic shapes, and the
committed rust/src/hd/greetings_letters.txt against its drawings.

The drawings in tools/letters/ are authored for the remake, not demo data.
"""
from pathlib import Path
import itertools
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools' / 'letters'))
sys.dont_write_bytecode = True
import export_letters as X  # noqa: E402
import kit  # noqa: E402

COMMITTED = ROOT / 'rust' / 'src' / 'hd' / 'greetings_letters.txt'


def square(x0, y0, x1, y1):
    return [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]


class KitTest(unittest.TestCase):
    """The checks themselves, on synthetic shapes."""

    def test_a_chamfer_cuts_its_corner_along_both_edges(self):
        self.assertEqual(kit.chamfer([(0, 0, 10), (100, 0), (100, 50), (0, 50)]),
                         [(0, 10), (10, 0), (100, 0), (100, 50), (0, 50)])
        # Along a 45-degree edge the cut runs 10 grid steps each way.
        self.assertEqual(kit.chamfer([(0, 0), (40, 40, 10), (0, 80)]),
                         [(0, 0), (30, 30), (30, 50), (0, 80)])
        with self.assertRaises(ValueError):  # off the unit grid
            kit.chamfer([(0, 0), (30, 10, 5), (0, 20)])

    def test_the_kit_winds_outers_counter_clockwise_and_holes_clockwise(self):
        g = kit.Glyph('X')
        g.outer(square(0, 0, 120, 120)[::-1])
        g.hole(square(40, 40, 80, 80))
        (k0, outer), (k1, hole) = g.contours
        self.assertEqual((k0, k1), ('outer', 'hole'))
        self.assertGreater(kit.signed_area(outer), 0)
        self.assertLess(kit.signed_area(hole), 0)
        self.assertEqual(X.problems(kit, g.contours), [])
        self.assertTrue(X.problems(kit, [('outer', square(0, 0, 100, 100)[::-1])]))
        self.assertTrue(X.problems(kit, [('outer', square(0, 0, 100, 100)),
                                         ('hole', square(40, 40, 60, 60))]))

    def test_crossings_finds_bow_ties_folds_and_touching_contours(self):
        clean = [('outer', square(0, 0, 100, 100)), ('hole', square(40, 40, 60, 60)[::-1])]
        self.assertEqual(kit.crossings(clean), [])
        self.assertTrue(kit.crossings([('outer', [(0, 0), (10, 10), (10, 0), (0, 10)])]))
        fold = [(0, 0), (20, 0), (20, 10), (10, 10), (15, 10), (0, 10)]
        self.assertTrue(kit.crossings([('outer', fold)]))
        touching = [('outer', square(0, 0, 100, 100)), ('hole', square(0, 40, 20, 60)[::-1])]
        self.assertTrue(kit.crossings(touching))
        crossing = [('outer', square(0, 0, 100, 100)), ('hole', square(90, 40, 120, 60)[::-1])]
        self.assertTrue(kit.crossings(crossing))

    def test_feature_widths_measure_slits_holes_necks_and_not_corners(self):
        # A block with a slit 15 high from the right, its spine 30 wide.
        slit = [(0, 0), (100, 0), (100, 40), (30, 40), (30, 55), (100, 55), (100, 100), (0, 100)]
        w = kit.feature_widths([('outer', slit)])
        self.assertAlmostEqual(w['counter'][0], 15)
        self.assertAlmostEqual(w['ink'][0], 30)
        # A hole 20 wide, 40 from the sides.
        w = kit.feature_widths([('outer', square(0, 0, 100, 100)),
                                ('hole', square(40, 30, 60, 70)[::-1])])
        self.assertAlmostEqual(w['counter'][0], 20)
        self.assertAlmostEqual(w['ink'][0], 30)
        # Two blocks joined by a bridge 10 high; the notches either side
        # are 20 across.
        bell = [(0, 0), (40, 0), (40, 15), (60, 15), (60, 0), (100, 0), (100, 40), (60, 40),
                (60, 25), (40, 25), (40, 40), (0, 40)]
        w = kit.feature_widths([('outer', bell)])
        self.assertAlmostEqual(w['ink'][0], 10)
        self.assertAlmostEqual(w['counter'][0], 20)
        # A reflex corner close to an edge across a slanted cut: the V
        # notch's tip is 12 from the far side.
        vee = [(0, 0), (100, 0), (100, 30), (88, 18), (76, 30), (76, 100), (0, 100)]
        w = kit.feature_widths([('outer', vee)])
        self.assertAlmostEqual(w['ink'][0], 18)
        # Chamfered corners are not narrow features: a chamfered square is
        # as wide as its flats, or across its chamfers if that is less.
        square_ish = kit.chamfer([(0, 0, 20), (100, 0, 20), (100, 100, 20), (0, 100, 20)])
        self.assertAlmostEqual(kit.feature_widths([('outer', square_ish)])['ink'][0], 100)
        octagon = kit.chamfer([(0, 0, 40), (100, 0, 40), (100, 100, 40), (0, 100, 40)])
        self.assertAlmostEqual(kit.feature_widths([('outer', octagon)])['ink'][0], 120 / 2 ** 0.5)

    def test_slivers_finds_short_edges_and_sharp_or_flat_corners(self):
        self.assertEqual(kit.slivers([('outer', square(0, 0, 100, 100))]), [])
        short = [(0, 0), (100, 0), (100, 100), (3, 100), (0, 97)]
        self.assertIn('short edge', {kind for kind, _, _ in kit.slivers([('outer', short)])})
        sharp = [(0, 0), (100, 0), (0, 20)]
        self.assertIn('sharp corner', {kind for kind, _, _ in kit.slivers([('outer', sharp)])})
        flat = [(0, 0), (50, 1), (100, 0), (100, 100), (0, 100)]
        self.assertIn('flat corner', {kind for kind, _, _ in kit.slivers([('outer', flat)])})

    def test_parse_reads_the_format(self):
        text = ('# a comment\n\nbox\t180\t180\nadvance\t216\npitch\t216\n'
                'minimum\tink\t36\tcounter\t24\n'
                'I\touter\t54,0 126,0 126,180 54,180\nO\touter\t0,0 180,0 180,180 0,180\n'
                'O\thole\t60,60 60,120 120,120 120,60\n')
        metrics, letters = X.parse(text)
        self.assertEqual(metrics, {'box': (180, 180), 'advance': 216, 'pitch': 216,
                                   'minimum': {'ink': 36, 'counter': 24}})
        self.assertEqual(letters['I'], [('outer', [(54, 0), (126, 0), (126, 180), (54, 180)])])
        self.assertEqual([kind for kind, _ in letters['O']], ['outer', 'hole'])
        for bad in ('box\t180\n', 'I\tinner\t0,0 1,0 1,1\n', 'II\touter\t0,0 1,0 1,1\n',
                    'I\touter\t0,0 1.5,0 1,1\n'):
            with self.assertRaises(ValueError):
                X.parse(bad)


class CommittedTest(unittest.TestCase):
    """The committed letters (authored, not demo data)."""

    @classmethod
    def setUpClass(cls):
        cls.text = COMMITTED.read_text()
        cls.metrics, cls.letters = X.parse(cls.text)

    def test_committed_file_matches_the_drawings(self):
        text, summary = X.export(X.HERE)
        self.assertEqual(text, self.text, 'run tools/letters/export_letters.py')
        self.assertEqual(len(summary), 10)
        self.assertLess(len(self.text.encode()), 128 * 1024)

    def test_all_ten_letters_and_the_metrics(self):
        self.assertEqual(sorted(self.letters), list('ACEGINORST'))
        self.assertEqual(self.metrics['box'], (kit.BOX, kit.BOX))
        self.assertEqual(self.metrics['advance'], kit.ADVANCE)
        self.assertEqual(self.metrics['pitch'], kit.PITCH)
        self.assertEqual(self.metrics['minimum'], {'ink': kit.MIN_INK, 'counter': kit.MIN_COUNTER})
        # The minimums relative to the letter's height.
        self.assertGreaterEqual(kit.MIN_INK / kit.BOX, 0.2)
        self.assertGreaterEqual(kit.MIN_COUNTER / kit.BOX, 0.13)

    def test_contours_close_and_neither_touch_nor_cross(self):
        for ch, contours in self.letters.items():
            for kind, pts in contours:
                self.assertGreaterEqual(len(pts), 3, ch)
                self.assertEqual(len(set(pts)), len(pts), f'{ch}: a vertex repeats')
                for i in range(len(pts)):  # the last edge closes onto the first vertex
                    self.assertNotEqual(pts[i - 1], pts[i], ch)
            self.assertEqual(kit.crossings(contours), [], ch)

    def test_windings_outers_counter_clockwise_holes_clockwise_and_inside(self):
        for ch, contours in self.letters.items():
            outers = [pts for kind, pts in contours if kind == 'outer']
            holes = [pts for kind, pts in contours if kind == 'hole']
            self.assertTrue(outers, ch)
            for pts in outers:
                self.assertGreater(kit.signed_area(pts), 0, f'{ch}: outer not counter-clockwise')
                others = [('outer', o) for o in outers if o is not pts]
                self.assertFalse(others and kit.inside(pts[0], others), f'{ch}: nested outers')
            for pts in holes:
                self.assertLess(kit.signed_area(pts), 0, f'{ch}: hole not clockwise')
                inner = [('outer', o) for o in outers]
                self.assertTrue(all(kit.inside(p, inner) for p in pts), f'{ch}: hole outside its letter')
                others = [('outer', h) for h in holes if h is not pts]
                self.assertFalse(others and kit.inside(pts[0], others), f'{ch}: nested holes')

    def test_minimum_feature_widths(self):
        minimum = self.metrics['minimum']
        narrowest = {'ink': [], 'counter': []}
        for ch, contours in self.letters.items():
            widths = kit.feature_widths(contours)
            for name in ('ink', 'counter'):
                self.assertGreaterEqual(widths[name][0], minimum[name] - 1e-6, f'{ch} {name}')
                narrowest[name].append(widths[name][0])
        # The stated minimums are met exactly by some letter.
        self.assertAlmostEqual(min(narrowest['ink']), minimum['ink'])
        self.assertAlmostEqual(min(narrowest['counter']), minimum['counter'])

    def test_no_slivers_and_whole_units_in_the_box(self):
        for ch, contours in self.letters.items():
            self.assertEqual(kit.slivers(contours), [], ch)
            ys = [y for _, pts in contours for _, y in pts]
            xs = [x for _, pts in contours for x, _ in pts]
            self.assertEqual((min(ys), max(ys)), (0, kit.BOX), f'{ch}: not full height')
            self.assertGreaterEqual(min(xs), 0, ch)
            self.assertLessEqual(max(xs), kit.BOX, ch)

    def test_every_two_letters_differ_over_a_tenth_of_the_box(self):
        n = 45
        grid = [((i + 0.5) * kit.BOX / n, (j + 0.5) * kit.BOX / n) for i in range(n) for j in range(n)]
        ink = {ch: [kit.inside(p, c) for p in grid] for ch, c in self.letters.items()}
        for a, b in itertools.combinations(sorted(ink), 2):
            differ = sum(x != y for x, y in zip(ink[a], ink[b])) / len(grid)
            self.assertGreater(differ, 0.1, f'{a} and {b}')


if __name__ == '__main__':
    unittest.main()
