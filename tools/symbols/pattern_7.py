"""Pattern 7, "AI": the last pattern before the original symbols return (authored).

New line art for the remake, not recovered from the original demo. Each pair
is a two-frame flip: B is A with one clear change, or what A turns into, and
is drawn at A's size and place so the change reads.
"""
import math

from neon import Shape

TITLE = "AI"
NOT_SHOWN = [3]  # masks 6 and 7 never appear in this pattern


# Helpers -----------------------------------------------------------------

def _arc_pts(c, r, a0, a1, n=None):
    n = n or max(8, int(abs(a1 - a0) / 4))
    return [(c[0] + r * math.cos(math.radians(a0 + (a1 - a0) * k / n)),
             c[1] + r * math.sin(math.radians(a0 + (a1 - a0) * k / n))) for k in range(n + 1)]


def _bez(p0, p1, p2, p3, n=16):
    return [((1 - t) ** 3 * p0[0] + 3 * (1 - t) ** 2 * t * p1[0] + 3 * (1 - t) * t * t * p2[0] + t ** 3 * p3[0],
             (1 - t) ** 3 * p0[1] + 3 * (1 - t) ** 2 * t * p1[1] + 3 * (1 - t) * t * t * p2[1] + t ** 3 * p3[1])
            for t in [k / n for k in range(n + 1)]]


def _xf(pts, at, deg, mirror=False):
    """Local points turned `deg` degrees and moved so the local origin is `at`.
    `mirror` flips local x first."""
    t = math.radians(deg)
    out = []
    for x, y in pts:
        if mirror:
            x = -x
        out.append((at[0] + x * math.cos(t) - y * math.sin(t), at[1] + x * math.sin(t) + y * math.cos(t)))
    return out


def sparkle(s, c, rx, ry, pinch=0.16):
    """A four-point star whose sides curve in towards the centre."""
    cx, cy = c
    tips = [(cx, cy - ry), (cx + rx, cy), (cx, cy + ry), (cx - rx, cy)]
    pts = []
    for i in range(4):
        a, b = tips[i], tips[(i + 1) % 4]
        k = (cx + (a[0] + b[0] - 2 * cx) * pinch, cy + (a[1] + b[1] - 2 * cy) * pinch)
        for j in range(24 if i < 3 else 25):
            t = j / 24
            u = 1 - t
            pts.append((u * u * a[0] + 2 * u * t * k[0] + t * t * b[0],
                        u * u * a[1] + 2 * u * t * k[1] + t * t * b[1]))
    return s.poly(pts)


def scallop(c, rx, ry, a0, a1, bumps, depth):
    """Points along an ellipse from a0 to a1 degrees, pushed out into `bumps`
    rounded lobes with notches between them."""
    pts = []
    n = bumps * 16
    for k in range(n + 1):
        f = k / n
        a = math.radians(a0 + (a1 - a0) * f)
        bump = abs(math.sin(math.pi * bumps * f)) ** 0.7
        g = 1 + depth * (bump - 1)
        pts.append((c[0] + rx * g * math.cos(a), c[1] + ry * g * math.sin(a)))
    return pts


def bubble(x0, y0, x1, y1, r, tail):
    """A rounded-rectangle speech bubble as one closed run. `tail` is
    (x_left, x_right, tip): the bottom edge leaves at x_right, goes to the
    tip and comes back at x_left."""
    xa, xb, tip = tail
    pts = _arc_pts((x0 + r, y0 + r), r, 180, 270, 8)
    pts += _arc_pts((x1 - r, y0 + r), r, 270, 360, 8)
    pts += _arc_pts((x1 - r, y1 - r), r, 0, 90, 8)
    pts += [(xb, y1), tip, (xa, y1)]
    pts += _arc_pts((x0 + r, y1 - r), r, 90, 180, 8)
    pts.append(pts[0])
    return pts


# 0: light bulb: the glass and its neck in one run, closed by the top of the
# screw base, two more base lines and a few rays.
bulb = Shape()
G, GR = (50, 40), 26
bulb.poly([(39, 74)] + _bez((39, 74), (39, 68), (36, 64), (G[0] + GR * math.cos(math.radians(125)),
                                                          G[1] + GR * math.sin(math.radians(125))), 6)[1:]
          + _arc_pts(G, GR, 125, 415)[1:]
          + _bez((G[0] + GR * math.cos(math.radians(55)), G[1] + GR * math.sin(math.radians(55))),
                 (64, 64), (61, 68), (61, 74), 6)[1:] + [(39, 74)])
