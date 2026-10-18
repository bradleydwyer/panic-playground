"""Pattern 2, Elements and alchemy: authored neon symbols, not recovered ones.

These are new line drawings for the remake, not the original demo's symbols.
Each mask is its own subject: the alchemical fire and water triangles,
lightning, a snowflake, the sun, a comet, a crescent moon, a ringed planet, a
tornado, a volcano, the Mercury symbol, an hourglass, the ouroboros and an
alembic. Pair 6 (masks 12 and 13) is never shown in this pattern, so it has
no shapes.
"""
import math

from neon import Shape

TITLE = "Elements"
NOT_SHOWN = [6]


def _pt(c, r, a):
    t = math.radians(a)
    return (c[0] + r * math.cos(t), c[1] + r * math.sin(t))


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


def _round(pts, at, d):
    """The polyline with its corner at index `at` rounded off: the points
    within about `d` of it, along the line, replaced by a curve."""
    def walk(step):
        k, gone = at, 0.0
        while 0 <= k + step < len(pts):
            seg = math.hypot(pts[k + step][0] - pts[k][0], pts[k + step][1] - pts[k][1])
            if gone + seg >= d:
                f = (d - gone) / seg
                return k + step, (pts[k][0] + (pts[k + step][0] - pts[k][0]) * f,
                                  pts[k][1] + (pts[k + step][1] - pts[k][1]) * f)
            gone += seg
            k += step
        return k, pts[k]
    i, a = walk(-1)
    j, b = walk(1)
    c = pts[at]
    curve = _bez(a, (a[0] + (c[0] - a[0]) * 2 / 3, a[1] + (c[1] - a[1]) * 2 / 3),
                 (b[0] + (c[0] - b[0]) * 2 / 3, b[1] + (c[1] - b[1]) * 2 / 3), b, steps=10)
    return pts[:i + 1] + curve + pts[j:]


# Triangles share one size: 88 wide, 76 tall, centred on the box.
UP = [(50, 12), (94, 88), (6, 88)]
DOWN = [(50, 88), (94, 12), (6, 12)]


def fire():
    return Shape().poly(UP, closed=True)


def water():
    return Shape().poly(DOWN, closed=True)


def lightning():
    # A bold bolt outline, drawn open at the middle of its top edge so the
    # sign's gap is there and not at the tip.
    pts = [(64, 6), (74, 6), (58, 42), (78, 42), (34, 96), (46, 56), (26, 56), (52, 6), (58, 6)]
    return Shape().poly(pts)


def snowflake():
    # Six arms that stop short of the centre, each with a pair of branches
    # drawn to the arm as separate strokes, so the arm stays whole and the
    # neon rules leave each branch an air gap where it meets it.
    s = Shape()
    c = (50, 50)
    for k in range(6):
        a = 90 + 60 * k
        s.line(_pt(c, 10, a), _pt(c, 46, a))
        for side, r0 in ((-1, 25.2), (1, 26.8)):
            base = _pt(c, r0, a)
            s.line(base, _pt(base, 21, a + side * 45))
    return s


SUN_C, SUN_R = (50, 50), 20


def sun():
    s = Shape().circle(SUN_C, SUN_R)
    for k in range(8):
        a = k * 45
        s.line(_pt(SUN_C, 31, a), _pt(SUN_C, 39, a))
    return s


def comet():
    # A round head inside the glow's leading edge, which runs back as the
    # two sides of a widening tail (one tube), and a streak down the middle.
    c = (68, 29)
    head = -45                      # the way it flies: up and to the right
    R = 21                          # the glow's leading edge, about the head
    spread = 6                      # degrees each side the tail widens
    pts = []
    for side, length in ((-1, 58), (1, 66)):
        edge = _pt(c, R, head + side * 90)
        a = math.radians(head + 180 - side * spread)
        tip = (edge[0] + length * math.cos(a), edge[1] + length * math.sin(a))
        arm = [(tip[0] + (edge[0] - tip[0]) * i / 12, tip[1] + (edge[1] - tip[1]) * i / 12)
               for i in range(13)]
        if side == -1:
            pts += arm + [_pt(c, R, head - 90 + 180 * i / 45) for i in range(1, 45)]
        else:
            pts += list(reversed(arm))
    s = Shape().poly(pts)
    s.circle(c, 10)
    s.line(_pt(c, R, head + 180), _pt(c, 70, head + 180))
    return s


