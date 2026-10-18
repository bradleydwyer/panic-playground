"""Pattern 7, "AI": the last pattern before the original symbols return (authored).

New line art for the remake, not recovered from the original demo. Fourteen
separate pictures of thinking machines and what they stand for: a light bulb,
a brain, a chat bubble, a gear, a neural network, a crystal ball, a magnifying
glass, sparkles, a calculator, a padlock, an eye, a voice waveform, a hand
reaching out and an industrial robot arm. Each is its own subject, drawn with
its own outline, size and place so that the flips the music plays between
them read as a change of picture.
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


def _turn(pts, deg):
    """Points turned `deg` degrees (clockwise on screen) about the origin."""
    t = math.radians(deg)
    return [(x * math.cos(t) - y * math.sin(t), x * math.sin(t) + y * math.cos(t)) for x, y in pts]


def _polar(c, r, a):
    return (c[0] + r * math.cos(math.radians(a)), c[1] + r * math.sin(math.radians(a)))


def _trim(pts, length, from_end=False):
    """The polyline with `length` of it cut off its start (or end)."""
    if from_end:
        return _trim(pts[::-1], length)[::-1]
    for i in range(1, len(pts)):
        d = math.dist(pts[i - 1], pts[i])
        if d >= length:
            f = length / d
            a, b = pts[i - 1], pts[i]
            return [(a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f)] + pts[i:]
        length -= d
    return pts[-1:]


def sparkle(s, c, rx, ry, pinch=0.2, gap=6.5):
    """A four-point star whose sides curve in towards the centre, as one open
    tube that starts and ends `gap` apart about the middle of its lower-left
    side, so the neon's gap sits on a side rather than at a tip."""
    cx, cy = c
    tips = [(cx, cy - ry), (cx + rx, cy), (cx, cy + ry), (cx - rx, cy)]
    loop = []
    for i in (2, 3, 0, 1):  # from the bottom tip round to it again
        a, b = tips[i], tips[(i + 1) % 4]
        k = (cx + (a[0] + b[0] - 2 * cx) * pinch, cy + (a[1] + b[1] - 2 * cy) * pinch)
        for j in range(24):
            t = j / 24
            u = 1 - t
            loop.append((u * u * a[0] + 2 * u * t * k[0] + t * t * b[0],
                         u * u * a[1] + 2 * u * t * k[1] + t * t * b[1]))
    ring = loop[12:] + loop[:13]  # from the lower-left side's middle round to it
    return s.poly(_trim(_trim(ring, gap / 2), gap / 2, from_end=True))


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
G, GR = (50, 43), 25
bulb.poly([(39, 75)] + _bez((39, 75), (39, 69), (36, 66), _polar(G, GR, 125), 6)[1:]
          + _arc_pts(G, GR, 125, 415)[1:]
          + _bez(_polar(G, GR, 55), (64, 66), (61, 69), (61, 75), 6)[1:] + [(39, 75)])
bulb.line((41, 85), (59, 85))
bulb.line((46, 95), (54, 95))
for a in (180, 225, 270, 315, 0):
    bulb.line(_polar(G, GR + 9, a), _polar(G, GR + 17, a))

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


# 3: gear: eight tapered teeth round one outline, and the hub hole. A tooth
# points straight down, so the loop opens on its flat top.
def _gear(c, root, tip, teeth, root_half, tip_half):
    pts = []
    for i in range(teeth):
        a = 90 + 360 * i / teeth
        pts += _arc_pts(c, tip, a - tip_half, a + tip_half, 4)
        pts += _arc_pts(c, root, a + root_half, a + 360 / teeth - root_half, 6)
    return pts + [pts[0]]


gear = Shape()
gear.poly(_gear((50, 50), 32, 45, 8, 12, 8.5))
gear.circle((50, 50), 13)

# 4: neural network, 2 - 3 - 2 nodes. Only the links that don't cross are
# drawn (each node to its neighbours in the next column), so no link is
# broken in two; they stop short of the nodes.
NET = [[(12, 32), (12, 68)], [(50, 14), (50, 50), (50, 86)], [(88, 32), (88, 68)]]
NET_LINKS = [((0, 0), (1, 0)), ((0, 0), (1, 1)), ((0, 1), (1, 1)), ((0, 1), (1, 2)),
             ((1, 0), (2, 0)), ((1, 1), (2, 0)), ((1, 1), (2, 1)), ((1, 2), (2, 1))]
