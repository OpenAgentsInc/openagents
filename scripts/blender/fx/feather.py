"""The `feather` sprite sheet: a feather made of light, a soft dot, and an
eyespot.

Painted directly in numpy (no Cycles render): the shapes are line art, so a
render would add nothing but time. Run it through Blender like the other
sheets so it shares `common.write_png`:

    scripts/blender/build-fx.sh feather

The sheet is one column of four 512 x 128 cells (`docs/verse/particles.md`):

- **0, the feather.** Drawn lying along the cell's long axis, the eyespot at
  the left (the sprite's head) and the shaft running right toward the
  quill, so a sprite laid along its `tail` (head at the particle, the quill
  back along the tail) draws a feather pointing away from where it grew.
  It is painted for a sprite three times as long as it is wide: a white eyespot rim round a pale blue disc
  with a soft halo, a bright shaft, and fine barbs swept toward the tip
  inside a teardrop vane. The guardian's tail and the drifting feathers of
  the Draconia peacock shot draw from it; a wand's light spell can too.
- **1, a soft dot.** A round glow with a hot core, painted four times as
  wide as tall in texels so it reads round on a square sprite: stars,
  bokeh, the halo round a bright point, and (its middle column) the
  profile of a stroke of light.
- **2, an eyespot.** Round on a square sprite: a dark blue pupil, a pale
  blue fill, a thick white rim, and a soft halo. A fanned tail draws one
  on each feather's tip, so the eyes stay round however slim the feather.
- **3, a disc.** A flat, soft-edged pale disc, a touch darker at its middle:
  the fill under a stroked eyespot rim (`verse::fx::feathers::fine`).

RGB is premultiplied linear color, sRGB-encoded; alpha is linear coverage.
"""

import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import args, write_png  # noqa: E402

W, H = 512, 128
# The sprite's length over its width, which the painting assumes.
ASPECT = 3.0


def srgb(linear):
    linear = np.clip(linear, 0.0, 1.0)
    return np.where(
        linear <= 0.0031308, linear * 12.92, 1.055 * np.power(linear, 1.0 / 2.4) - 0.055
    )


def splat(img, s, q, value, radius):
    """Adds a small Gaussian of `value` (RGB) at feather coordinates `s`
    (0 at the tip to 1 at the quill) and `q` (-1 to 1 across), `radius` in
    texels along the cell's long axis."""
    x = s * (W - 1)
    y = (q * 0.5 + 0.5) * (H - 1)
    r = max(radius, 0.6)
    span = int(np.ceil(r * 2.5))
    xs = np.arange(-span, span + 1)
    for cx, cy, v in zip(x, y, value):
        ix, iy = int(round(cx)), int(round(cy))
        gx = xs + ix
        gy = xs + iy
        okx = (gx >= 0) & (gx < W)
        oky = (gy >= 0) & (gy < H)
        if not okx.any() or not oky.any():
            continue
        dx = gx[okx] - cx
        # A texel across, in texels along, at the sprite's world size.
        dy = (gy[oky] - cy) * W / (ASPECT * H)
        k = np.exp(-(dy[:, None] ** 2 + dx[None, :] ** 2) / (2 * r * r))
        img[np.ix_(gy[oky], gx[okx])] += k[..., None] * v


