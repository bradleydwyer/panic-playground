"""Pattern 4, "Classic emoji": neon line drawings of well-known emoji (authored).

These are our own line drawings of the ideas behind fourteen classic emoji,
made for the remake in the kit's simple neon style. They are not any
vendor's emoji artwork (nothing here is traced or copied from Apple, Google
or anyone else's designs) and they are not recovered from the original demo.

The set: eggplant, sweat droplets, peach, party popper, face with tears of
joy, skull, thumbs up, hundred points, rocket, alien, ghost, jack-o'-lantern,
pile of poo and a unicorn's head. Each is its own subject. Where the music
shows one round head straight after another (tears of joy then skull, ghost
then jack-o'-lantern, the alien after the rocket), the two are drawn at
different sizes, shapes and places so the flip reads.
"""
import math

from neon import Shape

TITLE = "Classic emoji"
NOT_SHOWN = [3]  # masks 6 and 7 never appear in this pattern

GAP = 6.3  # a sign maker's air gap, centre to centre, in drawing units


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


def _arc_pts(c, r, a0, a1, n=None):
    n = n or max(8, int(abs(a1 - a0) / 4))
    return [(c[0] + r * math.cos(math.radians(a0 + (a1 - a0) * i / n)),
             c[1] + r * math.sin(math.radians(a0 + (a1 - a0) * i / n))) for i in range(n + 1)]


def _ellipse_arc(c, rx, ry, rot, a0, a1, n=24):
    """Points from angle a0 to a1 (degrees) of an ellipse turned by rot."""
    t = math.radians(rot)
    pts = []
    for i in range(n + 1):
        a = math.radians(a0 + (a1 - a0) * i / n)
        x, y = rx * math.cos(a), ry * math.sin(a)
        pts.append((c[0] + x * math.cos(t) - y * math.sin(t), c[1] + x * math.sin(t) + y * math.cos(t)))
    return pts


def _ellipse_pts(c, rx, ry, rot=0):
    return _ellipse_arc(c, rx, ry, rot, 0, 360, n=72)


def _rot(pts, pivot, deg):
    a = math.radians(deg)
    ca, sa = math.cos(a), math.sin(a)
    px, py = pivot
    return [(px + (x - px) * ca - (y - py) * sa, py + (x - px) * sa + (y - py) * ca) for x, y in pts]


def _move(pts, dx, dy):
    return [(x + dx, y + dy) for x, y in pts]


def _teardrop(tip, c, r):
    """A closed teardrop: tangent lines from the tip to a circle."""
    dx, dy = tip[0] - c[0], tip[1] - c[1]
    base = math.degrees(math.atan2(dy, dx))
    phi = math.degrees(math.acos(r / math.hypot(dx, dy)))
    return [tip] + _arc_pts(c, r, base + phi, base - phi + 360, n=40) + [tip]


def _open_at(loop, p, gap=GAP):
    """A closed point list opened into one run, with a gap `gap` across
    centred on its point nearest `p` (so the neon rules, which open a loop
    at its lowest point, leave the opening where it is drawn)."""
    ring = []
    for a, b in zip(loop, loop[1:] + loop[:1]):  # at least every half unit
        n = max(1, math.ceil(math.hypot(b[0] - a[0], b[1] - a[1]) / 0.5))
        ring += [(a[0] + (b[0] - a[0]) * i / n, a[1] + (b[1] - a[1]) * i / n) for i in range(n)]
    k = min(range(len(ring)), key=lambda i: math.hypot(ring[i][0] - p[0], ring[i][1] - p[1]))
    ring = ring[k:] + ring[:k] + [ring[k]]
    c = ring[0]
    far = lambda q: math.hypot(q[0] - c[0], q[1] - c[1]) >= gap / 2
    a = next(i for i in range(len(ring)) if far(ring[i]))
    b = next(i for i in range(len(ring) - 1, -1, -1) if far(ring[i]))
    return ring[a:b + 1]


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


def _behind(s, loop, fronts, gap=GAP):
    """Add the closed outline `loop` to s, broken wherever one of the `fronts`
    outlines sits in front of it: it stops `gap` short of them, centre to
    centre, so it reads as passing behind."""
    pts = loop[:-1] if loop[0] == loop[-1] else list(loop)
    hidden = [any(_inside(p, f) or _near(p, f, gap) for f in fronts) for p in pts]
    if not any(hidden):
        return s.poly(pts, closed=True)
    k = hidden.index(True)
    pts, hidden = pts[k:] + pts[:k], hidden[k:] + hidden[:k]
    run = []
    for p, h in zip(pts + [pts[0]], hidden + [hidden[0]]):
        if h:
            if len(run) > 1:
                s.poly(run)
            run = []
        else:
            run.append(p)
    if len(run) > 1:
        s.poly(run)
    return s


