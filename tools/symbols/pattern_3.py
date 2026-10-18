"""Pattern 3: 1992 machines (authored, not recovered from the original).

These are new line drawings made for the remake, not the original demo's
symbols. The set is the kit of the year the demo was made: a floppy disk, a
mouse, a joystick, a handheld console, an arcade cabinet, a digital watch, a
synthesizer, a microphone, an arrow pointer, a printer, a keyboard key, a
satellite dish, a loudspeaker, a power plug, a microchip and a VR headset.
Each is a different subject, drawn to look unlike the symbols the music
flips to before and after it.
"""
import math

from neon import Shape

TITLE = "1992 machines"


def _bez(p0, p1, p2, p3, steps=16):
    """Points along a cubic bezier."""
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


def _arcpts(c, r, a0, a1, n=None):
    n = n or max(4, int(abs(a1 - a0) / 6))
    return [(c[0] + r * math.cos(math.radians(a0 + (a1 - a0) * i / n)),
             c[1] + r * math.sin(math.radians(a0 + (a1 - a0) * i / n))) for i in range(n + 1)]


# 0: 3.5-inch floppy disk, its shutter at the top and label below.
def floppy():
    s = Shape()
    s.poly([(10, 8), (80, 8), (92, 20), (92, 92), (10, 92)], closed=True)
    s.rect((22, 58), (80, 92))                    # label
    s.rect((28, 8), (58, 42))                     # shutter over the window
    return s


# 1: two-button mouse, its cable curling away from the top.
def mouse():
    s = Shape()
    c, rx, ry = (58, 64), 24, 31
    s.ellipse(c, rx, ry)
    y = 56
    half = rx * math.sqrt(1 - ((y - c[1]) / ry) ** 2)
    s.line((c[0] - half, y), (c[0] + half, y))   # front of the two buttons
    s.line((c[0], c[1] - ry), (c[0], y))         # between the buttons
    s.poly(_path((c[0], c[1] - ry), ((58, 16), (36, 30), (30, 16)),
                 ((26, 6), (14, 6), (10, 14))))   # cable
    return s


# 2: joystick: base, rubber boot, stick and knob, fire button.
def joystick():
    s = Shape()
    s.rect((10, 70), (90, 92), r=6)              # base
    s.arc((52, 70), 14, 180, 360)                # rubber boot
    s.arc((24, 70), 9, 180, 360)                 # fire button
    s.line((52, 56), (52, 30))                   # stick
    s.circle((52, 18), 12)                       # knob
    return s


# 3: handheld game console: screen, d-pad and two buttons.
def handheld():
    s = Shape()
    x0, y0, x1, y1, r, big = 14, 4, 86, 96, 6, 18
    pts = (_arcpts((x0 + r, y0 + r), r, 180, 270) + _arcpts((x1 - r, y0 + r), r, 270, 360)
           + _arcpts((x1 - big, y1 - big), big, 0, 90) + _arcpts((x0 + r, y1 - r), r, 90, 180))
    s.poly(pts, closed=True)
    s.rect((26, 13), (74, 41), r=3)              # screen
    cx, cy, h, R = 34, 68, 4.5, 11               # d-pad
    s.poly([(cx - h, cy - R), (cx + h, cy - R), (cx + h, cy - h), (cx + R, cy - h),
            (cx + R, cy + h), (cx + h, cy + h), (cx + h, cy + R), (cx - h, cy + R),
            (cx - h, cy + h), (cx - R, cy + h), (cx - R, cy - h), (cx - h, cy - h)], closed=True)
    s.dot((62, 72))                              # B
    s.dot((74, 62))                              # A
    return s


# 4: arcade cabinet: marquee and screen, the control panel jutting out with
# a stick and two buttons, and the coin slots.
def arcade():
    s = Shape()
    x0, x1, top, bottom = 20, 80, 8, 92
    s.poly([(x0, top), (x1, top), (x1, 56), (90, 67), (90, 73), (x1, 73), (x1, bottom),
            (x0, bottom), (x0, 73), (10, 73), (10, 67), (x0, 56)], closed=True)
    s.line((x0, 18), (x1, 18))                   # marquee
    s.rect((32, 26), (68, 43), r=3)              # screen
    s.poly([(34, 65), (34, 57)] + _arcpts((34, 53), 4, 90, 420))   # stick and ball
    s.dot((52, 62))                              # buttons
    s.dot((62, 62))
    s.line((44, 80), (44, 86))                   # coin slots
    s.line((56, 80), (56, 86))
    return s


# 5: digital wristwatch showing 7:25 in seven-segment digits.
def watch():
    s = Shape()
    s.rect((8, 30), (92, 70), r=10)              # case
    s.poly([(30, 30), (34, 2), (66, 2), (70, 30)])   # strap
    s.poly([(30, 70), (34, 98), (66, 98), (70, 70)])
    t, m, b = 41, 50, 59
    s.poly([(23, t), (32, t), (32, b)])          # 7
    s.dot((41, 45))                              # :
    s.dot((41, 55))
    s.poly([(50, t), (59, t), (59, m), (50, m), (50, b), (59, b)])   # 2
    s.poly([(77, t), (68, t), (68, m), (77, m), (77, b), (68, b)])   # 5
    return s


# 6: synthesizer: knobs over a one-octave keyboard, its black keys hanging
# from the top of the keys and the white keys parted below them.
def synth():
    s = Shape()
    x0, x1, lid, top, low, bottom = 2, 98, 12, 44, 62, 88
    w = (x1 - x0) / 7
    for k in range(1, 7):
        x = x0 + w * k
        if k == 3:                               # no black key between E and F
            s.line((x, top), (x, bottom))
        else:
            s.poly([(x - 3.5, top)] + _arcpts((x, low), 3.5, 180, 0) + [(x + 3.5, top)])
            s.line((x, low + 3.5), (x, bottom))
    s.rect((x0, lid), (x1, bottom), r=5)
    s.line((x0, top), (x1, top))                 # top of the keys
    for x in (14, 26, 38, 50):
        s.dot((x, 28))                           # knobs
    s.line((64, 28), (86, 28))                   # slider
    return s


