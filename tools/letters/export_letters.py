#!/usr/bin/env python3
"""Export part 7's authored letters to rust/src/hd/greetings_letters.txt.

    python3 tools/letters/export_letters.py [LETTERS_DIR]

LETTERS_DIR (by default this folder) holds the drawing kit, kit.py, and the
drawings, glyphs.py, whose LETTERS maps each of A C E G I N O R S T to a
Glyph. The drawings are authored for the remake, not recovered from the
demo. LETTERS_DIR is only read: bytecode writing is turned off so nothing
lands there. The exporter refuses a letter whose outline crosses itself,
has a contour of the wrong winding, a sliver, or a feature narrower than
the kit's minimums.

parse() reads the file back; the tests use it, and it is a reference for
the format.
"""
import importlib
import sys
from pathlib import Path

sys.dont_write_bytecode = True

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
OUT = ROOT / 'rust' / 'src' / 'hd' / 'greetings_letters.txt'
ALPHABET = 'ACEGINORST'

HEADER = """\
# Part 7's greetings letters: authored for this remake, not recovered from
# the demo.
#
# These are new outlines of the ten letters part 7's five words use (A C E
# G I N O R S T), drawn from scratch in the spirit of the original's 18 by
# 15 pixel letters: very heavy, nearly square blocks, cut by narrow slits
# and small holes, with chamfered corners. They are not the original's
# glyphs, were not traced, vectorised or fitted from its data or from any
# font, and carry no shading: the original's letters are decoded locally
# from the TRUG maps when the preview loads and are never stored in Git.
#
# Units. A letter's box is 180 units wide and 180 tall, x to the right and
# y up, with the baseline at y = 0: the original's letter face of 18
# columns by 15 rows at 10 units a column and 12 a row, since its rows are
# 1.2 times as tall as its columns are wide on the 4:3 screen. So the box
# is square as shown, and a unit is the same length across and up. All
# vertices are whole units inside the box.
#
# Format. Lines starting with # are comments, and blank lines are ignored.
# Fields are separated by tabs.
#   box<TAB>180<TAB>180
# is the letter box's width and height.
#   advance<TAB>216
# is the step from one letter's box to the next set in a line (a gap of
# 36), and
#   pitch<TAB>216
# the step between letters stacked as part 7 stacks them, one to an
# 18-row slot (18 rows of 12 units).
#   minimum<TAB>ink<TAB>36<TAB>counter<TAB>24
# are the narrowest ink and the narrowest counter (slit, hole or notch,
# open or closed) any letter has, measured across, in units: the
# drawings keep at least these. A 45-degree bevel of 8 units on every
# edge, cutting in or building out, leaves at least 20 of ink face and 8
# of counter open.
# Then, for each letter, one line per closed contour:
#   C<TAB>outer<TAB>x,y x,y ...
#   C<TAB>hole<TAB>x,y x,y ...
# is a contour of letter C: its vertices "x,y" separated by spaces, in
# order, the last joining the first (which is not repeated). Edges are
# straight. Outer contours run counter-clockwise (y up) and holes
# clockwise, so the letter's ink is on the left of every edge; a hole lies
# inside its letter's outer contour. No contour touches or crosses itself
# or another.
#
# Made by tools/letters/export_letters.py from the drawings in
# tools/letters/; edit those and export again rather than editing this
# file.
"""


def load(letters_dir):
    """The kit and the drawings in `letters_dir`."""
    sys.path.insert(0, str(letters_dir))
    try:
        kit = importlib.import_module('kit')
        glyphs = importlib.import_module('glyphs')
    finally:
        sys.path.remove(str(letters_dir))
    return kit, glyphs