# 0: an eggplant lying on the diagonal, the bulb low on the left and the
# leafy cap and its stem at the top right.
def eggplant():
    s = Shape()
    # Drawn along a horizontal axis (bulb at the left), then tipped up. The
    # body runs from one outer sepal tip round the bulb to the other.
    body = _path((0, -17), ((-12, -17), (-22, -23), (-36, -23)),
                 ((-52, -23), (-60, -6), (-54, 8)),
                 ((-48, 24), (-28, 28), (-12, 23)),
                 ((-6, 21), (-3, 19), (0, 17)))
    # One tube: down the stem, round the cap and its three sepals and
    # back, stopping a gap short of where it began. Each sepal's point
    # opens at 40 degrees or more, so the neon rules keep it whole.
    cap = _path((62, -14), ((56, -5), (50, -3), (42, -3)))
    cap += _path((42, -3), ((40, -12), (30, -20), (16, -20)), ((10, -20), (4, -18), (0, -17)))[1:]
    cap += [(17, -6.5), (0, 0), (17, 6.5)]
    cap += _path((0, 17), ((4, 18), (10, 20), (16, 20)), ((30, 20), (40, 12), (42, 3)))

    def place(run):
        return _move(_rot(run, (0, 0), -38), 55, 53)

    s.poly(place(body))
    s.poly(place(cap))
    return s


# 1: sweat droplets, three drops splashing out, their points toward where
# they came from (lower right).
def droplets():
    s = Shape()
    src = (100, 100)
    for c, r in [((24, 70), 15), ((50, 36), 11.5), ((82, 18), 8.5)]:
        dx, dy = src[0] - c[0], src[1] - c[1]
        d = math.hypot(dx, dy)
        tip = (c[0] + dx / d * r * 2.1, c[1] + dy / d * r * 2.1)
        back = (c[0] - dx / d * r, c[1] - dy / d * r)
        s.poly(_open_at(_teardrop(tip, c, r), back))
    return s


# 2: a peach: a round fruit with a point at the top, its crease, and two
# leaves.
def peach():
    s = Shape()
    s.poly(_path((50, 26), ((70, 16), (88, 40), (86, 60)),
                 ((84, 82), (64, 94), (48, 94)),
                 ((28, 94), (12, 80), (14, 60)),
                 ((16, 40), (32, 28), (50, 26))))
    s.poly(_path((48, 36), ((36, 48), (34, 66), (42, 80))))
    s.poly(_path((54, 18), ((62, 6), (78, 4), (88, 8)), ((82, 18), (68, 22), (54, 18))))
    s.poly(_path((44, 18), ((38, 8), (28, 6), (20, 10)), ((26, 18), (36, 20), (44, 18))))
    return s