NODE_R = 7.5
net = Shape()
for (ca, ia), (cb, ib) in NET_LINKS:
    a, b = NET[ca][ia], NET[cb][ib]
    d = math.hypot(b[0] - a[0], b[1] - a[1])
    ux, uy = (b[0] - a[0]) / d, (b[1] - a[1]) / d
    gap = NODE_R + 7
    net.line((a[0] + ux * gap, a[1] + uy * gap), (b[0] - ux * gap, b[1] - uy * gap))
for col in NET:
    for p in col:
        net.circle(p, NODE_R)

# 5: crystal ball on its stand: the ball, a glint and a wisp of mist inside,
# and a flared stand whose sides stop short of the ball.
ball = Shape()
BALL, BALL_R = (50, 38), 32
ball.circle(BALL, BALL_R)
ball.arc(BALL, 22, 280, 335)
ball.poly(_bez((28, 50), (36, 40), (46, 58), (56, 46), 12) + _bez((56, 46), (62, 39), (68, 44), (70, 50), 8)[1:])
ball.poly([_polar(BALL, BALL_R, 116), (22, 94), (78, 94), _polar(BALL, BALL_R, 64)])

# 8: magnifying glass: the lens, a glint and the handle.
mag = Shape()
mag.circle((42, 42), 30)
mag.arc((42, 42), 19, 195, 255)
mag.line((63.2, 63.2), (88, 88))

# 9: sparkles: a big four-point sparkle low on the left and two small ones.
spark = Shape()
sparkle(spark, (40, 60), 30, 37, pinch=0.2)
sparkle(spark, (78, 24), 12, 16, pinch=0.2)
sparkle(spark, (84, 70), 8, 11, pinch=0.22)

# 10: pocket calculator: body, display and a grid of keys.
calc = Shape()
calc.rect((22, 4), (78, 96), r=8)
calc.rect((32, 14), (68, 32), r=3)
for y in (46, 59, 72, 85):
    for x in (36, 50, 64):
        calc.dot((x, y))

# 11: padlock: a wide body, the shackle (its legs stop at the body) and a
# keyhole, a ring with the slot below it.
lock = Shape()
lock.rect((12, 44), (88, 92), r=7)
lock.poly([(28, 44)] + _arc_pts((50, 30), 22, 180, 360) + [(72, 44)])
lock.circle((50, 62), 6.5)
lock.line((50, 68.5), (50, 82))

# 12: an eye: almond lids, the iris and a dot of a pupil.
LID_C, LID_R = 64.1, 48.1
eye = Shape()
_a = math.degrees(math.asin(46 / LID_R))
eye.poly(_arc_pts((50, LID_C), LID_R, 270 - _a, 270 + _a) + _arc_pts((50, 100 - LID_C), LID_R, 90 - _a, 90 + _a)[1:])
eye.circle((50, 50), 23)
eye.dot((50, 50))

# 13: voice waveform: a row of bars of a spoken word's loudness, centred on
# one line.
wave = Shape()
for i, h in enumerate((12, 30, 56, 82, 60, 36, 66, 22)):
    x = 10 + i * 80 / 7
    wave.line((x, 50 - h / 2), (x, 50 + h / 2))


# 14: a hand reaching out, palm open and fingers spread, on its forearm, up
# towards the top right. Drawn in local units with the wrist at the origin and
# the fingers up (-y), as one open outline: up the forearm's thumb side, round
# the thumb and the four fingers, and back down the other side. The fingers
# fan out from one point so the V between two of them opens wide enough
# (over 20 degrees) that its sides' tubes stay apart and none is broken.
def _finger(base_l, base_r, ang, length, tip_w):
    """A finger from its base edge along `ang` (degrees from up, clockwise),
    tapering to `tip_w` and rounded at the tip: its outline from base_l to
    base_r."""
    t = math.radians(ang)
    ux, uy = math.sin(t), -math.cos(t)  # along the finger
    nx, ny = math.cos(t), math.sin(t)  # across it, left to right
    tc = ((base_l[0] + base_r[0]) / 2 + ux * length, (base_l[1] + base_r[1]) / 2 + uy * length)
    a0 = math.degrees(math.atan2(-ny, -nx))
    return [base_l] + _arc_pts(tc, tip_w / 2, a0, a0 + 180, 10) + [base_r]


