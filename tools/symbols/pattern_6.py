"""Pattern 6, "Social: then -> now": a 1990s way of doing something (A), then the
social platform that does it now (B), drawn as that platform's logo.

Authored line art for the remake, not recovered from the original demo. The
logos are neon tracings of each brand's mark, simplified for the 7.5-unit tube
(gaps widened where the real mark's are narrower than a tube). Each B sits in
the same place and at about the same size as its A, so the back-to-back flash
reads as one thing becoming the other.
"""
import math

from neon import Shape

TITLE = "Social: then -> now"


def squircle(s, c, a, b, n=4.0, steps=96):
    """A superellipse |x/a|^n + |y/b|^n = 1: a box with bulging sides and
    rounded corners (a CRT screen, the YouTube play button)."""
    pts = []
    for i in range(steps + 1):
        t = 2 * math.pi * i / steps
        ct, st = math.cos(t), math.sin(t)
        pts.append((c[0] + a * math.copysign(abs(ct) ** (2 / n), ct),
                    c[1] + b * math.copysign(abs(st) ** (2 / n), st)))
    return s.poly(pts)


def tilted_rect(s, c, w, h, deg):
    """A plain rectangle `w` by `h` about `c`, turned `deg` degrees."""
    t = math.radians(deg)
    pts = []
    for x, y in [(-w / 2, -h / 2), (w / 2, -h / 2), (w / 2, h / 2), (-w / 2, h / 2)]:
        pts.append((c[0] + x * math.cos(t) - y * math.sin(t), c[1] + x * math.sin(t) + y * math.cos(t)))
    return s.poly(pts, closed=True)


def arc_pts(c, r, a0, a1, n=None):
    n = n or max(8, int(abs(a1 - a0) / 4))
    return [(c[0] + r * math.cos(math.radians(a0 + (a1 - a0) * i / n)),
             c[1] + r * math.sin(math.radians(a0 + (a1 - a0) * i / n))) for i in range(n + 1)]


def bez_pts(p0, p1, p2, p3, n=20):
    pts = []
    for i in range(n + 1):
        t = i / n
        u = 1 - t
        pts.append((u**3 * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t**3 * p3[0],
                    u**3 * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t**3 * p3[1]))
    return pts


def ear(s, c, r, head_c, head_rx, head_ry):
    """The part of a circle that shows outside an ellipse (an ear behind a head)."""
    pts = []
    for k in range(361):
        a = math.radians(k)
        p = (c[0] + r * math.cos(a), c[1] + r * math.sin(a))
        out = ((p[0] - head_c[0]) / head_rx) ** 2 + ((p[1] - head_c[1]) / head_ry) ** 2 > 1
        pts.append((p, out))
    # Rotate the list so it starts inside the head, then keep the outside run.
    start = next(i for i, (_, o) in enumerate(pts) if not o)
    pts = pts[start:] + pts[:start]
    run = [p for p, o in pts if o]
    return s.poly(run)


def _meet(p, d, q, e):
    """Where the line p + t d meets the line q + u e."""
    den = d[0] * e[1] - d[1] * e[0]
    t = ((q[0] - p[0]) * e[1] - (q[1] - p[1]) * e[0]) / den
    return (p[0] + t * d[0], p[1] + t * d[1])


# 0: a 1990s TV set: a deep box drawn in depth, a curved screen, two knobs and
# rabbit-ear antennas.
tv = Shape()
tv.poly([(6, 36), (76, 36), (76, 92), (6, 92)], closed=True)
tv.poly([(6, 36), (22, 22), (92, 22), (92, 78), (76, 92)])
tv.line((76, 36), (92, 22))
squircle(tv, (35, 64), 19, 18)
tv.dot((65, 54))
tv.dot((65, 72))
tv.poly([(36, 4), (56, 22), (78, 4)])

# 1: YouTube: the bulging rounded rectangle with the play triangle.
yt = Shape()
squircle(yt, (50, 52), 44, 31, n=5)
yt.poly([(41, 38), (41, 66), (65, 52)], closed=True)

