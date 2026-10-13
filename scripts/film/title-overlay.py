#!/usr/bin/env python3
"""The launch film's title as an overlay (#11432): "OpenAgents" (Geist
Regular), "CODER" (Berkeley Mono Bold), "v1.0.0" (Berkeley Mono) over a
soft darkening, on a transparent background, so the opening scene plays
underneath. cut.py composites it with a shot's `"over"`.

  scripts/film/title-overlay.py OUT.mov [--size 1920x1080] [--seconds 4.5]

Writes ProRes 4444 with alpha. The type comes up by about 1.5 s, holds, and
fades out with the darkening by about 4 s. No letterbox bars and no grain:
the scene underneath carries the picture. Fonts: FILM_FONTS (Geist, default
~/work/film-out/fonts) and BERKELEY_FONTS (default ~/Library/Fonts).
"""

import argparse
import os
import subprocess

from PIL import Image, ImageDraw, ImageFilter, ImageFont

ap = argparse.ArgumentParser()
ap.add_argument("out")
ap.add_argument("--size", default="1920x1080")
ap.add_argument("--seconds", type=float, default=4.5)
ap.add_argument("--fps", type=int, default=30)
args = ap.parse_args()

W, H = (int(v) for v in args.size.split("x"))
S = 2  # drawn at twice the size, then scaled down
w, h = W * S, H * S
k = H / 1080  # the type is sized for 1080p
geist = os.path.expanduser(os.environ.get("FILM_FONTS", "~/work/film-out/fonts"))
berkeley = os.path.expanduser(os.environ.get("BERKELEY_FONTS", "~/Library/Fonts"))


def layer(text, font, cy, fill, tracking):
    im = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    widths = [d.textlength(c, font=font) for c in text]
    x = (w - (sum(widths) + tracking * (len(text) - 1))) / 2
    a, b = font.getmetrics()
    y = cy - (a + b) / 2
    for c, cw in zip(text, widths):
        d.text((x, y), c, font=font, fill=fill)
        x += cw + tracking
    return im.resize((W, H), Image.LANCZOS)


u = S * k
cy = h * 0.5
oa = layer("OpenAgents", ImageFont.truetype(f"{geist}/Geist-Regular.ttf", int(32 * u)), cy - 132 * u,
           (215, 217, 222, 255), 7 * u)
coder = layer("CODER", ImageFont.truetype(f"{berkeley}/BerkeleyMono-Bold.ttf", int(165 * u)), cy - 4 * u,
              (242, 242, 238, 255), 8 * u)
ver = layer("v1.0.0", ImageFont.truetype(f"{berkeley}/BerkeleyMono-Regular.ttf", int(26 * u)), cy + 112 * u,
            (165, 167, 172, 255), 10 * u)
bloom = coder.split()[3].filter(ImageFilter.GaussianBlur(30 * k)).point(lambda v: int(v * 0.16))

# The darkening: a soft pool behind the type over a light dim of the whole
# frame, so white type reads over bright screens.
pool = Image.new("L", (W, H), 0)
ImageDraw.Draw(pool).ellipse([W * 0.2, H * 0.28, W * 0.8, H * 0.72], fill=255)
pool = pool.filter(ImageFilter.GaussianBlur(160 * k))
DARK = pool.point(lambda v: int(46 + v * 0.62))


def ease(t):
    t = max(0.0, min(1.0, t))
    return t * t * (3 - 2 * t)


def fade(t, a, b):
    return ease((t - a) / (b - a))


def with_alpha(im, a):
    if a >= 1:
        return im
    r, g, b, al = im.split()
    return Image.merge("RGBA", (r, g, b, al.point(lambda v: int(v * a))))


ff = subprocess.Popen(["ffmpeg", "-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", f"{W}x{H}",
                       "-r", str(args.fps), "-i", "-", "-c:v", "prores_ks", "-profile:v", "4444",
                       "-pix_fmt", "yuva444p10le", "-alpha_bits", "16", args.out], stdin=subprocess.PIPE)
frames = int(round(args.fps * args.seconds))
for i in range(frames):
    t = i / args.fps
    out = 1 - fade(t, 3.0, 4.0)  # everything leaves together
    d = fade(t, 0.0, 0.6) * (1 - fade(t, 3.0, 4.2))
    frame = Image.merge("RGBA", [Image.new("L", (W, H), 0)] * 3 + [DARK.point(lambda v: int(v * d))])
    a_oa, a_c, a_v = fade(t, 0.15, 0.9) * out, fade(t, 0.35, 1.35) * out, fade(t, 0.8, 1.5) * out
    blur = (1 - fade(t, 0.35, 1.35)) * 14 * k + fade(t, 3.0, 4.0) * 6 * k
    c = coder.filter(ImageFilter.GaussianBlur(blur)) if blur > 0.3 else coder
    b = bloom.point(lambda v: int(v * a_c))
    frame = Image.alpha_composite(frame, Image.merge("RGBA", [b, b, b, b]))
    for lay, a in ((oa, a_oa), (c, a_c), (ver, a_v)):
        if a > 0:
            frame = Image.alpha_composite(frame, with_alpha(lay, a))
    ff.stdin.write(frame.tobytes())
ff.stdin.close()
ff.wait()
print(f"{args.out}: {frames} frames")
