"""Pattern 1: Water (authored shapes, not recovered from the original).

Pairs are two-frame flips: mask 2k, then 2k+1 with one clear change.
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


def _transform(shape, fn):
    out = Shape()
    for run in shape.runs:
        out.poly([fn(x, y) for x, y in run])
    return out


def _shift(shape, dx, dy):
    return _transform(shape, lambda x, y: (x + dx, y + dy))


def _mirror(shape):
    return _transform(shape, lambda x, y: (100 - x, y))


def _rotate(shape, pivot, deg):
    a = math.radians(deg)
    ca, sa = math.cos(a), math.sin(a)
    px, py = pivot
    return _transform(shape, lambda x, y: (px + (x - px) * ca - (y - py) * sa,
                                           py + (x - px) * sa + (y - py) * ca))


def _teardrop(tip, c, r):
    """Closed teardrop: tangent lines from the tip to a circle."""
    dx, dy = tip[0] - c[0], tip[1] - c[1]
    d = math.hypot(dx, dy)
    base = math.degrees(math.atan2(dy, dx))
    phi = math.degrees(math.acos(r / d))
    a0, a1 = base + phi, base - phi + 360
    n = 40
    arc = [(c[0] + r * math.cos(math.radians(a0 + (a1 - a0) * i / n)),
            c[1] + r * math.sin(math.radians(a0 + (a1 - a0) * i / n))) for i in range(n + 1)]
    return [tip] + arc + [tip]


# 0/1: a falling drop above the water -> its crown splash on the same water.
WATER_Y = 88


def drop():
    s = Shape()
    s.poly(_teardrop((50, 8), (50, 52), 20))
    s.line((10, WATER_Y), (90, WATER_Y))
    return s


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


# 2/3: ripples seen in perspective (flat ellipses), one ring -> three.
RIP_C = (50, 50)


def ripple_one():
    s = Shape()
    s.ellipse(RIP_C, 31, 19)
    return s


def ripple_three():
    s = Shape()
    s.ellipse(RIP_C, 15, 7.5)
    s.ellipse(RIP_C, 31, 19)
    s.ellipse(RIP_C, 46, 30.5)
    return s


# 4/5: a crest leaning forward -> the lip pitched over into a curl, with
# spray thrown off and falling from it.
def crest():
    s = Shape()
    s.poly(_path((6, 86), ((30, 86), (36, 26), (74, 16)),
                 ((60, 30), (62, 64), (94, 86))))
    return s


def breaking():
    s = Shape()
    s.poly(_path((6, 86), ((28, 86), (32, 18), (58, 14)),
                 ((80, 10), (92, 30), (84, 46)),
                 ((78, 58), (62, 54), (62, 42)),
                 ((62, 34), (70, 32), (72, 38))))
    for p in [(93, 8), (99, 21), (92, 62), (86, 78)]:
        s.dot(p)
    return s


# 6/7: a fish facing right -> the same fish facing left.
def fish():
    s = Shape()
    s.poly(_path((90, 50), ((80, 26), (50, 22), (32, 44)),
                 ((24, 36), (16, 30), (10, 26)),
                 ((14, 40), (14, 60), (10, 74)),
                 ((16, 70), (24, 64), (32, 56)),
                 ((50, 78), (80, 74), (90, 50))))
    s.dot((72, 45))
    return s


# 8/9: one wave line -> a double wave line.
def _wave(y, amp=10, x0=10, x1=90, periods=2):
    n = 80
    return [(x0 + (x1 - x0) * i / n,
             y - amp * math.sin(2 * math.pi * periods * i / n)) for i in range(n + 1)]


def wave_one():
    s = Shape()
    s.poly(_wave(50))
    return s


def wave_two():
    s = Shape()
    s.poly(_wave(38))
    s.poly(_wave(62))
    return s


# 10/11: a small bubble -> a bigger bubble with a highlight.
BUB_C = (50, 50)


def bubble_small():
    s = Shape()
    s.circle(BUB_C, 20)
    return s


def bubble_big():
    s = Shape()
    s.circle(BUB_C, 40)
    s.arc(BUB_C, 27, 195, 250)
    return s


# 12/13: a whirlpool spiral -> the same spiral turned half a turn.
SP_C = (50, 50)


def whirl(start):
    s = Shape()
    s.spiral(SP_C, 4, 42, turns=2.5, start=start)
    return s


# 14/15: an anchor hanging on its rope -> swung to one side.
def anchor():
    s = Shape()
    s.line((50, 0), (50, 13))
    s.circle((50, 19), 6)
    s.line((50, 25), (50, 88))
    s.line((34, 36), (66, 36))
    s.arc((50, 60), 28, 15, 165)
    return s


SHAPES = {
    0: ("drop", drop()),
    1: ("splash", splash()),
    2: ("ripple", ripple_one()),
    3: ("three ripples", ripple_three()),
    4: ("wave crest", _shift(crest(), -3, 0)),
    5: ("wave breaking", _shift(breaking(), -3, 0)),
    6: ("fish right", fish()),
    7: ("fish left", _mirror(fish())),
    8: ("wave line", wave_one()),
    9: ("double wave", wave_two()),
    10: ("small bubble", bubble_small()),
    11: ("big bubble", bubble_big()),
    12: ("whirlpool", whirl(0)),
    13: ("whirlpool turned", whirl(180)),
    14: ("anchor", anchor()),
    15: ("anchor swung", _rotate(anchor(), (50, 0), -16)),
}