def crescent(d, r2, cx=0):
    # Outer arc of a circle R about (50+cx, 50), cut by a second circle r2
    # whose centre is d to the right; the tips are where the two meet.
    R = 40
    c1 = (50 + cx, 50)
    c2 = (50 + cx + d, 50)
    x = (d * d + R * R - r2 * r2) / (2 * d)
    y = math.sqrt(R * R - x * x)
    a1 = math.degrees(math.atan2(y, x))
    a2 = math.degrees(math.atan2(y, x - d))
    s = Shape()
    s.arc(c1, R, a1, 360 - a1)
    s.arc(c2, r2, a2, 360 - a2)
    return s


def ringed_planet():
    # A ball and a tilted ring. The ring's far side is left out where the
    # ball hides it, and the ball is drawn in two arcs that stop at the
    # ring's near side; the neon rules leave air gaps at all four, so the
    # ring passes behind the ball and then in front of it.
    c, r = (50, 50), 26
    rx, ry, rot = 47, 13, -18
    t = math.radians(rot)
    ring, crossings = [], []
    inside = None
    for i in range(361):
        a = math.radians(i - 90)  # from the far side's middle round the front
        x, y = rx * math.cos(a), ry * math.sin(a)
        p = (c[0] + x * math.cos(t) - y * math.sin(t), c[1] + x * math.sin(t) + y * math.cos(t))
        now = math.hypot(p[0] - c[0], p[1] - c[1]) < r
        if y > 0 and inside is not None and now != inside:
            crossings.append(math.degrees(math.atan2(p[1] - c[1], p[0] - c[0])))
        inside = now
        if not (y < 0 and now):
            ring.append(p)
    s = Shape().poly(ring)
    lo, hi = sorted(crossings)  # the near side's crossings, below the middle
    s.arc(c, r, lo + 2, hi - 2)
    s.arc(c, r, hi + 2, lo + 358)
    return s


def tornado():
    # One tube coiling down a funnel that narrows and sways to its tip: each
    # turn sweeps across the front, then doubles back round the edge.
    layers = [(16, 44, 50), (30, 34, 56), (44, 25, 57), (58, 17, 50), (72, 11, 44), (86, 6, 47)]
    pts = []
    for i, (y, half, x) in enumerate(layers):
        sweep = [(x - half + 2 * half * k / 30, y - 4 + 5 * math.sin(math.pi * k / 30))
                 for k in range(31)]
        if i % 2:
            sweep.reverse()
        if pts:
            a, b = pts[-1], sweep[0]
            out = 1 if i % 2 else -1
            pts += _bez(a, (a[0] + out * 7, a[1] + 3), (b[0] + out * 7, b[1] - 3), b, steps=12)[1:-1]
        pts += sweep
    return Shape().poly(pts)


def volcano():
    s = Shape()
    # The cone, open at the crater, and lava thrown up out of it.
    s.poly([(4, 94), (34, 50), (40, 54), (46, 52)])
    s.poly([(54, 52), (60, 54), (66, 50), (96, 94)])
    s.line((50, 40), (50, 14))
    s.poly(_path((42, 42), ((38, 32), (32, 24), (24, 20))))
    s.poly(_path((58, 42), ((62, 32), (68, 24), (76, 20))))
    s.dot((14, 32))
    s.dot((86, 32))
    return s


def mercury():
    s = Shape()
    c, r = (50, 42), 15
    s.circle(c, r)
    s.line((50, c[1] + r), (50, 94))
    s.line((36, 78), (64, 78))
    s.arc((50, 6), 13, 15, 165)
    return s


