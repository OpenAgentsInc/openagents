"""Alice's hand-painted face and eyes, painted by script (NumPy only).

`scripts/blender/alice.py` measures her reshaped head (the eye openings,
the eyeballs, and the mouth's line) and calls `face` and `eyes` here. Each
returns an image in a front projection: a pixel at (u, v) is the point
(x, z) of the rectangle the image covers, seen from in front of her, and the
face and eye materials sample it by each surface point's position. The bake
then carries the painting into her base-color atlas.

The painting follows what a high-fidelity reference character's base-color
texture showed (docs/verse/female-character.md, "Lessons from a painted
reference"): skin that varies in warmth rather than one flat tone; warm
cheeks, nose, and lips; a dark lash line on the upper lid that flicks out at
the outer corner; a warm, darker crease in the eye socket; brows painted as
strands; lips with a darker upper lip, a lighter lower lip with a highlight,
and a dark line between them; and eyes whose sclera is no brighter than the
skin, shaded under the upper lid, with a ringed iris and a small catchlight.

Colors are sRGB hex strings, painted in linear light.
"""

import math

import numpy as np


def linear(h):
    h = h.lstrip("#")
    c = np.array([int(h[i:i + 2], 16) / 255 for i in (0, 2, 4)], dtype=np.float64)
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4)


def smoothstep(a, b, x):
    t = np.clip((x - a) / (b - a), 0.0, 1.0)
    return t * t * (3 - 2 * t)


class Canvas:
    """A linear-RGB image over the rectangle [x0, x1] x [z0, z1], meters."""

    def __init__(self, x0, x1, z0, z1, width, height, base):
        self.x0, self.x1, self.z0, self.z1 = x0, x1, z0, z1
        self.w, self.h = width, height
        xs = x0 + (np.arange(width) + 0.5) / width * (x1 - x0)
        zs = z0 + (np.arange(height) + 0.5) / height * (z1 - z0)
        self.X, self.Z = np.meshgrid(xs, zs)
        self.px = (x1 - x0) / width
        self.img = np.ones((height, width, 3)) * linear(base)

    def tint(self, mask, color, amount=1.0):
        """Blends toward `color` by `mask` times `amount`."""
        m = np.clip(mask * amount, 0, 1)[..., None]
        self.img = self.img * (1 - m) + linear(color) * m

    def shade(self, mask, amount):
        """Darkens (amount > 0) or lightens (amount < 0) by `mask`."""
        self.img = self.img * (1 - np.clip(mask, 0, 1)[..., None] * amount)

    def rgba(self, alpha=None):
        a = np.ones(self.img.shape[:2]) if alpha is None else alpha
        return np.concatenate([np.clip(self.img, 0, 1), a[..., None]], axis=-1)


def blob(c, x, z, rx, rz, power=1.0):
    """A soft elliptical Gaussian at (x, z)."""
    r2 = ((c.X - x) / rx) ** 2 + ((c.Z - z) / rz) ** 2
    return np.exp(-(r2 ** power))


def soft_noise(c, seed, scale, octaves=3):
    """Smooth value noise in [-1, 1] at about `scale` meters a feature."""
    rng = np.random.default_rng(seed)
    out = np.zeros_like(c.X)
    amp, total = 1.0, 0.0
    for o in range(octaves):
        s = scale / (2 ** o)
        nx = int((c.x1 - c.x0) / s) + 3
        nz = int((c.z1 - c.z0) / s) + 3
        g = rng.uniform(-1, 1, (nz, nx))
        fx = (c.X - c.x0) / s
        fz = (c.Z - c.z0) / s
        ix, iz = fx.astype(int), fz.astype(int)
        tx, tz = fx - ix, fz - iz
        tx, tz = tx * tx * (3 - 2 * tx), tz * tz * (3 - 2 * tz)
        a = g[iz, ix] * (1 - tx) + g[iz, ix + 1] * tx
        b = g[iz + 1, ix] * (1 - tx) + g[iz + 1, ix + 1] * tx
        out += amp * (a * (1 - tz) + b * tz)
        total += amp
        amp *= 0.5
    return out / total