# 2: Polaroid instant camera: body, big lens, viewfinder and flash, and a print
# coming out of the slot, its picture above the wide bottom margin (hero).
pol = Shape()
pol.rect((6, 8), (94, 60), r=9)
pol.circle((50, 34), 15)
pol.circle((50, 34), 4)
pol.dot((19, 20))
pol.line((72, 20), (83, 20))
pol.poly([(24, 60), (24, 95), (76, 95), (76, 60)])
pol.poly([(35, 60), (35, 81), (65, 81), (65, 60)])

# 3: Instagram: rounded square, lens circle, dot top right (hero).
ig = Shape()
ig.rect((8, 8), (92, 92), r=22)
ig.circle((50, 50), 19.5)
ig.dot((73, 27))

# 4: folded newspaper: front page with a headline, a picture and columns, the
# inner pages folded behind on the left (hero).
news = Shape()
news.rect((24, 8), (90, 92), r=2)
news.poly([(24, 20), (11, 20), (11, 84), (16, 91), (24, 92)])
news.line((34, 20), (80, 20))
news.rect((34, 33), (54, 55), r=1)
news.line((64, 36), (80, 36))
news.line((64, 52), (80, 52))
news.line((34, 68), (80, 68))
news.line((34, 80), (80, 80))

# 5: X: the heavy top-left to bottom-right stroke as an outline (two parallel
# sides with flat ends), the thin stroke as two arms meeting it (hero).
# Traced from the mark's proportions in a 24-unit box, then scaled.
def _x(p):
    return (50 + (p[0] - 12.04) * 3.95, 50 + (p[1] - 12.0) * 3.95)


x_logo = Shape()
band = [(1.25, 2.25), (8.08, 2.25), (22.83, 21.75), (16.17, 21.75)]
x_logo.poly([_x(p) for p in band], closed=True)
arm_d = (-7.23, 8.26)
up = _meet((19.9, 2.25), arm_d, band[1], (band[2][0] - band[1][0], band[2][1] - band[1][1]))
low = _meet((3.33, 21.75), arm_d, band[0], (band[3][0] - band[0][0], band[3][1] - band[0][1]))
x_logo.line(_x((19.9, 2.25)), _x(up))
x_logo.line(_x((3.33, 21.75)), _x(low))

# 6: corded desk telephone: handset on the cradle, body, rotary dial.
tel = Shape()
tel.poly(bez_pts((8, 36), (12, 12), (88, 12), (92, 36))
         + [(92, 42), (74, 42), (72, 32), (28, 32), (26, 42), (8, 42), (8, 36)])
tel.poly([(30, 52), (70, 52), (90, 92), (10, 92)], closed=True)
tel.circle((50, 72), 9.5)
tel.dot((50, 72))

# 7: WhatsApp: round speech bubble with its tail at the lower left, and a
# handset inside, earpiece top left and mouthpiece bottom right.
wa = Shape()
WC, WR = (53, 47), 40
bubble = arc_pts(WC, WR, 150, 360 + 112)  # the circle, open at the lower left
wa.poly(bubble + [(10, 91), bubble[0]])
wa.poly([(45, 29)] + arc_pts((64, 33), 28, 184, 86) + [(70, 52)])

# 8: photo album: an open book with a mounted photo on each page.
alb = Shape()
alb.poly([(50, 22), (50, 88), (8, 82), (8, 16), (50, 22), (92, 16), (92, 82), (50, 88)])
alb.rect((18, 31), (40, 60), r=1)
alb.poly([(18, 60), (29, 44), (40, 60)])
alb.rect((60, 31), (82, 60), r=1)
alb.dot((71, 45))

# 9: Facebook: the circle and the lowercase "f", its stem running down to the
# circle's edge.
fb = Shape()
fb.circle((50, 50), 41)
fb.poly([(47.3, 91), (47.3, 46)] + bez_pts((47.3, 46), (47.3, 34), (54, 30), (68.5, 30)))
fb.line((32, 56.5), (67.5, 56.5))

