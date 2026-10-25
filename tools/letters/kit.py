"""Drawing kit and geometry checks for part 7's authored greetings letters.

The letters are new drawings made for the remake in the style of the
original's (heavy block letters cut by slits and small holes, chamfered
corners). They are not traced, vectorised or fitted from the demo's glyphs
or from any font.

Units and box. A letter is drawn in a box 180 units wide and 180 tall, x to
the right and y up, with the baseline at y = 0. That is the original's
letter face of 18 columns by 15 rows at 10 units a column and 12 a row: the
original's rows are 1.2 times as tall as its columns are wide on the 4:3
screen, so the face is square on screen and a unit is the same length
across and up. Letters set in a line advance 216 units; stacked as part 7
stacks them, one per 18-row slot, they are 216 apart (18 rows of 12).

Contours. Each letter is one or more closed outer contours, counter-
clockwise, with holes as separate closed contours, clockwise (y up). So
the letter's ink is always on the left of every edge. Vertices are whole
units; the last vertex joins the first and is not repeated.

    from kit import Glyph, K, k
    g = Glyph('O')
    g.outer([(0, 0, K), (180, 0, K), (180, 180, K), (0, 180, K)])
    g.hole([(70, 56, 12), (110, 56, 12), (110, 124, 12), (70, 124, 12)])

A third number at a corner chamfers it: the corner is replaced by a cut
that runs that many units along the grid on each of its two edges (45
degrees between a level and an upright edge).
"""
import math

BOX = 180  # the letter box's width and height, in units
ADVANCE = 216  # in a line: the box and a gap of 36
PITCH = 216  # stacked in part 7: an 18-row slot of 12-unit rows
COLUMN = 10  # units per original column
ROW = 12  # units per original row (on screen)

# Chamfers, in units along each edge: K on corners where the letter's pen
# would curve (the bowls of C, G, O and S, the shoulders of A and R), k on
# the plain corners of the block, J inside counters.
K = 36
k = 12
J = 8

# The minimum feature widths the drawings keep, in units (letter height
# 180): ink at least 36 (H/5) anywhere, and counters (slits, holes and
# notches) at least 24 (2H/15) across. A 45-degree bevel of 8 units on
# every edge, cutting in or building out, leaves 20 of ink face and 8 of
# counter open.
MIN_INK = 36
MIN_COUNTER = 24

EPS = 1e-6


class Glyph:
    def __init__(self, char, label=''):
        self.char = char
        self.label = label
        self.contours = []  # (kind, [(x, y), ...]); kind 'outer' or 'hole'

    def outer(self, corners):
        self.contours.append(('outer', orient(chamfer(corners), +1)))
        return self

    def hole(self, corners):
        self.contours.append(('hole', orient(chamfer(corners), -1)))
        return self


def rect(x0, y0, x1, y1, c=0):
    """The four corners of a box, each chamfered by `c`."""
    return [(x0, y0, c), (x1, y0, c), (x1, y1, c), (x0, y1, c)]


def chamfer(corners):
    """Corners (x, y) or (x, y, c) to whole-unit vertices, with each corner
    that has c > 0 replaced by a cut c grid units along each edge."""
    pts = [(c[0], c[1]) for c in corners]
    out = []
    n = len(pts)
    for i, c in enumerate(corners):
        amount = c[2] if len(c) > 2 else 0
        p = pts[i]
        if not amount:
            out.append(p)
            continue
        # Back along the incoming edge, then on along the outgoing one: the
        # cut runs between the two.
        for q in (pts[i - 1], pts[(i + 1) % n]):
            dx, dy = p[0] - q[0], p[1] - q[1]
            step = max(abs(dx), abs(dy))
            out.append((p[0] - amount * dx / step, p[1] - amount * dy / step))
    whole = []
    for x, y in out:
        if abs(x - round(x)) > EPS or abs(y - round(y)) > EPS:
            raise ValueError(f'vertex off the unit grid: {(x, y)}')
        whole.append((int(round(x)), int(round(y))))
    return whole


def signed_area(pts):
    """The shoelace area: positive when counter-clockwise (y up)."""
    s = 0.0
    for i in range(len(pts)):
        x0, y0 = pts[i - 1]
        x1, y1 = pts[i]
        s += x0 * y1 - x1 * y0
    return s / 2


def orient(pts, sign):
    return pts if signed_area(pts) * sign > 0 else pts[::-1]


def edges(contours):
    """Every edge of a letter: (a, b, contour index, edge index)."""
    out = []
    for ci, (_, pts) in enumerate(contours):
        for i in range(len(pts)):
            out.append((pts[i], pts[(i + 1) % len(pts)], ci, i))
    return out


def _cross(o, a, b):
    return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])


def _on_segment(p, a, b):
    return (min(a[0], b[0]) - EPS <= p[0] <= max(a[0], b[0]) + EPS
            and min(a[1], b[1]) - EPS <= p[1] <= max(a[1], b[1]) + EPS)


