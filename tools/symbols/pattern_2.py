"""Pattern 2, Elements and alchemy: authored neon symbols, not recovered ones.

Each pair is a two-frame flip, B being A with one change. Pair 6 (masks 12
and 13) is never shown in this pattern, so it has no shapes.
"""
import math

from neon import Shape

TITLE = "Elements"
NOT_SHOWN = [6]


def _pt(c, r, a):
    t = math.radians(a)
    return (c[0] + r * math.cos(t), c[1] + r * math.sin(t))


# Triangles share one size: 88 wide, 76 tall, centred on the box.
UP = [(50, 12), (94, 88), (6, 88)]
DOWN = [(50, 88), (94, 12), (6, 12)]


def fire():
    return Shape().poly(UP, closed=True)


def _spline(pts, closed=True, steps=8):
    """Catmull-Rom curve through the points, as a polyline."""
    p = list(pts)
    n = len(p)
    out = []
    segs = n if closed else n - 1
    for i in range(segs):
        p0 = p[(i - 1) % n] if closed or i > 0 else p[0]
        p1, p2 = p[i], p[(i + 1) % n]
        p3 = p[(i + 2) % n] if closed or i + 2 < n else p[-1]
        for k in range(steps):
            t = k / steps
            out.append(tuple(0.5 * (2 * p1[j] + (-p0[j] + p2[j]) * t
                                    + (2 * p0[j] - 5 * p1[j] + 4 * p2[j] - p3[j]) * t * t
                                    + (-p0[j] + 3 * p1[j] - 3 * p2[j] + p3[j]) * t ** 3)
                             for j in range(2)))
    out.append(p[0] if closed else p[-1])
    return out


def fire_flame():
    s = fire()
    # A flame: a round belly, a tall middle tongue and a short one each side.
    s.poly(_spline([(50, 78), (38, 76), (33, 67), (35, 57), (37, 62), (42, 58), (45, 46),
                    (50, 34), (55, 46), (58, 58), (63, 62), (65, 57), (67, 67), (62, 76)]))
    return s


def water():
    return Shape().poly(DOWN, closed=True)


def water_wave():
    s = water()
    # One full sine period across the wide top of the triangle.
    s.poly([(29 + 42 * i / 40, 34 - 8 * math.sin(2 * math.pi * i / 40)) for i in range(41)])
    return s


SUN_C, SUN_R = (50, 50), 20


def sun(r_out):
    s = Shape().circle(SUN_C, SUN_R)
    for k in range(8):
        a = k * 45
        s.line(_pt(SUN_C, 31, a), _pt(SUN_C, r_out, a))
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


def air_earth(tri):
    s = Shape().poly(tri, closed=True)
    s.line((18, 50), (82, 50))
    return s


def venus(horns=False):
    s = Shape()
    c, r = (50, 42), 15
    s.circle(c, r)
    s.line((50, c[1] + r), (50, 94))
    s.line((36, 78), (64, 78))
    if horns:
        s.arc((50, 6), 13, 15, 165)
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


def ouroboros(turn=0):
    c, R = (50, 50), 35
    s = Shape()
    # Head at the top facing clockwise, the tail tip inside its open mouth.
    # Outline the body: outer and inner edges from the tail tip to the jaws.
    for side in (1, -1):
        s.poly([_pt(c, R + side * _half_width(a), a + turn) for a in range(285, 285 + 368, 2)])
    s.dot(_pt(c, R + 2, 255 + turn))  # eye
    return s


SHAPES = {
    0: ("fire", fire()),
    1: ("fire, flame", fire_flame()),
    2: ("water", water()),
    3: ("water, wave", water_wave()),
    4: ("sun", sun(39)),
    5: ("sun, long rays", sun(48)),
    6: ("thin moon", crescent(14, 40, cx=16)),
    7: ("fuller moon", crescent(30, 36, cx=16)),
    8: ("air", air_earth(UP)),
    9: ("earth", air_earth(DOWN)),
    10: ("mercury", venus(horns=True)),
    11: ("venus", venus()),
    14: ("ouroboros", ouroboros()),
    15: ("ouroboros, turned", ouroboros(90)),
}
