"""A page's link-preview card (Open Graph and Twitter): a frame of the page
cropped to 1200x630, darkened on the left, with its words written in
Filament, the stroke font every Coaxial page draws in (the glyph table is
read from apps/openagents-web/font.coax, so the card and the pages agree).
Tooling only: it runs on this machine and ships a JPEG.

    python3 -I coax/web/og-card.py SHOT.png OUT.jpg --crop X0,Y0,X1,Y1 \
        --eyebrow PLAYTEST --title "ENTER THE|VERSE" \
        --line "EDIT OUR HAUNTED HOUSE." --line "CAN YOU MAKE IT SCARIER?" \
        --foot "OPENAGENTS · OPENAGENTS.COM/VERSE"

SHOT is a screenshot taken with coax/web/shot.ts; the crop keeps the HUD out.
It needs Pillow (pip install pillow, in a venv). /verse/og-verse.jpg came from
a 2000x1100 shot of /realm/haunted-house with --crop 150,112,1850,1004.
"""
import argparse, os, re
from PIL import Image, ImageChops, ImageDraw, ImageFilter

W, H = 1200, 630
ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")


def table(src, name):
    m = re.search(name + r" :: \[\d+\] u\d+\.\[(.*?)\];", src, re.S)
    return [int(v) for v in re.findall(r"\d+", m.group(1))]


src = open(os.path.join(ROOT, "apps/openagents-web/font.coax"), encoding="utf-8").read()
CODES, FIRST, COUNT, WIDTH, POINTS = (table(src, n) for n in ("FONT_CODES", "FONT_FIRST", "FONT_COUNT", "FONT_WIDTH", "FONT_POINTS"))
BREAK = 255


def glyph(ch):
    c = ord(ch.upper())
    k = CODES.index(c) if c in CODES else CODES.index(ord("?"))
    pts = POINTS[FIRST[k] * 2:(FIRST[k] + COUNT[k]) * 2]
    return [(pts[i], pts[i + 1]) for i in range(0, len(pts), 2)], WIDTH[k] / 12


def measure(text, gap, space):
    w, after = 0.0, False
    for ch in text:
        if ch == " ":
            w += space; after = False; continue
        if after: w += gap
        w += glyph(ch)[1]; after = True
    return w


def write(draw, text, x, base, cap, gap, space, colour, width):
    unit, pen, after = cap / 12, x, False
    for ch in text:
        if ch == " ":
            pen += space * cap; after = False; continue
        if after: pen += gap * cap
        pts, gw = glyph(ch)
        for (ax, ay), (bx, by) in zip(pts, pts[1:]):
            if BREAK in (ax, bx): continue
            a = (pen + ax * unit, base - (ay - 2) * unit)
            b = (pen + bx * unit, base - (by - 2) * unit)
            draw.line([a, b], fill=colour, width=width)
            for p in (a, b):
                r = width / 2
                draw.ellipse([p[0] - r, p[1] - r, p[0] + r, p[1] + r], fill=colour)
        pen += gw * cap; after = True


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("shot"); ap.add_argument("out")
    ap.add_argument("--crop", required=True)
    ap.add_argument("--eyebrow", default=""); ap.add_argument("--title", default="")
    ap.add_argument("--line", action="append", default=[]); ap.add_argument("--foot", default="")
    a = ap.parse_args()
    x0, y0, x1, y1 = (int(v) for v in a.crop.split(","))
    S = 2   # drawn at twice the size, then scaled down: smooth strokes
    img = Image.open(a.shot).convert("RGB").crop((x0, y0, x1, y1)).resize((W * S, H * S), Image.LANCZOS)
    # darker on the left, where the words go
    shade = Image.new("L", (W * S, H * S))
    sd = ImageDraw.Draw(shade)
    for x in range(W * S):
        f = max(0.0, 1 - x / (W * S * 0.62))
        sd.line([(x, 0), (x, H * S)], fill=int(205 * f ** 1.3))
    img = Image.composite(Image.new("RGB", img.size, (6, 6, 10)), img, shade)
    words = Image.new("RGB", img.size)
    d = ImageDraw.Draw(words)
    parch, bronze, ember = (232, 220, 192), (204, 163, 92), (232, 120, 43)
    left = 64 * S
    y = 200 * S
    if a.eyebrow:
        write(d, a.eyebrow, left, y, 13 * S, 1.1, 2.6, bronze, 2 * S); y += 72 * S
    for t in a.title.split("|") if a.title else []:
        write(d, t, left, y, 50 * S, 0.62, 1.5, parch, 4 * S); y += 72 * S
    y += 10 * S
    for t in a.line:
        write(d, t, left, y, 17 * S, 0.5, 1.2, parch, 2 * S); y += 32 * S
    if a.foot:
        r = 5 * S; cy = (H - 52) * S - 8 * S
        d.ellipse([left - r + r, cy - r, left + r + r, cy + r], fill=ember)
        write(d, a.foot, left + 22 * S, (H - 52) * S, 13 * S, 0.5, 1.2, parch, 2 * S)
    glow = words.filter(ImageFilter.GaussianBlur(7 * S))
    img = ImageChops.add(ImageChops.add(img, glow.point(lambda v: v * 0.55)), words)
    img.resize((W, H), Image.LANCZOS).save(a.out, "JPEG", quality=88, optimize=True, progressive=True)
    print(a.out)


main()