def segment_distance(c, a, b):
    """Each pixel's distance to segment ab, and its parameter along it."""
    ax, az = a
    bx, bz = b
    dx, dz = bx - ax, bz - az
    L2 = max(dx * dx + dz * dz, 1e-12)
    t = np.clip(((c.X - ax) * dx + (c.Z - az) * dz) / L2, 0, 1)
    px, pz = ax + t * dx, az + t * dz
    return np.hypot(c.X - px, c.Z - pz), t


def stroke(c, pts, widths, soft=0.35):
    """Coverage of a polyline whose half width at each point is `widths`."""
    cover = np.zeros_like(c.X)
    for (a, b), (wa, wb) in zip(zip(pts, pts[1:]), zip(widths, widths[1:])):
        x0, x1 = min(a[0], b[0]) - max(wa, wb) * 3, max(a[0], b[0]) + max(wa, wb) * 3
        z0, z1 = min(a[1], b[1]) - max(wa, wb) * 3, max(a[1], b[1]) + max(wa, wb) * 3
        i0 = max(0, int((x0 - c.x0) / c.px))
        i1 = min(c.w, int((x1 - c.x0) / c.px) + 2)
        j0 = max(0, int((z0 - c.z0) / ((c.z1 - c.z0) / c.h)))
        j1 = min(c.h, int((z1 - c.z0) / ((c.z1 - c.z0) / c.h)) + 2)
        if i1 <= i0 or j1 <= j0:
            continue
        sub = Canvas.__new__(Canvas)
        sub.X, sub.Z = c.X[j0:j1, i0:i1], c.Z[j0:j1, i0:i1]
        d, t = segment_distance(sub, a, b)
        w = wa + (wb - wa) * t
        edge = np.maximum(w * soft, c.px * 0.75)
        cov = np.clip((w - d) / edge + 0.5, 0, 1)
        cover[j0:j1, i0:i1] = np.maximum(cover[j0:j1, i0:i1], cov)
    return cover


def curve_z(points, xs):
    """The polyline `points` (sorted by x) sampled at `xs`, extended flat."""
    px = np.array([p[0] for p in points])
    pz = np.array([p[1] for p in points])
    return np.interp(xs, px, pz)


def lids(opening):
    """The upper and lower lid curves of one eye opening, each a list of
    (x, z) from the inner corner to the outer, from the opening's boundary
    points (x, z)."""
    pts = np.array(opening)
    xs = pts[:, 0]
    sign = 1.0 if xs.mean() > 0 else -1.0
    ax = np.abs(xs)
    inner, outer = ax.min(), ax.max()
    # The corners: the opening's extreme points.
    zi = pts[ax.argmin(), 1]
    zo = pts[ax.argmax(), 1]
    n = 24
    upper, lower = [], []
    for k in range(n + 1):
        t = k / n
        x = inner + (outer - inner) * t
        mid = zi + (zo - zi) * t
        near = np.abs(ax - x) < (outer - inner) / n * 1.5
        zs = pts[near, 1] if near.any() else np.array([mid])
        up = zs[zs >= mid - 1e-4]
        lo = zs[zs <= mid + 1e-4]
        upper.append((sign * x, up.max() if len(up) else mid))
        lower.append((sign * x, lo.min() if len(lo) else mid))
    upper[0] = lower[0] = (sign * inner, zi)
    upper[-1] = lower[-1] = (sign * outer, zo)
    return upper, lower


PALETTE = {
    "skin": "#E9AF92",
    "skin_side": "#DDA287",
    "blush": "#E48D7A",
    "nose": "#E59A86",
    "socket": "#9C6050",
    "lash": "#1E120D",
    "brow": "#3F1E12",
    "brow_light": "#6A3420",
    "lip_upper": "#B4524F",
    "lip_lower": "#C9645C",
    "lip_shine": "#E29A8E",
    "lip_line": "#5A2220",
    "caruncle": "#D8857A",
}