# 10: noticeboard: a framed board with three notes pinned to it.
board = Shape()
board.rect((5, 8), (95, 92), r=3)
tilted_rect(board, (29, 33), 26, 26, -6)
board.dot((29, 30.5))
tilted_rect(board, (69, 32.5), 28, 26, 5)
board.dot((69, 30))
tilted_rect(board, (45, 69), 34, 24, 3)
board.dot((45, 67))

# 11: Reddit: the alien's oval head, antenna and ball, ears, eyes and smile.
HC, HRX, HRY = (50, 60), 36, 26
red = Shape()
red.ellipse(HC, HRX, HRY)
red.poly([(50, 34), (55, 10), (70, 15)])
red.circle((77, 17), 7)
ear(red, (17, 42), 9, HC, HRX, HRY)
ear(red, (83, 42), 9, HC, HRX, HRY)
red.dot((37, 56))
red.dot((63, 56))
red.ellipse((50, 64), 16, 10, a0=20, a1=160)

# 12: business card: a logo mark, a name line and contact lines.
card = Shape()
card.rect((3, 19), (97, 81), r=4)
card.circle((21, 38), 6.5)
card.line((38, 36), (83, 36))
card.line((38, 50), (68, 50))
card.line((17, 66), (55, 66))

# 13: LinkedIn: "in" in a rounded square.
li = Shape()
li.rect((8, 8), (92, 92), r=9)
li.dot((26.7, 26.8))
li.line((26.7, 40), (26.7, 79.6))
li.poly([(46.7, 79.6), (46.7, 54)] + arc_pts((60, 54), 13.3, 180, 360)[1:] + [(73.3, 79.6)])

# 14: boombox: body, carry handle, two speakers and a tape door between them.
boom = Shape()
boom.rect((3, 38), (97, 90), r=6)
boom.poly([(22, 38), (26, 16), (74, 16), (78, 38)])
for x in (23, 77):
    boom.circle((x, 66), 10)
    boom.dot((x, 66))
boom.rect((43, 56), (57, 76), r=1)

# 15: TikTok: the note as one outline: stem, flag curling right, and the
# round head open at the top. Proportions follow the mark in a 24-unit box.
tt = Shape()
CX, CY, RO, RI = 39.6, 65.6, 28.0, 11.9
STEM_L, STEM_R, TOP = CX + RI, CX + RO, 6.4
CUT = 40.0
a_out = math.degrees(math.atan2(-math.sqrt(RO * RO - (CUT - CX) ** 2), CUT - CX)) % 360
a_in = math.degrees(math.atan2(-math.sqrt(RI * RI - (CUT - CX) ** 2), CUT - CX)) % 360
tt.poly([(STEM_L, TOP), (STEM_R, TOP)]
        + bez_pts((STEM_R, TOP), (68.5, 17), (78, 26), (88.4, 27.3))[1:]
        + [(88.4, 41.4)]
        + bez_pts((88.4, 41.4), (81.5, 41), (72, 38), (STEM_R, 34.7))[1:]
        + arc_pts((CX, CY), RO, 0, a_out)
        + arc_pts((CX, CY), RI, a_in, 0)
        + [(STEM_L, TOP)])

SHAPES = {
    0: ("1990s TV", tv),
    1: ("YouTube", yt),
    2: ("Polaroid camera", pol),
    3: ("Instagram", ig),
    4: ("newspaper", news),
    5: ("X", x_logo),
    6: ("corded phone", tel),
    7: ("WhatsApp", wa),
    8: ("photo album", alb),
    9: ("Facebook", fb),
    10: ("noticeboard", board),
    11: ("Reddit", red),
    12: ("business card", card),
    13: ("LinkedIn", li),
    14: ("boombox", boom),
    15: ("TikTok", tt),
}