# 7: microphone on a stand: capsule in its yoke, stem and tripod.
def microphone():
    s = Shape()
    s.rect((38, 4), (62, 44), r=12)              # capsule
    s.line((38, 17), (62, 17))                   # grille bands
    s.line((38, 30), (62, 30))
    s.arc((50, 30), 24, 0, 180)                  # yoke
    s.line((50, 54), (50, 80))                   # stem
    s.poly([(24, 96), (50, 80), (76, 96)])       # tripod
    return s


# 8: arrow pointer.
def arrow():
    s = Shape()
    s.poly([(30, 10), (30, 78), (45, 65), (56, 88), (68, 82), (57, 60), (76, 60)], closed=True)
    return s


# 9: printer with a printed sheet coming out of the top.
def printer():
    s = Shape()
    s.rect((6, 50), (94, 88), r=6)               # body
    s.poly([(26, 50), (26, 10), (74, 10), (74, 50)])   # sheet
    for y, x1 in ((20, 64), (30, 64), (40, 56)):
        s.line((36, y), (x1, y))                 # printed lines
    s.line((20, 68), (66, 68))                   # paper slot
    s.dot((80, 68))                              # power light
    return s


# 10: keyboard key with a letter Z on its top face.
def key():
    s = Shape()
    s.rect((20, 8), (80, 56), r=6)               # top face
    s.poly([(20, 50), (10, 90), (90, 90), (80, 50)])  # front skirt
    s.poly([(38, 20), (62, 20), (38, 44), (62, 44)])  # the letter Z
    return s


# 11: satellite dish on a stand, its feed arm out of the bowl.
def dish():
    s = Shape()
    a, b = (29, 13), (75, 59)                    # rim
    m = ((a[0] + b[0]) / 2, (a[1] + b[1]) / 2)
    n = (-1 / math.sqrt(2), 1 / math.sqrt(2))   # towards the back of the bowl
    d = 23
    s.poly(_bez(a, (a[0] + n[0] * d, a[1] + n[1] * d), (b[0] + n[0] * d, b[1] + n[1] * d), b, 24)
           + [a])                                # bowl
    s.line(m, (m[0] - n[0] * 26, m[1] - n[1] * 26))   # feed arm
    s.dot((m[0] - n[0] * 32, m[1] - n[1] * 32))       # feed horn
    s.poly([(31, 91), (45, 57), (63, 91)])       # stand
    return s


# 12: loudspeaker.
def speaker():
    s = Shape()
    s.poly([(28, 36), (46, 36), (72, 14), (72, 86), (46, 64), (28, 64)], closed=True)
    return s


# 13: power plug with its cable, held at a slant.
def plug():
    s = Shape()
    a = math.radians(35)

    def turn(pts):
        """Drawn upright, then turned 35 degrees clockwise about (50, 50) and moved 4 right."""
        return [(54 + (x - 50) * math.cos(a) - (y - 50) * math.sin(a),
                 50 + (x - 50) * math.sin(a) + (y - 50) * math.cos(a)) for x, y in pts]

    s.poly(turn([(42, 6), (42, 28)]))            # pins
    s.poly(turn([(58, 6), (58, 28)]))
    s.poly(turn([(28, 28), (72, 28), (72, 46), (60, 60), (40, 60), (28, 46)]), closed=True)
    s.poly(turn(_path((50, 60), ((50, 78), (30, 72), (28, 88)),
                      ((27, 96), (36, 102), (46, 100)))))   # cable
    return s


# 14: DIP microchip: body, pin-1 notch and four pins a side.
def chip():
    s = Shape()
    s.rect((34, 10), (66, 90), r=3)
    s.arc((50, 10), 9, 0, 180)                   # pin-1 notch
    for y in (26, 42, 58, 74):
        s.line((34, y), (18, y))
        s.line((66, y), (82, y))
    return s


# 15: VR headset from the front: visor with a nose bridge, lenses, strap.
def vr():
    s = Shape()
    x0, y0, x1, y1, r = 10, 40, 90, 80, 8
    pts = (_arcpts((x0 + r, y0 + r), r, 180, 270) + _arcpts((x1 - r, y0 + r), r, 270, 360)
           + _arcpts((x1 - r, y1 - r), r, 0, 90) + _arcpts((50, y1), 8, 0, -180)
           + _arcpts((x0 + r, y1 - r), r, 90, 180))
    s.poly(pts, closed=True)
    s.circle((30, 58), 9)                        # lenses
    s.circle((70, 58), 9)
    s.ellipse((50, 50), 40, 32, 0, 180, 360)     # strap over the head
    return s


SHAPES = {
    0: ("floppy disk", floppy()),
    1: ("mouse", mouse()),
    2: ("joystick", joystick()),
    3: ("handheld console", handheld()),
    4: ("arcade cabinet", arcade()),
    5: ("digital watch", watch()),
    6: ("synthesizer", synth()),
    7: ("microphone", microphone()),
    8: ("arrow pointer", arrow()),
    9: ("printer", printer()),
    10: ("keyboard key", key()),
    11: ("satellite dish", dish()),
    12: ("loudspeaker", speaker()),
    13: ("power plug", plug()),
    14: ("microchip", chip()),
    15: ("VR headset", vr()),
}
