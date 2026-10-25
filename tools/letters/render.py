#!/usr/bin/env python3
"""Preview sheets for part 7's authored letters, written as PNGs.

    python3 tools/letters/render.py [letters|words|solid|all] [--trug PNG] [--out DIR]

letters: the ten letters large, their contours (outer counter-clockwise,
    holes clockwise, arrows on the edges), vertices and narrowest ink and
    counter cuts marked, and each shaded as a raised bevelled face. With
    --trug, the path to a local type-study PNG of the original's front
    views (not in Git), each original letter is shown beside ours for
    style comparison only.
words: the five greetings as part 7 stacks them, one letter per slot in
    five strips, and set in lines.
solid: G, R, S and A extruded and bevelled, ray marched on the CPU and lit
    from one side.

Sheets go to artifacts/letters/ unless --out says otherwise. Nothing opens
a window. Needs numpy and Pillow.
"""
import argparse
import math
import sys
from pathlib import Path

sys.dont_write_bytecode = True

import numpy as np
from PIL import Image, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))

import kit  # noqa: E402
from glyphs import LETTERS  # noqa: E402

BG = (10, 12, 14)
INK = np.array([0.80, 0.93, 0.92])
WORDS = ['GREETINGS', 'GOTO', 'RENAISSANCE', 'TRITON', 'SONIC']
SET_WORDS = ['GREETINGS', 'GO TO', 'RENAISSANCE', 'TRITON', 'SONIC']
# Part 7's strips: left columns, and the face's first column in a strip
# (from hd/greetings.rs and hd/greetings_type.rs).
STRIP_X = [8, 72, 136, 200, 264]
FACE_FIRST_COLUMN = 5

# The type study's front-view panel, as the local PNG lays it out (3860 x
# 620): 8 pixels a column, 9.6 a row, letters 176 pixels apart from x = 26,
# rows 221 to 364. Only used when --trug is given.
TRUG_SIZE = (3860, 620)
TRUG_X0, TRUG_PITCH, TRUG_Y0, TRUG_SIDE = 26, 176, 221, 144
TRUG_ORDER = 'ACEGINORST'


def font(size):
    for name in ('Helvetica.ttc', 'Arial.ttf', 'DejaVuSans.ttf'):
        try:
            return ImageFont.truetype(name, size)
        except OSError:
            continue
    return ImageFont.load_default()


def sdf(glyph, scale, pad, size=None):
    """Signed distance to the outline in units (negative in the ink), on a
    grid of pixel centres, `scale` pixels a unit, `pad` units of margin; y
    up in units, rows down in the image."""
    n = size or int(round((kit.BOX + 2 * pad) * scale))
    c = (np.arange(n) + 0.5) / scale - pad
    x = c[None, :]
    y = (kit.BOX + pad - (np.arange(n) + 0.5) / scale)[:, None]
    x, y = np.broadcast_arrays(x, y)
    best = np.full(x.shape, np.inf)
    odd = np.zeros(x.shape, bool)
    for a, b, _, _ in kit.edges(glyph.contours):
        ax, ay = a
        dx, dy = b[0] - ax, b[1] - ay
        t = np.clip(((x - ax) * dx + (y - ay) * dy) / (dx * dx + dy * dy), 0, 1)
        best = np.minimum(best, np.hypot(x - ax - t * dx, y - ay - t * dy))
        if dy:
            s = (y - ay) / dy
            cross = ((ay > y) != (b[1] > y)) & (x < ax + s * dx)
            odd ^= cross
    return np.where(odd, -best, best)


def coverage(d, scale):
    return np.clip(0.5 - d * scale, 0, 1)


