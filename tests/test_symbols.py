"""Part 6's authored symbols: the exporter, and the committed file against its sources.

The drawings in tools/symbols/ are authored for the remake, not demo data.
"""
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools' / 'symbols'))
sys.dont_write_bytecode = True
import export_symbols as E

try:  # neon.py imports these for its renderer.
    import numpy  # noqa: F401
    import PIL  # noqa: F401
    KIT = True
except ImportError:
    KIT = False


class ExportTest(unittest.TestCase):
    def test_simplify_keeps_the_ends_and_drops_small_wobbles(self):
        wobbly = [(0, 0), (1, 0.1), (2, -0.1), (3, 0.05), (10, 0)]
        self.assertEqual(E.simplify(wobbly, 0.25), [(0, 0), (10, 0)])
        corner = [(0, 0), (5, 0.1), (10, 0), (10, 5), (10, 10)]
        self.assertEqual(E.simplify(corner, 0.25), [(0, 0), (10, 0), (10, 10)])
        self.assertEqual(E.simplify([(1, 2)], 0.25), [(1, 2)])

    def test_runs_round_to_the_grid_and_a_dot_is_one_point(self):
        self.assertEqual(E.encode_run([(0, 0), (12.34, 56.78)]), ('0,0 123,568', 2))
        self.assertEqual(E.encode_run([(49.99, 50), (50.01, 50)]), ('500,500', 1))
        self.assertEqual(E.clean('a\tb   c '), 'a b c')

    @unittest.skipUnless(KIT, 'neon.py needs numpy and Pillow')
    def test_committed_file_matches_the_sources(self):
        text, summary = E.export(E.HERE)
        committed = (ROOT / 'rust' / 'src' / 'hd' / 'rings_symbols.txt').read_text()
        self.assertEqual(text, committed, 'run tools/symbols/export_symbols.py')
        self.assertEqual(len(summary), E.SETS)
        self.assertLess(len(committed.encode()), 100 * 1024)


if __name__ == '__main__':
    unittest.main()