bulb.line((41, 84), (59, 84))
bulb.line((46, 94), (54, 94))
for a in (180, 225, 270, 315, 0):
    bulb.line(_arc_pts(G, GR + 10, a, a, 1)[0], _arc_pts(G, GR + 19, a, a, 1)[0])

# 1: brain, side view facing left: a lobed outline, the cerebellum tucked
# under the back, the brain stem, and three folds.
brain = Shape()
top = scallop((50, 44), 42, 32, 150, 395, 7, 0.10)
under = _bez(top[-1], (70, 70), (40, 78), top[0], 20)[1:]
brain.poly(top + under)
brain.poly(_arc_pts((73, 68), 12, -10, 150))
brain.poly(_bez((58, 72), (60, 80), (60, 86), (62, 94), 8))
brain.poly(_bez((24, 56), (36, 46), (50, 54), (66, 42), 16))
brain.poly(_bez((50, 14), (44, 22), (54, 28), (48, 38), 12))
brain.poly(_bez((24, 30), (30, 28), (34, 34), (32, 42), 10))

# 2: chat bubble.
chat = Shape()
chat.poly(bubble(8, 12, 92, 72, 16, (24, 40, (18, 92))))

# 3: the same bubble with three typing dots.
typing = Shape()
typing.poly(bubble(8, 12, 92, 72, 16, (24, 40, (18, 92))))
for x in (31, 50, 69):
    typing.circle((x, 42), 3)

# 4 and 5: a small neural network, 3 - 3 - 2 nodes, every node joined to
# every node in the next column. In 5 one path, bottom left to top right,
# fires: its nodes grow and light at the core, and only its links stay lit.
NET = [[(12, 20), (12, 50), (12, 80)], [(50, 20), (50, 50), (50, 80)], [(88, 35), (88, 65)]]
NODE_R = 6.5
LIT_R = 7.5
LIT = [(0, 2), (1, 1), (2, 0)]  # the lit path in 5: (column, row)


def _net(lit=False):
    s = Shape()

    def r(c, k):
        return LIT_R if lit and (c, k) in LIT else NODE_R

    for c in range(2):
        for i, a in enumerate(NET[c]):
            for j, b in enumerate(NET[c + 1]):
                if lit and not ((c, i) in LIT and (c + 1, j) in LIT):
                    continue
                d = math.hypot(b[0] - a[0], b[1] - a[1])
                ux, uy = (b[0] - a[0]) / d, (b[1] - a[1]) / d
                ra, rb = r(c, i), r(c + 1, j)
                s.line((a[0] + ux * ra, a[1] + uy * ra), (b[0] - ux * rb, b[1] - uy * rb))
    for c, col in enumerate(NET):
        for k, p in enumerate(col):
            s.circle(p, r(c, k))
            if lit and (c, k) in LIT:
                s.dot(p)
    return s


net = _net()
net_lit = _net(lit=True)

# 8: magnifying glass, searching: the lens, a glint and the handle.
mag = Shape()
mag.circle((42, 42), 30)
mag.arc((42, 42), 19, 195, 255)
mag.line((63.2, 63.2), (88, 88))

# 9: asking: a round chat bubble in the lens's place, its tail where the
# handle was, with a four-point sparkle inside.
ask = Shape()
ask.poly(_arc_pts((42, 42), 30, 62, 388)[:-1] + [(84, 86)] + [_arc_pts((42, 42), 30, 62, 62, 1)[0]])
sparkle(ask, (42, 42), 15, 19)

# 10: pocket calculator: body, display and a grid of keys.
calc = Shape()
calc.rect((22, 4), (78, 96), r=8)
calc.rect((32, 14), (68, 32), r=3)
for y in (46, 59, 72, 85):
    for x in (36, 50, 64):
        calc.dot((x, y))

# 11: AI chip: a square package with legs on every side and a small brain
# outline inside.
chip = Shape()
chip.rect((18, 18), (82, 82), r=5)
for p in (32, 50, 68):
    chip.line((p, 18), (p, 5))
    chip.line((p, 82), (p, 95))
    chip.line((18, p), (5, p))
    chip.line((82, p), (95, p))
mini = scallop((50, 50), 19, 15, 160, 380, 4, 0.14)
chip.poly(mini + _bez(mini[-1], (62, 66), (38, 66), mini[0], 12)[1:])
chip.line((50, 38), (50, 58))

# 12: an eye: almond lids, iris and pupil.
LID_C, LID_R = 64.1, 48.1


def _lids(s):
    a = math.degrees(math.asin(46 / LID_R))
    s.poly(_arc_pts((50, LID_C), LID_R, 270 - a, 270 + a) + _arc_pts((50, 100 - LID_C), LID_R, 90 - a, 90 + a)[1:])
    return s


