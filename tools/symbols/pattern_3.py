"""Pattern 3: 1992 machines (authored, not recovered from the original).

The demoscene's kit at the time the demo was made, as neon pairs. Each pair
is a two-frame flip: B is A with one change.
"""
import math

from neon import Shape

TITLE = "1992 machines"


def _bez(p0, p1, p2, p3, steps=12):
    """Points along a cubic bezier, for joining curved sides into one poly."""
    pts = []
    for i in range(steps + 1):
        t = i / steps
        u = 1 - t
        pts.append((u**3 * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t**3 * p3[0],
                    u**3 * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t**3 * p3[1]))
    return pts


def _arcpts(c, r, a0, a1, n=None):
    n = n or max(4, int(abs(a1 - a0) / 6))
    return [(c[0] + r * math.cos(math.radians(a0 + (a1 - a0) * i / n)),
             c[1] + r * math.sin(math.radians(a0 + (a1 - a0) * i / n))) for i in range(n + 1)]


# 0/1: 3.5-inch floppy disk; the metal shutter slides open.
def floppy(open_):
    s = Shape()
    s.poly([(10, 8), (80, 8), (92, 20), (92, 92), (10, 92)], closed=True)
    s.rect((22, 58), (80, 92))                    # label
    if not open_:
        s.rect((28, 8), (58, 42))                 # shutter over the window
    else:
        s.rect((48, 8), (78, 42))                 # shutter slid aside
        s.rect((31, 17), (38, 33), r=3.5)         # the exposed window
    return s


# 2/3: joystick, upright then tilted about the same pivot.
def joystick(tilt):
    s = Shape()
    s.rect((10, 70), (90, 92), r=6)              # base
    s.arc((52, 70), 14, 180, 360)                # rubber boot
    s.arc((22, 70), 6, 180, 360)                 # fire button
    a = math.radians(tilt)
    d = (math.sin(a), -math.cos(a))
    p = (52, 70)
    s.line((p[0] + d[0] * 14, p[1] + d[1] * 14), (p[0] + d[0] * 40, p[1] + d[1] * 40))
    s.circle((p[0] + d[0] * 52, p[1] + d[1] * 52), 12)   # knob
    return s


# 4/5: CRT monitor, a deep box with a bulging screen; then a cursor on it.
def monitor(cursor):
    s = Shape()
    s.rect((8, 10), (74, 70), r=6)               # front bezel
    # screen with gently bulging sides
    x0, y0, x1, y1, b = 20, 22, 62, 58, 2.5
    pts = (_bez((x0, y0), (x0 + 15, y0 - b), (x1 - 15, y0 - b), (x1, y0))
           + _bez((x1, y0), (x1 + b, y0 + 12), (x1 + b, y1 - 12), (x1, y1))[1:]
           + _bez((x1, y1), (x1 - 15, y1 + b), (x0 + 15, y1 + b), (x0, y1))[1:]
           + _bez((x0, y1), (x0 - b, y1 - 12), (x0 - b, y0 + 12), (x0, y0))[1:])
    s.poly(pts)
    s.poly([(74, 16), (92, 26), (92, 56), (74, 64)])   # depth of the tube
    s.poly([(30, 70), (24, 86), (66, 86), (60, 70)])   # foot
    if cursor:
        s.line((29, 47), (42, 47))                # DOS-style underscore cursor
    return s


# 6/7: audio cassette; the reels turn (three spokes, a sixth of a turn).
def cassette(turn):
    s = Shape()
    s.rect((6, 16), (94, 84), r=7)
    s.poly([(22, 84), (28, 70), (72, 70), (78, 84)])
    for cx in (30, 70):
        c = (cx, 44)
        s.circle(c, 14)
        run = []
        for k in range(3):
            a = math.radians(90 + turn + 120 * k)
            run += [c, (c[0] + 14 * math.cos(a), c[1] + 14 * math.sin(a))]
        run.append(c)
        s.poly(run)
    return s


# 8/9: arrow pointer, then a pointing hand.
def arrow():
    s = Shape()
    s.poly([(30, 10), (30, 78), (45, 65), (56, 88), (68, 82), (57, 60), (76, 60)], closed=True)
    return s


def hand():
    s = Shape()
    pts = [(36, 56), (36, 18)]
    pts += _arcpts((43, 18), 7, 180, 360)[1:]
    pts += [(50, 46)]
    pts += _arcpts((56, 46), 6, 180, 360)[1:]
    pts += _arcpts((68, 50), 6, 180, 360)[1:]
    pts += _arcpts((79, 55), 5, 180, 360)[1:]
    pts += [(84, 72), (76, 88), (46, 88), (24, 68)]
    pts += _arcpts((27, 61), 7, 135, 300)[1:]
    pts += [(36, 56)]
    s.poly(pts)
    return s


# 10/11: keyboard key with a letter; then pressed down into the deck.
def key(pressed):
    s = Shape()
    dy = 16 if pressed else 0
    s.line((4, 88), (96, 88))                    # keyboard deck
    s.rect((20, 10 + dy), (80, 56 + dy), r=6)    # top face
    s.poly([(20, 50 + dy), (12, 88), (88, 88), (80, 50 + dy)])  # front skirt
    # the letter A
    s.poly([(40, 45 + dy), (50, 22 + dy), (60, 45 + dy)])
    s.line((44, 37 + dy), (56, 37 + dy))
    return s


# 12/13: loudspeaker; then with sound waves.
def speaker(waves):
    s = Shape()
    s.poly([(14, 36), (32, 36), (58, 14), (58, 86), (32, 64), (14, 64)], closed=True)
    if waves:
        s.arc((58, 50), 17, -48, 48)
        s.arc((58, 50), 32, -50, 50)
    return s


# 14/15: DIP microchip; then a trace lit from one pin to a via.
def chip(trace):
    s = Shape()
    s.rect((28, 12), (60, 88), r=3)
    s.arc((44, 12), 6, 0, 180)                   # pin-1 notch
    for x0, x1 in ((28, 16), (60, 72)):
        run = []
        for y in (26, 42, 58, 74):
            run += [(x0, y), (x1, y), (x0, y)]
        s.poly(run)
    if trace:
        s.poly([(72, 58), (80, 58), (88, 66), (88, 76)])
        s.circle((88, 82), 6)                      # via
    return s


SHAPES = {
    0: ("floppy disk", floppy(False)),
    1: ("shutter open", floppy(True)),
    2: ("joystick", joystick(0)),
    3: ("joystick tilted", joystick(28)),
    4: ("CRT monitor", monitor(False)),
    5: ("monitor + cursor", monitor(True)),
    6: ("cassette", cassette(0)),
    7: ("reels turned", cassette(60)),
    8: ("arrow pointer", arrow()),
    9: ("hand pointer", hand()),
    10: ("key", key(False)),
    11: ("key pressed", key(True)),
    12: ("loudspeaker", speaker(False)),
    13: ("speaker + waves", speaker(True)),
    14: ("microchip", chip(False)),
    15: ("chip + lit trace", chip(True)),
}