def bevel_shade(d, scale, bevel=8.0, light=(-0.55, 0.65, 0.52)):
    """The letter as a raised face with a 45-degree bevel `bevel` units
    wide, lit from the upper left, in the original's white-to-teal."""
    h = np.clip(-d / bevel, 0, 1)
    gy, gx = np.gradient(h * bevel * scale)  # rows run down
    nx, ny, nz = -gx, gy, np.ones_like(h)
    norm = np.sqrt(nx * nx + ny * ny + nz * nz)
    lx, ly, lz = light
    ll = math.sqrt(lx * lx + ly * ly + lz * lz)
    lam = np.clip((nx * lx + ny * ly + nz * lz) / (norm * ll), 0, 1)
    white = np.array([0.95, 0.97, 0.96])
    teal = np.array([0.10, 0.42, 0.42])
    t = np.clip((lam - 0.25) / 0.7, 0, 1)[..., None]
    col = teal + (white - teal) * t
    a = coverage(d, scale)[..., None]
    out = np.array(BG) / 255 * (1 - a) + col * a
    return Image.fromarray((np.clip(out, 0, 1) * 255).astype(np.uint8), 'RGB')


def arrow(draw, p, q, col, size):
    """An arrowhead at the middle of pq pointing from p to q."""
    mx, my = (p[0] + q[0]) / 2, (p[1] + q[1]) / 2
    dx, dy = q[0] - p[0], q[1] - p[1]
    length = math.hypot(dx, dy)
    if length < size * 3:
        return
    ux, uy = dx / length, dy / length
    tip = (mx + ux * size, my + uy * size)
    left = (mx - ux * size - uy * size * 0.8, my - uy * size + ux * size * 0.8)
    right = (mx - ux * size + uy * size * 0.8, my - uy * size - ux * size * 0.8)
    draw.polygon([tip, left, right], fill=col)


def construction(glyph, side, pad=14):
    """The letter's contours, vertices, directions and narrowest cuts."""
    scale = side / (kit.BOX + 2 * pad)
    d = sdf(glyph, scale, pad, side)
    a = coverage(d, scale)[..., None]
    fill = np.array([0.16, 0.30, 0.31])
    base = np.array(BG) / 255 * (1 - a) + fill * a
    img = Image.fromarray((base * 255).astype(np.uint8), 'RGB')
    draw = ImageDraw.Draw(img)

    def P(p):
        return ((p[0] + pad) * scale, (kit.BOX + pad - p[1]) * scale)

    # The original's 18 x 15 grid, faintly, and the box.
    for i in range(19):
        x = P((i * kit.COLUMN, 0))[0]
        draw.line([(x, P((0, 0))[1]), (x, P((0, kit.BOX))[1])], fill=(30, 36, 40))
    for j in range(16):
        y = P((0, j * kit.ROW))[1]
        draw.line([(P((0, 0))[0], y), (P((kit.BOX, 0))[0], y)], fill=(30, 36, 40))
    draw.rectangle([P((0, kit.BOX)), P((kit.BOX, 0))], outline=(70, 80, 88))
    for kind, pts in glyph.contours:
        col = (110, 220, 255) if kind == 'outer' else (255, 170, 60)
        sp = [P(p) for p in pts]
        draw.line(sp + [sp[0]], fill=col, width=max(2, int(scale)))
        for i in range(len(sp)):
            arrow(draw, sp[i], sp[(i + 1) % len(sp)], col, max(4, scale * 3))
        for p in sp:
            r = max(2.5, scale * 1.1)
            draw.ellipse([p[0] - r, p[1] - r, p[0] + r, p[1] + r], fill=(245, 245, 245))
    widths = kit.feature_widths(glyph.contours)
    f = font(max(12, int(side / 26)))
    labels = []
    for name, col in (('ink', (255, 225, 60)), ('counter', (255, 70, 200))):
        w, p, q = widths[name]
        if p is None:
            labels.append((f'{name}: none', col))
            continue
        sp, sq = P(p), P(q)
        draw.line([sp, sq], fill=col, width=max(3, int(scale * 1.5)))
        ux, uy = sq[0] - sp[0], sq[1] - sp[1]
        length = math.hypot(ux, uy)
        nx, ny = -uy / length * 7, ux / length * 7
        for e in (sp, sq):
            draw.line([(e[0] - nx, e[1] - ny), (e[0] + nx, e[1] + ny)], fill=col, width=3)
        labels.append((f'{name} {w:.1f}', col))
    return img, labels


