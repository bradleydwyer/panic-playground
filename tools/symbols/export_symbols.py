#!/usr/bin/env python3
"""Export the authored part-6 symbol sets to rust/src/hd/rings_symbols.txt.

    python3 tools/symbols/export_symbols.py [SHAPES_DIR]

SHAPES_DIR (by default this folder) holds the drawing kit, neon.py, and
pattern_1.py to pattern_7.py, each with TITLE, SHAPES = {mask: (label,
Shape)} and optionally NOT_SHOWN. The drawings are authored for the remake,
not recovered from the demo. SHAPES_DIR is only read: bytecode writing is
turned off so nothing lands there. A missing pattern is exported as an empty
set; one that fails to import is an error. neon.py imports numpy and Pillow
for its renderer.

Every run is simplified (Ramer-Douglas-Peucker, 0.25 drawing units) and its
points rounded to a 0..1000 grid (the 100-unit box times 10).
"""
import importlib
import math
import sys
from pathlib import Path

sys.dont_write_bytecode = True

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
OUT = ROOT / 'rust' / 'src' / 'hd' / 'rings_symbols.txt'
SETS = 7
TOLERANCE = 0.25  # drawing units
GRID = 10  # grid steps per drawing unit

HEADER = """\
# Part 6's new symbols: authored for this remake, not recovered from the demo.
#
# These are new line drawings made for the modern rendering of part 6. They
# are not the original's symbols and were not derived from its data: those 16
# masks are decoded locally from M.SUM when the preview loads and are never
# stored in Git. In the middle of part 6 (the tune's 7 orders from about 50.8
# to 76.6 seconds, symbol windows 64 to 287) each order shows one set below
# in place of the originals, set k in the k-th of those orders; the music
# still picks which mask lights and when. The original symbols play before
# and after, and faithful HD always shows them.
#
# Some drawings are simplified neon tracings of company logos, at the owner's
# request: Claude, ChatGPT, Apple, Bitcoin, YouTube, Instagram, X, WhatsApp,
# Facebook, Reddit, LinkedIn and TikTok. Those logos are trademarks of their
# owners; their use here implies no affiliation or endorsement.
#
# Format. Lines starting with # are comments, and blank lines are ignored.
#   set<TAB>k<TAB>title
# names set k (1 to 7). Then, for each drawn mask,
#   k<TAB>n<TAB>label<TAB>runs
# is set k's drawing for the original's mask n (0 to 15). Masks a set leaves
# out (the music never lights them in its pattern) are empty. The runs are
# separated by " | "; each is a polyline of points "x,y" separated by
# spaces, in a 100-unit drawing box on a 0 to 1000 grid, x right and y down
# (points may fall a little outside it). A run of one point is a dot. Runs
# are simplified to within a quarter of a unit (Ramer-Douglas-Peucker).
# hd/rings_authored.rs makes the runs into neon tubes as a sign maker would:
# a run whose last point is its first is a loop, runs that meet end to end
# are one bent tube, a loop opens at its lowest point, and where tubes meet
# or cross, one stops short of the other with an air gap.
#
# Made by tools/symbols/export_symbols.py from the drawings in tools/symbols/;
# edit those and export again rather than editing this file.
"""


def simplify(points, tolerance):
    """Ramer-Douglas-Peucker, keeping both ends."""
    if len(points) < 3:
        return list(points)
    (ax, ay), (bx, by) = points[0], points[-1]
    dx, dy = bx - ax, by - ay
    length = math.hypot(dx, dy)
    far, at = 0.0, 0
    for i in range(1, len(points) - 1):
        px, py = points[i]
        if length < 1e-9:
            d = math.hypot(px - ax, py - ay)
        else:
            d = abs((px - ax) * dy - (py - ay) * dx) / length
        if d > far:
            far, at = d, i
    if far > tolerance:
        left = simplify(points[:at + 1], tolerance)
        right = simplify(points[at:], tolerance)
        return left[:-1] + right
    return [points[0], points[-1]]


def encode_run(run):
    points = simplify([(float(x), float(y)) for x, y in run], TOLERANCE)
    grid = []
    for x, y in points:
        p = (round(x * GRID), round(y * GRID))
        if not grid or grid[-1] != p:
            grid.append(p)
    return ' '.join(f'{x},{y}' for x, y in grid), len(grid)


def clean(text):
    return ' '.join(str(text).replace('\t', ' ').split())


def export(shapes_dir):
    """The file's text, and a line of summary per set."""
    sys.path.insert(0, str(shapes_dir))
    lines, summary = [HEADER], []
    for k in range(1, SETS + 1):
        if not (shapes_dir / f'pattern_{k}.py').exists():  # Still being drawn.
            summary.append(f'set {k}: no pattern_{k}.py; exported empty')
            lines.append(f'set\t{k}\t(not drawn yet)')
            continue
        module = importlib.import_module(f'pattern_{k}')
        title = clean(module.TITLE)
        lines.append(f'set\t{k}\t{title}')
        not_shown = set(getattr(module, 'NOT_SHOWN', []))
        count = 0
        for n in range(16):
            if n not in module.SHAPES or n // 2 in not_shown:
                continue
            label, shape = module.SHAPES[n]
            runs = [text for text, points in map(encode_run, shape.runs) if points]
            if runs:
                lines.append(f'{k}\t{n}\t{clean(label)}\t' + ' | '.join(runs))
                count += 1
        summary.append(f'set {k}: {title}: {count} shapes, not shown {sorted(not_shown)}')
    return '\n'.join(lines) + '\n', summary


def main():
    shapes_dir = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else HERE
    text, summary = export(shapes_dir)
    print('\n'.join(summary))
    OUT.write_text(text)
    print(f'{OUT.relative_to(ROOT)}: {len(text.encode())} bytes')


if __name__ == '__main__':
    main()
