"""Build a slim poured-concrete tower and write it as binary glTF.

Run headless:
    Blender -b --factory-startup --python scripts/blender/concrete_tower.py -- \
        [OUT.glb | OUT_DIR]

OUT_DIR defaults to `assets/verse/generated/tower`; the script also writes
`concrete_tower.footprint.json` beside the glb.

A Reference model: built from boxes, with a concrete texture this script
bakes itself (no kit pieces or kit images). A square brutalist lookout: a
hollow shaft 5.6 m on a side and 28.5 m tall, walls 0.35 m thick with real
window openings (reveals and a dark glass pane set 0.15 m in), floor slabs
inside every 5 m, expressed outside as thin dark bands, a plinth, a door on
the front, and a roof platform with a parapet, a stair housing, and a mast.

Frame: 1 unit = 1 m. In Blender the front faces -Y and +Z is up; the glTF
export turns that into +Z front and +Y up. The origin is the base's center
on the ground. Everything stays within x, z in [-3.1, 3.1].

Measurements, in glTF terms (y is height):

- Shaft: outer x, z in [-2.8, 2.8], inner [-2.45, 2.45], y 0 to 28.5.
- Floor slabs: 0.25 m thick, top surface at y = 5, 10, 15, 20, and 25
  (each slab spans y in [s - 0.25, s]); a dark band 0.04 m proud marks each
  outside.
- Plinth: y 0 to 0.6, out to 2.95.
- Windows: 0.7 m by 1.8 m slits, sills 1.0 m above each floor; pairs on
  floors 1, 3, and 5, one centered slit on floors 2 and 4; at least 0.8 m
  from every corner. Glass is the inner side of a pane 0.15 m in.
- Door: front (+Z) face, 1.2 m by 2.3 m, its panel recessed 0.2 m.
- Roof slab: y 28.5 to 28.9, x, z in [-3.1, 3.1]; parapet 0.2 m thick to
  y 30.0; stair housing 2 by 2 by 2.4 m with a cap to y 31.45; a mast with a beacon to y 33.68.

The texture `T_Concrete_BaseColor` (256 px, tileable) covers 2.4 m: two
1.2 m pour lifts, 1.2 m form panels with fainter board seams, form-tie holes
every 0.6 m, mottling, and streaks running down. Concrete faces take
world-space UVs (u along the face, v by height), so the pour lines run level
and continuous around the tower. Randomness is seeded from the model name.
"""

import hashlib
import json
import os
import struct
import sys
import tempfile
import zlib

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

import bmesh  # noqa: E402
import bpy  # noqa: E402
import numpy as np  # noqa: E402
from mathutils import Vector  # noqa: E402

NAME = "concrete_tower"
TILE = 2.4  # meters one texture repeat covers
EDGE = 256

# --- Dimensions (Blender frame: x right, y back, z up) ----------------------
HALF = 2.8  # outer half-width of the shaft
WALL = 0.35
INNER = HALF - WALL  # 2.45
TOP = 28.5  # top of the walls, the roof slab's underside
SLABS = [5.0, 10.0, 15.0, 20.0, 25.0]  # floor surfaces
SLAB_T = 0.25
BAND = 0.04  # how far a floor band stands proud
PLINTH_H, PLINTH = 0.6, 2.95
ROOF_T, ROOF = 0.4, 3.1
PARAPET_T, PARAPET_H = 0.2, 1.1
WIN_W, WIN_H, SILL = 0.7, 1.8, 1.0
GLASS_IN, GLASS_T = 0.15, 0.02
DOOR_W, DOOR_H, DOOR_IN = 1.2, 2.3, 0.2
EPS = 0.002  # gap that keeps inset panels off the reveals


def seed():
    return int(hashlib.sha256(NAME.encode()).hexdigest()[:8], 16)


# --- Texture ----------------------------------------------------------------


def periodic_noise(rng, n, fx_scale, fy_scale):
    """Tileable noise: white noise low-passed in frequency space.

    `fx_scale` and `fy_scale` are the cutoffs in cycles per tile across and
    down; a small `fy_scale` stretches features vertically into streaks.
    """
    white = rng.standard_normal((n, n))
    f = np.fft.fftfreq(n) * n
    fy, fx = np.meshgrid(f, f, indexing="ij")
    filt = np.exp(-((fx / fx_scale) ** 2) - ((fy / fy_scale) ** 2))
    out = np.real(np.fft.ifft2(np.fft.fft2(white) * filt))
    return (out - out.mean()) / (out.std() + 1e-9)