def problems(kit, contours):
    """What is wrong with a letter's contours, as text; empty if nothing."""
    found = []
    for kind, pts in contours:
        area = kit.signed_area(pts)
        if len(pts) < 3 or (kind == 'outer') != (area > 0):
            found.append(f'{kind} contour wound the wrong way or empty')
        for x, y in pts:
            if not (isinstance(x, int) and isinstance(y, int) and 0 <= x <= kit.BOX and 0 <= y <= kit.BOX):
                found.append(f'vertex {(x, y)} not whole units in the box')
    if kit.crossings(contours):
        found.append(f'outline touches or crosses itself: {kit.crossings(contours)}')
    if kit.slivers(contours):
        found.append(f'slivers: {kit.slivers(contours)}')
    widths = kit.feature_widths(contours)
    if widths['ink'][0] < kit.MIN_INK - 1e-6:
        found.append(f'ink {widths["ink"][0]:.1f} across, under {kit.MIN_INK}')
    if widths['counter'][0] < kit.MIN_COUNTER - 1e-6:
        found.append(f'counter {widths["counter"][0]:.1f} across, under {kit.MIN_COUNTER}')
    return found


def export(letters_dir):
    """The file's text, and a line of summary per letter."""
    kit, glyphs = load(letters_dir)
    lines = [HEADER,
             f'box\t{kit.BOX}\t{kit.BOX}',
             f'advance\t{kit.ADVANCE}',
             f'pitch\t{kit.PITCH}',
             f'minimum\tink\t{kit.MIN_INK}\tcounter\t{kit.MIN_COUNTER}']
    summary = []
    if sorted(glyphs.LETTERS) != sorted(ALPHABET):
        raise ValueError(f'letters drawn: {sorted(glyphs.LETTERS)}, wanted {list(ALPHABET)}')
    for ch in ALPHABET:
        glyph = glyphs.LETTERS[ch]
        wrong = problems(kit, glyph.contours)
        if wrong:
            raise ValueError(f'{ch}: ' + '; '.join(wrong))
        lines.append('')
        lines.append(f'# {ch}: {" ".join(glyph.label.split())}')
        for kind, pts in glyph.contours:
            lines.append(f'{ch}\t{kind}\t' + ' '.join(f'{x},{y}' for x, y in pts))
        widths = kit.feature_widths(glyph.contours)
        holes = sum(kind == 'hole' for kind, _ in glyph.contours)
        summary.append(f'{ch}: {sum(len(p) for _, p in glyph.contours)} vertices, {holes} holes, '
                       f'ink {widths["ink"][0]:.1f}, counter {widths["counter"][0]:.1f}')
    return '\n'.join(lines) + '\n', summary


def parse(text):
    """The file's metrics and letters: ({'box': (w, h), 'advance': a,
    'pitch': p, 'minimum': {'ink': i, 'counter': c}}, {letter: [(kind,
    [(x, y), ...]), ...]})."""
    metrics, letters = {}, {}
    for number, line in enumerate(text.splitlines(), 1):
        if not line.strip() or line.startswith('#'):
            continue
        fields = line.split('\t')
        key = fields[0]
        if key == 'box' and len(fields) == 3:
            metrics['box'] = (int(fields[1]), int(fields[2]))
        elif key in ('advance', 'pitch') and len(fields) == 2:
            metrics[key] = int(fields[1])
        elif key == 'minimum' and len(fields) == 5 and fields[1::2] == ['ink', 'counter']:
            metrics['minimum'] = {'ink': int(fields[2]), 'counter': int(fields[4])}
        elif len(key) == 1 and key.isupper() and len(fields) == 3 and fields[1] in ('outer', 'hole'):
            pts = []
            for pair in fields[2].split(' '):
                x, y = pair.split(',')
                pts.append((int(x), int(y)))
            letters.setdefault(key, []).append((fields[1], pts))
        else:
            raise ValueError(f'line {number}: not understood: {line!r}')
    return metrics, letters


def main():
    letters_dir = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else HERE
    text, summary = export(letters_dir)
    print('\n'.join(summary))
    OUT.write_text(text)
    print(f'{OUT.relative_to(ROOT)}: {len(text.encode())} bytes')


if __name__ == '__main__':
    main()
