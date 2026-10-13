#!/usr/bin/env python3
"""The launch film's ending: a first-person walk on the Grid into the
Everglade portal, a fade to white as you pass through, and a calm card,
"The Verse" over "Coming soon".

  scripts/film/portal-walk.py WALK.mp4 OUT.mp4 [--cross S] [--frames-dir DIR]

WALK.mp4 is the engine's own render of the walk, 1080p 30 fps:

  VERSE_QUALITY=high cargo run -p verse --example film_shot -- \\
      docs/verse/film/verse-shots/portal-walk.json

(it writes ~/work/film-out/verse/portal-walk-raw.mp4; it crosses the
portal's plane at 12.0 s, which --cross names). This script eases
the walk's last second to white, holds the white, and settles it into the
film's noir card (colors from scripts/film/revenue-sharing.py), the words
coming up in Geist Regular. --frames-dir also saves one PNG per second.

Fonts (licensed, not in the repo): FILM_FONTS (Geist, default
~/work/film-out/fonts). Needs Pillow, numpy and ffmpeg.
"""
import argparse
import os
import subprocess
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont

ap = argparse.ArgumentParser()
ap.add_argument("walk")
ap.add_argument("out")
ap.add_argument("--frames-dir", help="also save a PNG every second here")
ap.add_argument("--cross", type=float, default=12.0,
                help="when the walk crosses the portal's plane, s (the shot's last key less 0.4)")
args = ap.parse_args()

GEIST = Path(os.path.expanduser(os.environ.get("FILM_FONTS", "~/work/film-out/fonts")))
W, H, FPS = 1920, 1080, 30
CANVAS = (10, 10, 10)
CONTENT = (237, 237, 237)
SECONDARY = (129, 129, 129)

# The timeline, seconds, from when the walk crosses the portal's plane.
C = args.cross
FADE_IN = 0.0, 0.6  # from black, as the shot opens
WHITE = C - 1.3, C + 0.1  # the portal's light takes the frame
SETTLE = C + 0.7, C + 2.1  # the white settles into the card
WORDS = C + 1.6, C + 2.8  # the words come up
END = C + 4.4


def ease(t, a, b):
    """0 before a, 1 after b, smootherstep between."""
    x = min(max((t - a) / (b - a), 0.0), 1.0)
    return x * x * x * (x * (x * 6 - 15) + 10)


def text_layer():
    """The card's words, white on transparent, drawn at twice the size."""
    s = 2
    im = Image.new("L", (W * s, H * s), 0)
    d = ImageDraw.Draw(im)
    small = ImageFont.truetype(str(GEIST / "Geist-Regular.ttf"), 30 * s)
    big = ImageFont.truetype(str(GEIST / "Geist-Regular.ttf"), 76 * s)
    cy = H * s // 2

    def line(text, font, y, fill, tracking):
        widths = [d.textlength(c, font=font) for c in text]
        x = (W * s - (sum(widths) + tracking * (len(text) - 1))) / 2
        a, b = font.getmetrics()
        top = y - (a + b) / 2
        for c, cw in zip(text, widths):
            d.text((x, top), c, font=font, fill=fill)
            x += cw + tracking

    # Two masks so the two lines keep their own colors.
    line("The Verse", small, cy - 62 * s, 255, 5 * s)
    upper = np.asarray(im.resize((W, H), Image.LANCZOS), np.float32) / 255
    im.paste(0, (0, 0, W * s, H * s))
    line("Coming soon", big, cy + 22 * s, 255, 1 * s)
    lower = np.asarray(im.resize((W, H), Image.LANCZOS), np.float32) / 255
    return upper, lower


UPPER, LOWER = text_layer()
canvas = np.array(CANVAS, np.float32)
white = np.array([255, 255, 255], np.float32)


# Distance from the frame's center, 0 there and 1 at the corners: the light
# recedes from the edges in, so the white closes like a doorway's glow.
_y, _x = np.mgrid[0:H, 0:W].astype(np.float32)
RADIUS = np.hypot((_x - W / 2) / (W / 2), (_y - H / 2) / (H / 2)) / np.sqrt(2)


def card(t):
    a, b = SETTLE
    span = b - a
    # Each pixel's own settle: the corners start first, the center last.
    x = np.clip((t - a - (1 - RADIUS) * span * 0.45) / (span * 0.55), 0, 1)[..., None]
    k = x * x * x * (x * (x * 6 - 15) + 10)
    frame = white * (1 - k) + canvas * k
    w = ease(t, *WORDS)
    for mask, color in ((UPPER, SECONDARY), (LOWER, CONTENT)):
        a = (mask * w)[..., None]
        frame = frame * (1 - a) + np.array(color, np.float32) * a
    return frame


reader = subprocess.Popen(
    ["ffmpeg", "-loglevel", "error", "-i", args.walk, "-f", "rawvideo", "-pix_fmt", "rgb24",
     "-s", f"{W}x{H}", "-"],
    stdout=subprocess.PIPE)
writer = subprocess.Popen(
    ["ffmpeg", "-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", f"{W}x{H}",
     "-r", str(FPS), "-i", "-", "-an", "-c:v", "libx264", "-preset", "slow", "-crf", "14",
     "-pix_fmt", "yuv420p", "-movflags", "+faststart", args.out],
    stdin=subprocess.PIPE)
frames_dir = Path(args.frames_dir) if args.frames_dir else None
if frames_dir:
    frames_dir.mkdir(parents=True, exist_ok=True)

last = None
for k in range(round(END * FPS)):
    t = k / FPS
    raw = reader.stdout.read(W * H * 3)
    if len(raw) == W * H * 3:
        last = np.frombuffer(raw, np.uint8).reshape(H, W, 3).astype(np.float32)
    if t < SETTLE[0] and last is not None:
        # The walk, opening from black and washing to white in the portal.
        frame = last * ease(t, *FADE_IN)
        w = ease(t, *WHITE)
        # Brighter parts go a little sooner, as light floods a lens.
        lum = frame.mean(axis=2, keepdims=True) / 255
        a = np.clip(w * (0.8 + 0.4 * lum), 0, 1)
        frame = frame + (white - frame) * (a + (1 - a) * w ** 3)
    else:
        frame = card(t)
    out = np.clip(frame + np.random.uniform(-0.5, 0.5, frame.shape), 0, 255).astype(np.uint8)
    writer.stdin.write(out.tobytes())
    if frames_dir and k % FPS == 0:
        Image.fromarray(out).save(frames_dir / f"portal-walk-{k // FPS:02d}.png")
reader.stdout.close()
reader.wait()
writer.stdin.close()
writer.wait()
print(f"wrote {args.out}")