def segments_meet(a, b, c, d):
    """True if closed segments ab and cd share any point."""
    d1, d2 = _cross(c, d, a), _cross(c, d, b)
    d3, d4 = _cross(a, b, c), _cross(a, b, d)
    if ((d1 > EPS and d2 < -EPS) or (d1 < -EPS and d2 > EPS)) and \
       ((d3 > EPS and d4 < -EPS) or (d3 < -EPS and d4 > EPS)):
        return True
    return ((abs(d1) <= EPS and _on_segment(a, c, d)) or (abs(d2) <= EPS and _on_segment(b, c, d))
            or (abs(d3) <= EPS and _on_segment(c, a, b)) or (abs(d4) <= EPS and _on_segment(d, a, b)))


def crossings(contours):
    """Places where the letter's outline touches or crosses itself: any two
    edges that share a point, other than consecutive edges at their shared
    vertex. Consecutive edges that fold back over each other are reported
    too. Empty for a clean letter."""
    es = edges(contours)
    sizes = [len(pts) for _, pts in contours]
    found = []
    for i in range(len(es)):
        a, b, ci, ei = es[i]
        for j in range(i + 1, len(es)):
            c, d, cj, ej = es[j]
            if ci == cj and (ej - ei) % sizes[ci] in (1, sizes[ci] - 1):
                # Neighbours: they share a vertex; they must not overlap.
                if sizes[ci] == 2:
                    found.append((ci, ei, cj, ej))
                    continue
                shared = b if (ej - ei) % sizes[ci] == 1 else a
                other_i = a if shared == b else b
                other_j = d if shared == c else c
                u = (other_i[0] - shared[0], other_i[1] - shared[1])
                v = (other_j[0] - shared[0], other_j[1] - shared[1])
                if abs(u[0] * v[1] - u[1] * v[0]) <= EPS and u[0] * v[0] + u[1] * v[1] > 0:
                    found.append((ci, ei, cj, ej))
                continue
            if segments_meet(a, b, c, d):
                found.append((ci, ei, cj, ej))
    return found


def inside(p, contours):
    """Even-odd: is p in the letter's ink? (Not for points on the outline.)"""
    x, y = p
    hit = False
    for _, pts in contours:
        for i in range(len(pts)):
            (x0, y0), (x1, y1) = pts[i - 1], pts[i]
            if (y0 > y) != (y1 > y):
                t = (y - y0) / (y1 - y0)
                if x < x0 + t * (x1 - x0):
                    hit = not hit
    return hit


def distance_to_segment(p, a, b):
    dx, dy = b[0] - a[0], b[1] - a[1]
    t = ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy)
    t = max(0.0, min(1.0, t))
    return math.hypot(p[0] - a[0] - t * dx, p[1] - a[1] - t * dy)


def distance_to_outline(p, contours):
    return min(distance_to_segment(p, a, b) for a, b, _, _ in edges(contours))


def _clear(p, q, contours):
    """True if the open segment pq crosses or touches no edge of the letter
    (its two ends may lie on edges) and runs off the outline."""
    length = math.hypot(q[0] - p[0], q[1] - p[1])
    if length < EPS:
        return False
    trim = min(0.01, length / 4) / length
    p2 = (p[0] + (q[0] - p[0]) * trim, p[1] + (q[1] - p[1]) * trim)
    q2 = (q[0] - (q[0] - p[0]) * trim, q[1] - (q[1] - p[1]) * trim)
    for a, b, _, _ in edges(contours):
        if segments_meet(p2, q2, a, b):
            return False
    return True


def turns(pts):
    """Per vertex: the cross product of the incoming and outgoing edges
    (positive for a left turn) and the interior angle in degrees on the ink
    side (the left)."""
    out = []
    n = len(pts)
    for i in range(n):
        a, p, b = pts[i - 1], pts[i], pts[(i + 1) % n]
        u = (p[0] - a[0], p[1] - a[1])
        v = (b[0] - p[0], b[1] - p[1])
        cross = u[0] * v[1] - u[1] * v[0]
        dot = u[0] * v[0] + u[1] * v[1]
        turn = math.degrees(math.atan2(cross, dot))  # left turn positive
        out.append((cross, 180.0 - turn))
    return out


