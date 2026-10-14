#!/usr/bin/env python3
"""A contact sheet of the best-scoring presets (round 7).

    draconia_preset_sheet.py RENDER_DIR OUT.png [N] [--skip NAME,NAME]

Reads RENDER_DIR/scores.json (draconia_preset_score.py) and lays out the
reference still at 0:54 and the top N presets' renders, each labelled with
its name and score, four to a row.
"""

import json
import os
import sys

from PIL import Image, ImageDraw

sys.path.insert(0, os.path.dirname(__file__))
from draconia_parity import font  # noqa: E402

REF = os.path.expanduser("~/work/film-out/draconia/parity/ref/owner-target-54s.png")
W, H = 640, 360


def tile(path, text):
    im = Image.open(path).convert("RGB")
    s = max(W / im.width, H / im.height)
    im = im.resize((round(im.width * s), round(im.height * s)))
    x, y = (im.width - W) // 2, (im.height - H) // 2
    im = im.crop((x, y, x + W, y + H))
    d = ImageDraw.Draw(im)
    f = font(24)
    b = d.textbbox((0, 0), text, font=f)
    d.rectangle((10, 10, 26 + b[2] - b[0], 22 + b[3] - b[1]), fill=(0, 0, 0))
    d.text((18, 12), text, font=f, fill=(255, 255, 255))
    return im


def main():
    a = sys.argv[1:]
    rdir, out = a[0], a[1]
    n = int(a[2]) if len(a) > 2 and not a[2].startswith("--") else 7
    skip = set(a[a.index("--skip") + 1].split(",")) if "--skip" in a else set()
    rows = [r for r in json.load(open(os.path.join(rdir, "scores.json"))) if r["name"] not in skip][:n]
    tiles = [tile(REF, "reference 0:54")] + [tile(os.path.join(rdir, r["name"] + ".png"), f"{r['name']}  {r['score'] * 100:.1f}%") for r in rows]
    cols = 4
    sheet = Image.new("RGB", (cols * W + (cols - 1) * 6, ((len(tiles) + cols - 1) // cols) * (H + 6)), (0, 0, 0))
    for i, t in enumerate(tiles):
        sheet.paste(t, ((i % cols) * (W + 6), (i // cols) * (H + 6)))
    sheet.save(out)
    print("wrote", out)


if __name__ == "__main__":
    main()
