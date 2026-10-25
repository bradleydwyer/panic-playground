#!/usr/bin/env python3
"""Render panic-hd at set times and compare the frames with an earlier set.

A local check for shader work: render a set of shots from a build you trust,
then render the same shots from the current build and compare them pixel by
pixel. Sets and reports are written under artifacts/look-check/ and stay out
of Git. Only the standard library is used: PNGs are read with zlib, so this
handles 8-bit, non-interlaced greyscale, RGB and RGBA images, which covers
panic-hd's captures.

    python3 tools/look_check.py render baseline   # before editing
    python3 tools/look_check.py check baseline    # after rebuilding panic-hd

`--bin` renders with any other build, such as an older copy kept for
comparison. The shot times and flags are authored test points chosen to
cover each look; they aren't cues read from the original.
"""
from concurrent.futures import ProcessPoolExecutor
from datetime import datetime
from fnmatch import fnmatchcase
from itertools import accumulate
from operator import add, sub
from pathlib import Path
from typing import NamedTuple, Tuple
import argparse
import hashlib
import json
import math
import os
import re
import shlex
import shutil
import struct
import subprocess
import sys
import tempfile
import time
import zlib

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DIR = ROOT / 'artifacts' / 'look-check'
DEFAULT_BIN = ROOT / 'target' / 'release' / 'panic-hd'
DEFAULT_DATA = ROOT / 'artifacts' / 'local-data'
DEFAULT_SIZE = (960, 540)
# Differences are in 1/255 steps of a colour channel. Shader compiler
# reordering can move a handful of pixels by a few steps; a shot counts as
# noise while only a few stray pixels differ, few of them go beyond
# TOLERANCE and the mean stays tiny. A faint change over a broad area is a
# change, however small its steps.
TOLERANCE = 2
MAX_DIFFERING = 0.1  # percent of pixels that differ at all
MAX_CHANGED = 0.01  # percent of pixels beyond the tolerance
MAX_MEAN = 0.25  # mean RGB difference over the frame
# A shot the new set skipped or failed to render counts as a failure too.
FAILING = ('changed', 'size', 'failed', 'skipped')


class LookError(Exception):
    pass


class Shot(NamedTuple):
    name: str
    group: str
    seek: float
    flags: Tuple[str, ...]


def _shot(prefix, group, seek, *flags):
    # Every shot plays the whole show (--scroller) unless it says otherwise.
    return Shot(f'{prefix}-{seek:g}', group, seek, flags or ('--scroller',))