def face(marks, size=2048):
    """Alice's painted face, a front projection over FACE_RECT. `marks`:
    `openings` (two lists of (x, z) eye-opening boundary points), `mouth`
    ((x, z) points of the mouth's line), and `nose_tip` (z). Returns RGBA
    floats, linear, rows bottom to top (Blender's order)."""
    x0, x1, z0, z1 = FACE_RECT
    c = Canvas(x0, x1, z0, z1, size, size, PALETTE["skin"])
    P = PALETTE
    # Skin that varies: a little darker and warmer toward the sides of the
    # face and the jaw, lighter on the forehead and the bridge.
    side = smoothstep(0.035, 0.085, np.abs(c.X))
    c.tint(side, P["skin_side"], 0.55)
    n = soft_noise(c, 3, 0.03)
    c.shade(n, 0.035)
    fine = soft_noise(c, 5, 0.004, octaves=2)
    c.shade(fine, 0.012)
    c.shade(blob(c, 0, 1.72, 0.05, 0.03), -0.04)
    # Warm cheeks, nose, and chin.
    for s in (1, -1):
        c.tint(blob(c, s * 0.047, 1.622, 0.021, 0.017), P["blush"], 0.38)
    nose_z = marks["nose_tip"]
    c.tint(blob(c, 0, nose_z - 0.002, 0.008, 0.008), P["nose"], 0.45)
    c.tint(blob(c, 0, 1.556, 0.012, 0.008), P["blush"], 0.12)
    # The nose's planes: shadowed sides, a light bridge, soft nostrils, and
    # the creases around its wings.
    for s in (1, -1):
        c.shade(blob(c, s * 0.0115, nose_z + 0.016, 0.0028, 0.020), 0.10)
        c.shade(blob(c, s * 0.0068, nose_z - 0.0150, 0.0030, 0.0013, 1.2), 0.26)
        wing = stroke(c, [(s * 0.0128, nose_z - 0.002), (s * 0.0142, nose_z - 0.008),
                          (s * 0.0125, nose_z - 0.0135), (s * 0.0100, nose_z - 0.0160)],
                      [0.0005, 0.0009, 0.0009, 0.0005], soft=1.6)
        c.shade(wing, 0.10)
    c.shade(blob(c, 0, nose_z + 0.022, 0.0022, 0.018), -0.07)
    c.shade(blob(c, 0, nose_z - 0.0175, 0.0035, 0.0011), 0.08)
    # The philtrum, faintly.
    c.shade(blob(c, 0, nose_z - 0.025, 0.0025, 0.004), 0.05)
    # Eyes: the socket's warm crease, the lash lines, and the corners.
    for opening in marks["openings"]:
        upper, lower = lids(opening)
        s = 1.0 if upper[-1][0] > 0 else -1.0
        xin, xout = upper[0][0], upper[-1][0]
        span = abs(xout - xin)
        zu = curve_z(sorted(upper), c.X)
        zl = curve_z(sorted(lower), c.X)
        u = (c.X - xin) * s / span
        inside = smoothstep(-0.25, 0.05, u) * smoothstep(1.35, 0.95, u)
        above = c.Z - zu
        crease = np.exp(-(((above - 0.0035) / 0.0035) ** 2)) * inside
        c.tint(crease, P["socket"], 0.30)
        c.shade(crease, 0.10)
        below = zl - c.Z
        c.shade(np.exp(-(((below - 0.004) / 0.0035) ** 2)) * inside * (0.6 + 0.4 * u.clip(0, 1)), 0.08)
        # The upper lash line: thin at the inner corner, full at the outer,
        # from just inside the opening to above its edge.
        t = np.array([abs(p[0] - xin) / span for p in upper])
        lash_w = 0.00045 + 0.00085 * np.clip(t, 0, 1) ** 0.8
        pts = [(p[0], p[1] + 0.0002) for p in upper]
        cover = stroke(c, pts, lash_w, soft=0.5)
        # The wing at the outer corner, up and out.
        ox, oz = upper[-1]
        wing = stroke(c, [(ox - s * 0.002, oz + 0.0006), (ox + s * 0.0025, oz + 0.0016),
                          (ox + s * 0.0052, oz + 0.0031)], [0.0009, 0.0006, 0.00012], soft=0.6)
        c.tint(np.maximum(cover, wing), P["lash"], 0.92)
        # The lower lash line: fainter, from the outer corner inward.
        tl = np.array([abs(p[0] - xin) / span for p in lower])
        low = stroke(c, [(p[0], p[1] - 0.0003) for p in lower], 0.0002 + 0.0003 * tl, soft=0.8)
        fade = smoothstep(0.2, 0.8, u)
        c.tint(low * fade, P["lash"], 0.45)
        # The inner corner's warm caruncle.
        c.tint(blob(c, xin + s * 0.0012, upper[0][1], 0.0016, 0.0012), P["caruncle"], 0.55)
        brows(c, upper, s)
    lips(c, marks["mouth"])
    # Under the jaw: the neck in the chin's shadow.
    c.shade(smoothstep(1.548, 1.528, c.Z) * smoothstep(0.075, 0.02, np.abs(c.X)), 0.14)
    return c.rgba()