def reaching_hand(forearm=40):
    fan, knuckle, half = (0, 14), 48, 6.4  # fingers radiate from `fan`; bases `half` wide each side
    fingers = [(-28.5, 31, 10), (-9.5, 34, 10.5), (9.5, 31, 10), (28.5, 23, 9)]  # angle, length, tip width
    pts = [(-17, forearm), (-17, 4)]
    thumb = ((-27, -4), (-19, -18))
    pts += _bez((-17, 4), (-18, 0), (-22, -2), thumb[0], 6)[1:]
    pts += _finger(*thumb, -66, 25, 10.5)[1:]
    for i, (ang, length, tip_w) in enumerate(fingers):
        c = _polar(fan, knuckle - (4 if i == 3 else 0), ang - 90)
        nx, ny = math.cos(math.radians(ang)), math.sin(math.radians(ang))
        pts += _finger((c[0] - nx * half, c[1] - ny * half), (c[0] + nx * half, c[1] + ny * half),
                       ang, length, tip_w)
    end = pts[-1]
    pts += _bez(end, (end[0] + 1, end[1] + 10), (18, -4), (18, 4), 6)[1:] + [(18, forearm)]
    return pts


def _fit(pts, turn, lo=3, hi=97):
    """Points turned `turn` degrees and scaled to fill lo..hi, centred."""
    q = _turn(pts, turn)
    xs, ys = [x for x, _ in q], [y for _, y in q]
    k = (hi - lo) / max(max(xs) - min(xs), max(ys) - min(ys))
    cx, cy = (max(xs) + min(xs)) / 2, (max(ys) + min(ys)) / 2
    return [(50 + (x - cx) * k, 50 + (y - cy) * k) for x, y in q]


hand = Shape()
hand.poly(_fit(reaching_hand(), 35))


# 15: industrial robot arm: a base, the lower arm up from the shoulder to the
# elbow, the upper arm out and down to the wrist, a pin at each joint and a
# two-jaw gripper under the wrist. Each arm is a rounded link; the upper arm
# passes behind the lower one, its sides stopping at the lower arm's outline
# (where the neon rules leave their gap).
def _seg_dist(q, a, b):
    dx, dy = b[0] - a[0], b[1] - a[1]
    t = max(0, min(1, ((q[0] - a[0]) * dx + (q[1] - a[1]) * dy) / (dx * dx + dy * dy)))
    return math.hypot(q[0] - a[0] - t * dx, q[1] - a[1] - t * dy)


def _link(a, b, r):
    """A rounded link round the segment a -> b, `r` wide each side, as a loop."""
    ang = math.degrees(math.atan2(b[1] - a[1], b[0] - a[0]))
    pts = _arc_pts(b, r, ang - 90, ang + 90, 12) + _arc_pts(a, r, ang + 90, ang + 270, 12)
    return pts + [pts[0]]


def _link_behind(a, b, r, front, front_r):
    """A link from a to b, rounded round b, whose sides run back towards a
    until they meet the outline of the link `front` (a pair of points) of
    half-width `front_r`."""
    ang = math.degrees(math.atan2(b[1] - a[1], b[0] - a[0]))
    ux, uy = math.cos(math.radians(ang)), math.sin(math.radians(ang))

    def side(sgn):
        t = 0.0
        while True:
            q = (b[0] - ux * t - sgn * uy * r, b[1] - uy * t + sgn * ux * r)
            if _seg_dist(q, *front) <= front_r:
                return q
            t += 0.25

    return [side(-1)] + _arc_pts(b, r, ang - 90, ang + 90, 12) + [side(1)]


arm = Shape()
SH, EL, WR = (75, 64), (57, 20), (22, 42)
arm.poly([(60, 94), (64, 82), (86, 82), (90, 94)], closed=True)
arm.poly(_link(SH, EL, 9))
arm.poly(_link_behind(EL, WR, 7, (SH, EL), 9))
for q in (SH, EL, WR):
    arm.dot(q)
arm.poly([(18, 80), (9, 70), (13, 58), (31, 58), (35, 70), (26, 80)])


SHAPES = {
    0: ("light bulb", bulb),
    1: ("brain", brain),
    2: ("chat bubble", chat),
    3: ("gear", gear),
    4: ("neural network", net),
    5: ("crystal ball", ball),
    8: ("magnifying glass", mag),
    9: ("sparkles", spark),
    10: ("calculator", calc),
    11: ("padlock", lock),
    12: ("eye", eye),
    13: ("voice waveform", wave),
    14: ("reaching hand", hand),
    15: ("robot arm", arm),
}
