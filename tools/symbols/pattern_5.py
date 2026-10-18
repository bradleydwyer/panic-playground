"""Pattern 5, "Then -> now": each pair is a 1990s thing (A) and what replaced it (B).

Authored line art for the remake, not recovered from the original demo. Each
B is drawn in the same place and at about the same size as its A, so the
back-to-back flash reads as one thing becoming the other.
"""
import math

from neon import Shape

TITLE = "Then -> now"


def _circle_hits(c0, r0, c1, r1):
    """Both intersection points of two circles, ordered by x."""
    dx, dy = c1[0] - c0[0], c1[1] - c0[1]
    d = math.hypot(dx, dy)
    a = (r0 * r0 - r1 * r1 + d * d) / (2 * d)
    h = math.sqrt(max(0.0, r0 * r0 - a * a))
    mx, my = c0[0] + a * dx / d, c0[1] + a * dy / d
    p = (mx + h * dy / d, my - h * dx / d)
    q = (mx - h * dy / d, my + h * dx / d)
    return sorted([p, q])


def _angle(c, p):
    return math.degrees(math.atan2(p[1] - c[1], p[0] - c[0]))


def _upper_hit(c0, r0, c1, r1):
    """The intersection of two circles that is higher on screen (smaller y)."""
    p, q = _circle_hits(c0, r0, c1, r1)
    return p if p[1] < q[1] else q


def cloud(s, circles, base):
    """A cloud: the top outline of left-to-right overlapping circles on a flat base."""
    pts = []
    (c0, r0) = circles[0]
    # Start where the first circle meets the base, on its left.
    start = 180 - math.degrees(math.asin(max(-1, min(1, (base - c0[1]) / r0))))
    for i, (c, r) in enumerate(circles):
        if i == 0:
            a0 = start
        else:
            a0 = _angle(c, _upper_hit(circles[i - 1][0], circles[i - 1][1], c, r))
        if i == len(circles) - 1:
            a1 = 360 + math.degrees(math.asin(max(-1, min(1, (base - c[1]) / r))))
        else:
            a1 = _angle(c, _upper_hit(c, r, circles[i + 1][0], circles[i + 1][1]))
        # Walk clockwise on screen (increasing angle) over the top, starting
        # from straight down (90 degrees) so each arc is under one turn.
        a0 %= 360
        if a0 < 90:
            a0 += 360
        while a1 <= a0:
            a1 += 360
        while a1 - a0 > 360:
            a1 -= 360
        n = max(8, int((a1 - a0) / 4))
        for k in range(n + 1):
            a = math.radians(a0 + (a1 - a0) * k / n)
            pts.append((c[0] + r * math.cos(a), c[1] + r * math.sin(a)))
    pts.append(pts[0])
    return s.poly(pts)


def sparkle(s, c, rx, ry, pinch=0.16):
    """A four-point star whose sides curve in towards the centre."""
    cx, cy = c
    tips = [(cx, cy - ry), (cx + rx, cy), (cx, cy + ry), (cx - rx, cy)]
    pts = []
    for i in range(4):
        a, b = tips[i], tips[(i + 1) % 4]
        # Control point pulled in towards the centre, between the two tips.
        k = (cx + (a[0] + b[0] - 2 * cx) * pinch, cy + (a[1] + b[1] - 2 * cy) * pinch)
        for j in range(24 if i < 3 else 25):
            t = j / 24
            u = 1 - t
            pts.append((u * u * a[0] + 2 * u * t * k[0] + t * t * b[0],
                        u * u * a[1] + 2 * u * t * k[1] + t * t * b[1]))
    return s.poly(pts)