def brows(c, upper, s):
    """A brow over the eye whose upper lid is `upper`, painted as a soft base
    and a few hundred strands that rise at the head and follow the arch to
    the tail."""
    xin, zin = upper[0]
    xout, zout = upper[-1]
    top = max(p[1] for p in upper)
    span = abs(xout - xin)
    head = (xin - s * 0.0012, top + 0.0078)
    arch = (xin + s * span * 0.66, top + 0.0108)
    tail = (xout + s * 0.0055, top + 0.0062)
    path = [head, (xin + s * span * 0.25, top + 0.0094), arch, (xin + s * span * 0.92, top + 0.0098), tail]
    widths = [0.0026, 0.0024, 0.0019, 0.0013, 0.0004]
    base = stroke(c, path, widths, soft=1.4)
    c.tint(base, PALETTE["brow_light"], 0.62)
    rng = np.random.default_rng(11 if s > 0 else 12)
    xs = np.array([p[0] for p in path])
    zs = np.array([p[1] for p in path])
    ws = np.array(widths)
    seg = np.concatenate([[0], np.cumsum(np.hypot(np.diff(xs), np.diff(zs)))])
    total = seg[-1]
    cover = np.zeros_like(c.X)
    for k in range(340):
        # Denser toward the head.
        f = rng.random() ** 1.25
        d = f * total
        x = np.interp(d, seg, xs)
        z = np.interp(d, seg, zs)
        w = np.interp(d, seg, ws)
        k2 = min(len(xs) - 2, np.searchsorted(seg, d, side="right") - 1)
        tx, tz = xs[k2 + 1] - xs[k2], zs[k2 + 1] - zs[k2]
        ang = math.atan2(tz, tx * s)
        # At the head strands stand up; along the arch they lie with it.
        lean = (1 - smoothstep(0.0, 0.3, f)) * 1.0 + 0.25
        a = ang + lean * (0.9 if f < 0.3 else 0.35) + rng.normal(0, 0.12)
        z += rng.uniform(-1, 1) * w * 0.9
        L = rng.uniform(0.0028, 0.0046) * (1 - 0.4 * f)
        dx, dz = math.cos(a) * L * s, math.sin(a) * L
        cov = stroke(c, [(x - dx * 0.5, z - dz * 0.5), (x + dx * 0.5, z + dz * 0.5)],
                     [0.00011, 0.00006], soft=0.8)
        cover = np.maximum(cover, cov * rng.uniform(0.55, 0.95))
    c.tint(cover, PALETTE["brow"], 0.9)