SHOTS = (
    _shot('intro', 'intro', 5.5),  # distant glass letters
    _shot('intro', 'intro', 6.2),  # close pass
    _shot('intro', 'intro', 6.8),
    _shot('intro', 'intro', 7.6),  # close pass
    _shot('intro', 'intro', 8.0),  # inside a letter's pass
    _shot('intro', 'intro', 8.6),
    _shot('plate', 'plate', 13.8),  # warp between the closing doors
    _shot('plate', 'plate', 14.6),  # logo arriving on the plate
    _shot('logo', 'logo', 16.0),
    _shot('laser', 'laser', 18.6),
    _shot('laser', 'laser', 19.5),
    _shot('transition', 'laser', 19.5, '--transition'),  # plain backdrop
    _shot('scroller', 'scroller', 22.0),
    _shot('scroller', 'scroller', 27.0),
    _shot('scroller', 'scroller', 31.0),
    _shot('scroller', 'scroller', 34.0),
    _shot('scroller', 'scroller', 37.0),
    _shot('chamber', 'variants', 27.0, '--scroller', '--scroller-backdrop', 'chamber'),
    _shot('wall', 'variants', 27.0, '--scroller', '--scroller-backdrop', 'wall'),
    _shot('musical', 'variants', 31.0, '--scroller', '--scroller-lighting', 'musical'),
    _shot('faithful-intro', 'faithful', 7.6, '--scroller', '--faithful'),
    _shot('faithful-scroller', 'faithful', 31.0, '--scroller', '--faithful'),
    _shot('original', 'original', 31.0, '--scroller', '--original'),
    # Part 6 (--rings): the first symbol on black, the rings fading in, rings
    # and symbols, the fade out, symbols on black again and the PSTART fade.
    _shot('rings', 'rings', 43.5, '--rings'),
    _shot('rings', 'rings', 50.8, '--rings'),
    _shot('rings', 'rings', 57.2, '--rings'),
    _shot('rings', 'rings', 66.0, '--rings'),
    _shot('rings', 'rings', 73.4, '--rings'),
    _shot('rings', 'rings', 80.8, '--rings'),
    _shot('faithful-rings', 'faithful', 57.2, '--rings', '--faithful'),
    _shot('original-rings', 'original', 57.2, '--rings', '--original'),
    # Part 5 (--field): the zoom early and late, the mountain rising in front
    # of the still zoom, the orbit with the window cleared, the sink and the
    # dark window at the end.
    _shot('field', 'field', 84.0, '--field'),
    _shot('field', 'field', 94.5, '--field'),
    _shot('field', 'field', 97.6, '--field'),
    _shot('field', 'field', 105.0, '--field'),
    _shot('field', 'field', 125.0, '--field'),
    _shot('field', 'field', 129.5, '--field'),
    _shot('faithful-field', 'faithful', 89.0, '--field', '--faithful'),
    _shot('faithful-field', 'faithful', 97.6, '--field', '--faithful'),
    _shot('faithful-field', 'faithful', 105.0, '--field', '--faithful'),
    _shot('faithful-field', 'faithful', 125.0, '--field', '--faithful'),
    _shot('original-field', 'original', 105.0, '--field', '--original'),
    # Part 3 (--dots): part 5's frame held, the zoom into its window, the
    # shade bobs, the sphere, a morph, the cube edges, a music wave on the
    # vase, the pyramid, the box landed and held, its flare and part 4's
    # slab.
    _shot('dots', 'dots', 131.0, '--dots'),
    _shot('dots', 'dots', 133.5, '--dots'),
    _shot('dots', 'dots', 136.5, '--dots'),
    _shot('dots', 'dots', 141.8, '--dots'),
    _shot('dots', 'dots', 145.7, '--dots'),
    _shot('dots', 'dots', 146.9, '--dots'),
    _shot('dots', 'dots', 151.4, '--dots'),
    _shot('dots', 'dots', 168.9, '--dots'),
    _shot('dots', 'dots', 177.5, '--dots'),
    _shot('dots', 'dots', 179.8, '--dots'),
    _shot('dots', 'dots', 179.96, '--dots'),
    _shot('faithful-dots', 'faithful', 133.5, '--dots', '--faithful'),
    _shot('faithful-dots', 'faithful', 136.5, '--dots', '--faithful'),
    _shot('faithful-dots', 'faithful', 151.4, '--dots', '--faithful'),
    _shot('faithful-dots', 'faithful', 179.96, '--dots', '--faithful'),
    _shot('original-dots', 'original', 151.4, '--dots', '--original'),
    # Part 4 (--plasma): the handover from part 3's flare, the slab coming
    # out of the white, the slab alone, the plasma fading in, its light on
    # the stone at full level, fading out, the slab sliding away and part
    # 7's first frame.
    _shot('plasma', 'plasma', 180.1, '--plasma'),
    _shot('plasma', 'plasma', 180.6, '--plasma'),
    _shot('plasma', 'plasma', 184.0, '--plasma'),
    _shot('plasma', 'plasma', 189.0, '--plasma'),
    _shot('plasma', 'plasma', 197.0, '--plasma'),
    _shot('plasma', 'plasma', 214.0, '--plasma'),
    _shot('plasma', 'plasma', 221.0, '--plasma'),
    _shot('plasma', 'plasma', 224.8, '--plasma'),
    _shot('plasma', 'plasma', 225.43, '--plasma'),
    _shot('faithful-plasma', 'faithful', 180.3, '--plasma', '--faithful'),
    _shot('faithful-plasma', 'faithful', 197.0, '--plasma', '--faithful'),
    _shot('faithful-plasma', 'faithful', 224.8, '--plasma', '--faithful'),
    _shot('original-plasma', 'original', 197.0, '--plasma', '--original'),
)
CAPTURE_FLAGS = ('--seek', '--headless', '--size', '--data', '--out')


def select(items, patterns):
    """Items (with name and group) matching any glob pattern; all without patterns."""
    if not patterns:
        return list(items)
    def hits(item, pattern):
        return fnmatchcase(item.name, pattern) or fnmatchcase(item.group, pattern)
    unmatched = [p for p in patterns if not any(hits(i, p) for i in items)]
    if unmatched:
        raise LookError(f'No shot or group matches {", ".join(unmatched)}; see the list command')
    return [i for i in items if any(hits(i, p) for p in patterns)]


# PNG reading and writing ----------------------------------------------------

PNG_SIGNATURE = b'\x89PNG\r\n\x1a\n'
CHANNELS = {0: 1, 2: 3, 4: 2, 6: 4}
_BYTE = (255).__and__


class Image(NamedTuple):
    width: int
    height: int
    rows: list  # RGBA bytes per row


def _sub(line, bpp):
    out = bytearray(len(line))
    for c in range(bpp):
        out[c::bpp] = bytes(map(_BYTE, accumulate(line[c::bpp])))
    return bytes(out)


def _average(line, prior, bpp):
    out = bytearray(len(line))
    for c in range(bpp):
        left, lane = 0, []
        for f, up in zip(line[c::bpp], prior[c::bpp]):
            left = (f + ((left + up) >> 1)) & 255
            lane.append(left)
        out[c::bpp] = bytes(lane)
    return bytes(out)


def _paeth(line, prior, bpp):
    out = bytearray(len(line))
    for c in range(bpp):
        ups = prior[c::bpp]
        left, lane = 0, []
        for f, b, cc in zip(line[c::bpp], ups, b'\0' + ups[:-1]):
            pa, pb = b - cc, left - cc
            pc = pa + pb
            pa = pa if pa >= 0 else -pa
            pb = pb if pb >= 0 else -pb
            pc = pc if pc >= 0 else -pc
            if pa <= pb and pa <= pc:
                p = left
            elif pb <= pc:
                p = b
            else:
                p = cc
            left = (f + p) & 255
            lane.append(left)
        out[c::bpp] = bytes(lane)
    return bytes(out)


