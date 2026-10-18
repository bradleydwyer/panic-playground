"""Pattern 1, Water: authored neon symbols for the remake, not recovered ones.

New line art, not taken from the original demo. Sixteen distinct things of
the sea and of water: a lighthouse, a crown splash, a ripple, a sailboat, a
breaking wave's crest, a jellyfish, a fish, a starfish, a scallop shell, an
umbrella in the rain, a whale, a tap, a whirlpool, a submarine, an anchor
and a rubber duck. Each is drawn with its own silhouette, size and place,
so that the music's flips from one to the next read as a change.
"""
import math

from neon import Shape

TITLE = "Water"


def _bez(p0, p1, p2, p3, steps=20):
    pts = []
    for i in range(steps + 1):
        t = i / steps
        u = 1 - t
        pts.append((u**3 * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t**3 * p3[0],
                    u**3 * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t**3 * p3[1]))
    return pts


def _path(start, *segs):
    """One polyline from cubic segments: each seg is (c1, c2, end)."""
    pts = [start]
    cur = start
    for c1, c2, end in segs:
        pts += _bez(cur, c1, c2, end)[1:]
        cur = end
    return pts


def _arc_pts(c, r, a0, a1):
    n = max(8, int(abs(a1 - a0) / 4))
    return [(c[0] + r * math.cos(math.radians(a0 + (a1 - a0) * k / n)),
             c[1] + r * math.sin(math.radians(a0 + (a1 - a0) * k / n))) for k in range(n + 1)]


def _chaikin(pts, times):
    """Round a closed polygon's corners by corner cutting."""
    for _ in range(times):
        out = []
        for a, b in zip(pts, pts[1:] + pts[:1]):
            out += [(0.75 * a[0] + 0.25 * b[0], 0.75 * a[1] + 0.25 * b[1]),
                    (0.25 * a[0] + 0.75 * b[0], 0.25 * a[1] + 0.75 * b[1])]
        pts = out
    return pts


def _rotate(shape, pivot, deg):
    a = math.radians(deg)
    ca, sa = math.cos(a), math.sin(a)
    px, py = pivot
    out = Shape()
    for run in shape.runs:
        out.poly([(px + (x - px) * ca - (y - py) * sa, py + (x - px) * sa + (y - py) * ca)
                  for x, y in run])
    return out


# 0: a lighthouse throwing its beam both ways.
def lighthouse():
    s = Shape()
    s.dot((50, 8))
    s.poly([(42, 35), (42, 26)] + _arc_pts((50, 26), 8, 180, 360) + [(58, 35)])
    s.line((30, 42), (70, 42))
    s.poly([(39, 49), (33, 94), (67, 94), (61, 49)])
    s.line((43.5, 64), (56.5, 64))
    s.line((41.5, 79), (58.5, 79))
    for side in (-1, 1):
        s.line((50 + side * 15, 25), (50 + side * 44, 14))
        s.line((50 + side * 15, 32), (50 + side * 44, 40))
    return s


# 1: a drop's crown splash rising out of the water line.
WATER_Y = 88


def splash():
    s = Shape()
    # A crown wall rising out of the water line, with four spikes, each
    # throwing a droplet outward.
    s.line((8, WATER_Y), (92, WATER_Y))
    s.poly(_path((34, WATER_Y), ((33, 74), (22, 58), (14, 44)),
                 ((22, 54), (26, 60), (30, 60)),
                 ((34, 60), (37, 48), (38, 34)),
                 ((42, 48), (46, 58), (50, 58)),
                 ((54, 58), (58, 48), (62, 34)),
                 ((63, 48), (66, 60), (70, 60)),
                 ((74, 60), (78, 54), (86, 44)),
                 ((78, 58), (67, 74), (66, WATER_Y))))
    for p in [(8, 28), (33, 19), (67, 19), (92, 28)]:
        s.dot(p)
    return s


# 2: a ripple seen in perspective, one flat ring.
def ripple():
    s = Shape()
    s.ellipse((50, 50), 31, 19)
    return s


# 3: a sailboat: hull, mast, mainsail and jib.
def sailboat():
    s = Shape()
    s.poly([(8, 72), (92, 72), (78, 90), (22, 90)], closed=True)
    s.line((50, 6), (50, 64))
    s.poly([(57, 10), (88, 62), (57, 62)])
    s.poly([(43, 20), (14, 62), (43, 62)])
    return s


# 4: a wave's crest leaning forward.
def crest():
    s = Shape()
    s.poly(_path((3, 86), ((27, 86), (33, 26), (71, 16)),
                 ((57, 30), (59, 64), (91, 86))))
    return s