def cube(s, yaw, pitch, size, centre=(50, 50)):
    """A wireframe cube, every edge drawn, under an orthographic view."""
    cy_, sy_ = math.cos(math.radians(yaw)), math.sin(math.radians(yaw))
    cp, sp = math.cos(math.radians(pitch)), math.sin(math.radians(pitch))
    verts = []
    for x in (-1, 1):
        for y in (-1, 1):
            for z in (-1, 1):
                x1, z1 = x * cy_ + z * sy_, -x * sy_ + z * cy_
                y1, z2 = y * cp - z1 * sp, y * sp + z1 * cp
                verts.append((x1, y1))
    xs = [v[0] for v in verts]
    ys = [v[1] for v in verts]
    k = size / max(max(xs) - min(xs), max(ys) - min(ys))
    mx, my = (max(xs) + min(xs)) / 2, (max(ys) + min(ys)) / 2
    p = [(centre[0] + (vx - mx) * k, centre[1] + (vy - my) * k) for vx, vy in verts]
    # Vertex index bits are (x, y, z). Face z=-1: 0, 2, 6, 4; face z=+1:
    # 1, 3, 7, 5. One run takes both faces and one edge between them.
    s.poly([p[0], p[2], p[6], p[4], p[0], p[1], p[3], p[7], p[5], p[1]])
    for i in (2, 6, 4):
        s.line(p[i], p[i + 1])
    return s


def _turn(p, c, deg):
    t = math.radians(deg)
    x, y = p[0] - c[0], p[1] - c[1]
    return (c[0] + x * math.cos(t) - y * math.sin(t), c[1] + x * math.sin(t) + y * math.cos(t))


def earbud(s, head, r, half, bottom, tilt):
    """A stemmed earbud as one outline: a round head over a stem `2 * half` wide,
    turned `tilt` degrees about the head's centre."""
    cx, cy = head
    a = math.degrees(math.acos(half / r))
    pts = []
    for k in range(73):
        ang = math.radians(a - (360 - 2 * a) * k / 72)
        pts.append((cx + r * math.cos(ang), cy + r * math.sin(ang)))
    pts.append((cx - half, bottom - half))
    for k in range(1, 12):
        ang = math.radians(180 - 180 * k / 12)
        pts.append((cx + half * math.cos(ang), bottom - half + half * math.sin(ang)))
    pts.append((cx + half, bottom - half))
    pts.append(pts[0])
    return s.poly([_turn(p, head, tilt) for p in pts])


def torus(s, outer, smile, arch_peak, arch_half):
    """A torus seen from above at an angle: the outer edge, the near rim of the
    hole (a smile) and the far rim (a shorter arch whose ends sit on the smile)."""
    (oc, orx, ory) = outer
    s.ellipse(oc, orx, ory)
    (sc, srx, sry) = smile
    s.ellipse(sc, srx, sry, a0=14, a1=166)
    ang = math.acos(-arch_half / srx)
    yj = sc[1] + sry * math.sin(ang)
    ry = (yj - arch_peak) * 1.25
    cy = arch_peak + ry
    c = (yj - cy) / ry
    rx = arch_half / math.sqrt(1 - c * c)
    a = math.degrees(math.asin(c))
    return s.ellipse((sc[0], cy), rx, ry, a0=180 - a, a1=360 + a)


# 0: brick phone: chunky body with its pull-up antenna in one run, a small
# screen and a keypad.
brick = Shape()
brick.poly([(60, 4), (60, 30), (64, 30), (70, 36), (70, 90), (64, 96), (36, 96),
            (30, 90), (30, 36), (36, 30), (60, 30)])
brick.rect((40, 40), (60, 54), r=2)
for y in (68, 82):
    for x in (40, 50, 60):
        brick.dot((x, y))

# 1: smartphone: a slim rounded slab in the same place, camera dot, home bar.
phone = Shape()
phone.rect((31, 6), (69, 96), r=10)
phone.dot((50, 16))
phone.line((43, 86), (57, 86))

# 2: cassette player with its headphones resting above.
walkman = Shape()
walkman.arc((50, 36), 31, 180, 360)
walkman.rect((12, 30), (26, 48), r=5)
walkman.rect((74, 30), (88, 48), r=5)
walkman.rect((24, 58), (76, 94), r=5)
walkman.circle((39, 75), 5)
walkman.circle((61, 75), 5)

# 3: wireless earbuds: a mirrored pair of stemmed buds, splayed a little.
buds = Shape()
earbud(buds, (27, 34), 16, 5.5, 82, 15)
earbud(buds, (73, 34), 16, 5.5, 82, -15)