def feather():
    barbs_img = np.zeros((H, W, 3), dtype=np.float64)
    img = np.zeros((H, W, 3), dtype=np.float64)
    ys, xs = np.mgrid[0:H, 0:W]
    s = xs / (W - 1)
    q = ys / (H - 1) * 2.0 - 1.0
    # Visual units: lengths in feather lengths, so across is q * width / 2.
    across = q / (2.0 * ASPECT)
    cs, a, b = 0.2, 0.11, 0.074

    # The vane: barbs from the shaft, swept toward the tip, to a teardrop
    # envelope widest round the eyespot and nearly bare toward the quill.
    def envelope(sv):
        head = np.clip((sv - 0.035) / 0.07, 0.0, 1.0) ** 0.5
        tail = np.clip((0.75 - sv) / 0.55, 0.0, 1.0) ** 1.4
        return 0.15 * head * (0.1 + 0.9 * tail)

    rng = np.random.default_rng(7)
    barbs = 520
    for side in (-1.0, 1.0):
        for i in range(barbs):
            s0 = 0.08 + 0.78 * (i + rng.uniform(0, 1)) / barbs
            sweep = np.radians(rng.uniform(30, 44))
            reach = envelope(s0 - 0.04) * rng.uniform(0.85, 1.1)
            if reach <= 0.004:
                continue
            t = np.linspace(0.0, 1.0, 90)
            bend = 0.3 * t * t
            sv = s0 - (reach * np.cos(sweep)) * t * (1 + bend)
            av = side * reach * np.sin(sweep) * t * (1 - 0.2 * bend)
            qv = av * 2.0 * ASPECT
            fade = (1.0 - t) ** 0.8 * (0.5 + 0.5 * t)
            col = np.array([0.7, 0.8, 1.0])[None, :] * (0.12 * fade)[:, None]
            splat(barbs_img, sv, qv, col, 0.42)

    # An egg: wider toward the tip than toward the shaft.
    b_egg = b * np.clip(1.0 + 0.22 * (cs - s) / a, 0.6, 1.4)
    e = np.sqrt(((s - cs) / a) ** 2 + (across / b_egg) ** 2)
    # Barbs pass behind the eyespot, so the disc stays clean.
    barbs_img *= np.clip((e - 0.9) / 0.25, 0.0, 1.0)[..., None]
    img += np.minimum(barbs_img, 0.6)

    # The eyespot: a white rim round a pale blue disc, egg-shaped along
    # the shaft, with a soft halo.
    # A thick bright rim round a soft, slightly darker fill.
    disc = np.clip((0.62 - e) / 0.08, 0.0, 1.0)
    rim = np.clip((e - 0.54) / 0.08, 0.0, 1.0) * np.clip((1.0 - e) / 0.05, 0.0, 1.0)
    halo = np.exp(-np.maximum(e - 0.98, 0.0) ** 2 / (2 * 0.16**2)) * (e > 0.95)
    inner = np.array([0.68, 0.82, 1.0]) * (0.72 - 0.08 * np.clip(1 - e / 0.62, 0, 1))[..., None]
    img += disc[..., None] * inner
    img += rim[..., None] * np.array([1.0, 1.0, 1.0])
    img += halo[..., None] * np.array([0.4, 0.5, 0.8]) * 0.35

    # The shaft, from under the eyespot to the quill, thinning and dimming.
    shaft_start = cs + a * 0.97
    on = s >= shaft_start - 0.005
    along = np.clip((s - shaft_start) / (1.0 - shaft_start), 0.0, 1.0)
    width = 0.0042 - 0.0022 * along
    core = np.exp(-(across**2) / (2 * width**2)) * on
    glow = np.exp(-(across**2) / (2 * (width * 4.0) ** 2)) * on
    bright = (1.0 - 0.35 * along) * np.clip((0.995 - s) / 0.03, 0, 1)
    img += (core * bright)[..., None] * np.array([1.0, 1.0, 1.0])
    img += (glow * bright * 0.22)[..., None] * np.array([0.6, 0.75, 1.0])

    # Nothing reaches the cell's edges, so the quad never shows.
    edge = np.clip(np.minimum(s, 1.0 - s) / 0.03, 0, 1) * np.clip((1.0 - np.abs(q)) / 0.08, 0, 1)
    img = np.clip(img, 0.0, 1.0) * edge[..., None]
    alpha = np.clip(img.max(axis=2) * 1.2, 0.0, 1.0)
    return img, alpha


def dot():
    ys, xs = np.mgrid[0:H, 0:W]
    dx = (xs - (W - 1) / 2) / (W / 2)
    dy = (ys - (H - 1) / 2) / (H / 2)
    r = np.sqrt(dx * dx + dy * dy)
    v = 0.75 * np.exp(-((r / 0.09) ** 2)) + 0.35 * np.exp(-((r / 0.32) ** 2)) * (1 - r).clip(0, 1)
    v = np.clip(v / v.max(), 0.0, 1.0)
    img = v[..., None] * np.array([1.0, 1.0, 1.0])
    return img, np.clip(v * 1.2, 0, 1)


def eyespot():
    ys, xs = np.mgrid[0:H, 0:W]
    dx = (xs - (W - 1) / 2) / (W / 2)
    dy = (ys - (H - 1) / 2) / (H / 2)
    r = np.sqrt(dx * dx + dy * dy)
    pupil = np.clip((0.24 - r) / 0.06, 0.0, 1.0)
    fill = np.clip((0.5 - r) / 0.04, 0.0, 1.0)
    rim = np.clip((r - 0.46) / 0.04, 0.0, 1.0) * np.clip((0.74 - r) / 0.05, 0.0, 1.0)
    halo = np.exp(-np.maximum(r - 0.72, 0.0) ** 2 / (2 * 0.1**2)) * (r > 0.7)
    img = fill[..., None] * np.array([0.7, 0.84, 1.0]) * 0.78
    img = img * (1.0 - pupil[..., None]) + pupil[..., None] * np.array([0.3, 0.45, 0.85]) * 0.45
    img += rim[..., None] * np.array([1.0, 1.0, 1.0])
    img += halo[..., None] * np.array([0.45, 0.6, 0.9]) * 0.4
    edge = np.clip((1.0 - np.maximum(np.abs(dx), np.abs(dy))) / 0.05, 0, 1)
    img = np.clip(img, 0.0, 1.0) * edge[..., None]
    return img, np.clip(img.max(axis=2) * 1.2, 0, 1)


def disc():
    ys, xs = np.mgrid[0:H, 0:W]
    dx = (xs - (W - 1) / 2) / (W / 2)
    dy = (ys - (H - 1) / 2) / (H / 2)
    r = np.sqrt(dx * dx + dy * dy)
    v = np.clip((0.92 - r) / 0.08, 0.0, 1.0) * (0.85 + 0.15 * np.clip(r / 0.8, 0, 1))
    img = v[..., None] * np.array([0.72, 0.85, 1.0])
    return img, np.clip(v, 0, 1)


def main():
    out = args()[0] if args() else "feather.png"
    cells = [feather(), dot(), eyespot(), disc()]
    sheet = np.zeros((H * len(cells), W, 4), dtype=np.float64)
    for i, (rgb, alpha) in enumerate(cells):
        sheet[i * H : (i + 1) * H, :, :3] = srgb(rgb)
        sheet[i * H : (i + 1) * H, :, 3] = alpha
    rgba8 = np.round(sheet * 255.0).astype(np.uint8)
    write_png(out, rgba8)
    print(f"SHEET feather {out} {W}x{H * len(cells)}")


main()
