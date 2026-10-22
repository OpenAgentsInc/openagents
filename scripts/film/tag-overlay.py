#!/usr/bin/env python3
"""A small on-screen tag as an overlay with alpha, for a shot that needs a
plain label over it (#12458): CONCEPT over a beat whose feature isn't built
yet, or a short line naming what real footage shows. cut.py composites it
with a shot's `"over"`.

  scripts/film/tag-overlay.py OUT.mov --tag CONCEPT [--note TEXT]
      [--seconds 4] [--size 1920x1080] [--corner top-left|top-right] [--fade 0.3]

Writes ProRes 4444 with alpha. The tag is a bordered pill in Geist Mono
(FILM_FONTS, default ~/work/film-out/fonts), with the note in Geist Regular
beside it, inside the action-safe area. It fades in over `--fade` seconds
(0.3 by default; 0 when it carries on from the shot before) and holds to the
end, so the shot's own cut takes it away.
"""

import argparse
import os
import subprocess

from PIL import Image, ImageDraw, ImageFont

ap = argparse.ArgumentParser()
ap.add_argument("out")
ap.add_argument("--tag", required=True)
ap.add_argument("--note", default="")
ap.add_argument("--seconds", type=float, default=4.0)
ap.add_argument("--size", default="1920x1080")
ap.add_argument("--fps", type=int, default=30)
ap.add_argument("--corner", choices=["top-left", "top-right"], default="top-left")
ap.add_argument("--fade", type=float, default=0.3, help="fade-in seconds; 0 when it carries on from the shot before")
args = ap.parse_args()

W, H = (int(v) for v in args.size.split("x"))
k = H / 1080
fonts = os.path.expanduser(os.environ.get("FILM_FONTS", "~/work/film-out/fonts"))
mono = ImageFont.truetype(f"{fonts}/GeistMono-Medium.ttf", int(26 * k))
sans = ImageFont.truetype(f"{fonts}/Geist-Regular.ttf", int(26 * k))


def layer():
    im = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    track = 4 * k
    tag_w = sum(d.textlength(c, font=mono) for c in args.tag) + track * (len(args.tag) - 1)
    a, b = mono.getmetrics()
    padx, pady = 16 * k, 9 * k
    pill_w, pill_h = tag_w + 2 * padx, a + b + 2 * pady
    note_w = d.textlength(args.note, font=sans) if args.note else 0
    gap = 18 * k
    total = pill_w + (gap + note_w if args.note else 0)
    # Inside the action-safe area (5% in from each edge).
    x0 = W * 0.05 if args.corner == "top-left" else W * 0.95 - total
    y0 = H * 0.05
    # A soft dark band behind it all, so it reads over bright worlds.
    d.rounded_rectangle([x0 - 10 * k, y0 - 8 * k, x0 + total + 14 * k, y0 + pill_h + 8 * k],
                        radius=int(12 * k), fill=(0, 0, 0, 120))
    d.rounded_rectangle([x0, y0, x0 + pill_w, y0 + pill_h], radius=int(pill_h / 2),
                        outline=(236, 238, 241, 235), width=max(1, int(2 * k)))
    x = x0 + padx
    for c in args.tag:
        d.text((x, y0 + pady), c, font=mono, fill=(236, 238, 241, 255))
        x += d.textlength(c, font=mono) + track
    if args.note:
        na, nb = sans.getmetrics()
        d.text((x0 + pill_w + gap, y0 + (pill_h - na - nb) / 2), args.note, font=sans,
               fill=(214, 216, 221, 245))
    return im


TAG = layer()
ff = subprocess.Popen(["ffmpeg", "-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", f"{W}x{H}",
                       "-r", str(args.fps), "-i", "-", "-c:v", "prores_ks", "-profile:v", "4444",
                       "-pix_fmt", "yuva444p10le", "-alpha_bits", "16", args.out], stdin=subprocess.PIPE)
frames = int(round(args.fps * args.seconds))
for i in range(frames):
    a = min(1.0, i / (args.fade * args.fps)) if args.fade > 0 else 1.0
    a = a * a * (3 - 2 * a)
    if a >= 1:
        frame = TAG
    else:
        r, g, b, al = TAG.split()
        frame = Image.merge("RGBA", (r, g, b, al.point(lambda v: int(v * a))))
    ff.stdin.write(frame.tobytes())
ff.stdin.close()
ff.wait()
print(f"{args.out}: {frames} frames")