# 5: a jellyfish: a domed bell with a scalloped rim, trailing tentacles.
def jellyfish():
    s = Shape()
    pts = _arc_pts((50, 48), 32, 180, 360)
    # The rim, right to left, in four scallops.
    x = 82
    for k in range(4):
        pts += _bez((x, 48), (x, 56), (x - 16, 56), (x - 16, 48))[1:]
        x -= 16
    s.poly(pts)
    s.arc((50, 48), 20, 215, 290)
    # Four wavy tentacles from under the rim, the outer two shorter.
    for k, (x, y1) in enumerate([(29, 84), (43, 95), (57, 95), (71, 84)]):
        s.poly([(x + 3.5 * math.sin(2 * math.pi * (y - 62) / 16 + k), y)
                for y in [62 + (y1 - 62) * i / 30 for i in range(31)]])
    return s


# 6: a fish facing right.
def fish():
    s = Shape()
    s.poly(_path((90, 50), ((80, 26), (50, 22), (32, 44)),
                 ((24, 36), (16, 30), (10, 26)),
                 ((14, 40), (14, 60), (10, 74)),
                 ((16, 70), (24, 64), (32, 56)),
                 ((50, 78), (80, 74), (90, 50))))
    s.dot((72, 45))
    return s


# 7: a starfish: five fat arms tapering to rounded tips, bent a little as
# a live one lies, dotted with tubercles.
def starfish():
    s = Shape()
    c = (50, 51)
    reach = [38, 37, 39, 36, 38]  # tip centres from the middle, a little uneven
    tip, valley = 9.5, 19  # tip radius; the valleys' distance from the middle
    turn, curl = 8, 0.3  # degrees off upright; radians of bend at the tips

    def at(r, a):
        return (c[0] + r * math.cos(math.radians(a)), c[1] + r * math.sin(math.radians(a)))

    def bent(p):
        # Bend the arms a little, more towards their tips.
        r = math.hypot(p[0] - c[0], p[1] - c[1])
        a = math.atan2(p[1] - c[1], p[0] - c[0]) + curl * (r / 44) ** 2
        return (c[0] + r * math.cos(a), c[1] + r * math.sin(a))

    poly = []
    for k in range(5):
        a = -90 + turn + 72 * k
        t = at(reach[k], a)
        # Each side runs from a valley to its tangent on the tip's circle,
        # so the arm tapers; the tip is the far arc between the tangents.
        v0, v1 = at(valley, a - 36), at(valley, a + 36)
        g0 = (math.degrees(math.atan2(v0[1] - t[1], v0[0] - t[0]))
              + math.degrees(math.acos(tip / math.hypot(v0[0] - t[0], v0[1] - t[1]))))
        g1 = (math.degrees(math.atan2(v1[1] - t[1], v1[0] - t[0]))
              - math.degrees(math.acos(tip / math.hypot(v1[0] - t[0], v1[1] - t[1]))))
        while g1 < g0:
            g1 += 360
        for i in range(13):
            g = math.radians(g0 + (g1 - g0) * i / 12)
            poly.append((t[0] + tip * math.cos(g), t[1] + tip * math.sin(g)))
        poly.append(v1)
    pts = _chaikin([bent(p) for p in poly], 3)
    # Leave the tube's return in the valley between the two lower arms
    # rather than at the lowest arm's tip, where the neon rules would put it.
    v = bent(at(valley, -90 + turn + 72 * 2 + 36))
    k = min(range(len(pts)), key=lambda i: math.hypot(pts[i][0] - v[0], pts[i][1] - v[1]))
    pts = pts[k:] + pts[:k]
    s.poly([p for p in pts if math.hypot(p[0] - v[0], p[1] - v[1]) > 3.4])
    for k in range(5):
        s.dot(bent(at(23, -90 + turn + 72 * k)))
    s.dot(c)
    return s


# 8: a scallop shell: a ribbed fan with a lobed rim on a hinge with ears.
def shell():
    s = Shape()
    h = (50, 88)
    a0, a1, lobes, R = 215, 325, 5, 54
    span = (a1 - a0) / lobes
    rim = []
    for i in range(lobes * 12 + 1):
        a = a0 + (a1 - a0) * i / (lobes * 12)
        frac = ((a - a0) / span) % 1
        r = R + 7 * math.sin(math.pi * frac)
        rim.append((h[0] + r * math.cos(math.radians(a)), h[1] + r * math.sin(math.radians(a))))
    pts = rim + [(60, 82), (74, 80), (70, 94), (30, 94), (26, 80), (40, 82)]
    s.poly(pts + [rim[0]])
    for k in range(1, lobes):
        a = math.radians(a0 + span * k)
        s.line((h[0] + 24 * math.cos(a), h[1] + 24 * math.sin(a)),
               (h[0] + (R - 7) * math.cos(a), h[1] + (R - 7) * math.sin(a)))
    return s


