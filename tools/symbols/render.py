"""Render one pattern's shapes as a neon sheet, or all patterns together.

    python3 render.py 3        # pattern_3.py -> pattern_3.png
    python3 render.py all      # every pattern_N.py -> contact-sheet.png

A pattern file defines TITLE and SHAPES, a dict from mask index (0..15) to
(label, Shape). Pairs are masks 2k and 2k+1; NOT_SHOWN lists pairs the music
never shows in that pattern.
"""
import importlib
import sys

from PIL import Image, ImageDraw

from neon import render

CELL = 150
LABEL = 34


def pattern_strip(n):
    mod = importlib.import_module(f"pattern_{n}")
    importlib.reload(mod)
    not_shown = set(getattr(mod, "NOT_SHOWN", []))
    strip = Image.new("RGB", (16 * CELL + 7 * 10, CELL + LABEL + 26), (8, 8, 10))
    d = ImageDraw.Draw(strip)
    d.text((8, 6), f"{n}. {mod.TITLE}", fill=(235, 235, 235))
    for pair in range(8):
        x0 = pair * (2 * CELL + 10)
        if pair in not_shown:
            d.text((x0 + 10, 26 + CELL // 2), f"pair {pair}: not shown in this pattern", fill=(110, 110, 110))
            continue
        for k in range(2):
            m = 2 * pair + k
            label, shape = mod.SHAPES[m]
            strip.paste(render(shape, CELL), (x0 + k * CELL, 26))
            d.text((x0 + k * CELL + 4, 26 + CELL + 4), f"{m}: {label}"[:24], fill=(200, 200, 200))
        d.text((x0 + CELL - 4, 26 + CELL // 2 - 6), ">", fill=(90, 90, 90))
    return strip


def main():
    which = sys.argv[1]
    if which == "all":
        strips = [pattern_strip(n) for n in range(1, 8)]
        sheet = Image.new("RGB", (strips[0].width, sum(s.height for s in strips)), (8, 8, 10))
        y = 0
        for s in strips:
            sheet.paste(s, (0, y))
            y += s.height
        sheet.save("contact-sheet.png")
        print("contact-sheet.png", sheet.size)
    else:
        strip = pattern_strip(int(which))
        strip.save(f"pattern_{which}.png")
        print(f"pattern_{which}.png", strip.size)


if __name__ == "__main__":
    main()
