"""Pattern 4, "Faces": circular neon faces whose expression flips per pair (authored).

These are new drawings for the remake, not recovered from the original demo.
Every face uses the same outline, so within a pair only the features change.
"""
import math

from neon import Shape

TITLE = "Faces"
NOT_SHOWN = [3]  # masks 6 and 7 never appear in this pattern

C = (50, 50)      # face centre
R = 43            # face outline radius, shared by every face
EYE_X = (36, 64)
EYE_Y = (27, 41)  # top and bottom of an open eye


def _arc_pts(c, r, a0, a1, n=24):
    return [(c[0] + r * math.cos(math.radians(a0 + (a1 - a0) * i / n)),
             c[1] + r * math.sin(math.radians(a0 + (a1 - a0) * i / n))) for i in range(n + 1)]


def _inside(p, poly):
    """Even-odd point-in-polygon test."""
    x, y = p
    hit = False
    for (x0, y0), (x1, y1) in zip(poly, poly[1:] + poly[:1]):
        if (y0 > y) != (y1 > y) and x < x0 + (y - y0) * (x1 - x0) / (y1 - y0):
            hit = not hit
    return hit


def _near(p, pts, d):
    """True if p is within d of the polyline through pts."""
    for (x0, y0), (x1, y1) in zip(pts, pts[1:]):
        vx, vy = x1 - x0, y1 - y0
        t = max(0.0, min(1.0, ((p[0] - x0) * vx + (p[1] - y0) * vy) / (vx * vx + vy * vy or 1)))
        if math.hypot(p[0] - x0 - t * vx, p[1] - y0 - t * vy) < d:
            return True
    return False


def face(hidden=(), gap=2):
    """The shared outline. Where something sits in front of the face (the
    `hidden` outlines, closed across their open ends), the outline stops
    `gap` short of it, centre to centre, so it reads as passing behind."""
    s = Shape()
    if not hidden:
        return s.circle(C, R)
    runs, run = [], []
    for k in range(361):
        a = math.radians(k)
        p = (C[0] + R * math.cos(a), C[1] + R * math.sin(a))
        if any(_inside(p, h) or _near(p, h, gap) for h in hidden):
            if len(run) > 1:
                runs.append(run)
            run = []
        else:
            run.append(p)
    if len(run) > 1:
        if runs and runs[0][0] == (C[0] + R, C[1]):
            runs[0] = run + runs[0][1:]  # join the run that wraps through 0 degrees
        else:
            runs.append(run)
    for r in runs:
        s.poly(r)
    return s


def open_eyes(s, which=(0, 1)):
    for i in which:
        s.line((EYE_X[i], EYE_Y[0]), (EYE_X[i], EYE_Y[1]))
    return s


def closed_eye(s, i):
    """A blink: a flat line across where the open eye was."""
    x, y = EYE_X[i], (EYE_Y[0] + EYE_Y[1]) / 2
    return s.line((x - 8, y), (x + 8, y))


def wink_eye(s, i):
    """A wink: a closed eye bowed upward, like a cheek pushing it shut."""
    x = EYE_X[i]
    return s.arc((x, 43), 10, 205, 335)


def smile(s):
    return s.arc((50, 44), 24, 38, 142)


def neutral():
    s = open_eyes(face())
    return s.line((34, 65), (66, 65))


def blink():
    s = face()
    closed_eye(s, 0)
    closed_eye(s, 1)
    return s.line((34, 65), (66, 65))


def smiling():
    return smile(open_eyes(face()))


def grin():
    s = open_eyes(face())
    # an open D-shaped mouth: flat top lip, round bottom
    top, r = 55, 23
    return s.poly(_arc_pts((50, top), r, 180, 0), closed=True)


def small_o():
    return open_eyes(face()).circle((50, 64), 7.5)


def big_o():
    return open_eyes(face()).ellipse((50, 64), 13, 15)


def wink(i):
    """Eye i (0 = left of the picture) winks; the other stays open."""
    s = face()
    open_eyes(s, (1 - i,))
    wink_eye(s, i)
    return smile(s)


def tongue(length):
    """A flat mouth with a U-shaped tongue hanging `length` below it."""
    s = open_eyes(face())
    y, r = 54, 9
    s.line((31, y), (69, y))
    bottom = y + length
    s.poly([(50 - r, y)] + _arc_pts((50, bottom - r), r, 180, 0) + [(50 + r, y)])
    return s


def _drop(apex, centre, r):
    """A teardrop outline: a point at `apex`, round below about `centre`."""
    dx, dy = apex[0] - centre[0], apex[1] - centre[1]
    up = math.degrees(math.atan2(dy, dx))
    t = math.degrees(math.acos(r / math.hypot(dx, dy)))
    return [apex] + _arc_pts(centre, r, up + t, up + 360 - t)


def sad(tear=False):
    s = open_eyes(face())
    s.arc((50, 88), 20, 220, 320)
    if tear:
        # a drop on the cheek below and outside the right eye
        s.poly(_drop((72.5, 47.5), (73, 59), 5.5), closed=True)
    return s


def _hull(pts):
    """Convex hull of a point list, counter-clockwise (monotone chain)."""
    pts = sorted(set(pts))

    def cross(o, a, b):
        return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])

    lo, hi = [], []
    for q in pts:
        while len(lo) >= 2 and cross(lo[-2], lo[-1], q) <= 0:
            lo.pop()
        lo.append(q)
    for q in reversed(pts):
        while len(hi) >= 2 and cross(hi[-2], hi[-1], q) <= 0:
            hi.pop()
        hi.append(q)
    return lo[:-1] + hi[:-1]


def _left_hand(palm=(15, 70), tilt=20):
    """The left hand, seen from the back and pressed to the cheek: a round
    palm with a knuckle line, three fingers up and the thumb toward the
    mouth. Drawn as one tube that doubles back along the fingers. Local
    units: x toward the face, y down, fingers 10 apart; `tilt` leans the
    fingertips toward the face."""
    local = ([(-10, -22)] + _arc_pts((0, 8), 10, 180, 0, n=12)
             + [(10, 2), (17, -5), (10, 2),          # thumb
                (10, -20), (10, -4),                 # index finger
                (0, -4), (0, -25), (0, -4),          # middle finger
                (-10, -4)])                          # knuckles, meeting the little finger
    t = math.radians(tilt)
    return [(palm[0] + x * math.cos(t) - y * math.sin(t),
             palm[1] + x * math.sin(t) + y * math.cos(t)) for x, y in local]


def scream(mouth_rx, mouth_ry, mouth_cy):
    """Mouth wide open, hands pressed to the cheeks in front of the outline."""
    left = _left_hand()
    right = [(100 - x, y) for x, y in left]
    s = face(hidden=[_hull(left), _hull(right)])
    open_eyes(s)
    s.ellipse((50, mouth_cy), mouth_rx, mouth_ry)
    s.poly(left)
    s.poly(right)
    return s


SHAPES = {
    0: ("neutral", neutral()),
    1: ("blink", blink()),
    2: ("smile", smiling()),
    3: ("big grin", grin()),
    4: ("small o", small_o()),
    5: ("big O", big_o()),
    8: ("wink left", wink(0)),
    9: ("wink right", wink(1)),
    10: ("tongue out", tongue(14)),
    11: ("tongue further", tongue(26)),
    12: ("sad", sad()),
    13: ("sad, tear", sad(tear=True)),
    14: ("scream", scream(6.5, 10, 57)),
    15: ("scream wider", scream(8.5, 15, 61)),
}