def concrete_texture():
    """The concrete base color as an (EDGE, EDGE, 3) uint8 array, row 0 on top."""
    rng = np.random.default_rng(seed())
    n = EDGE
    px_per_m = n / TILE
    rows = np.arange(n)[:, None].astype(np.float64)
    cols = np.arange(n)[None, :].astype(np.float64)
    lum = np.full((n, n), 160.0)

    # Mottling at three scales, and fine grain.
    lum += 6.0 * periodic_noise(rng, n, 3.0, 3.0)
    lum += 4.0 * periodic_noise(rng, n, 10.0, 10.0)
    lum += 2.5 * periodic_noise(rng, n, 40.0, 40.0)
    lum += 2.5 * rng.standard_normal((n, n))

    # Two pour lifts per tile, each a shade apart, darker at its foot where
    # water lingers, with a dark pour line between them.
    lift = int(round(1.2 * px_per_m))  # 128 px
    in_lift = rows % lift
    lum += np.where((rows // lift) % 2 == 0, 2.5, -2.5)
    lum -= 5.0 * np.clip((in_lift - (lift - 40)) / 40.0, 0, 1) ** 2
    pour = np.minimum(in_lift, lift - in_lift)
    lum -= np.where(pour < 1.5, 26.0, np.where(pour < 3.0, 9.0, 0.0))

    # Form panels 1.2 m wide, and fainter board seams every 0.3 m.
    panel = cols % lift
    pd = np.minimum(panel, lift - panel)
    lum -= np.where(pd < 1.0, 13.0, 0.0)
    board = int(round(0.3 * px_per_m))  # 32 px
    bd = np.minimum(cols % board, board - cols % board)
    lum -= np.where(bd < 1.0, 4.0, 0.0)
    # A faint ridge on one side of each board seam, as plank edges leave.
    lum += np.where((cols % board) == 2, 2.0, 0.0)

    # Long vertical streaks: noise stretched down the wall, kept to its dark
    # side so it reads as stains, not stripes.
    streak = periodic_noise(rng, n, 24.0, 1.5)
    lum -= 9.0 * np.clip(streak - 0.4, 0, None)
    # Darker runs below each pour line.
    below = in_lift
    runs = np.clip(periodic_noise(rng, n, 30.0, 4.0) - 0.2, 0, None)
    lum -= 10.0 * runs * np.exp(-below / 70.0)

    # Form-tie holes every 0.6 m, at 0.3 m into each lift and panel, each
    # with a small drip running down from it.
    step = int(round(0.6 * px_per_m))  # 128 px
    for hy in range(step // 2, n, step):
        for hx in range(step // 2, n, step):
            jx = hx + rng.integers(-2, 3)
            jy = hy + rng.integers(-2, 3)
            dx = (cols - jx + n / 2) % n - n / 2
            dy = (rows - jy + n / 2) % n - n / 2
            r = np.sqrt(dx * dx + dy * dy)
            lum += np.where((r >= 3.2) & (r < 4.6), 6.0, 0.0)  # cone rim
            lum -= np.where(r < 3.2, 70.0, 0.0)
            length = rng.uniform(25, 90)
            width = rng.uniform(1.6, 3.0)
            drip = np.exp(-(dx / width) ** 2) * np.where(dy > 3, np.exp(-(dy - 3) / length), 0.0)
            lum -= rng.uniform(8.0, 16.0) * drip

    lum = np.clip(lum, 0, 255)
    # A slight warm tint.
    rgb = np.stack([lum * 1.02 + 1.0, lum * 1.0, lum * 0.95 - 1.0], axis=-1)
    return np.clip(np.round(rgb), 0, 255).astype(np.uint8)


def png_bytes(rgb):
    """Encode RGB8 as a PNG, deterministically, without Pillow."""
    h, w, _ = rgb.shape
    raw = b"".join(b"\0" + rgb[y].tobytes() for y in range(h))

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


# --- Materials --------------------------------------------------------------


def concrete_materials(png_path):
    img = bpy.data.images.load(png_path)
    img.pack()
    img.name = "T_Concrete_BaseColor"
    mats = []
    for name, factor in [("Concrete", 1.0), ("ConcreteDark", 0.75)]:
        m = bpy.data.materials.new(name)
        m.use_nodes = True
        nodes = m.node_tree.nodes
        bsdf = nodes["Principled BSDF"]
        bsdf.inputs["Roughness"].default_value = 0.95
        tex = nodes.new("ShaderNodeTexImage")
        tex.image = img
        if factor == 1.0:
            m.node_tree.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
        else:
            mix = nodes.new("ShaderNodeMix")
            mix.data_type = "RGBA"
            mix.blend_type = "MULTIPLY"
            mix.inputs["Factor"].default_value = 1.0
            m.node_tree.links.new(tex.outputs["Color"], mix.inputs["A"])
            mix.inputs["B"].default_value = (factor, factor, factor, 1.0)
            m.node_tree.links.new(mix.outputs["Result"], bsdf.inputs["Base Color"])
        mats.append(m)
    return mats


# --- Geometry ---------------------------------------------------------------

bm = bmesh.new()
uv_layer = bm.loops.layers.uv.new("UVMap")
CONCRETE, DARK, GLASS, DOOR, METAL = range(5)
_verts = {}


def V(p):
    """A shared vertex at `p`, so touching faces of one part weld."""
    key = tuple(round(c, 5) for c in p)
    v = _verts.get(key)
    if v is None:
        v = bm.verts.new(key)
        _verts[key] = v
    return v


def face(pts, mat, normal):
    """Add a quad whose winding faces `normal`, with world-space UVs."""
    a, b, c = (Vector(p) for p in pts[:3])
    if (b - a).cross(c - a).dot(Vector(normal)) < 0:
        pts = list(reversed(pts))
    f = bm.faces.new([V(p) for p in pts])
    f.material_index = mat
    n = Vector(normal)
    ax = max(range(3), key=lambda i: abs(n[i]))
    for loop in f.loops:
        x, y, z = loop.vert.co
        if ax == 0:
            u, v = (y if n.x > 0 else -y), z
        elif ax == 1:
            u, v = (-x if n.y > 0 else x), z
        else:
            u, v = x, y
        loop[uv_layer].uv = (u / TILE, v / TILE)
    return f


def box(lo, hi, mat, skip=(), top=None, sides=None):
    """An axis-aligned box from `lo` to `hi`, without the faces in `skip`.

    `top` and `sides` override the material of the +Z face and the four
    side faces.
    """
    (x0, y0, z0), (x1, y1, z1) = lo, hi
    quads = {
        "+x": ([(x1, y0, z0), (x1, y1, z0), (x1, y1, z1), (x1, y0, z1)], (1, 0, 0)),
        "-x": ([(x0, y0, z0), (x0, y1, z0), (x0, y1, z1), (x0, y0, z1)], (-1, 0, 0)),
        "+y": ([(x0, y1, z0), (x1, y1, z0), (x1, y1, z1), (x0, y1, z1)], (0, 1, 0)),
        "-y": ([(x0, y0, z0), (x1, y0, z0), (x1, y0, z1), (x0, y0, z1)], (0, -1, 0)),
        "+z": ([(x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)], (0, 0, 1)),
        "-z": ([(x0, y0, z0), (x1, y0, z0), (x1, y1, z0), (x0, y1, z0)], (0, 0, -1)),
    }
    for side, (pts, n) in quads.items():
        if side in skip:
            continue
        m = mat
        if side == "+z" and top is not None:
            m = top
        elif side[1] in "xy" and sides is not None:
            m = sides
        face(pts, m, n)


def ring(z0, z1, inner, outer, mat, skip=("in",), top=None, gap=None):
    """Four boxes around the shaft from `inner` to `outer`, not overlapping.

    "in" in `skip` leaves out the faces toward the shaft. `gap` is an
    (x0, x1) span left open in the front (-Y) side, for the door.
    """
    rest = [s for s in skip if s != "in"]
    # Front and back run the full width; the sides fit between them.
    for sy in (-1, 1):
        y0, y1 = sorted((sy * inner, sy * outer))
        inner_face = ["+y" if sy < 0 else "-y"] if "in" in skip else []
        spans = [(-outer, outer)]
        if gap is not None and sy < 0:
            spans = [(-outer, gap[0]), (gap[1], outer)]
        for x0, x1 in spans:
            box((x0, y0, z0), (x1, y1, z1), mat, skip=inner_face + rest, top=top)
    for sx in (-1, 1):
        x0, x1 = sorted((sx * inner, sx * outer))
        inner_face = ["+x" if sx < 0 else "-x"] if "in" in skip else []
        # The ends against the front and back boxes are hidden.
        box((x0, -inner, z0), (x1, inner, z1), mat, skip=inner_face + rest + ["+y", "-y"], top=top)


def wall(side, openings):
    """One face of the shaft: outer and inner skins on a shared grid, the
    reveals of every opening, and (front and back) the corner end caps.

    `side` is "-y", "+y", "-x", or "+x". Openings are (u0, u1, v0, v1), u
    along the face and v the height.
    """
    axis, sign = side[1], (1 if side[0] == "+" else -1)
    # Front and back walls run the full width; the sides fit between them.
    span = HALF if axis == "y" else INNER

    def P(u, v, d):
        return (u, sign * d, v) if axis == "y" else (sign * d, u, v)

    out_n = (0, sign, 0) if axis == "y" else (sign, 0, 0)
    in_n = tuple(-c for c in out_n)
    t_n = (1, 0, 0) if axis == "y" else (0, 1, 0)
    us = sorted({-span, span, *[o[0] for o in openings], *[o[1] for o in openings]})
    vs = sorted({0.0, TOP, *[o[2] for o in openings], *[o[3] for o in openings]})

    def open_at(u, v):
        return any(o[0] < u < o[1] and o[2] < v < o[3] for o in openings)

    for i in range(len(us) - 1):
        for j in range(len(vs) - 1):
            uc, vc = (us[i] + us[i + 1]) / 2, (vs[j] + vs[j + 1]) / 2
            if open_at(uc, vc):
                continue
            for d, n in ((HALF, out_n), (INNER, in_n)):
                face([P(us[i], vs[j], d), P(us[i + 1], vs[j], d), P(us[i + 1], vs[j + 1], d), P(us[i], vs[j + 1], d)], CONCRETE, n)
    # Reveals, split on the grid lines so they weld to the skins.
    for u0, u1, v0, v1 in openings:
        vv = [v for v in vs if v0 <= v <= v1]
        uu = [u for u in us if u0 <= u <= u1]
        for a, b in zip(vv, vv[1:]):
            face([P(u0, a, INNER), P(u0, b, INNER), P(u0, b, HALF), P(u0, a, HALF)], CONCRETE, t_n)
            face([P(u1, a, INNER), P(u1, b, INNER), P(u1, b, HALF), P(u1, a, HALF)], CONCRETE, tuple(-c for c in t_n))
        for a, b in zip(uu, uu[1:]):
            if v0 > 0:
                face([P(a, v0, INNER), P(b, v0, INNER), P(b, v0, HALF), P(a, v0, HALF)], CONCRETE, (0, 0, 1))
            face([P(a, v1, INNER), P(b, v1, INNER), P(b, v1, HALF), P(a, v1, HALF)], CONCRETE, (0, 0, -1))
    if axis == "y":
        for u, n in ((-span, (-1, 0, 0)), (span, (1, 0, 0))):
            for a, b in zip(vs, vs[1:]):
                face([P(u, a, INNER), P(u, b, INNER), P(u, b, HALF), P(u, a, HALF)], CONCRETE, n)
    return P


def windows(floor):
    """The window spans (u0, u1) on floor `floor` (1 to 5)."""
    if floor % 2 == 1:
        return [(-0.9 - WIN_W / 2, -0.9 + WIN_W / 2), (0.9 - WIN_W / 2, 0.9 + WIN_W / 2)]
    return [(-WIN_W / 2, WIN_W / 2)]


def build():
    # The shaft.
    for side in ("-y", "+y", "-x", "+x"):
        openings = []
        for k, s in enumerate(SLABS, start=1):
            for u0, u1 in windows(k):
                openings.append((u0, u1, s + SILL, s + SILL + WIN_H))
        if side == "-y":
            openings.append((-DOOR_W / 2, DOOR_W / 2, 0.0, DOOR_H))
        P = wall(side, openings)
        sign = 1 if side[0] == "+" else -1
        out_n = (0, sign, 0) if side[1] == "y" else (sign, 0, 0)
        in_n = tuple(-c for c in out_n)
        # Glass: a thin pane 0.15 m in from the outer face.
        for u0, u1, v0, v1 in openings:
            if v0 == 0.0:
                continue
            for d, n in ((HALF - GLASS_IN, out_n), (HALF - GLASS_IN - GLASS_T, in_n)):
                a, b, c, e = u0 + EPS, u1 - EPS, v0 + EPS, v1 - EPS
                face([P(a, c, d), P(b, c, d), P(b, e, d), P(a, e, d)], GLASS, n)

    # Floor slabs inside (top and soffit; their edges hide in the walls).
    for s in SLABS:
        box((-INNER, -INNER, s - SLAB_T), (INNER, INNER, s), CONCRETE, skip=("+x", "-x", "+y", "-y"))
    # Floor bands outside, at each slab.
    for s in SLABS:
        ring(s - SLAB_T, s, HALF, HALF + BAND, DARK, skip=("in",))
    # Plinth, open at the door, and a step.
    ring(0.0, PLINTH_H, HALF, PLINTH, DARK, skip=("in", "-z"), gap=(-DOOR_W / 2, DOOR_W / 2))
    box((-0.75, -3.1, 0.0), (0.75, -HALF, 0.15), DARK, skip=("-z", "+y"))
    # The door: a dark panel recessed in its opening.
    y0 = -HALF + DOOR_IN
    box((-DOOR_W / 2 + EPS, y0, 0.0), (DOOR_W / 2 - EPS, y0 + 0.06, DOOR_H - EPS), DOOR, skip=("-z",))

    # Roof slab, overhanging a little, with dark edges.
    box((-ROOF, -ROOF, TOP), (ROOF, ROOF, TOP + ROOF_T), CONCRETE, sides=DARK)
    roof_top = TOP + ROOF_T
    # Parapet on the slab's edge, its coping dark.
    ring(roof_top, roof_top + PARAPET_H, ROOF - PARAPET_T, ROOF, CONCRETE, skip=("-z",), top=DARK)
    # Stair housing, its cap, its door, and a mast.
    hx0, hx1, hy0, hy1 = -1.0, 1.0, -0.5, 1.5
    h_top = roof_top + 2.4
    box((hx0, hy0, roof_top), (hx1, hy1, h_top), CONCRETE, skip=("-z", "+z"))
    box((hx0 - 0.1, hy0 - 0.1, h_top), (hx1 + 0.1, hy1 + 0.1, h_top + 0.15), DARK)
    box((-0.45, hy0 - 0.05, roof_top), (0.45, hy0, roof_top + 2.0), DOOR, skip=("-z", "+y"))
    m = 0.05
    box((0.7 - m, 1.1 - m, h_top + 0.15), (0.7 + m, 1.1 + m, h_top + 2.2), METAL, skip=("-z",))
    # A small beacon box at the mast's tip.
    box((0.7 - 0.09, 1.1 - 0.09, h_top + 2.2), (0.7 + 0.09, 1.1 + 0.09, h_top + 2.38), METAL)


def main():
    a = kit.args()
    folder = os.path.join(kit.REPO, "assets", "verse", "generated", "tower")
    out = os.path.join(folder, NAME + ".glb")
    if a and a[0].endswith(".glb"):
        out = a[0]
    elif a:
        out = os.path.join(a[0], NAME + ".glb")
    kit.reset()

    tmp = tempfile.mkdtemp()
    png_path = os.path.join(tmp, "T_Concrete_BaseColor.png")
    with open(png_path, "wb") as f:
        f.write(png_bytes(concrete_texture()))
    concrete, dark = concrete_materials(png_path)
    glass = kit.mat("Glass", (0.035, 0.045, 0.06), 0.35)
    door = kit.mat("Door", (0.06, 0.06, 0.065), 0.8)
    metal = kit.mat("Metal", (0.2, 0.2, 0.21), 0.5, 0.6)

    global bm
    build()
    me = bpy.data.meshes.new(NAME)
    bm.to_mesh(me)
    bm.free()
    obj = bpy.data.objects.new(NAME, me)
    bpy.context.scene.collection.objects.link(obj)
    for m in (concrete, dark, glass, door, metal):
        me.materials.append(m)
    kit.flat([obj])
    info = kit.export(out)
    print("MODEL", NAME, info["triangles"])

    footprint = {
        "model": NAME + ".glb",
        "frame": "glTF: 1 unit = 1 m, +Y up, +Z out of the front door, origin on the ground at the center of the base",
        "triangles": info["triangles"],
        "boxes": [
            {"name": "shaft", "center": [0.0, TOP / 2, 0.0], "half_extents": [HALF, TOP / 2, HALF]},
        ],
        "roofs": [],
        "front": [0.0, 3.6],
        "inside": None,
    }
    with open(os.path.join(os.path.dirname(out), NAME + ".footprint.json"), "w") as f:
        f.write(json.dumps(footprint, indent=2) + "\n")


main()