eye = _lids(Shape())
eye.circle((50, 50), 23)
eye.circle((50, 50), 4)

# 13: the eye as a camera lens: two iris rings and a glint.
lens = _lids(Shape())
lens.circle((50, 50), 23)
lens.circle((50, 50), 12.5)
lens.dot((48, 48))

# 14 and 15: the reaching hands (hero), after the Creation of Adam. A hand is
# drawn pointing along local +x, fingertip at the local origin, palm down: the
# index finger out, the other three fingers curled under it as stacked lobes,
# and a cuff at the wrist. The robot hand is the same pose made mechanical
# (square lobes and fingertip, two joints across the finger, a square wrist
# band) and mirrored, so it reaches in from the right as the human hand
# reaches up from the left. Both are drawn at 0.94 scale about the centre so
# the pair fits the box with the fingertips about 12 apart. The human hand in
# 14 sits where it is in 15, so the robot hand is what the flip adds.
FR = 5.5           # finger half-width
FIST = -22         # where the index finger leaves the fist
WRIST = FIST - 13
CUFF = 10.5        # cuff and wrist band width
LOBES = [(FIST, 2 * FR), (FIST - 1, 4 * FR), (FIST - 3, 6 * FR)]
PALM = LOBES[-1][1] + FR  # bottom of the last lobe


def human_hand():
    pts = ([(WRIST, -6)] + _bez((WRIST, -6), (WRIST + 3, -11), (WRIST + 8, -11.5), (FIST - 6, -FR), 8)[1:]
           + [(-FR, -FR)] + _arc_pts((-FR, 0), FR, -90, 90, 10)[1:] + [(FIST, FR)])
    for c in LOBES:
        pts += _arc_pts(c, FR, -90, 90, 10)[1:]
    pts += _bez((LOBES[-1][0], PALM), (FIST - 7, PALM + 1), (WRIST + 6, PALM), (WRIST, PALM - 5), 8)[1:]
    runs = [pts]
    runs.append(_rrect(WRIST - CUFF, -10, WRIST, PALM - 3, 3))
    return runs


def robot_hand():
    pts = [(WRIST, -6), (WRIST + 3, -11), (WRIST + 9, -11), (FIST - 6, -FR),
           (-2, -FR), (0, -FR + 2), (0, FR - 2), (-2, FR), (FIST, FR)]
    for cx, cy in LOBES:
        pts += [(cx + FR - 1, cy - FR), (cx + FR, cy - FR + 1), (cx + FR, cy + FR - 1), (cx + FR - 1, cy + FR), (cx, cy + FR)]
    pts += [(WRIST + 4, PALM), (WRIST, PALM - 6)]
    runs = [pts]
    runs += [[(x, -FR), (x, FR)] for x in (-9.5, -20)]  # finger joints
    runs.append(_rrect(WRIST - CUFF, -8, WRIST, PALM - 5, 0.5))
    return runs


def _rrect(x0, y0, x1, y1, r):
    pts = []
    for cx, cy, a in [(x1 - r, y0 + r, -90), (x1 - r, y1 - r, 0), (x0 + r, y1 - r, 90), (x0 + r, y0 + r, 180)]:
        pts += _arc_pts((cx, cy), r, a, a + 90, 6)
    return pts + [pts[0]]


HAND_K = 0.94
HUMAN_AT, HUMAN_TURN = (44.4, 36.65), -10
ROBOT_AT, ROBOT_TURN = (56.19, 31.52), 0


def _place(s, runs, at, turn, mirror=False):
    for r in runs:
        s.poly([(50 + (x - 50) * HAND_K, 50 + (y - 50) * HAND_K) for x, y in _xf(r, at, turn, mirror)])
    return s


hand = _place(Shape(), human_hand(), HUMAN_AT, HUMAN_TURN)
touch = _place(Shape(), human_hand(), HUMAN_AT, HUMAN_TURN)
_place(touch, robot_hand(), ROBOT_AT, ROBOT_TURN, mirror=True)


SHAPES = {
    0: ("light bulb", bulb),
    1: ("brain", brain),
    2: ("chat bubble", chat),
    3: ("typing", typing),
    4: ("neural network", net),
    5: ("network, path lit", net_lit),
    8: ("searching", mag),
    9: ("asking", ask),
    10: ("calculator", calc),
    11: ("AI chip", chip),
    12: ("eye", eye),
    13: ("camera-lens eye", lens),
    14: ("reaching hand", hand),
    15: ("human meets robot", touch),
}