# 4: dial-up modem: a flat box drawn in depth, with a row of lights.
modem = Shape()
modem.poly([(6, 48), (82, 48), (82, 84), (6, 84)], closed=True)
modem.poly([(6, 48), (18, 34), (94, 34), (94, 70), (82, 84)])
modem.line((82, 48), (94, 34))
for x in (20, 32, 44, 56):
    modem.dot((x, 66))

# 5: Wi-Fi: three arcs and a dot.
wifi = Shape()
wifi.dot((50, 86))
for r in (17, 34, 51):
    wifi.arc((50, 86), r, 222, 318)

# 6: VHS tape: a wide cassette with the two reels showing through its window.
vhs = Shape()
vhs.rect((4, 24), (96, 76), r=3)
vhs.rect((24, 36), (76, 64), r=4)
vhs.circle((38, 50), 4)
vhs.circle((62, 50), 4)

# 7: streaming: a play triangle in a rounded player, with a progress bar.
play = Shape()
play.rect((4, 20), (96, 80), r=12)
play.poly([(40, 31), (40, 55), (62, 43)], closed=True)
play.line((20, 67), (80, 67))
play.circle((34, 67), 2.5)

# 8: pager: a small box with a one-line text screen and buttons.
pager = Shape()
pager.rect((10, 24), (90, 80), r=10)
pager.rect((21, 35), (79, 55), r=3)
pager.line((32, 45), (60, 45))
for x in (35, 50, 65):
    pager.dot((x, 67))

# 9: smartwatch: a rounded square face with straps, a crown and a pulse trace.
watch = Shape()
watch.rect((25, 24), (75, 76), r=13)
watch.poly([(34, 24), (37, 4), (63, 4), (66, 24)])
watch.poly([(34, 76), (37, 96), (63, 96), (66, 76)])
watch.dot((77, 44))
watch.poly([(35, 52), (43, 52), (47, 42), (53, 62), (57, 52), (65, 52)])

# 10: CD: a disc with a hole and two shine arcs.
cd = Shape()
cd.circle((50, 50), 44)
cd.circle((50, 50), 9)
cd.arc((50, 50), 29, 200, 250)
cd.arc((50, 50), 29, 20, 70)

# 11: cloud, about the disc's size and place.
cl = Shape()
cloud(cl, [((25, 60), 17), ((48, 42), 25), ((74, 57), 20)], 77)

# 12: wireframe cube, every edge drawn (hero).
cb = Shape()
cube(cb, 30, -22, 84)

# 13: smooth torus: outer ellipse and the two rims of the hole (hero).
tor = Shape()
torus(tor, ((50, 50), 46, 34), ((50, 34), 30, 22), 40, 19)

# 14: a boxy 1990s robot: antenna and head in one run, eyes, neck and body,
# arms (hero).
robot = Shape()
robot.poly([(50, 4), (50, 12), (68, 12), (68, 42), (32, 42), (32, 12), (50, 12)])
robot.dot((42, 26))
robot.dot((58, 26))
robot.poly([(50, 42), (50, 54), (76, 54), (76, 94), (24, 94), (24, 54), (50, 54)])
robot.poly([(24, 62), (12, 62), (12, 84)])
robot.poly([(76, 62), (88, 62), (88, 84)])

# 15: the AI sparkle: a four-point star and a smaller one (hero).
spark = Shape()
sparkle(spark, (44, 56), 36, 42)
sparkle(spark, (80, 20), 11, 13)


def _rot(pts, c, deg):
    t = math.radians(deg)
    return [(c[0] + (x - c[0]) * math.cos(t) - (y - c[1]) * math.sin(t),
             c[1] + (x - c[0]) * math.sin(t) + (y - c[1]) * math.cos(t)) for x, y in pts]


def _arc_pts(c, r, a0, a1, n=24):
    return [(c[0] + r * math.cos(math.radians(a0 + (a1 - a0) * k / n)),
             c[1] + r * math.sin(math.radians(a0 + (a1 - a0) * k / n))) for k in range(n + 1)]


# 6: a DOS prompt, C:\>_ , how you talked to a computer in 1992.
dos = Shape()
dos.arc((15, 50), 12, 45, 315)
dos.dot((35, 42))
dos.dot((35, 58))
dos.line((44, 36), (57, 64))
dos.poly([(64, 38), (78, 50), (64, 62)])
dos.line((86, 64), (98, 64))

