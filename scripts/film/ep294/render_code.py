"""Render a real source file as a tall editor-style PNG (dark glass, GeistMono, line numbers),
for a slow scroll on screen.

    python3 render_code.py SRC.coax OUT.png TITLE [WIDTH] [FIRST_LINE] [N_LINES]
"""
import re, sys
from PIL import Image, ImageDraw, ImageFont

FONTS = "/Users/christopherdavid/work/film-out/fonts"
KEYWORDS = r"\b(if|else|for|while|return|import|struct|enum|defer|cast|ifx|then|true|false|break|continue|using|proc|case|switch)\b"


def main(src, out, title, width=1280, first=1, n=140):
    lines = open(src, encoding="utf-8", errors="replace").read().split("\n")[first - 1:first - 1 + n]
    mono = ImageFont.truetype(f"{FONTS}/GeistMono-Regular.ttf", 22)
    head = ImageFont.truetype(f"{FONTS}/GeistMono-Medium.ttf", 24)
    lh, top, left = 34, 92, 96
    h = top + lh * len(lines) + 60
    im = Image.new("RGB", (width, h), (13, 15, 18))
    d = ImageDraw.Draw(im)
    d.rectangle([0, 0, width, 64], fill=(24, 27, 32))
    for i, c in enumerate([(255, 95, 86), (255, 189, 46), (39, 201, 63)]):
        d.ellipse([24 + i * 28, 24, 40 + i * 28, 40], fill=c)
    d.text((130, 18), title, font=head, fill=(200, 205, 212))
    maxc = (width - left - 24) // 13
    for k, raw in enumerate(lines):
        y = top + k * lh
        d.text((20, y), f"{first + k:4d}", font=mono, fill=(80, 88, 98))
        s = raw.replace("\t", "  ")[:maxc]
        x = left
        if s.lstrip().startswith("//"):
            d.text((x, y), s, font=mono, fill=(110, 128, 120)); continue
        # light syntax colour: strings, numbers, keywords, '::' declarations
        for tok in re.findall(r'"[^"]*"|\w+|\s+|.', s):
            col = (214, 218, 224)
            if tok.startswith('"'): col = (152, 205, 140)
            elif re.fullmatch(KEYWORDS, tok): col = (247, 147, 26)
            elif re.fullmatch(r"\d[\w.]*", tok): col = (120, 190, 235)
            elif tok in ("::", ":", "=", "->"): col = (160, 166, 176)
            d.text((x, y), tok, font=mono, fill=col)
            x += d.textlength(tok, font=mono)
    im.save(out)
    print(out, im.size)


if __name__ == "__main__":
    a = sys.argv
    main(a[1], a[2], a[3], *(int(v) for v in a[4:]))