def feature_widths(contours):
    """The letter's narrowest ink and narrowest counter, as
    {'ink': (width, p, q), 'counter': (width, p, q)}, p and q the ends of
    the cut that measures it.

    A feature's width is measured where the region's medial axis has a
    neck: between two facing parallel edges (straight across), from a
    reflex corner of the region straight to an edge, or between two reflex
    corners of the region, wherever that cut runs through the region
    without touching the outline and leaves each corner it ends at between
    the normals of the corner's two edges. Corners that point into the
    region (a convex corner of the ink, the end of a slit) are not necks
    and are not measured. The counter is everything outside the ink, so a
    notch or slit open to the edge of the box counts as a counter as a
    hole does.
    """
    es = edges(contours)
    sizes = [len(pts) for _, pts in contours]
    result = {}
    for name, sign in (('ink', +1), ('counter', -1)):
        best = (math.inf, None, None)

        def consider(w, p, q, best_ref):
            if w < best_ref[0] - EPS:
                mid = ((p[0] + q[0]) / 2, (p[1] + q[1]) / 2)
                if inside(mid, contours) == (sign > 0) and distance_to_outline(mid, contours) > 1e-3 \
                        and _clear(p, q, contours):
                    return (w, p, q)
            return best_ref

        normals = []
        for a, b, _, _ in es:
            length = math.hypot(b[0] - a[0], b[1] - a[1])
            # The ink is on the left; the region's inward normal.
            normals.append((-(b[1] - a[1]) / length * sign, (b[0] - a[0]) / length * sign))

        # Facing parallel edges.
        for i, (a, b, _, _) in enumerate(es):
            n1 = normals[i]
            d = ((b[0] - a[0]), (b[1] - a[1]))
            length = math.hypot(*d)
            u = (d[0] / length, d[1] / length)
            for j, (c, e, _, _) in enumerate(es):
                if j == i:
                    continue
                n2 = normals[j]
                if n1[0] * n2[0] + n1[1] * n2[1] > -1 + 1e-9:
                    continue
                gap = (c[0] - a[0]) * n1[0] + (c[1] - a[1]) * n1[1]
                if gap <= EPS:
                    continue
                s0, s1 = sorted(((c[0] - a[0]) * u[0] + (c[1] - a[1]) * u[1],
                                 (e[0] - a[0]) * u[0] + (e[1] - a[1]) * u[1]))
                lo, hi = max(0.0, s0), min(length, s1)
                if hi - lo <= EPS:
                    continue
                for f in (0.02, 0.25, 0.5, 0.75, 0.98):
                    s = lo + (hi - lo) * f
                    p = (a[0] + u[0] * s, a[1] + u[1] * s)
                    q = (p[0] + n1[0] * gap, p[1] + n1[1] * gap)
                    best = consider(gap, p, q, best)

        # Reflex corners of the region, with the normals of the two edges
        # that meet there: a disk in the region touches the corner only if
        # its centre lies between them (the corner's normal cone).
        reflex = []
        start = 0
        for ci, (_, pts) in enumerate(contours):
            for vi, (cross, _) in enumerate(turns(pts)):
                if cross * sign < -EPS:
                    n_in = normals[start + (vi - 1) % sizes[ci]]
                    n_out = normals[start + vi]
                    reflex.append((pts[vi], ci, vi, n_in, n_out))
            start += sizes[ci]

        def in_cone(u, n_in, n_out):
            if u[0] * (n_in[0] + n_out[0]) + u[1] * (n_in[1] + n_out[1]) <= 0:
                return False
            c = n_in[0] * n_out[1] - n_in[1] * n_out[0]
            a = n_in[0] * u[1] - n_in[1] * u[0]
            b = u[0] * n_out[1] - u[1] * n_out[0]
            return (a >= -1e-9 and b >= -1e-9) if c >= 0 else (a <= 1e-9 and b <= 1e-9)

        for v, ci, vi, v_in, v_out in reflex:
            for j, (a, b, cj, ej) in enumerate(es):
                if cj == ci and (ej == vi or (ej + 1) % sizes[ci] == vi):
                    continue  # an edge at v
                n = normals[j]
                gap = (v[0] - a[0]) * n[0] + (v[1] - a[1]) * n[1]
                if gap <= EPS or not in_cone((-n[0], -n[1]), v_in, v_out):
                    continue
                d = (b[0] - a[0], b[1] - a[1])
                t = ((v[0] - a[0]) * d[0] + (v[1] - a[1]) * d[1]) / (d[0] ** 2 + d[1] ** 2)
                if not (EPS < t < 1 - EPS):
                    continue
                f = (a[0] + t * d[0], a[1] + t * d[1])
                best = consider(gap, f, v, best)
            for w, cj, wj, w_in, w_out in reflex:
                if (cj, wj) <= (ci, vi):
                    continue
                if cj == ci and (wj - vi) % sizes[ci] in (1, sizes[ci] - 1):
                    continue
                u = (w[0] - v[0], w[1] - v[1])
                if not (in_cone(u, v_in, v_out) and in_cone((-u[0], -u[1]), w_in, w_out)):
                    continue
                best = consider(math.hypot(*u), v, w, best)
        result[name] = best
    return result


def slivers(contours, shortest=8, sharpest=60, flattest=170):
    """Edges shorter than `shortest` units, corners sharper than `sharpest`
    degrees (on the ink side or the counter side) and corners within
    180 - `flattest` degrees of straight. Empty for a clean letter."""
    found = []
    for ci, (_, pts) in enumerate(contours):
        n = len(pts)
        for i in range(n):
            a, b = pts[i], pts[(i + 1) % n]
            if math.hypot(b[0] - a[0], b[1] - a[1]) < shortest:
                found.append(('short edge', ci, i))
        for i, (_, angle) in enumerate(turns(pts)):
            if angle < sharpest or angle > 360 - sharpest:
                found.append(('sharp corner', ci, i))
            elif flattest < angle < 360 - flattest:
                found.append(('flat corner', ci, i))
    return found