# 9: an umbrella tilted into the rain, the rain in short slanted dashes.
def umbrella():
    u = Shape()
    pts = _arc_pts((50, 44), 40, 180, 360)
    x = 90
    for k in range(4):
        pts += _bez((x, 44), (x - 2, 36), (x - 18, 36), (x - 20, 44))[1:]
        x -= 20
    u.poly(pts)
    u.poly([(50, 51), (50, 80)] + _arc_pts((42, 80), 8, 0, 180)[1:])
    s = _rotate(u, (50, 50), -16)
    dx, dy = 3.0, 8.0
    for x, y in [(2, 2), (82, 0), (94, 12), (5, 66), (19, 74), (7, 86), (27, 90),
                 (68, 56), (83, 50), (92, 66), (76, 72), (89, 84), (69, 88)]:
        s.line((x, y), (x + dx, y + dy))
    return s


# 10: a whale blowing, its tail raised and flukes spread.
def whale():
    s = Shape()
    s.poly(_path((8, 64), ((8, 46), (24, 38), (42, 38)),
                 ((58, 38), (66, 46), (74, 42)),
                 ((78, 40), (78, 32), (74, 26)),
                 ((70, 20), (66, 18), (62, 14)),
                 ((72, 14), (80, 18), (85, 24)),
                 ((88, 16), (94, 10), (99, 8)),
                 ((98, 18), (94, 26), (89, 32)),
                 ((90, 56), (78, 86), (44, 86)),
                 ((20, 86), (8, 78), (8, 64))))
    s.dot((25, 56))
    s.poly(_path((17, 69), ((22, 73), (30, 73), (36, 70))))
    s.poly(_path((28, 30), ((26, 22), (20, 16), (12, 16))))
    s.poly(_path((38, 30), ((40, 22), (46, 16), (54, 16))))
    return s


# 11: a tap: a curved spout on its wall, a handle on top, water running.
def tap():
    s = Shape()
    s.poly([(84, 74), (84, 62)] + _arc_pts((62, 62), 22, 0, -90)[1:] + [(10, 40), (10, 54), (62, 54)]
           + _arc_pts((62, 62), 8, -90, 0)[1:] + [(70, 74)])
    s.poly([(33, 33), (33, 24), (22, 24), (22, 14), (54, 14), (54, 24), (43, 24), (43, 33)])
    s.line((77, 81), (77, 98))
    return s


# 12: a whirlpool spiral.
def whirlpool():
    s = Shape()
    s.spiral((50, 50), 4, 42, turns=2.5, start=0)
    return s


# 13: a submarine: hull, conning tower and periscope, portholes, propeller.
def submarine():
    s = Shape()
    s.rect((6, 52), (88, 84), r=16)
    s.poly([(36, 46), (40, 34), (62, 34), (66, 46)])
    s.poly([(46, 28), (46, 14), (58, 14)])
    for x in (28, 48, 68):
        s.circle((x, 68), 6)
    s.line((96, 58), (96, 78))
    return s


# 14: an anchor on its ring and rope.
def anchor():
    s = Shape()
    s.line((50, 0), (50, 13))
    s.circle((50, 19), 6)
    s.line((50, 25), (50, 88))
    s.line((34, 36), (66, 36))
    s.arc((50, 60), 28, 15, 165)
    return s


# 15: a rubber duck facing left, beak, eye and wing.
def rubber_duck():
    s = Shape()
    s.poly(_path((52, 50), ((60, 40), (56, 18), (38, 18)),
                 ((28, 18), (22, 24), (22, 32)),
                 ((16, 30), (8, 32), (6, 36)),
                 ((10, 42), (18, 44), (24, 44)),
                 ((14, 52), (10, 62), (14, 72)),
                 ((20, 88), (40, 90), (60, 90)),
                 ((80, 90), (94, 78), (94, 50)),
                 ((88, 58), (76, 60), (68, 56)),
                 ((62, 54), (56, 52), (52, 50))))
    s.dot((38, 32))
    s.poly(_path((40, 66), ((46, 78), (66, 80), (76, 70))))
    return s


SHAPES = {
    0: ("lighthouse", lighthouse()),
    1: ("splash", splash()),
    2: ("ripple", ripple()),
    3: ("sailboat", sailboat()),
    4: ("wave crest", crest()),
    5: ("jellyfish", jellyfish()),
    6: ("fish", fish()),
    7: ("starfish", starfish()),
    8: ("shell", shell()),
    9: ("umbrella", umbrella()),
    10: ("whale", whale()),
    11: ("tap", tap()),
    12: ("whirlpool", whirlpool()),
    13: ("submarine", submarine()),
    14: ("anchor", anchor()),
    15: ("rubber duck", rubber_duck()),
}
