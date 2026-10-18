"""Drawing kit and neon renderer for the proposed part-6 shapes (authored).

Each shape is drawn in a 100 x 100 box, x right and y down, as a set of
strokes: polylines that the renderer draws as neon tubes with rounded ends.
The tube is about 7.5 units across (the scene's 3.8-row radius on a symbol
about 100 rows tall), so keep strokes simple and parallel strokes at least
10 units apart, centre to centre, or they merge into one glow.

    from neon import Shape
    s = Shape()
    s.line((20, 80), (80, 80))
    s.circle((50, 45), 25)
    s.arc((50, 45), 35, 200, 340)       # degrees, 0 = right, 90 = down
    s.ellipse((50, 50), 30, 12, rot=20)
    s.rect((25, 25), (75, 75), r=8)     # rounded corners
    s.poly([(10, 10), (30, 40), (50, 10)], closed=False)
    s.bezier((10, 90), (30, 10), (70, 10), (90, 90))
    s.spiral((50, 50), 6, 40, turns=2.5)
    s.dot((50, 50))                      # a short tube end, about a tube wide

Render one pattern's sheet with `python3 render.py N`.
"""
import math

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

TUBE = 7.5  # tube width, in shape units


class Shape:
    def __init__(self):
        self.runs = []  # each run: list of (x, y) points, open polylines

    def _add(self, pts):
        self.runs.append([(float(x), float(y)) for x, y in pts])
        return self

    def line(self, a, b):
        return self._add([a, b])

    def poly(self, pts, closed=False):
        pts = list(pts)
        if closed:
            pts = pts + [pts[0]]
        return self._add(pts)

    def arc(self, c, r, a0, a1, steps=None):
        n = steps or max(8, int(abs(a1 - a0) / 4))
        return self._add([(c[0] + r * math.cos(math.radians(a0 + (a1 - a0) * i / n)),
                           c[1] + r * math.sin(math.radians(a0 + (a1 - a0) * i / n)))
                          for i in range(n + 1)])

    def circle(self, c, r):
        return self.arc(c, r, 0, 360)

    def ellipse(self, c, rx, ry, rot=0, a0=0, a1=360):
        t = math.radians(rot)
        pts = []
        n = max(24, int(abs(a1 - a0) / 4))
        for i in range(n + 1):
            a = math.radians(a0 + (a1 - a0) * i / n)
            x, y = rx * math.cos(a), ry * math.sin(a)
            pts.append((c[0] + x * math.cos(t) - y * math.sin(t),
                        c[1] + x * math.sin(t) + y * math.cos(t)))
        return self._add(pts)

    def rect(self, a, b, r=0):
        (x0, y0), (x1, y1) = a, b
        if r <= 0:
            return self.poly([(x0, y0), (x1, y0), (x1, y1), (x0, y1)], closed=True)
        pts = []
        for cx, cy, s in [(x1 - r, y0 + r, -90), (x1 - r, y1 - r, 0),
                          (x0 + r, y1 - r, 90), (x0 + r, y0 + r, 180)]:
            for i in range(7):
                a = math.radians(s + 90 * i / 6)
                pts.append((cx + r * math.cos(a), cy + r * math.sin(a)))
        return self.poly(pts, closed=True)

    def bezier(self, p0, p1, p2, p3, steps=24):
        pts = []
        for i in range(steps + 1):
            t = i / steps
            u = 1 - t
            pts.append((u**3 * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t**3 * p3[0],
                        u**3 * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t**3 * p3[1]))
        return self._add(pts)

    def spiral(self, c, r0, r1, turns=2.0, start=0):
        n = int(turns * 90)
        return self._add([(c[0] + (r0 + (r1 - r0) * i / n) * math.cos(math.radians(start + 360 * turns * i / n)),
                           c[1] + (r0 + (r1 - r0) * i / n) * math.sin(math.radians(start + 360 * turns * i / n)))
                          for i in range(n + 1)])

    def dot(self, p):
        return self._add([(p[0] - 0.01, p[1]), (p[0] + 0.01, p[1])])


def _stroke_mask(shape, size, scale, width, margin):
    """Supersampled mask of the shape's tubes, `width` units wide."""
    ss = 3
    img = Image.new("L", (size * ss, size * ss), 0)
    d = ImageDraw.Draw(img)
    k = scale * ss
    w = max(1, int(round(width * k)))
    for run in shape.runs:
        pts = [((x * scale + margin) * ss, (y * scale + margin) * ss) for x, y in run]
        if len(pts) > 1:
            d.line(pts, fill=255, width=w, joint="curve")
        for (x, y) in (pts[0], pts[-1]):
            d.ellipse([x - w / 2, y - w / 2, x + w / 2, y + w / 2], fill=255)
    return np.asarray(img.resize((size, size), Image.LANCZOS), dtype=np.float32) / 255.0


def render(shape, size=200, lit=True):
    """The shape as a neon sign on black: glow, amber gas, white-hot core."""
    margin = size * 0.1
    scale = (size - 2 * margin) / 100.0
    glass = _stroke_mask(shape, size, scale, TUBE, margin)
    core = _stroke_mask(shape, size, scale, TUBE * 0.38, margin)
    hot = _stroke_mask(shape, size, scale, TUBE * 0.14, margin)
    glow_img = Image.fromarray((glass * 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(size * 0.03))
    glow = np.asarray(glow_img, dtype=np.float32) / 255.0
    wide = np.asarray(Image.fromarray((glass * 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(size * 0.08)),
                      dtype=np.float32) / 255.0
    amber = np.array([1.0, 0.62, 0.10])
    gas = np.array([1.0, 0.86, 0.35])
    out = (glow[..., None] * amber * 0.9 + wide[..., None] * amber * 0.5
           + glass[..., None] * np.array([0.55, 0.40, 0.12]) * 0.6
           + core[..., None] * gas * 1.1 + hot[..., None] * np.array([1.0, 1.0, 0.9]) * 0.9)
    out = 1.0 - np.exp(-out * 1.6)  # soft tonemap
    return Image.fromarray((np.clip(out, 0, 1) * 255).astype(np.uint8), "RGB")
