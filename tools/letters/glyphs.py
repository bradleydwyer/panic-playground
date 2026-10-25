"""Part 7's greetings letters: A C E G I N O R S T, authored for the remake.

New drawings in the spirit of the original's 18 x 15 letters (very heavy,
nearly square blocks; counters cut as narrow slits and small holes;
chamfered corners), drawn from scratch on a 180-unit grid with whole-unit
vertices. Not traced, vectorised or fitted from the demo's glyphs or from
any font. See kit.py for the box, the units and the contour rules.

Weights: upright stems 64 (E, C, G, R, S, T), horizontal arms 44 to 48 where
two slits share the height (E, S, G), slits 24 across, holes and notches 36
to 40 across. Chamfers are K = 36 where a pen would curve, k = 12 on plain
corners, J = 8 inside counters.
"""
from kit import Glyph, J, K, k, rect

# A: an arch: broad shoulders, a hole under the apex with a roof at 45
# degrees parallel to them, and a notch between the legs up to the
# crossbar, as wide as the hole.
A = Glyph('A', 'arch, peaked hole and notch')
A.outer([(0, 0, k), (68, 0), (68, 56, J), (112, 56, J), (112, 0), (180, 0, k),
         (180, 180, 48), (0, 180, 48)])
A.hole([(68, 92, J), (112, 92, J), (112, 116), (90, 138), (68, 116)])

# C: rounded on the left, one wide slit from the right, its inner end eased.
C = Glyph('C', 'one wide slit from the right')
C.outer([(0, 0, K), (180, 0, k), (180, 70), (64, 70, 12), (64, 110, 12), (180, 110),
         (180, 180, k), (0, 180, K)])

# E: rounded on the left like C, two slits from the right, the middle arm
# a little short of the others.
E = Glyph('E', 'two slits from the right')
E.outer([(0, 0, K), (180, 0, k), (180, 44), (64, 44), (64, 68), (156, 68), (156, 112),
         (64, 112), (64, 136), (180, 136), (180, 180, k), (0, 180, K)])

# G: the C's block with one slit that enters high on the right, runs down
# the inside of the bowl and turns a little way back along the bottom,
# leaving the jaw (bar and spur) joined to the bottom arm on the right.
G = Glyph('G', 'hooked slit, jaw joined at the lower right')
G.outer([(0, 0, K), (180, 0, k), (180, 108), (88, 108), (88, 72), (120, 72), (120, 48),
         (64, 48, 12), (64, 132, 12), (180, 132), (180, 180, k), (0, 180, K)])

# I: one stem, a little wider than the others' stems as it stands alone.
I = Glyph('I', 'a single stem')
I.outer(rect(54, 0, 126, 180, k))

# N: two stems and the diagonal between them, shown by two slits, one up
# from the bottom on the left and one down from the top on the right, each
# end chamfered along the diagonal (the letter turned half round is
# itself). The diagonal band between the chamfers is 38 across.
N = Glyph('N', 'two staggered slits, ends cut along the diagonal')
N.outer([(0, 0, k), (60, 0), (60, 85), (68, 85), (84, 69), (84, 0), (180, 0, k),
         (180, 180, k), (120, 180), (120, 95), (112, 95), (96, 111), (96, 180), (0, 180, k)])

# O: rounded at every corner, with a small upright hole.
O = Glyph('O', 'block with a small upright hole')
O.outer(rect(0, 0, 180, 180, K))
O.hole(rect(70, 56, 110, 124, 12))

# R: a square stem side, the bowl rounded at the top right with a hole in
# it, a notch from the right where the bowl meets the leg, and a notch
# between the stem and the leg.
R = Glyph('R', 'bowl hole, waist notch, notch under the bowl')
R.outer([(0, 0, k), (64, 0), (64, 72, J), (104, 72, J), (104, 0), (180, 0, k),
         (180, 64), (156, 88), (180, 112), (180, 180, K), (0, 180, k)])
R.hole(rect(64, 108, 120, 140, J))

# S: rounded at the top left and bottom right; a slit from the right above
# the middle and one from the left below it.
S = Glyph('S', 'slit from the right above, from the left below')
S.outer([(0, 0, k), (180, 0, K), (180, 112, 12), (64, 112), (64, 136, 12), (180, 136),
         (180, 180, k), (0, 180, K), (0, 68, 12), (116, 68), (116, 44, 12), (0, 44)])

# T: a full-width bar over a centred stem.
T = Glyph('T', 'bar and stem')
T.outer([(58, 0, k), (122, 0, k), (122, 124), (180, 124, k), (180, 180, k), (0, 180, k),
         (0, 124, k), (58, 124)])

LETTERS = {g.char: g for g in (A, C, E, G, I, N, O, R, S, T)}