def trug_views(path):
    """The original's ten front views from the local type study, by letter,
    or {} when it is not given or not laid out as expected."""
    if not path:
        return {}
    study = Image.open(path).convert('RGB')
    if study.size != TRUG_SIZE:
        print(f'{path}: expected a {TRUG_SIZE} type study; skipping the comparison')
        return {}
    views = {}
    for i, ch in enumerate(TRUG_ORDER):
        x = TRUG_X0 + i * TRUG_PITCH
        views[ch] = study.crop((x, TRUG_Y0, x + TRUG_SIDE, TRUG_Y0 + TRUG_SIDE))
    return views


def letters_sheet(trug):
    big, small, gap = 470, 225, 20
    cell_w, cell_h = big + gap, 60 + big + 34 + small + 40
    sheet = Image.new('RGB', (5 * cell_w + gap, 2 * cell_h + 120), BG)
    draw = ImageDraw.Draw(sheet)
    title = font(26)
    body = font(17)
    draw.text((gap, 18), "Part 7's greetings letters, authored for the remake (not traced from "
              "the original or any font)", fill=(235, 235, 235), font=title)
    draw.text((gap, 54), f'Box {kit.BOX} x {kit.BOX} units (18 columns x 15 rows of the original at '
              f'{kit.COLUMN} and {kit.ROW} units), y up, baseline 0, advance {kit.ADVANCE}. '
              f'Outer contours counter-clockwise (blue), holes clockwise (orange). Narrowest ink '
              f'(yellow, at least {kit.MIN_INK}) and counter (pink, at least {kit.MIN_COUNTER}) marked.',
              fill=(170, 175, 180), font=body)
    for i, ch in enumerate('ACEGINORST'):
        g = LETTERS[ch]
        x0 = gap + (i % 5) * cell_w
        y0 = 110 + (i // 5) * cell_h
        img, labels = construction(g, big)
        draw.text((x0, y0), f'{ch}: {g.label}', fill=(235, 235, 235), font=font(20))
        sheet.paste(img, (x0, y0 + 30))
        lx = x0
        for text, col in labels:
            draw.text((lx, y0 + 34 + big), text, fill=col, font=body)
            lx += 150
        n = sum(len(p) for _, p in g.contours)
        draw.text((lx, y0 + 34 + big), f'{n} vertices', fill=(150, 155, 160), font=body)
        ys = y0 + 60 + big
        scale = small / (kit.BOX + 28)
        shaded = bevel_shade(sdf(g, scale, 14, small), scale)
        sheet.paste(shaded, (x0, ys))
        draw.text((x0, ys + small + 4), 'ours, bevel-shaded', fill=(150, 155, 160), font=body)
        if ch in trug:
            view = trug[ch].resize((small - 28, small - 28), Image.NEAREST)
            frame = Image.new('RGB', (small, small), BG)
            frame.paste(view, (14, 14))
            sheet.paste(frame, (x0 + small + gap, ys))
            draw.text((x0 + small + gap, ys + small + 4), "original front view (comparison)",
                      fill=(150, 155, 160), font=body)
    return sheet


def word_tiles(scale):
    tiles = {}
    for ch, g in LETTERS.items():
        tiles[ch] = bevel_shade(sdf(g, scale, 0, int(round(kit.BOX * scale))), scale)
    return tiles


def words_sheet():
    scale = 0.5  # pixels a unit
    tiles = word_tiles(scale)
    side = tiles['A'].size[0]
    # Part 7: the 320 x 200 screen at 4:3 is 3200 x 2400 units.
    screen_w, screen_h = int(3200 * scale), int(2400 * scale)
    pitch = kit.PITCH * scale
    line_w = int((11 * kit.ADVANCE - (kit.ADVANCE - kit.BOX)) * scale)
    margin = 40
    sheet = Image.new('RGB', (margin * 3 + screen_w + line_w, screen_h + 140), BG)
    draw = ImageDraw.Draw(sheet)
    draw.text((margin, 16), "Part 7's five greetings, stacked one letter per 18-row slot in "
              'the five strips (all shown whole), and set in lines', fill=(235, 235, 235), font=font(24))
    sx, sy = margin, 80
    draw.rectangle([sx - 1, sy - 1, sx + screen_w, sy + screen_h], outline=(50, 56, 62))
    for s, word in enumerate(WORDS):
        x = sx + int((STRIP_X[s] + FACE_FIRST_COLUMN) * kit.COLUMN * scale)
        for j, ch in enumerate(word):
            sheet.paste(tiles[ch], (x, sy + int(j * pitch) + 12))
    draw.text((sx, sy + screen_h + 10), 'the 320 x 200 screen at 4:3 (strips 64 columns apart)',
              fill=(150, 155, 160), font=font(16))
    lx = sx + screen_w + margin
    for j, word in enumerate(SET_WORDS):
        y = sy + 12 + int(j * (kit.BOX + 120) * scale)
        for i, ch in enumerate(word):
            if ch != ' ':
                sheet.paste(tiles[ch], (lx + int(i * kit.ADVANCE * scale), y))
    draw.text((lx, sy + screen_h + 10), f'set in lines, advance {kit.ADVANCE}',
              fill=(150, 155, 160), font=font(16))
    return sheet


def solid_view(glyph, width, height, yaw, pitch, depth=90.0, bevel=8.0, light=(-0.85, 0.35, 0.40)):
    """Sphere-trace the letter extruded `depth` units with a 45-degree
    bevel of `bevel` on its front and back edges, turned `yaw` degrees
    about the upright and `pitch` degrees toward the camera, lit by one
    light from the side (`light`, toward the light, in view space) with a
    faint fill, and its shadow."""
    pad = 24.0
    res = 2.0  # samples a unit
    grid = sdf(glyph, res, pad)
    n = grid.shape[0]

    def q2(x, y):
        # Bilinear lookup in units; y up. Outside the grid, the distance
        # to the grid plus its edge value.
        gx = (x + pad) * res - 0.5
        gy = (kit.BOX + pad - y) * res - 0.5
        cx = np.clip(gx, 0, n - 1.001)
        cy = np.clip(gy, 0, n - 1.001)
        ix, iy = cx.astype(int), cy.astype(int)
        fx, fy = cx - ix, cy - iy
        v = (grid[iy, ix] * (1 - fx) * (1 - fy) + grid[iy, ix + 1] * fx * (1 - fy)
             + grid[iy + 1, ix] * (1 - fx) * fy + grid[iy + 1, ix + 1] * fx * fy)
        out = np.hypot(gx - cx, gy - cy) / res
        return v + out

    half = depth / 2

    def field(p):
        x, y, z = p[..., 0] + 90, p[..., 1] + 90, p[..., 2]
        q = q2(x, y)
        w = np.abs(z) - half
        return np.maximum(np.maximum(q, w), (q + w + bevel) / math.sqrt(2))

    # Camera: the letter's centre at the origin, turned by yaw and pitch.
    cy_, sy_ = math.cos(math.radians(yaw)), math.sin(math.radians(yaw))
    cp, sp = math.cos(math.radians(pitch)), math.sin(math.radians(pitch))
    rot = np.array([[cy_, 0, sy_], [0, 1, 0], [-sy_, 0, cy_]]) @ \
        np.array([[1, 0, 0], [0, cp, -sp], [0, sp, cp]])
    eye = rot @ np.array([0, 0, 520.0])
    fov = math.radians(32)
    u = (np.arange(width) + 0.5) / width * 2 - 1
    v = 1 - (np.arange(height) + 0.5) / height * 2
    U, V = np.meshgrid(u * math.tan(fov / 2) * width / height, v * math.tan(fov / 2))
    dirs = np.stack([U, V, -np.ones_like(U)], -1)
    dirs = dirs @ rot.T
    dirs /= np.linalg.norm(dirs, axis=-1, keepdims=True)
    t = np.full(U.shape, 300.0)
    hit = np.zeros(U.shape, bool)
    live = np.ones(U.shape, bool)
    for _ in range(220):
        p = eye + dirs * t[..., None]
        d = field(p)
        hit |= live & (d < 0.02)
        live &= ~hit & (t < 760)
        if not live.any():
            break
        t = np.where(live, t + np.maximum(d, 0.05), t)
    p = eye + dirs * t[..., None]
    e = 0.25
    grad = np.stack([field(p + np.array(o)) - field(p - np.array(o))
                     for o in ([e, 0, 0], [0, e, 0], [0, 0, e])], -1)
    grad = np.where(hit[..., None], grad, [0, 0, 1])
    nrm = grad / np.maximum(np.linalg.norm(grad, axis=-1, keepdims=True), 1e-9)
    L = rot @ np.array(light)
    L = L / np.linalg.norm(L)
    lam = np.clip((nrm * L).sum(-1), 0, 1)
    # Shadow: march toward the light.
    s = np.full(U.shape, 1.5)
    shade = np.ones(U.shape)
    origin = np.where(hit[..., None], p + nrm * 0.6, eye)
    for _ in range(90):
        d = field(origin + L * s[..., None])
        shade = np.minimum(shade, np.clip(8 * d / s, 0, 1))
        s = s + np.clip(d, 0.4, 20)
    shade = np.where(hit, shade, 1)
    h = L - dirs
    h /= np.linalg.norm(h, axis=-1, keepdims=True)
    spec = np.clip((nrm * h).sum(-1), 0, 1) ** 60
    fill = np.clip((nrm * (rot @ np.array([0.5, 0.2, 0.8]))).sum(-1), 0, 1)
    base = np.array([0.72, 0.88, 0.88])
    col = (base * (0.05 + 0.85 * (lam * shade)[..., None] + 0.12 * fill[..., None])
           + 0.6 * (spec * shade)[..., None])
    col = 1 - np.exp(-col * 1.4)
    out = np.where(hit[..., None], col, np.array(BG) / 255)
    return Image.fromarray((np.clip(out, 0, 1) * 255).astype(np.uint8), 'RGB')


def solid_sheet(chars='GRSA'):
    w, h, gap = 520, 520, 16
    sheet = Image.new('RGB', (len(chars) * (w + gap) + gap, 2 * (h + gap) + 90), BG)
    draw = ImageDraw.Draw(sheet)
    draw.text((gap, 14), 'G, R, S and A extruded 90 units with a 45-degree bevel of 8 on the '
              'front and back edges, lit from the left (CPU sphere tracing)', fill=(235, 235, 235),
              font=font(22))
    draw.text((gap, 46), 'Top: turned 28 degrees, tipped 12 toward the camera. Bottom: turned '
              '-40 degrees, tipped -10.', fill=(150, 155, 160), font=font(16))
    for i, ch in enumerate(chars):
        for row, (yaw, pitch) in enumerate(((28, 12), (-40, -10))):
            img = solid_view(LETTERS[ch], w, h, yaw, pitch)
            sheet.paste(img, (gap + i * (w + gap), 80 + row * (h + gap)))
    return sheet


def main():
    parser = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    parser.add_argument('sheet', nargs='?', default='all', choices=['letters', 'words', 'solid', 'all'])
    parser.add_argument('--trug', help="local type-study PNG of the original's front views")
    parser.add_argument('--out', default=str(ROOT / 'artifacts' / 'letters'))
    args = parser.parse_args()
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    jobs = {
        'letters': lambda: letters_sheet(trug_views(args.trug)),
        'words': words_sheet,
        'solid': solid_sheet,
    }
    for name, job in jobs.items():
        if args.sheet in (name, 'all'):
            path = out / f'{name}.png'
            job().save(path)
            print(path)


if __name__ == '__main__':
    main()