def hourglass():
    s = Shape()
    s.line((22, 7), (78, 7))
    s.line((22, 93), (78, 93))
    for side in (-1, 1):
        x = lambda v: 50 + side * v
        s.poly(_path((x(22), 15), ((x(25), 34), (x(6), 40), (x(5), 50)),
                     ((x(6), 60), (x(25), 66), (x(22), 85))))
    # Sand: a heap in the lower bulb and two grains falling to it.
    s.poly(_path((37, 85), ((42, 81), (46, 74), (50, 74)), ((54, 74), (58, 81), (63, 85))))
    s.dot((50, 59))
    s.dot((50, 67))
    return s


def _half_width(a):
    """Half the snake's width at angle a, the tail tip at 285 degrees."""
    a = a - 285
    if a < 70:                       # tail, tapering to the tip
        return 6.5 * math.sin(math.radians(90 * a / 70))
    if a < 315:                      # body
        return 6.5
    if a < 333:                      # back of the head, swelling
        return 6.5 + 6.5 * math.sin(math.radians(90 * (a - 315) / 18))
    return 13 - 2.5 * (a - 333) / 34  # jaws, reaching past the tail tip


def ouroboros():
    c, R = (50, 50), 35
    s = Shape()
    # Head at the top facing clockwise, the tail tip inside its open mouth.
    # Outline the body: outer and inner edges from the tail tip to the jaws.
    for side in (1, -1):
        s.poly([_pt(c, R + side * _half_width(a), a) for a in range(285, 285 + 368, 2)])
    s.dot(_pt(c, R + 2, 255))  # eye
    return s


def alembic():
    # A round-bottomed still whose neck arches over to a spout at the right,
    # drawn as one tube: in along the neck's top wall, round the bulb and out
    # along its lower wall. Liquid and two bubbles in the bulb.
    c, r = (30, 62), 28
    centre = _path((46, 42), ((54, 28), (62, 20), (70, 20)), ((80, 20), (88, 36), (95, 66)))
    walls = []
    for side in (-1, 1):
        wall = []
        for i, p in enumerate(centre):
            q = centre[min(i + 1, len(centre) - 1)]
            o = centre[max(i - 1, 0)]
            dx, dy = q[0] - o[0], q[1] - o[1]
            l = math.hypot(dx, dy)
            w = 6.0 - 1.5 * i / (len(centre) - 1)  # half the neck, tapering to the spout
            wall.append((p[0] - side * dy / l * w, p[1] + side * dx / l * w))
        walls.append([p for p in wall if math.hypot(p[0] - c[0], p[1] - c[1]) >= r])
    outer, inner = walls  # outer: above the centreline
    a0 = math.degrees(math.atan2(outer[0][1] - c[1], outer[0][0] - c[0]))
    a1 = math.degrees(math.atan2(inner[0][1] - c[1], inner[0][0] - c[0]))
    if a1 > a0:
        a1 -= 360
    bulb = [_pt(c, r, a0 + (a1 - a0) * i / 90) for i in range(91)]
    run = list(reversed(outer)) + bulb + inner
    run = _round(run, len(outer) - 1, 7)            # where the neck leaves the bulb
    run = _round(run, len(run) - len(inner), 4)
    s = Shape().poly(run)
    s.poly([(c[0] - r + 9 + (2 * r - 18) * i / 24, 68 + 2.5 * math.sin(2 * math.pi * i / 12))
            for i in range(25)])
    s.dot((c[0] - 9, c[1] + 19))
    s.dot((c[0] + 4, c[1] + 15))
    return s


SHAPES = {
    0: ("fire", fire()),
    1: ("lightning", lightning()),
    2: ("water", water()),
    3: ("snowflake", snowflake()),
    4: ("sun", sun()),
    5: ("comet", comet()),
    6: ("crescent moon", crescent(14, 40, cx=16)),
    7: ("ringed planet", ringed_planet()),
    8: ("tornado", tornado()),
    9: ("volcano", volcano()),
    10: ("mercury", mercury()),
    11: ("hourglass", hourglass()),
    14: ("ouroboros", ouroboros()),
    15: ("alembic", alembic()),
}