def _rgba(row, channels, width):
    if channels == 4:
        return row
    out = bytearray(b'\xff' * (width * 4))
    if channels == 3:
        for c in range(3):
            out[c::4] = row[c::3]
    else:
        grey = row[0::channels]
        out[0::4] = out[1::4] = out[2::4] = grey
        if channels == 2:
            out[3::4] = row[1::2]
    return bytes(out)


def decode_png(data):
    """Decode an 8-bit, non-interlaced greyscale/RGB(A) PNG into RGBA rows."""
    if not data.startswith(PNG_SIGNATURE):
        raise ValueError('not a PNG file')
    pos, header, idat = 8, None, []
    while pos + 8 <= len(data):
        length, kind = struct.unpack_from('>I4s', data, pos)
        body = data[pos + 8:pos + 8 + length]
        if len(body) != length:
            raise ValueError('truncated PNG chunk')
        if kind == b'IHDR':
            header = struct.unpack('>IIBBBBB', body)
        elif kind == b'IDAT':
            idat.append(body)
        elif kind == b'IEND':
            break
        pos += 12 + length
    if header is None or not idat:
        raise ValueError('PNG without IHDR or IDAT')
    width, height, depth, color, _, _, interlace = header
    if depth != 8 or color not in CHANNELS or interlace:
        raise ValueError(f'unsupported PNG: bit depth {depth}, colour type {color}, '
                         f'interlace {interlace}; expected 8-bit, non-interlaced grey/RGB(A)')
    bpp = CHANNELS[color]
    stride = width * bpp
    raw = zlib.decompress(b''.join(idat))
    if len(raw) != height * (stride + 1):
        raise ValueError('PNG image data has the wrong length')
    rows, prior = [], bytes(stride)
    for y in range(height):
        kind = raw[y * (stride + 1)]
        line = raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)]
        if kind == 0:
            row = line
        elif kind == 1:
            row = _sub(line, bpp)
        elif kind == 2:
            row = bytes(map(_BYTE, map(add, line, prior)))
        elif kind == 3:
            row = _average(line, prior, bpp)
        elif kind == 4:
            row = _paeth(line, prior, bpp)
        else:
            raise ValueError(f'unknown PNG filter {kind}')
        rows.append(row)
        prior = row
    return Image(width, height, [_rgba(r, bpp, width) for r in rows])


def read_png(path):
    return decode_png(Path(path).read_bytes())


def encode_png(width, height, rows, channels=3):
    """Encode rows of 8-bit pixels (1 to 4 channels) without filtering."""
    color = {1: 0, 2: 4, 3: 2, 4: 6}[channels]
    if len(rows) != height or any(len(r) != width * channels for r in rows):
        raise ValueError('rows do not match the image size')

    def chunk(kind, body):
        return (struct.pack('>I', len(body)) + kind + body
                + struct.pack('>I', zlib.crc32(kind + body) & 0xffffffff))
    raw = b''.join(b'\0' + bytes(r) for r in rows)
    return (PNG_SIGNATURE
            + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, color, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(raw, 6)) + chunk(b'IEND', b''))


def write_png(path, width, height, rows, channels=3):
    Path(path).write_bytes(encode_png(width, height, rows, channels))


# Comparison -----------------------------------------------------------------

class Diff(NamedTuple):
    rows: list  # largest channel difference per pixel, one byte each
    peak: int  # largest channel difference anywhere
    total: int  # sum of RGB differences
    beyond: int  # pixels with a channel beyond the tolerance
    differing: int  # pixels with any difference


def diff_images(a, b, tolerance=TOLERANCE):
    if (a.width, a.height) != (b.width, b.height):
        raise ValueError(f'sizes differ: {a.width}x{a.height} and {b.width}x{b.height}')
    zero = bytes(a.width)
    rows, peak, total, beyond, differing = [], 0, 0, 0, 0
    over = int(tolerance).__lt__
    for ra, rb in zip(a.rows, b.rows):
        if ra == rb:
            rows.append(zero)
            continue
        chans = [list(map(abs, map(sub, ra[c::4], rb[c::4]))) for c in range(4)]
        total += sum(chans[0]) + sum(chans[1]) + sum(chans[2])
        d = bytes(map(max, *chans))
        peak = max(peak, max(d))
        beyond += sum(map(over, d))
        differing += a.width - d.count(0)
        rows.append(d)
    return Diff(rows, peak, total, beyond, differing)


def classify(diff, pixels, max_changed=MAX_CHANGED, max_mean=MAX_MEAN,
             max_differing=MAX_DIFFERING):
    if diff.peak == 0:
        return 'identical'
    if (diff.differing <= pixels * max_differing / 100
            and diff.beyond <= pixels * max_changed / 100
            and diff.total / (3 * pixels) <= max_mean):
        return 'noise'
    return 'changed'


def shrink(rows, width, channels, k, mode='mean'):
    """Downscale by an integer factor: box mean, or max to keep lone pixels."""
    if k <= 1:
        return list(rows), width
    out_w = width // k
    span = out_w * k * channels
    out = []
    combine = max if mode == 'max' else add
    for y in range(0, len(rows) - k + 1, k):
        acc = rows[y][:span]
        for row in rows[y + 1:y + k]:
            acc = list(map(combine, acc, row[:span]))
        line = bytearray(out_w * channels)
        for c in range(channels):
            lanes = [acc[c + channels * j::channels * k] for j in range(k)]
            v = lanes[0]
            for lane in lanes[1:]:
                v = list(map(combine, v, lane))
            if mode != 'max':
                v = map((k * k).__rfloordiv__, v)
            line[c::channels] = bytes(v)
        out.append(bytes(line))
    return out, out_w