def lips(c, mouth):
    """Lips around the mouth's line: a Cupid's bow, a darker upper lip, a
    fuller lighter lower lip with a highlight, and a dark line between."""
    pts = np.array(mouth)
    half = np.abs(pts[:, 0]).max()
    zm = float(np.median(pts[:, 1]))
    X, Z = c.X, c.Z
    ax = np.abs(X)
    r = np.clip(ax / half, 0, 1.2)
    # The line, its corners turned up a little.
    line = zm + 0.0006 * np.clip(r, 0, 1) ** 2
    upper_h = 0.0086 * np.clip(1 - r ** 2.2, 0, 1) ** 0.55
    upper_h *= 1 - 0.16 * np.exp(-((X / 0.0026) ** 2))
    upper_h += 0.0007 * np.exp(-(((ax - 0.0046) / 0.0024) ** 2)) * (r < 1)
    lower_h = 0.0098 * np.clip(1 - (ax / (half * 0.94)) ** 2, 0, 1) ** 0.6
    edge = 0.0006
    up = smoothstep(line + upper_h + edge, line + upper_h - edge, Z) * smoothstep(line - edge, line + edge, Z)
    lo = smoothstep(line - lower_h - edge, line - lower_h + edge, Z) * smoothstep(line + edge, line - edge, Z)
    up *= smoothstep(0.0, 0.0012, upper_h)
    lo *= smoothstep(0.0, 0.0012, lower_h)
    P = PALETTE
    c.tint(up, P["lip_upper"], 0.85)
    c.tint(lo, P["lip_lower"], 0.85)
    # Deeper toward the line, a highlight on the lower lip's fullest part.
    c.shade(up * smoothstep(line + upper_h, line, Z) * 0.6, 0.18)
    c.tint(lo * blob(c, 0, line - lower_h * 0.45, 0.009, 0.0022), P["lip_shine"], 0.55)
    # The border catches light just above the upper lip.
    c.shade(np.exp(-(((Z - (line + upper_h + 0.0008)) / 0.0006) ** 2)) * (r < 0.95), -0.08)
    # The line between the lips, darker into the corners.
    lw = 0.00028 + 0.00025 * np.exp(-((X / 0.008) ** 2))
    d = np.abs(Z - line)
    mouthline = np.clip((lw - d) / 0.00025 + 0.5, 0, 1) * smoothstep(half + 0.0012, half - 0.0008, ax)
    c.tint(mouthline, P["lip_line"], 0.9)
    for s in (1, -1):
        c.shade(blob(c, s * (half + 0.0004), zm + 0.0008, 0.0016, 0.0011), 0.18)
    # A soft shadow under the lower lip.
    c.shade(np.exp(-(((Z - (line - lower_h - 0.0035)) / 0.0025) ** 2)) * smoothstep(0.02, 0.008, ax), 0.04)


FACE_RECT = (-0.105, 0.105, 1.525, 1.735)
# Both eyes: x from -EYE_HALF to EYE_HALF, z over EYE_Z.
EYE_RECT = (-0.068, 0.068, 1.627, 1.695)

IRIS = {"center": "#A2843E", "mid": "#5E7C38", "outer": "#3D5A2A", "ring": "#18200F",
        "pupil": "#090706", "sclera": "#DAD0C6", "sclera_corner": "#D3B2A6", "lid_shadow": "#5A3A30"}