# 3: a party popper: a cone with its point at the lower left, streamers
# and confetti bursting from its mouth.
def party_popper():
    s = Shape()
    tip, mouth, half = (8, 94), (42, 60), 19
    deg = math.degrees(math.atan2(mouth[1] - tip[1], mouth[0] - tip[0]))
    # The cone: its two sides and the far half of the mouth's rim, opened
    # where the confetti bursts out rather than at its point (its lowest)...
    far = _ellipse_arc(mouth, 7.5, half, deg, -90, 90)
    s.poly(_open_at([tip] + far + [tip], far[len(far) // 2]))
    # ...and the near half, across the opening.
    s.poly(_ellipse_arc(mouth, 7.5, half, deg, 90, 270))
    s.poly(_path((54, 40), ((60, 30), (50, 22), (56, 14)), ((62, 6), (70, 10), (72, 2))))
    s.poly(_path((62, 54), ((72, 48), (76, 38), (86, 40)), ((94, 42), (96, 30), (98, 24))))
    s.poly(_path((66, 70), ((74, 74), (80, 66), (88, 70)), ((94, 73), (96, 80), (94, 86))))
    for p in [(38, 30), (72, 24), (82, 56), (88, 10), (58, 84), (30, 10)]:
        s.dot(p)
    return s


# 4: face with tears of joy: a round face, smaller than the skull that
# follows it, eyes squeezed shut, mouth wide open, and a big tear running
# from each eye down over the cheek, in front of the outline.
def tears_of_joy():
    s = Shape()
    c, r = (50, 52), 32
    tears = [_teardrop((30, 52), (18, 76), 8.5), _teardrop((70, 52), (82, 76), 8.5)]
    _behind(s, _arc_pts(c, r, 0, 360, n=180), tears)
    for t in tears:
        s.poly(t)
    s.arc((37, 47), 8, 205, 335)
    s.arc((63, 47), 8, 205, 335)
    s.poly(_arc_pts((50, 57), 15, 0, 180) + [(65, 57)])
    return s


# 5: a skull: a tall cranium filling the box, wide sockets, a small nose and
# a narrower jaw with its teeth.
def skull():
    s = Shape()
    s.poly(_path((50, 94), ((40, 94), (30, 92), (30, 84)),
                 ((30, 76), (18, 72), (14, 60)),
                 ((6, 36), (22, 4), (50, 4)),
                 ((78, 4), (94, 36), (86, 60)),
                 ((82, 72), (70, 76), (70, 84)),
                 ((70, 92), (60, 94), (50, 94))))
    s.circle((34, 44), 11)
    s.circle((66, 44), 11)
    s.poly([(44, 70), (50, 60), (56, 70)])
    for x in (40, 50, 60):
        s.line((x, 76), (x, 85))
    return s


# 8: thumbs up: a fist with its thumb raised, the cuff at the left.
def thumbs_up():
    s = Shape()
    s.rect((8, 46), (26, 92), r=3)
    hand = _path((26, 50), ((32, 44), (40, 34), (42, 20)),
                 ((43, 8), (58, 6), (58, 18)),
                 ((58, 26), (56, 34), (54, 40)))
    hand += [(80, 40)]
    for y, x in [(46, 80), (58, 84), (70, 83), (82, 79)]:  # the four fingers
        hand += _arc_pts((x, y), 6, -90, 90, n=12)
    hand += [(26, 88)]
    s.poly(hand)
    for y, x in [(52, 80), (64, 83), (76, 79)]:
        s.line((x, y), (58, y))
    return s


# 9: hundred points: "100" in slanted strokes, underlined twice.
def hundred():
    s = Shape()
    lean = 0.2
    base = 58

    def slant(pts):
        return [(x + (base - y) * lean, y) for x, y in pts]

    s.poly(slant([(14, 18), (23, 8), (23, 58)]))
    s.poly(slant(_ellipse_pts((46, 33), 11, 25)))
    s.poly(slant(_ellipse_pts((76, 33), 11, 25)))
    s.line((10, 72), (94, 68))
    s.line((14, 84), (88, 80))
    return s


# 10: a rocket climbing to the upper right: a pointed body with a round
# window, two fins and a flame.
def rocket():
    s = Shape()
    # Drawn nose up about (0, 0), then turned to climb to the upper right.
    # The body is one tube from the nozzle round the fins and nose and back,
    # its opening at the nozzle, where the flame comes out.
    half = _path((0, -50), ((9, -42), (14, -26), (14, -8)), ((14, 0), (13.5, 4), (13, 8)))
    half += [(25, 24), (25, 34), (11, 24), (0, 24)]
    body = [(-x, y) for x, y in reversed(half)] + half[1:]
    body = _open_at(body, (0, 24))
    flame = _path((-6, 32), ((-9, 40), (-4, 46), (0, 54)), ((4, 46), (9, 40), (6, 32)))
    for run in (body, flame):
        s.poly(_move(_rot(run, (0, 0), 45), 50, 50))
    s.circle(_move(_rot([(0, -18)], (0, 0), 45), 50, 50)[0], 7.5)
    return s


# 11: an alien's head: a wide cranium tapering to a narrow chin, big
# slanting eyes and a small mouth.
def alien():
    s = Shape()
    s.poly(_path((50, 95), ((64, 93), (88, 66), (88, 40)),
                 ((88, 16), (70, 5), (50, 5)),
                 ((30, 5), (12, 16), (12, 40)),
                 ((12, 66), (36, 93), (50, 95))))
    s.ellipse((33, 50), 14, 7.5, rot=30)
    s.ellipse((67, 50), 14, 7.5, rot=-30)
    s.line((44, 76), (56, 76))
    return s


# 12: a ghost: a tall sheet with a domed top and a scalloped hem, leaning,
# with two eyes and an open mouth.
def ghost():
    s = Shape()
    sheet = _path((24, 80), ((22, 62), (22, 44), (24, 32)),
                  ((28, 14), (38, 6), (50, 6)),
                  ((64, 6), (76, 16), (77, 34)),
                  ((78, 50), (78, 66), (78, 80)))
    # Three scallops; each cusp opens wide enough for the neon rules to keep.
    for x0, x1 in [(78, 60), (60, 42), (42, 24)]:
        sheet += _path((x0, 80), ((x0 - 7, 98), (x1 + 7, 98), (x1, 80)))[1:]
    s.poly(_rot(sheet, (50, 50), 8))
    for run in ([(41, 30), (41, 42)], [(59, 30), (59, 42)]):
        s.poly(_rot(run, (50, 50), 8))
    s.poly(_rot(_ellipse_pts((50, 62), 7, 9), (50, 50), 8))
    return s


# 13: a jack-o'-lantern: a wide, squat pumpkin with a stem, triangle eyes
# and a jagged grin.
def jack_o_lantern():
    s = Shape()
    s.poly(_path((50, 30), ((60, 22), (72, 22), (80, 26)),
                 ((96, 34), (100, 64), (86, 80)),
                 ((76, 92), (60, 92), (50, 90)),
                 ((40, 92), (24, 92), (14, 80)),
                 ((0, 64), (4, 34), (20, 26)),
                 ((28, 22), (40, 22), (50, 30))))
    s.poly(_path((50, 22), ((50, 14), (54, 8), (62, 6))))
    s.poly([(24, 54), (32, 40), (40, 54)], closed=True)
    s.poly([(60, 54), (68, 40), (76, 54)], closed=True)
    s.poly([(22, 64), (34, 64), (40, 70), (46, 64), (54, 64), (60, 70), (66, 64), (78, 64)]
           + _path((78, 64), ((70, 84), (30, 84), (22, 64)))[1:])
    return s


# 14: pile of poo: three soft tiers, each sitting on the one below (the
# neon gaps where they meet read as the folds), a pointed tip, and a face.
def poo():
    s = Shape()
    s.poly(_path((50, 94), ((26, 94), (8, 92), (8, 80)),
                 ((8, 70), (18, 66), (30, 66)),
                 ((40, 66), (60, 66), (70, 66)),
                 ((82, 66), (92, 70), (92, 80)),
                 ((92, 92), (74, 94), (50, 94))))
    s.poly(_path((22, 66), ((12, 58), (16, 42), (30, 42)),
                 ((42, 42), (58, 42), (70, 42)),
                 ((84, 42), (88, 58), (78, 66))))
    s.poly(_path((32, 42), ((26, 32), (34, 22), (44, 18)),
                 ((48, 16), (52, 11), (54, 3)))
           + _path((54, 3), ((57, 11), (62, 15), (66, 19)),
                   ((73, 25), (74, 36), (68, 42)))[1:])
    s.line((40, 51), (40, 56))
    s.line((60, 51), (60, 56))
    s.arc((50, 66), 17, 30, 150)
    return s


# 15: a unicorn's head in profile, facing left: ear, eye, nostril, two
# locks of mane down its neck and the horn rising from its forehead.
def unicorn():
    s = Shape()
    s.poly(_path((52, 96), ((52, 88), (54, 80), (56, 74)),
                 ((46, 70), (36, 74), (26, 78)),
                 ((16, 80), (8, 72), (12, 64)),
                 ((18, 52), (32, 40), (44, 30)),
                 ((48, 26), (50, 18), (52, 6)),
                 ((58, 10), (62, 14), (64, 22)),
                 ((72, 36), (76, 62), (78, 96))))
    s.poly([(36, 32), (20, 4), (46, 24)])
    s.poly(_path((66, 10), ((80, 14), (86, 30), (84, 46)), ((82, 62), (88, 76), (96, 86))))
    s.poly(_path((76, 6), ((92, 10), (98, 26), (96, 42))))
    s.dot((40, 44))
    s.dot((18, 68))
    return s


SHAPES = {
    0: ("eggplant", eggplant()),
    1: ("droplets", droplets()),
    2: ("peach", peach()),
    3: ("party popper", party_popper()),
    4: ("tears of joy", tears_of_joy()),
    5: ("skull", skull()),
    8: ("thumbs up", thumbs_up()),
    9: ("100", hundred()),
    10: ("rocket", rocket()),
    11: ("alien", alien()),
    12: ("ghost", ghost()),
    13: ("jack-o'-lantern", jack_o_lantern()),
    14: ("pile of poo", poo()),
    15: ("unicorn", unicorn()),
}