# 7: the ChatGPT logo (OpenAI's knot): six rounded loops, each turned 60
# degrees on from the last round the centre, interlocking.
gpt = Shape()
for k in range(6):
    loop = []
    # A capsule beside the centre: two half circles joined by straights.
    loop += _arc_pts((50 - 15, 50 - 19), 15, 180, 360, 12)
    loop += _arc_pts((50 - 15, 50 + 19), 15, 0, 180, 12)
    loop.append(loop[0])
    gpt.poly(_rot([(x + 15, y) for x, y in loop], (50, 50), 60 * k))

# 8: a coin with a dollar sign: the then of money.
dollar = Shape()
dollar.circle((50, 50), 44)
dollar.arc((50, 39), 11, 330, 90)
dollar.arc((50, 61), 11, 270, 510)
dollar.line((50, 17), (50, 27))
dollar.line((50, 73), (50, 83))

# 9: the Bitcoin logo: the same coin with its B, two strokes above and below,
# tilted as the logo is.
btc = Shape()
btc.circle((50, 50), 44)
b_pts = [(40, 28), (58, 28)] + _arc_pts((58, 39), 11, 270, 450, 14) + [(40, 50), (60, 50)] \
    + _arc_pts((60, 61), 11, 270, 450, 14) + [(40, 72), (40, 28)]
btc.poly(_rot(b_pts, (50, 50), 14))
for x in (45, 56):
    btc.poly(_rot([(x, 18), (x, 28)], (50, 50), 14))
    btc.poly(_rot([(x, 72), (x, 82)], (50, 50), 14))

# 15: the Claude logo: a sunburst of rounded rays of uneven length.
claude = Shape()
lengths = [44, 35, 42, 33, 46, 37, 43, 34, 45, 36, 41, 32]
for k, length in enumerate(lengths):
    a = math.radians(30 * k + (7 if k % 2 else 0) - 90)
    claude.line((50 + 9 * math.cos(a), 50 + 9 * math.sin(a)),
                (50 + length * math.cos(a), 50 + length * math.sin(a)))

# 3: the Apple logo: the bitten apple and its leaf, as the Walkman's heir.
def _bez(p0, p1, p2, p3, n=16):
    return [((1 - t) ** 3 * p0[0] + 3 * (1 - t) ** 2 * t * p1[0] + 3 * (1 - t) * t * t * p2[0] + t ** 3 * p3[0],
             (1 - t) ** 3 * p0[1] + 3 * (1 - t) ** 2 * t * p1[1] + 3 * (1 - t) * t * t * p2[1] + t ** 3 * p3[1])
            for t in [k / n for k in range(n + 1)]]


apple = Shape()
apple.poly(_bez((50, 30), (60, 21), (80, 21), (86.7, 37.7)) + _arc_pts((98, 49), 16, 225, 135, 12)
           + _bez((86.7, 60.3), (84, 78), (72, 94), (62, 93)) + _bez((62, 93), (57, 92), (54, 89), (50, 89), 6)
           + _bez((50, 89), (46, 89), (43, 92), (38, 93), 6) + _bez((38, 93), (25, 94), (12, 76), (12, 54))
           + _bez((12, 54), (12, 30), (36, 20), (50, 30)))
apple.poly(_bez((54, 18), (54, 6), (62, 2), (70, 2)) + _bez((70, 2), (70, 12), (62, 18), (54, 18)))

SHAPES = {
    0: ("brick phone", brick),
    1: ("smartphone", phone),
    2: ("cassette player", walkman),
    3: ("Apple", apple),
    4: ("dial-up modem", modem),
    5: ("Wi-Fi", wifi),
    6: ("DOS prompt", dos),
    7: ("ChatGPT", gpt),
    8: ("dollar coin", dollar),
    9: ("Bitcoin", btc),
    10: ("CD", cd),
    11: ("cloud", cl),
    12: ("wireframe cube", cb),
    13: ("torus", tor),
    14: ("boxy robot", robot),
    15: ("Claude", claude),
}
