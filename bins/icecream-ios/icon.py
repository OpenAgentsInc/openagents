#!/usr/bin/env python3
"""Draws The Ice Cream Man's app icon: a cartoon soft-serve cone, with a
wary pair of eyes, on a dusk-blue ground. Original art, no text.

    python3 bins/icecream-ios/icon.py OUT.png [SIZE]

Needs Pillow. Drawn at 4x and scaled down for smooth edges. App Store
icons must be square and opaque; iOS rounds the corners itself.
"""

import sys

from PIL import Image, ImageDraw, ImageFilter

out = sys.argv[1]
size = int(sys.argv[2]) if len(sys.argv) > 2 else 1024
S = 4096
k = S / 1024


def p(*xs):
    return [x * k for x in xs]


img = Image.new("RGB", (S, S))
draw = ImageDraw.Draw(img)
# Dusk gradient: deep blue at the top to a warm pink at the bottom.
top, bottom = (24, 38, 94), (232, 98, 124)
for y in range(S):
    t = y / (S - 1)
    t = t * t * (3 - 2 * t)
    draw.line([(0, y), (S, y)], fill=tuple(int(a + (b - a) * t) for a, b in zip(top, bottom)))

# A soft glow behind the cone.
glow = Image.new("L", (S, S), 0)
ImageDraw.Draw(glow).ellipse(p(212, 150, 812, 750), fill=150)
glow = glow.filter(ImageFilter.GaussianBlur(160 * k))
img.paste((255, 214, 170), (0, 0), glow)

# A few stars.
for x, y, r in [(150, 140, 7), (880, 110, 9), (820, 300, 6), (110, 380, 5), (930, 470, 6), (260, 70, 5)]:
    draw.ellipse(p(x - r, y - r, x + r, y + r), fill=(255, 246, 220))

outline = (58, 36, 24)
line = int(14 * k)

# The cone: a waffle triangle.
cone = p(330, 560, 694, 560, 512, 960)
draw.polygon(cone, fill=(226, 160, 84), outline=outline)
mask = Image.new("L", (S, S), 0)
ImageDraw.Draw(mask).polygon(cone, fill=255)
grid = Image.new("RGB", (S, S), (190, 120, 52))
grid_draw = ImageDraw.Draw(grid)
waffle = Image.new("L", (S, S), 0)
waffle_draw = ImageDraw.Draw(waffle)
for i in range(-12, 14):
    x0 = 512 + i * 54
    waffle_draw.line(p(x0 - 400, 560, x0 + 400, 1360), fill=255, width=int(10 * k))
    waffle_draw.line(p(x0 + 400, 560, x0 - 400, 1360), fill=255, width=int(10 * k))
waffle = Image.composite(waffle, Image.new("L", (S, S), 0), mask)
img.paste(grid, (0, 0), waffle)
draw.line(cone + cone[:2], fill=outline, width=line, joint="curve")

cream = (255, 246, 236)
# Drips down the bottom scoop.
for x, h in [(370, 130), (476, 96), (606, 150)]:
    draw.rounded_rectangle(p(x, 600, x + 40, 600 + h), radius=20 * k, fill=cream, outline=outline, width=int(10 * k))
# Soft serve: three stacked swirls, strawberry pink on vanilla.
pink = (255, 150, 186)
swirls = [(300, 470, 724, 640, cream), (350, 360, 674, 520, pink), (400, 262, 624, 410, cream)]
for x0, y0, x1, y1, colour in swirls:
    draw.rounded_rectangle(p(x0, y0, x1, y1), radius=80 * k, fill=colour, outline=outline, width=line)
# The tip.
draw.polygon(p(470, 280, 554, 280, 520, 190), fill=cream, outline=outline)
draw.line(p(470, 280, 520, 190, 554, 280), fill=outline, width=line, joint="curve")
draw.rectangle(p(478, 268, 548, 292), fill=cream)
# A cherry.
draw.ellipse(p(474, 120, 554, 200), fill=(214, 34, 54), outline=outline, width=line)
draw.ellipse(p(492, 136, 514, 158), fill=(255, 170, 170))
draw.line(p(516, 120, 548, 62), fill=outline, width=int(10 * k))

# Wary eyes on the middle scoop.
for cx in (462, 562):
    draw.ellipse(p(cx - 34, 424, cx + 34, 500), fill=(255, 255, 255), outline=outline, width=int(10 * k))
    draw.ellipse(p(cx - 6, 450, cx + 22, 486), fill=outline)
# Raised brows.
draw.line(p(424, 418, 494, 410), fill=outline, width=int(12 * k))
draw.line(p(530, 410, 600, 418), fill=outline, width=int(12 * k))

img = img.resize((size, size), Image.LANCZOS)
img.save(out, optimize=True)