def eyes(marks, width=2048):
    """Both eyes' painting, a front projection over EYE_RECT: an off-white
    sclera no brighter than the skin, pinker toward the corners and shaded
    under the upper lid; a hazel-green iris with radial fibers, a dark limbal
    ring, a pupil, and a catchlight up and to her left in both eyes. `marks`:
    `irises` (each iris center (x, z)), `iris_radius`, and `openings`."""
    x0, x1, z0, z1 = EYE_RECT
    height = int(round(width * (z1 - z0) / (x1 - x0)))
    c = Canvas(x0, x1, z0, z1, width, height, IRIS["sclera"])
    ri = marks["iris_radius"]
    blank = c.img.copy()
    out = c.img.copy()
    for (ix, iz), opening in zip(marks["irises"], marks["openings"]):
        upper, lower = lids(opening)
        s = 1.0 if ix > 0 else -1.0
        # Each eye paints its own half of the image.
        c.img = blank.copy()
        dx, dz = c.X - ix, c.Z - iz
        r = np.hypot(dx, dz)
        corner = smoothstep(ri * 1.1, ri * 2.6, r)
        c.tint(corner, IRIS["sclera_corner"], 0.45)
        ang = np.arctan2(dz, dx)
        rn = r / ri
        # Iris: amber at the pupil, green, then darker to the ring.
        iris = smoothstep(1.02, 0.97, rn)
        col = np.where(rn[..., None] < 0.55,
                       linear(IRIS["center"]) * (1 - smoothstep(0.3, 0.55, rn)[..., None])
                       + linear(IRIS["mid"]) * smoothstep(0.3, 0.55, rn)[..., None],
                       linear(IRIS["mid"]) * (1 - smoothstep(0.55, 0.92, rn)[..., None])
                       + linear(IRIS["outer"]) * smoothstep(0.55, 0.92, rn)[..., None])
        rng = np.random.default_rng(21 if s > 0 else 22)
        phases = rng.uniform(0, 2 * np.pi, 3)
        fibers = (0.55 * np.sin(ang * 37 + phases[0]) + 0.3 * np.sin(ang * 71 + phases[1] + rn * 3)
                  + 0.15 * np.sin(ang * 13 + phases[2]))
        col = col * (1 + 0.22 * fibers[..., None] * smoothstep(0.35, 0.6, rn)[..., None])
        m = iris[..., None]
        c.img = c.img * (1 - m) + col * m
        c.tint(smoothstep(0.80, 0.98, rn) * iris, IRIS["ring"], 0.85)
        c.tint(smoothstep(0.43, 0.39, rn), IRIS["pupil"], 1.0)
        # The upper lid's shadow across the eye, and the lashes' line.
        zu = curve_z(sorted(upper), c.X)
        lid = smoothstep(zu - 0.0032, zu - 0.0002, c.Z)
        c.tint(lid, IRIS["lid_shadow"], 0.5)
        zl = curve_z(sorted(lower), c.X)
        c.shade(smoothstep(zl + 0.0015, zl - 0.0002, c.Z), 0.18)
        # The catchlight, up and to her left (the image's right) in both
        # eyes, and a fainter one opposite.
        c.tint(blob(c, ix + 0.34 * ri, iz + 0.38 * ri, 0.15 * ri, 0.15 * ri, 2.5), "#FFFFFF", 1.0)
        c.tint(blob(c, ix - 0.36 * ri, iz - 0.30 * ri, 0.07 * ri, 0.07 * ri, 2.5), "#FFFFFF", 0.55)
        half = (c.X * s > 0)[..., None]
        out = np.where(half, c.img, out)
    c.img = out
    return c.rgba()


# The hair strips: lanes of painted strands every card samples, root at the
# left and tip at the right. The first two lanes are the under layer's
# (darker), the next five the top layer's, each a little lighter or darker
# than the next so neighboring cards read as separate locks, and the last
# the wisps'. A natural auburn: brown in the shade, copper in the light.
HAIR_LANES = 8
HAIR = {"dark": "#24100A", "mid": "#5C2616", "light": "#9A4A2A", "gloss": "#C27A52"}
# Each lane's brightness.
LANE_SHADE = (0.66, 0.74, 0.86, 0.94, 1.0, 1.06, 0.97, 0.9)