def _heat_lut(tolerance):
    lut = [bytes(3)] * 256
    for d in range(1, 256):
        if d <= tolerance:
            lut[d] = bytes((40, 70, 170))  # within tolerance: dim blue
            continue
        t = min(1.0, math.log2(d - tolerance) / 6)  # red, yellow, then white from 64 up
        if t < 0.5:
            rgb = (200 + 110 * t, 440 * t, 0)
        else:
            rgb = (255, 220 + 70 * (t - 0.5), 510 * (t - 0.5))
        lut[d] = bytes(min(255, int(v)) for v in rgb)
    return lut


_GREY = [bytes((v * 3 // 10,) * 3) for v in range(256)]


def heatmap(diff_rows, after_rows, tolerance=TOLERANCE):
    """RGB rows: differences in colour over a dim grey copy of the new frame."""
    heat, out = _heat_lut(tolerance), []
    quarter = (2).__rrshift__
    for d, row in zip(diff_rows, after_rows):
        r, g, b = row[0::4], row[1::4], row[2::4]
        luma = map(quarter, map(add, map(add, r, g), map(add, g, b)))
        if d.count(0) == len(d):
            out.append(b''.join(map(_GREY.__getitem__, luma)))
        else:
            out.append(b''.join([heat[x] if x else _GREY[y] for x, y in zip(d, luma)]))
    return out


def _rgb(rows):
    out = []
    for row in rows:
        line = bytearray(len(row) // 4 * 3)
        for c in range(3):
            line[c::3] = row[c::4]
        out.append(bytes(line))
    return out


def compare_shot(job):
    """Compare one pair of captures; runs in a worker process."""
    before, after = read_png(job['before']), read_png(job['after'])
    result = {'name': job['name']}
    if (before.width, before.height) != (after.width, after.height):
        result.update(status='size', detail=f'{before.width}x{before.height} vs '
                                            f'{after.width}x{after.height}')
        return result
    pixels = before.width * before.height
    diff = diff_images(before, after, job['tolerance'])
    status = classify(diff, pixels, job['max_changed'], job['max_mean'], job['max_differing'])
    result.update(status=status, max=diff.peak, mean=diff.total / (3 * pixels),
                  beyond=diff.beyond, differing=diff.differing, pixels=pixels)
    if status == 'identical':
        return result
    write_png(job['heatmap'], after.width, after.height,
              heatmap(diff.rows, after.rows, job['tolerance']))
    result['heatmap'] = job['heatmap']
    if status == 'changed':
        k = max(1, math.ceil(after.width / job['panel_width']))
        small_before, w = shrink(before.rows, before.width, 4, k)
        small_after, _ = shrink(after.rows, after.width, 4, k)
        small_diff, _ = shrink(diff.rows, after.width, 1, k, 'max')
        result['panels'] = (w, len(small_after), [
            _rgb(small_before), _rgb(small_after),
            heatmap(small_diff, small_after, job['tolerance'])])
    return result


# Contact sheet ----------------------------------------------------------------

# 5 x 7 capitals, digits and punctuation; one hex byte per row, bit 4 leftmost.
_FONT = {
    'A': '0e11111f111111', 'B': '1e11111e11111e', 'C': '0e11101010110e', 'D': '1e11111111111e',
    'E': '1f10101e10101f', 'F': '1f10101e101010', 'G': '0e11101711110f',
    'H': '1111111f111111', 'I': '0e04040404040e', 'J': '0702020202120c', 'K': '11121418141211',
    'L': '1010101010101f', 'M': '111b1515111111', 'N': '11111915131111', 'O': '0e11111111110e',
    'P': '1e11111e101010', 'Q': '0e11111115120d', 'R': '1e11111e141211', 'S': '0f10100e01011e',
    'T': '1f040404040404', 'U': '1111111111110e', 'V': '11111111110a04',
    'W': '1111111515150a', 'X': '11110a040a1111', 'Y': '1111110a040404', 'Z': '1f01020408101f',
    '0': '0e11131519110e', '1': '040c040404040e', '2': '0e11010204081f', '3': '1f02040201110e',
    '4': '02060a121f0202', '5': '1f101e0101110e', '6': '0608101e11110e', '7': '1f010204080808',
    '8': '0e11110e11110e', '9': '0e11110f01020c', '.': '00000000000c0c', '-': '0000001f000000',
    '_': '0000000000001f', ':': '000c0c000c0c00', '/': '00010204081000', '%': '18190204081303',
    '(': '02040808080402', ')': '08040202020408', '+': '0004041f040400', ',': '000000000c0408',
    '=': '00001f001f0000', '|': '04040404040404', ' ': '00000000000000', '?': '0e110102040004',
}
_GLYPHS = {ch: bytes.fromhex(rows) for ch, rows in _FONT.items()}


def draw_text(canvas, width, x, y, text, color, scale=2, limit=None):
    for ch in text.upper():
        if x + 5 * scale > (limit or width):
            break
        for gy, bits in enumerate(_GLYPHS.get(ch, _GLYPHS['?'])):
            for gx in range(5):
                if bits >> (4 - gx) & 1:
                    for sy in range(scale):
                        at = ((y + gy * scale + sy) * width + x + gx * scale) * 3
                        canvas[at:at + 3 * scale] = color * scale
        x += 6 * scale


def contact_sheet(titles, rows):
    """An RGB sheet: a title per column, then a label and three panels per row."""
    w, h = rows[0][1][0], rows[0][1][1]
    margin, gap, strip = 10, 8, 24
    width = 2 * margin + 3 * w + 2 * gap
    height = 2 * margin + strip + len(rows) * (strip + h + gap) - gap
    canvas = bytearray(bytes((18, 18, 20)) * (width * height))
    for j, title in enumerate(titles):
        left = margin + j * (w + gap)
        draw_text(canvas, width, left, margin + 4, title, bytes((235, 235, 235)), limit=left + w)
    for i, (label, (_, _, panels)) in enumerate(rows):
        top = margin + strip + i * (strip + h + gap)
        draw_text(canvas, width, margin, top + 4, label, bytes((255, 200, 90)))
        for j, panel in enumerate(panels):
            left = margin + j * (w + gap)
            for y, line in enumerate(panel):
                at = ((top + strip + y) * width + left) * 3
                canvas[at:at + len(line)] = line
    return width, height, [bytes(canvas[y * width * 3:(y + 1) * width * 3]) for y in range(height)]


# Rendering ------------------------------------------------------------------

def _git(*args):
    try:
        return subprocess.run(['git', '-C', str(ROOT), *args], capture_output=True,
                              text=True, timeout=30).stdout.strip() or None
    except (OSError, subprocess.SubprocessError):
        return None


def _sha256(path):
    digest = hashlib.sha256()
    with open(path, 'rb') as handle:
        for block in iter(lambda: handle.read(1 << 20), b''):
            digest.update(block)
    return digest.hexdigest()


def _set_path(ref, sets_dir):
    if os.sep in ref or '/' in ref or ref.startswith('.'):
        return Path(ref).resolve()
    if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9._-]*', ref):
        raise LookError(f'Invalid set name {ref!r}: use letters, digits, ".", "_" and "-"')
    return (Path(sets_dir) / ref).resolve()


def _is_set(path):
    """Whether path holds a set this tool wrote, so it may be replaced."""
    try:
        manifest = json.loads((path / 'manifest.json').read_text())
    except (OSError, ValueError):
        return False
    return isinstance(manifest, dict) and 'format' in manifest


def load_set(ref, sets_dir):
    path = _set_path(ref, sets_dir)
    try:
        return path, json.loads((path / 'manifest.json').read_text())
    except FileNotFoundError:
        raise LookError(f'No set at {path}; render one first, e.g. '
                        f'python3 tools/look_check.py render {path.name}') from None


def help_flags(binary):
    """The --options the binary's --help mentions."""
    try:
        run = subprocess.run([str(binary), '--help'], capture_output=True, text=True, timeout=60)
    except OSError as error:
        raise LookError(f'Cannot run {binary}: {error}') from None
    if run.returncode != 0:
        raise LookError(f'{binary} --help failed: {(run.stderr or run.stdout).strip()}')
    return set(re.findall(r'(?<![\w-])--[a-z][a-z0-9-]*', run.stdout + run.stderr))


def _warn_if_stale(binary, log):
    try:
        built = binary.stat().st_mtime
        sources = [p for p in (ROOT / 'rust' / 'src').rglob('*') if p.is_file()]
    except OSError:
        return
    newer = [p for p in sources if p.stat().st_mtime > built]
    if newer and ROOT in binary.parents:
        log(f'Warning: {binary.relative_to(ROOT)} is older than {len(newer)} file(s) under '
            'rust/src; rebuild with:\n'
            '  cargo build --release --locked --features hires --bin panic-hd')


def render_set(name, shots, *, binary=DEFAULT_BIN, data=None, size=DEFAULT_SIZE, extra=(),
               sets_dir=DEFAULT_DIR / 'sets', force=False, timeout=300, log=print):
    """Render shots into a named set; returns (path, manifest)."""
    binary = Path(binary).resolve()
    if not binary.is_file():
        raise LookError(f'No panic-hd at {binary}; build it with cargo build --release '
                        '--locked --features hires --bin panic-hd, or pass --bin')
    data = Path(data or os.environ.get('PANIC_DATA') or DEFAULT_DATA).resolve()
    if not data.is_dir():
        raise LookError(f'No data directory at {data}; import it first (rust/README.md#setup) '
                        'or pass --data')
    target = _set_path(name, sets_dir)
    if target.exists():
        # Replacing deletes the old directory, so only ever replace a set.
        if not (target.is_dir() and (_is_set(target) or not any(target.iterdir()))):
            raise LookError(f'{target} is not a look check set (it has no manifest.json from '
                            'this tool); refusing to replace it')
        if not force:
            raise LookError(f'Set {target} already exists; use --force to replace it')
    flags = help_flags(binary)
    missing = [f for f in CAPTURE_FLAGS if f not in flags]
    if missing:
        raise LookError(f'{binary} has no {", ".join(missing)}; it cannot capture shots')
    _warn_if_stale(binary, log)
    extra = list(extra)
    target.parent.mkdir(parents=True, exist_ok=True)
    work = Path(tempfile.mkdtemp(prefix=f'.{target.name}.', dir=target.parent))
    manifest = {
        'format': 1, 'name': target.name, 'created': datetime.now().astimezone().isoformat(),
        'binary': str(binary), 'binary_sha256': _sha256(binary),
        'git': _git('describe', '--always', '--dirty', '--tags'),
        'branch': _git('rev-parse', '--abbrev-ref', 'HEAD'), 'worktree': str(ROOT),
        'data': str(data), 'size': list(size), 'extra': extra, 'adapter': None,
        'shots': [], 'skipped': [], 'failed': [],
    }
    started = time.monotonic()
    try:
        for index, shot in enumerate(shots, 1):
            unknown = [f for f in (*shot.flags, *extra) if f.startswith('--') and f not in flags]
            if unknown:
                manifest['skipped'].append({'name': shot.name, 'group': shot.group,
                                            'reason': f'not in --help: {" ".join(unknown)}'})
                continue
            out = work / f'{shot.name}.png'
            command = [str(binary), *shot.flags, *extra, '--seek', f'{shot.seek:g}', '--headless',
                       '--size', f'{size[0]}x{size[1]}', '--data', str(data), '--out', str(out)]
            if sys.stderr.isatty():
                print(f'\r{index}/{len(shots)} {shot.name:<24}', end='', file=sys.stderr,
                      flush=True)
            began = time.monotonic()
            try:
                run = subprocess.run(command, capture_output=True, text=True, timeout=timeout,
                                     cwd=ROOT)
                error = None if run.returncode == 0 and out.is_file() else (
                    (run.stderr or run.stdout).strip().splitlines() or ['no output'])[-1]
            except (OSError, subprocess.TimeoutExpired) as problem:
                run, error = None, str(problem)
            entry = {'name': shot.name, 'group': shot.group, 'seek': shot.seek,
                     'flags': list(shot.flags), 'command': [*command[:-1], str(target / out.name)]}
            if error:
                manifest['failed'].append({**entry, 'error': error})
                continue
            adapter = re.search(r' using (.+)$', run.stdout, re.M)
            if adapter and not manifest['adapter']:
                manifest['adapter'] = adapter.group(1).strip()
            manifest['shots'].append({**entry, 'file': out.name, 'sha256': _sha256(out),
                                      'seconds': round(time.monotonic() - began, 3)})
        if sys.stderr.isatty():
            print('\r' + ' ' * 40 + '\r', end='', file=sys.stderr, flush=True)
        manifest['seconds'] = round(time.monotonic() - started, 2)
        (work / 'manifest.json').write_text(json.dumps(manifest, indent=1) + '\n')
        if target.exists():
            old = Path(tempfile.mkdtemp(prefix=f'.{target.name}.old.', dir=target.parent))
            target.rename(old / 'set')
            work.rename(target)
            shutil.rmtree(old)
        else:
            work.rename(target)
    finally:
        if work.exists():
            shutil.rmtree(work)
    times = [s['seconds'] for s in manifest['shots']]
    summary = f'Rendered {len(times)} shots into {target} in {manifest["seconds"]:.1f} s'
    if times:
        summary += f' ({min(times):.2f} to {max(times):.2f} s each)'
    log(summary)
    for skipped in manifest['skipped']:
        log(f'  skipped {skipped["name"]}: {skipped["reason"]}')
    for failed in manifest['failed']:
        log(f'  FAILED {failed["name"]}: {failed["error"]}')
    return target, manifest


def manifest_shots(manifest):
    return [Shot(s['name'], s['group'], s['seek'], tuple(s['flags']))
            for s in manifest['shots'] + manifest.get('failed', [])]


# Comparing sets ---------------------------------------------------------------

class _Named(NamedTuple):
    name: str
    group: str


def compare_sets(base_ref, new_ref, *, sets_dir=DEFAULT_DIR / 'sets',
                 reports_dir=DEFAULT_DIR / 'reports', patterns=(), tolerance=TOLERANCE,
                 max_changed=MAX_CHANGED, max_mean=MAX_MEAN, max_differing=MAX_DIFFERING,
                 require_all=False, jobs=None, panel_width=640, log=print):
    """Compare two sets, write heatmaps and a sheet, and return (failed, report dir).

    With require_all, a base shot missing from the new set also fails, as
    check renders every base shot into it.
    """
    failing = FAILING + (('not rendered',) if require_all else ())
    base_dir, base = load_set(base_ref, sets_dir)
    new_dir, new = load_set(new_ref, sets_dir)
    report = Path(reports_dir) / f'{base_dir.name}--{new_dir.name}'
    (report / 'diff').mkdir(parents=True, exist_ok=True)
    for stale in [*(report / 'diff').glob('*.png'), report / 'sheet.png']:
        stale.unlink(missing_ok=True)
    base_shots = {s['name']: s for s in base['shots']}
    new_shots = {s['name']: s for s in new['shots']}
    new_failed = {s['name']: s for s in new.get('failed', [])}
    new_skipped = {s['name']: s for s in new.get('skipped', [])}
    groups = {s['name']: s['group'] for s in
              base['shots'] + new['shots'] + new.get('failed', []) + new.get('skipped', [])}
    order = list(dict.fromkeys([*base_shots, *new_shots, *new_failed]))
    order = [n.name for n in select([_Named(n, groups[n]) for n in order], patterns)]
    results, jobs_todo = {}, []
    for name in order:
        if name not in base_shots:
            results[name] = {'name': name, 'status': 'no base'}
        elif name in new_failed:
            results[name] = {'name': name, 'status': 'failed', 'detail': new_failed[name]['error']}
        elif name in new_skipped:
            results[name] = {'name': name, 'status': 'skipped',
                             'detail': new_skipped[name]['reason']}
        elif name not in new_shots:
            results[name] = {'name': name, 'status': 'not rendered'}
        elif base_shots[name]['sha256'] == new_shots[name]['sha256']:
            results[name] = {'name': name, 'status': 'identical', 'max': 0, 'mean': 0.0,
                             'beyond': 0, 'differing': 0}
        else:
            jobs_todo.append({'name': name, 'before': str(base_dir / base_shots[name]['file']),
                              'after': str(new_dir / new_shots[name]['file']),
                              'heatmap': str(report / 'diff' / f'{name}.png'),
                              'tolerance': tolerance, 'max_changed': max_changed,
                              'max_mean': max_mean, 'max_differing': max_differing,
                              'panel_width': panel_width})
    jobs = jobs or min(8, os.cpu_count() or 1)
    if jobs > 1 and len(jobs_todo) > 1:
        with ProcessPoolExecutor(min(jobs, len(jobs_todo))) as pool:
            done = list(pool.map(compare_shot, jobs_todo))
    else:
        done = [compare_shot(job) for job in jobs_todo]
    results.update((r['name'], r) for r in done)
    results = [results[name] for name in order]
    if not any(r['status'] in ('identical', 'noise', *failing) for r in results):
        raise LookError('Nothing to compare: the sets share no rendered shots')

    lines = []
    for label, path, m in (('base', base_dir, base), ('new', new_dir, new)):
        extra = f'  extra: {shlex.join(m["extra"])}' if m.get('extra') else ''
        lines.append(f'{label:<4} {path.name}: {m.get("git") or "?"}, {m["binary"]} '
                     f'(sha256 {m["binary_sha256"][:12]}), {m["size"][0]}x{m["size"][1]}, '
                     f'{m.get("adapter") or "unknown GPU"}{extra}')
    if base.get('adapter') != new.get('adapter'):
        lines.append('Note: the sets come from different GPUs; small differences are expected.')
    lines.append(f'{"shot":<22} {"status":<12} {"max":>4} {"mean":>7} {"differing":>16} '
                 f'{f"beyond {tolerance}/255":>16}')
    for r in results:
        if 'max' in r:
            share = 100 / r['pixels'] if r.get('pixels') else 0.0
            lines.append(f'{r["name"]:<22} {r["status"]:<12} {r["max"]:>4} {r["mean"]:>7.3f} '
                         f'{r["differing"]:>8} {r["differing"] * share:>6.2f}% '
                         f'{r["beyond"]:>8} {r["beyond"] * share:>6.2f}%')
        elif r['status'] not in ('not rendered', 'no base'):
            lines.append(f'{r["name"]:<22} {r["status"]:<12} {r.get("detail", "")}'.rstrip())
    for status, where in (('not rendered', f'Not rendered in {new_dir.name}'),
                          ('no base', f'Not in {base_dir.name}')):
        names = [r['name'] for r in results if r['status'] == status]
        if names:
            lines.append(f'{where}: {", ".join(names)}')
    counts = {}
    for r in results:
        counts[r['status']] = counts.get(r['status'], 0) + 1
    lines.append(f'{len(results)} shots: ' + ', '.join(f'{n} {s}' for s, n in counts.items())
                 + f'. Noise limits: {max_differing}% of pixels differing, {max_changed}% '
                 f'beyond {tolerance}/255, mean {max_mean}/255.')

    changed = [r for r in results if r.get('panels')]
    if changed:
        titles = (f'before: {base_dir.name}', f'after: {new_dir.name}',
                  f'diff, tolerance {tolerance}/255')
        rows = [(f'{r["name"]}  max {r["max"]}  mean {r["mean"]:.3f}  beyond '
                 f'{100 * r["beyond"] / r["pixels"]:.2f}%', r['panels']) for r in changed]
        write_png(report / 'sheet.png', *contact_sheet(titles, rows))
    if any('heatmap' in r for r in results):
        lines.append(f'Heatmaps: {report / "diff"}')
    if changed:
        lines.append(f'Sheet: {report / "sheet.png"}')
    text = '\n'.join(lines)
    (report / 'report.txt').write_text(text + '\n')
    plain = [{k: v for k, v in r.items() if k != 'panels'} for r in results]
    (report / 'report.json').write_text(json.dumps({
        'base': str(base_dir), 'new': str(new_dir), 'tolerance': tolerance,
        'max_differing_percent': max_differing, 'max_changed_percent': max_changed,
        'max_mean': max_mean, 'shots': plain}, indent=1) + '\n')
    log(text)
    failed = [r for r in results if r['status'] in failing]
    return failed, report


# Command line -----------------------------------------------------------------

def _size(text):
    match = re.fullmatch(r'(\d+)x(\d+)', text)
    if not match:
        raise argparse.ArgumentTypeError('use WIDTHxHEIGHT, e.g. 960x540')
    return int(match.group(1)), int(match.group(2))


def _patterns(values):
    return [p for value in values or () for p in value.split(',') if p]


def main(argv=None):
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument('--dir', type=Path, default=DEFAULT_DIR,
                        help='Where sets and reports go (default artifacts/look-check)')
    common.add_argument('--only', action='append', metavar='PATTERN',
                        help='Shot names or groups, as globs; repeat or separate with commas')
    rendering = argparse.ArgumentParser(add_help=False)
    rendering.add_argument('--bin', type=Path, default=DEFAULT_BIN,
                           help='panic-hd to render with (default target/release/panic-hd)')
    rendering.add_argument('--data', type=Path,
                           help='Imported data (default PANIC_DATA or artifacts/local-data)')
    rendering.add_argument('--size', type=_size, help='Capture size (default 960x540)')
    rendering.add_argument('--extra', metavar='ARGS',
                           help="Flags added to every shot, e.g. --extra='--wall-strength 0.8'; "
                                "check reuses the base set's unless given")
    rendering.add_argument('--timeout', type=float, default=300, help='Seconds per shot')
    comparing = argparse.ArgumentParser(add_help=False)
    comparing.add_argument('--tolerance', type=int, default=TOLERANCE,
                           help='Ignore channel differences up to this, in 1/255 '
                                f'(default {TOLERANCE})')
    comparing.add_argument('--max-differing', type=float, default=MAX_DIFFERING,
                           metavar='PERCENT',
                           help=f'Noise allows this share of pixels to differ at all '
                                f'(default {MAX_DIFFERING}%%)')
    comparing.add_argument('--max-changed', type=float, default=MAX_CHANGED, metavar='PERCENT',
                           help=f'Noise allows this share of pixels beyond the tolerance '
                                f'(default {MAX_CHANGED}%%)')
    comparing.add_argument('--max-mean', type=float, default=MAX_MEAN,
                           help='Noise allows this mean RGB difference, in 1/255 '
                                f'(default {MAX_MEAN})')
    comparing.add_argument('--jobs', type=int, help='Worker processes for comparing')
    parser = argparse.ArgumentParser(description=__doc__.split('\n\n')[0])
    commands = parser.add_subparsers(dest='command', required=True)
    commands.add_parser('list', parents=[common], help='List the shots')
    render = commands.add_parser('render', parents=[common, rendering], help='Render a named set')
    render.add_argument('name')
    render.add_argument('--force', action='store_true', help='Replace an existing set')
    compare = commands.add_parser('compare', parents=[common, comparing],
                                  help='Compare two sets (names or directories)')
    compare.add_argument('base')
    compare.add_argument('new')
    check = commands.add_parser('check', parents=[common, rendering, comparing],
                                help="Render the base set's shots again and compare")
    check.add_argument('base', nargs='?', default='baseline')
    check.add_argument('--name', default='current', help='Set to render into (default current)')
    args = parser.parse_args(argv)
    sets_dir, reports_dir = args.dir / 'sets', args.dir / 'reports'
    patterns = _patterns(args.only)
    try:
        if args.command == 'list':
            for shot in select(SHOTS, patterns):
                flags = ' '.join(shot.flags)
                print(f'{shot.name:<22} {shot.group:<9} --seek {shot.seek:<5g} {flags}')
            return 0
        if args.command in ('render', 'check'):
            extra = shlex.split(args.extra or '')
            if args.command == 'render':
                shots, size, name = select(SHOTS, patterns), args.size or DEFAULT_SIZE, args.name
            else:
                base_path, base = load_set(args.base, sets_dir)
                target = _set_path(args.name, sets_dir)
                if target == base_path or target in base_path.parents:
                    raise LookError('Render the check into a different set from its base, '
                                    'not into the base or a directory holding it')
                shots = select(manifest_shots(base), patterns)
                size, name = args.size or tuple(base['size']), args.name
                if args.extra is None and base.get('extra'):
                    extra = list(base['extra'])
                    print(f"Using the base set's extra flags {shlex.join(extra)}; "
                          "pass --extra= to drop them")
            _, manifest = render_set(name, shots, binary=args.bin, data=args.data, size=size,
                                     extra=extra, sets_dir=sets_dir,
                                     force=args.force if args.command == 'render' else True,
                                     timeout=args.timeout)
            if args.command == 'render':
                return 1 if manifest['failed'] or not manifest['shots'] else 0
            compare_base = args.base
        else:
            compare_base, name = args.base, args.new
        failed, _ = compare_sets(compare_base, name, sets_dir=sets_dir, reports_dir=reports_dir,
                                 patterns=patterns, tolerance=args.tolerance,
                                 max_changed=args.max_changed, max_mean=args.max_mean,
                                 max_differing=args.max_differing,
                                 require_all=args.command == 'check', jobs=args.jobs)
        return 1 if failed else 0
    except (LookError, OSError, ValueError) as error:
        print(f'look_check: {error}', file=sys.stderr)
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
