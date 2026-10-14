#!/usr/bin/env python3
"""Side-by-sides and the progress sheet for Draconia's parity rounds.

    draconia_parity.py sbs REF.jpg OURS.png LABEL OUT.png
    draconia_parity.py row ROUND OUT_ROW.png SBS1.png [SBS2.png ...]
    draconia_parity.py sheet OUT.png ROW1.png [ROW2.png ...]
    draconia_parity.py grid OUT.png LABEL REF1 OURS1 [REF2 OURS2 ...]
    draconia_parity.py columns OUT.png LABEL1,LABEL2,... A1 B1 ... [A2 B2 ...]

`columns` is the grid with any number of labelled columns (the owner's
three: the photoreal reference, Unreal's render, Verse's); a path of `-`
is an empty panel marked "pending". `sbs` puts the reference still on the left ("reference") and our render on
the right (LABEL, such as "Verse round 3"), each 1280x720. `row` makes one
round's row of the progress sheet from its side-by-sides, and `sheet`
stacks the rows. Reference stills never enter the repository; outputs go
to ~/work/film-out/draconia/parity/ and ~/Downloads/.
"""

import sys

from PIL import Image, ImageDraw, ImageFont

W, H = 1280, 720


def font(size):
    for path in ("/System/Library/Fonts/Helvetica.ttc", "/System/Library/Fonts/SFNS.ttf"):
        try:
            return ImageFont.truetype(path, size)
        except OSError:
            continue
    return ImageFont.load_default()


def fit(path):
    im = Image.open(path).convert("RGB")
    # Cover W x H, centred; a wider frame (21:9) is letterboxed whole.
    if im.width / im.height > W / H * 1.15:
        s = W / im.width
        im = im.resize((W, round(im.height * s)), Image.LANCZOS)
        out = Image.new("RGB", (W, H), (0, 0, 0))
        out.paste(im, (0, (H - im.height) // 2))
        return out
    s = max(W / im.width, H / im.height)
    im = im.resize((round(im.width * s), round(im.height * s)), Image.LANCZOS)
    x = (im.width - W) // 2
    y = (im.height - H) // 2
    return im.crop((x, y, x + W, y + H))


def label(im, text):
    d = ImageDraw.Draw(im)
    f = font(34)
    box = d.textbbox((0, 0), text, font=f)
    d.rectangle((16, 16, 16 + box[2] - box[0] + 24, 16 + box[3] - box[1] + 20), fill=(0, 0, 0))
    d.text((28, 22), text, font=f, fill=(255, 255, 255))


def sbs(ref, ours, text, out):
    a, b = fit(ref), fit(ours)
    label(a, "reference")
    label(b, text)
    im = Image.new("RGB", (W * 2 + 8, H), (0, 0, 0))
    im.paste(a, (0, 0))
    im.paste(b, (W + 8, 0))
    im.save(out)
    print("wrote", out)


def row(round_name, out, parts):
    ims = [Image.open(p).convert("RGB") for p in parts]
    h = 360
    ims = [im.resize((round(im.width * h / im.height), h), Image.LANCZOS) for im in ims]
    lab = 160
    im = Image.new("RGB", (lab + sum(i.width for i in ims) + 8 * len(ims), h), (12, 12, 12))
    d = ImageDraw.Draw(im)
    d.text((16, h // 2 - 20), round_name, font=font(32), fill=(255, 255, 255))
    x = lab
    for i in ims:
        im.paste(i, (x, 0))
        x += i.width + 8
    im.save(out)
    print("wrote", out)


def sheet(out, rows):
    ims = [Image.open(p).convert("RGB") for p in rows]
    w = max(i.width for i in ims)
    im = Image.new("RGB", (w, sum(i.height + 8 for i in ims)), (0, 0, 0))
    y = 0
    for i in ims:
        im.paste(i, (0, y))
        y += i.height + 8
    im.save(out)
    print("wrote", out)


def grid(out, text, pairs):
    """Rows of reference | ours, each pair labelled; the full-face grid."""
    rows = []
    for ref, ours in zip(pairs[::2], pairs[1::2]):
        a, b = fit(ref), fit(ours)
        label(a, "reference")
        label(b, text)
        row = Image.new("RGB", (W * 2 + 8, H), (0, 0, 0))
        row.paste(a, (0, 0))
        row.paste(b, (W + 8, 0))
        rows.append(row)
    im = Image.new("RGB", (W * 2 + 8, len(rows) * (H + 8)), (0, 0, 0))
    for k, r in enumerate(rows):
        im.paste(r, (0, k * (H + 8)))
    im.save(out)
    print("wrote", out)


def columns(out, labels, paths):
    """Rows of labelled panels, len(labels) to a row."""
    n = len(labels)
    rows = [paths[i : i + n] for i in range(0, len(paths), n)]
    im = Image.new("RGB", (n * W + (n - 1) * 8, len(rows) * (H + 8)), (0, 0, 0))
    for r, row in enumerate(rows):
        for c, path in enumerate(row):
            if path == "-":
                panel = Image.new("RGB", (W, H), (12, 12, 12))
                d = ImageDraw.Draw(panel)
                d.text((W // 2 - 70, H // 2 - 20), "pending", font=font(40), fill=(120, 120, 120))
            else:
                panel = fit(path)
            label(panel, labels[c])
            im.paste(panel, (c * (W + 8), r * (H + 8)))
    im.save(out)
    print("wrote", out)


if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "sbs":
        sbs(*sys.argv[2:6])
    elif cmd == "row":
        row(sys.argv[2], sys.argv[3], sys.argv[4:])
    elif cmd == "grid":
        grid(sys.argv[2], sys.argv[3], sys.argv[4:])
    elif cmd == "columns":
        columns(sys.argv[2], sys.argv[3].split(","), sys.argv[4:])
    elif cmd == "sheet":
        sheet(sys.argv[2], sys.argv[3:])
    else:
        raise SystemExit(__doc__)