def hair_strips(length, height, lanes=HAIR_LANES, seed=31):
    """RGBA (linear, rows bottom to top) of `lanes` hair strips stacked in
    `height` rows, each `length` pixels from root to tip: fine strands with
    dark gaps between them, shadow toward the clump's edges, dark roots, a
    sheen near the crown, lighter ends, and tips cut into strands of
    different lengths, frayed at the edges (the alpha)."""
    rng = np.random.default_rng(seed)
    t = (np.arange(length) + 0.5) / length
    rows = np.zeros((height, length, 3))
    alpha = np.zeros((height, length))
    dark, mid, light, gloss = (linear(HAIR[k]) for k in ("dark", "mid", "light", "gloss"))
    edges = np.linspace(0, height, lanes + 1).round().astype(int)
    for lane in range(lanes):
        h0, h1 = edges[lane], edges[lane + 1]
        n = h1 - h0
        s = (np.arange(n) + 0.5) / n
        S, T = np.meshgrid(s, t, indexing="ij")
        # Strands: noise across the strip, stretched along it, a few
        # octaves, each wandering a little as it runs root to tip.
        v = np.zeros_like(S)
        for freq, amp in ((7, 0.30), (19, 0.24), (47, 0.22), (95, 0.14)):
            phase = rng.uniform(0, 1, freq + 2)
            wander = 0.25 * np.sin(2 * np.pi * (T * rng.uniform(0.6, 1.4) + rng.uniform()))
            x = (S + wander / freq) * freq
            i = np.floor(x).astype(int) % (freq + 1)
            f = x - np.floor(x)
            f = f * f * (3 - 2 * f)
            v += amp * (phase[i] * (1 - f) + phase[(i + 1) % (freq + 1)] * f - 0.5)
        tone = 0.5 + v
        # The gaps between strands go dark.
        gaps = np.clip((0.33 - tone) / 0.2, 0, 1)
        edge = np.minimum(S, 1 - S)
        clump = 0.74 + 0.26 * np.clip(edge / 0.33, 0, 1) ** 0.8
        root = 0.45 + 0.55 * np.clip(T / 0.16, 0, 1) ** 0.7
        ends = 1 + 0.12 * np.clip((T - 0.55) / 0.45, 0, 1)
        k = np.clip(tone * clump * root * ends * (1 - 0.25 * gaps), 0, 1.4) * LANE_SHADE[lane]
        col = np.where((k < 0.6)[..., None], dark + (mid - dark) * (k / 0.6)[..., None],
                       mid + (light - mid) * np.clip((k - 0.6) / 0.6, 0, 1)[..., None])
        # Gloss: a soft band near the crown and a fainter one lower, broken
        # by the strands, as light catches smooth hair.
        band = np.exp(-(((T - 0.22) / 0.06) ** 2)) + 0.45 * np.exp(-(((T - 0.5) / 0.05) ** 2))
        shine = np.clip(band * (0.6 + 0.8 * (tone - 0.35)) * clump, 0, 1) * 0.55 * LANE_SHADE[lane]
        col = col * (1 - shine[..., None]) + gloss * shine[..., None]
        # Tips: each strand ends at its own length; the edges fray.
        strand_len = 0.84 + 0.13 * np.interp(s, np.linspace(0, 1, 40), rng.uniform(0, 1, 40))
        fine = 0.04 * np.interp(s, np.linspace(0, 1, 160), rng.uniform(-1, 1, 160))
        a = np.clip((strand_len[:, None] + fine[:, None] - T) / 0.012, 0, 1)
        a *= np.clip((edge + 0.5 * fine[:, None] - 0.03) / 0.05, 0, 1)
        if lane == lanes - 1:
            # Wisps: thin, nearly every strand reaching the tip.
            a = np.clip((0.97 - T) / 0.02, 0, 1) * np.clip((edge - 0.15) / 0.1, 0, 1)
        rows[h0:h1] = col
        alpha[h0:h1] = a
    return np.concatenate([rows, alpha[..., None]], axis=-1)
