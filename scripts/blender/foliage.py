"""Build Everglade's foliage, woodland, and garden greenery as binary glTF.

Run headless:
    Blender -b --factory-startup --python scripts/blender/foliage.py -- [OUT_DIR] [NAME ...]

OUT_DIR defaults to `assets/verse/generated/foliage`. Every model is a
Reference-mode model (`docs/verse/asset-runbook.md`): built here from
primitives in the Stylized Nature MegaKit's look, sampling only that kit's
admitted textures (bark, the broadleaf cluster, the leaf atlas, flowers,
grass, and rocks). `scripts/blender/foliage_admit.py` points each image at
the admitted file in `assets/verse/everglade/nature/`, so the pack carries no
new image.

The kinds of objects were chosen by studying a local list of a game
client's woodland file names (which kinds of canopy, roots, stumps, logs,
bushes, rocks, and clearing props a forest has and how they group). No mesh,
texture, or image of that client was opened; nothing here is derived from it.

Style rules, from the nature kit:

- Canopies are clumps of alpha-masked leaf cards, each card a quad facing
  out of its clump, with normals that point out of the whole crown so the
  crown shades as one soft mass. A dark core inside each clump hides the gaps
  between cards from below.
- Trunks and limbs are smooth-shaded, tapered tubes with a root flare,
  sampling the bark texture around and along their length.
- Small plants are a few cards or blades; rocks are faceted and flat-shaded.

The frame: 1 unit = 1 m; fronts face -Y in Blender, +Z after export; the
origin is on the ground at the base's center (a wall-hung piece's origin is on
the wall plane at its base). Materials whose names start with `Bark` or `Core`
are solid; the far level of detail (`everglade_lod.py`) collapses them and
thins the other, card, materials.

Randomness is seeded from each model's name, so a rebuild gives the same
model.
"""

import hashlib
import math
import os
import random
import sys

import bpy
from mathutils import Matrix, Vector

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

NATURE = os.path.join(kit.REPO, "assets", "verse", "everglade", "nature")
OUT = os.path.join(kit.REPO, "assets", "verse", "generated", "foliage")
UP = Vector((0, 0, 1))

# Image regions, as (u0, v0, u1, v1) in image coordinates (v down from the
# top), measured from the admitted images' alpha.
CLUSTER = ("Leaves_NormalTree_C", (0.09, 0.12, 0.88, 0.91))
FERN = ("Leaves", (0.016, 0.455, 0.287, 0.984))
ROUND_LEAF = ("Leaves", (0.773, 0.045, 0.959, 0.266))
HEART_LEAF = ("Leaves", (0.773, 0.316, 0.959, 0.559))
LONG_LEAF = ("Leaves", (0.053, 0.047, 0.182, 0.428))
CLOVER = ("Leaves", (0.611, 0.857, 0.742, 0.98))
FLOWER = {
    "cream": ("Flowers", (0.012, 0.0, 0.457, 0.442)),
    "violet": ("Flowers", (0.559, 0.02, 0.977, 0.442)),
    "pink": ("Flowers", (0.34, 0.337, 0.691, 0.687)),
    "red": ("Flowers", (0.0, 0.57, 0.43, 0.992)),
    "yellow": ("Flowers", (0.648, 0.59, 0.996, 0.996)),
}
GRASS = ("Grass", (0.062, 0.0, 0.1, 1.0))
GRASS_LIGHT = ("Grass", (0.168, 0.0, 0.205, 1.0))
FIR_BRANCH = ("Leaf_Pine_C", (0.197, 0.029, 0.787, 0.951))
BARK = "Bark_NormalTree"
ROCKS = "Rocks_Diffuse"


def seed(name):
    return int(hashlib.sha256(name.encode()).hexdigest()[:8], 16)


# --- Materials ---------------------------------------------------------------

_MATS = {}


def tex(name, image, tint=(1.0, 1.0, 1.0)):
    """A material sampling admitted nature image `image`, multiplied by `tint`.

    The image packed into the glb is a 16 px stand-in; admission replaces it
    with the admitted file of the same name.
    """
    if name in _MATS:
        return _MATS[name]
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nodes = m.node_tree.nodes
    bsdf = nodes["Principled BSDF"]
    bsdf.inputs["Roughness"].default_value = 0.9
    img = bpy.data.images.get(image)
    if img is None:
        img = bpy.data.images.load(os.path.join(NATURE, image + ".png"))
        img.scale(16, 16)
        img.pack()
        img.name = image
    t = nodes.new("ShaderNodeTexImage")
    t.image = img
    mix = nodes.new("ShaderNodeMix")
    mix.data_type = "RGBA"
    mix.blend_type = "MULTIPLY"
    mix.inputs["Factor"].default_value = 1.0
    m.node_tree.links.new(t.outputs["Color"], mix.inputs["A"])
    mix.inputs["B"].default_value = (*tint, 1.0)
    m.node_tree.links.new(mix.outputs["Result"], bsdf.inputs["Base Color"])
    m.node_tree.links.new(t.outputs["Alpha"], bsdf.inputs["Alpha"])
    _MATS[name] = m
    return m


def flat(name, rgb, rough=0.9, emit=None):
    if name not in _MATS:
        _MATS[name] = kit.mat(name, rgb, rough, emit=emit)
    return _MATS[name]


# Shared palettes: the same names and values in every model, so the pack keeps
# one copy of each material.
def leaf(kind="mid"):
    # The image is olive green with no blue: red shifts the hue toward
    # yellow, green sets the brightness.
    tints = {
        "mid": (0.8, 1.0, 1.0),
        "dark": (0.55, 0.78, 1.0),
        "light": (1.0, 1.0, 1.0),
        "willow": (1.0, 1.0, 1.0),
        "hedge": (0.55, 0.8, 1.0),
        "ivy": (0.6, 0.85, 1.0),
    }
    return tex("Leaf_" + kind.capitalize(), CLUSTER[0], tints[kind])


def core(kind="mid"):
    rgb = {
        "mid": (0.05, 0.13, 0.0),
        "dark": (0.035, 0.1, 0.0),
        "light": (0.07, 0.15, 0.0),
        "willow": (0.07, 0.15, 0.0),
        "hedge": (0.035, 0.1, 0.0),
        "fir": (0.02, 0.06, 0.01),
    }[kind]
    return flat("Core_" + kind.capitalize(), rgb, 1.0)


def bark(kind="brown"):
    tints = {
        "brown": (1.0, 1.0, 1.0),
        "dark": (0.6, 0.55, 0.5),
        "gray": (0.78, 0.74, 0.7),
        "silver": (1.0, 1.0, 1.0),
    }
    if kind == "silver":
        return tex("Bark_Silver", BARK, (1.0, 0.96, 0.9))
    return tex("Bark_" + kind.capitalize(), BARK, tints[kind])


def atlas(name, region, tint=(1.0, 1.0, 1.0)):
    return tex(name, region[0], tint)


# --- Geometry ------------------------------------------------------------------


class Mesh:
    """Faces for one material: vertices, and per corner a UV and a normal."""

    def __init__(self):
        self.verts, self.faces, self.uvs, self.normals = [], [], [], []
        self.index = {}

    def vert(self, p):
        """The index of a vertex at `p`, welded with any already there."""
        key = tuple(round(c, 5) for c in p)
        if key not in self.index:
            self.index[key] = len(self.verts)
            self.verts.append(Vector(p))
        return self.index[key]

    def face(self, pts, uvs, normals=None):
        idx = tuple(self.vert(p) for p in pts)
        if len(set(idx)) < 3:
            return
        self.faces.append(idx)
        self.uvs.append([tuple(u) for u in uvs])
        if normals is None:
            a, b, c = (Vector(p) for p in pts[:3])
            n = (b - a).cross(c - a)
            if n.length < 1e-9 and len(pts) > 3:
                n = (Vector(pts[2]) - a).cross(Vector(pts[3]) - a)
            normals = [n.normalized()] * len(pts)
        self.normals.append([Vector(n).normalized() for n in normals])


class Model:
    """A model being built: one `Mesh` per material."""

    def __init__(self, name):
        self.name = name
        self.rng = random.Random(seed(name))
        self.parts = {}
        # Moves the whole model, for wall-hung pieces whose origin is the
        # wall's center line rather than its face.
        self.offset = Vector((0, 0, 0))

    def mesh(self, material):
        if material.name not in self.parts:
            self.parts[material.name] = (material, Mesh())
        return self.parts[material.name][1]

    def build(self):
        for name, (material, m) in self.parts.items():
            if not m.faces:
                continue
            me = bpy.data.meshes.new(self.name + "_" + name)
            me.from_pydata([tuple(v + self.offset) for v in m.verts], [], m.faces)
            me.update()
            uv = me.uv_layers.new(name="UVMap")
            loop_normals = []
            for poly, uvs, ns in zip(me.polygons, m.uvs, m.normals):
                for li, u, n in zip(poly.loop_indices, uvs, ns):
                    uv.data[li].uv = u
                    loop_normals.append(n)
            me.materials.append(material)
            me.shade_smooth()
            me.normals_split_custom_set(loop_normals)
            o = bpy.data.objects.new(self.name + "_" + name, me)
            bpy.context.scene.collection.objects.link(o)


def uv_in(region, u, v):
    """Blender UV for a point `(u, v)` in 0..1 of an image region (v up)."""
    u0, v0, u1, v1 = region
    return (u0 + (u1 - u0) * u, 1.0 - (v1 - (v1 - v0) * v))


def frame(d):
    """Two unit vectors perpendicular to `d` and each other."""
    d = Vector(d).normalized()
    a = UP if abs(d.z) < 0.95 else Vector((1, 0, 0))
    s = d.cross(a).normalized()
    return s, s.cross(d).normalized()


def tube(model, material, path, radii, sides=7, tile=1.2, cap=False, jitter=0.0, flat_shade=False):
    """A tapered tube along `path` with bark-like UVs around and along it."""
    m = model.mesh(material)
    path = [Vector(p) for p in path]
    rings, along = [], 0.0
    prev_s = None
    for i, p in enumerate(path):
        if i == 0:
            d = path[1] - path[0]
        elif i == len(path) - 1:
            d = path[-1] - path[-2]
        else:
            d = (path[i + 1] - path[i - 1])
        s, t = frame(d)
        if prev_s is not None:
            # Parallel transport, so the tube doesn't twist.
            s = (prev_s - prev_s.dot(d.normalized()) * d.normalized()).normalized()
            t = d.normalized().cross(s).normalized()
        prev_s = s
        if i:
            along += (path[i] - path[i - 1]).length
        ring = []
        for k in range(sides + 1):
            a = 2 * math.pi * k / sides
            off = s * math.cos(a) + t * math.sin(a)
            r = radii[i] * (1 + (model.rng.uniform(-jitter, jitter) if jitter and k < sides else 0))
            ring.append((p + off * r, off, (k / sides * max(radii[0], 0.15) * 2 * math.pi / tile, along / tile)))
        # Close the seam with the first corner's position.
        ring[sides] = (ring[0][0], ring[0][1], ring[sides][2])
        rings.append(ring)
    for a, b in zip(rings, rings[1:]):
        for k in range(sides):
            corners = [a[k], a[k + 1], b[k + 1], b[k]]
            m.face(
                [c[0] for c in corners],
                [c[2] for c in corners],
                None if flat_shade else [c[1] for c in corners],
            )
    if cap:
        end = rings[-1]
        c = path[-1]
        for k in range(sides):
            m.face([end[k][0], end[k + 1][0], c], [(0.5, 0.5)] * 3, None)


def card(model, material, region, center, normal, w, h, spin=0.0, shade=None, bend=0.0):
    """A quad facing `normal`, `w` by `h` m, mapped to an image region.

    `shade` is the normal its corners shade with (default: its own); `bend`
    folds the card along its long axis by that many meters at the edges.
    """
    m = model.mesh(material)
    c = Vector(center)
    n = Vector(normal).normalized()
    s, t = frame(n)
    rot = Matrix.Rotation(spin, 3, n)
    s, t = rot @ s, rot @ t
    pts = []
    for (u, v) in [(0, 0), (1, 0), (1, 1), (0, 1)]:
        p = c + s * (u - 0.5) * w + t * (v - 0.5) * h
        p += n * (-bend * abs(u - 0.5) * 2)
        pts.append(p)
    uvs = [uv_in(region, u, v) for (u, v) in [(0, 0), (1, 0), (1, 1), (0, 1)]]
    m.face(pts, uvs, [shade or n] * 4 if not callable(shade) else [shade(p) for p in pts])


def blob(model, material, center, radii, subdiv=1, jitter=0.12, flat_shade=False, tile=2.0, squash_base=False):
    """A lumpy ellipsoid from an icosphere, box-mapped."""
    import bmesh

    bm = bmesh.new()
    bmesh.ops.create_icosphere(bm, subdivisions=subdiv, radius=1.0)
    m = model.mesh(material)
    c = Vector(center)
    rng = model.rng
    moved = {}
    for v in bm.verts:
        k = 1 + rng.uniform(-jitter, jitter)
        p = Vector((v.co.x * radii[0] * k, v.co.y * radii[1] * k, v.co.z * radii[2] * k))
        if squash_base and p.z < -radii[2] * 0.3:
            p.z = -radii[2] * 0.3 + (p.z + radii[2] * 0.3) * 0.25
        moved[v.index] = p
    for f in bm.faces:
        pts = [c + moved[v.index] for v in f.verts]
        normal = (pts[1] - pts[0]).cross(pts[2] - pts[0]).normalized()
        ax = max(range(3), key=lambda i: abs(normal[i]))
        uvs = [[(p.y / tile, p.z / tile), (p.x / tile, p.z / tile), (p.x / tile, p.y / tile)][ax] for p in pts]
        ns = None if flat_shade else [(moved[v.index]).normalized() for v in f.verts]
        m.face(pts, uvs, ns)
    bm.free()


def lathe(model, material, profile, center=(0, 0, 0), segs=8, flat_shade=True, uv_scale=1.0):
    """Revolve `(radius, height)` points about a vertical axis at `center`."""
    m = model.mesh(material)
    c = Vector(center)
    rings = []
    for r, z in profile:
        rings.append([c + Vector((r * math.cos(2 * math.pi * k / segs), r * math.sin(2 * math.pi * k / segs), z))
                      for k in range(segs)])
    for i, (a, b) in enumerate(zip(rings, rings[1:])):
        for k in range(segs):
            j = (k + 1) % segs
            pts = [a[k], a[j], b[j], b[k]]
            if (pts[0] - pts[1]).length < 1e-6:
                pts = [a[k], b[j], b[k]]
            elif (pts[2] - pts[3]).length < 1e-6:
                pts = [a[k], a[j], b[j]]
            uvs = [((p.x - c.x) * uv_scale + 0.5, (p.y - c.y) * uv_scale + 0.5) for p in pts]
            m.face(pts, uvs, None)


def box(model, material, center, size, yaw=0.0, tile=1.0, tilt=(0.0, 0.0)):
    m = model.mesh(material)
    c = Vector(center)
    hx, hy, hz = (s / 2 for s in size)
    rot = Matrix.Rotation(yaw, 3, "Z") @ Matrix.Rotation(tilt[0], 3, "X") @ Matrix.Rotation(tilt[1], 3, "Y")
    corner = lambda x, y, z: c + rot @ Vector((x * hx, y * hy, z * hz))
    faces = [
        ((-1, -1, -1), (1, -1, -1), (1, -1, 1), (-1, -1, 1)),
        ((1, 1, -1), (-1, 1, -1), (-1, 1, 1), (1, 1, 1)),
        ((1, -1, -1), (1, 1, -1), (1, 1, 1), (1, -1, 1)),
        ((-1, 1, -1), (-1, -1, -1), (-1, -1, 1), (-1, 1, 1)),
        ((-1, -1, 1), (1, -1, 1), (1, 1, 1), (-1, 1, 1)),
        ((-1, 1, -1), (1, 1, -1), (1, -1, -1), (-1, -1, -1)),
    ]
    for f in faces:
        pts = [corner(*k) for k in f]
        n = (pts[1] - pts[0]).cross(pts[2] - pts[0]).normalized()
        ax = max(range(3), key=lambda i: abs(n[i]))
        uvs = [[(p.y / tile, p.z / tile), (p.x / tile, p.z / tile), (p.x / tile, p.y / tile)][ax] for p in pts]
        m.face(pts, uvs, None)


def rock(model, material, center, size, subdiv=1, jitter=0.18, moss=None, tile=2.0):
    """A faceted rock: flat-shaded, its upward faces in `moss` when given."""
    import bmesh

    bm = bmesh.new()
    bmesh.ops.create_icosphere(bm, subdivisions=subdiv, radius=1.0)
    c = Vector(center)
    rng = model.rng
    moved = {}
    for v in bm.verts:
        k = 1 + rng.uniform(-jitter, jitter)
        p = Vector((v.co.x * size[0] * k, v.co.y * size[1] * k, v.co.z * size[2] * k))
        if p.z < 0:
            p.z *= 0.35
        moved[v.index] = p
    for f in bm.faces:
        pts = [c + moved[v.index] for v in f.verts]
        n = (pts[1] - pts[0]).cross(pts[2] - pts[0]).normalized()
        ax = max(range(3), key=lambda i: abs(n[i]))
        uvs = [[(p.y / tile, p.z / tile), (p.x / tile, p.z / tile), (p.x / tile, p.y / tile)][ax] for p in pts]
        target = moss if (moss is not None and n.z > 0.72) else material
        model.mesh(target).face(pts, uvs, None)
    bm.free()


# --- Canopies ------------------------------------------------------------------


def sphere_dirs(n, rng, up_bias=0.25):
    """`n` directions spread over a sphere, leaning up by `up_bias`."""
    golden = math.pi * (3 - math.sqrt(5))
    out = []
    for i in range(n):
        z = 1 - 2 * (i + 0.5) / n
        r = math.sqrt(max(0.0, 1 - z * z))
        a = golden * i + rng.uniform(-0.3, 0.3)
        d = Vector((r * math.cos(a), r * math.sin(a), z + up_bias)).normalized()
        out.append(d)
    return out


def canopy(model, lobes, kind="mid", density=7.0, size=(1.3, 2.0), crown=None, core_scale=0.62, under=0.35):
    """Leaf-card clumps over ellipsoid lobes `(center, radii)`.

    `density` is cards per square meter of lobe surface / 2; cards sit at 70
    to 100 percent of each lobe's radius, facing out, and shade with the
    crown's outward normal. Each lobe has a dark core.
    """
    rng = model.rng
    material = leaf(kind)
    centers = [Vector(c) for c, _ in lobes]
    crown = Vector(crown) if crown else sum(centers, Vector()) / len(centers)
    lo = min(c.z - r[2] for c, (_, r) in zip(centers, lobes))
    hi = max(c.z + r[2] for c, (_, r) in zip(centers, lobes))
    mid_z = (lo + hi) / 2

    def shade(p):
        d = Vector((p.x - crown.x, p.y - crown.y, (p.z - mid_z) * 1.3))
        return (d.normalized() + UP * 0.35).normalized()

    for c, (_, r) in zip(centers, lobes):
        area = 4 * math.pi * ((r[0] * r[1] + r[1] * r[2] + r[0] * r[2]) / 3)
        n = max(6, int(area * density / 4))
        for d in sphere_dirs(n, rng):
            if d.z < -under:
                continue
            k = rng.uniform(0.7, 1.02)
            p = c + Vector((d.x * r[0], d.y * r[1], d.z * r[2])) * k
            facing = (d + Vector((rng.uniform(-0.35, 0.35), rng.uniform(-0.35, 0.35), rng.uniform(-0.2, 0.35)))).normalized()
            s = rng.uniform(*size)
            card(model, material, CLUSTER[1], p, facing, s, s, spin=rng.uniform(0, 2 * math.pi), shade=shade, bend=s * 0.12)
        blob(model, core(kind), c, [x * core_scale for x in r], subdiv=1, jitter=0.1)


def limb(model, material, start, end, r0, r1, bend=(0, 0, 0), sides=6, steps=3):
    """A limb from `start` to `end`, bowed by `bend` at its middle."""
    a, b, bend = Vector(start), Vector(end), Vector(bend)
    pts, radii = [], []
    for i in range(steps + 1):
        t = i / steps
        pts.append(a.lerp(b, t) + bend * math.sin(math.pi * t))
        radii.append(r0 + (r1 - r0) * t)
    tube(model, material, pts, radii, sides=sides)


def trunk(model, material, path, radii, flare=1.6, sides=8, roots=4, root_len=0.9):
    """A trunk with a flared base and root buttresses into the ground."""
    path = [Vector(p) for p in path]
    base = [Vector((path[0].x, path[0].y, -0.25))] + path
    rs = [radii[0] * flare] + list(radii)
    rs[1] = radii[0] * (1 + (flare - 1) * 0.6)
    tube(model, material, base, rs, sides=sides)
    rng = model.rng
    for i in range(roots):
        a = 2 * math.pi * (i + rng.uniform(-0.2, 0.2)) / roots
        d = Vector((math.cos(a), math.sin(a), 0))
        start = path[0] + d * radii[0] * 0.5 + UP * radii[0] * 0.9
        end = path[0] + d * (radii[0] + root_len * rng.uniform(0.8, 1.2)) - UP * 0.12
        limb(model, material, start, end, radii[0] * 0.42, radii[0] * 0.1, sides=4, steps=1)


# --- Trees ---------------------------------------------------------------------


def oak_forked(m):
    """A broadleaf whose trunk forks low into two leaders, each with its crown."""
    b = bark("brown")
    trunk(m, b, [(0, 0, 0), (0, 0, 1.4), (0.05, 0, 2.5)], [0.36, 0.32, 0.28], roots=5)
    tops = []
    for side, lean in ((1, 0.15), (-1, -0.1)):
        top = Vector((side * 1.7, lean, 6.4 + 0.4 * side))
        limb(m, b, (0.05, 0, 2.4), top, 0.24, 0.12, bend=(side * 0.25, 0, 0.2), sides=5, steps=2)
        for k in range(2):
            a = side * 0.9 + (k - 0.5) * 1.6
            end = top + Vector((math.cos(a) * 1.6, math.sin(a) * 1.4, 0.4 + k * 0.5))
            limb(m, b, top - UP * 1.2, end, 0.1, 0.04, sides=4, steps=1)
        tops.append(top)
    lobes = []
    for t in tops:
        lobes += [
            (t + Vector((0, 0, 0.9)), (2.2, 2.0, 1.7)),
            (t + Vector((t.x * 0.4, 0.9, -0.3)), (1.5, 1.4, 1.2)),
            (t + Vector((t.x * 0.3, -1.0, 0.1)), (1.5, 1.4, 1.2)),
        ]
    canopy(m, lobes, "mid", density=6.0, size=(1.5, 2.2))


def beech_tall(m):
    """A tall, straight broadleaf with a high oval crown over a clear bole."""
    b = bark("gray")
    trunk(m, b, [(0, 0, 0), (0, 0, 3.0), (0.1, 0.05, 6.0), (0.15, 0, 9.5)], [0.3, 0.26, 0.2, 0.1], roots=4)
    for i in range(5):
        a = i * 2.4
        z = 5.0 + i * 0.9
        end = Vector((math.cos(a) * 1.9, math.sin(a) * 1.9, z + 1.4))
        limb(m, b, (0.1, 0.05, z), end, 0.1, 0.04, sides=4, steps=1)
    canopy(
        m,
        [
            ((0.1, 0, 8.2), (2.6, 2.5, 3.6)),
            ((0.2, 0.1, 11.4), (1.8, 1.8, 1.6)),
            ((-0.9, 0.6, 6.4), (1.6, 1.5, 1.3)),
            ((1.0, -0.7, 6.8), (1.6, 1.5, 1.3)),
        ],
        "light",
        density=5.5,
        size=(1.4, 2.0),
    )


def linden_broad(m):
    """A short-trunked broadleaf with a wide, low dome of a crown."""
    b = bark("brown")
    trunk(m, b, [(0, 0, 0), (0, 0, 1.6), (-0.1, 0.1, 3.0)], [0.4, 0.35, 0.3], roots=5)
    for i in range(6):
        a = i * 1.05 + 0.3
        end = Vector((math.cos(a) * 3.2, math.sin(a) * 3.2, 4.6 + (i % 2) * 0.6))
        limb(m, b, (-0.1, 0.1, 2.6), end, 0.15, 0.05, bend=(0, 0, 0.5), sides=4, steps=2)
    lobes = [((0, 0, 6.0), (3.4, 3.4, 2.1))]
    for i in range(5):
        a = i * 2 * math.pi / 5 + 0.4
        lobes.append(((math.cos(a) * 2.8, math.sin(a) * 2.8, 4.9), (1.8, 1.8, 1.4)))
    canopy(m, lobes, "dark", density=5.0, size=(1.6, 2.3))


def willow_weeping(m):
    """A pond-side willow: a leaning trunk, an umbrella of limbs, and curtains
    of leaves falling nearly to the ground."""
    b = bark("dark")
    trunk(m, b, [(0, 0, 0), (0.2, 0, 1.6), (0.5, 0.1, 3.2)], [0.42, 0.36, 0.3], roots=5)
    rng = m.rng
    top = Vector((0.5, 0.1, 3.2))
    canopy(m, [(top + Vector((0, 0, 2.0)), (2.6, 2.6, 1.3))], "willow", density=4.5, size=(1.3, 1.8), under=0.1)
    material = leaf("willow")
    for i in range(7):
        a = i * 2 * math.pi / 7
        end = top + Vector((math.cos(a) * 3.0, math.sin(a) * 3.0, 2.4))
        limb(m, b, top, end, 0.16, 0.05, bend=(0, 0, 0.7), sides=4, steps=2)
    strands = 46
    for i in range(strands):
        a = 2 * math.pi * i / strands + rng.uniform(-0.05, 0.05)
        r = rng.uniform(2.4, 3.9)
        out = Vector((math.cos(a), math.sin(a), 0))
        top_z = 5.6 - (r - 2.4) * 0.5 + rng.uniform(-0.2, 0.2)
        length = rng.uniform(2.8, 4.2)
        center = top + Vector((out.x * r, out.y * r, 0))
        center.z = top_z - length / 2
        facing = (out + Vector((0, 0, 0.15))).normalized()
        w = rng.uniform(0.8, 1.1)
        card(m, material, CLUSTER[1], center, facing, w, length, spin=0.0,
             shade=(out + UP * 0.4).normalized(), bend=0.1)


def oak_old(m):
    """A gnarled old oak: a short, massive, twisting bole, crooked limbs, and a
    broad, lumpy crown."""
    b = bark("dark")
    path = [(0, 0, 0), (0.15, 0.1, 0.9), (-0.1, 0.2, 1.8), (0.1, 0, 2.6)]
    trunk(m, b, path, [0.75, 0.62, 0.58, 0.5], flare=1.5, sides=9, roots=6, root_len=1.4)
    rng = m.rng
    lobes = [((0, 0, 7.0), (3.0, 3.0, 1.9))]
    for i in range(5):
        a = i * 2 * math.pi / 5 + 0.2
        reach = rng.uniform(3.2, 4.2)
        end = Vector((math.cos(a) * reach, math.sin(a) * reach, rng.uniform(4.6, 5.8)))
        mid_bend = Vector((rng.uniform(-0.6, 0.6), rng.uniform(-0.6, 0.6), 0.6))
        limb(m, b, (0.1, 0, 2.4), end, 0.3, 0.1, bend=mid_bend, sides=5, steps=3)
        twig = end + Vector((math.cos(a + 0.6) * 1.2, math.sin(a + 0.6) * 1.2, 0.8))
        limb(m, b, end, twig, 0.1, 0.04, sides=4, steps=1)
        lobes.append((end + Vector((0, 0, 0.9)), (2.0, 2.0, 1.5)))
    canopy(m, lobes, "dark", density=5.0, size=(1.6, 2.4))


def snag(m):
    """A leafless dead tree: a gray bole with a broken top and bare limbs."""
    b = bark("silver")
    rng = m.rng
    trunk(m, b, [(0, 0, 0), (0.1, 0, 2.5), (0.3, 0.1, 5.0), (0.2, 0.2, 6.6)], [0.34, 0.28, 0.2, 0.16], roots=4)
    # A jagged, broken top.
    m.mesh(b)
    top = Vector((0.2, 0.2, 6.6))
    for i in range(4):
        a = i * math.pi / 2 + 0.3
        tip = top + Vector((math.cos(a) * 0.05, math.sin(a) * 0.05, rng.uniform(0.25, 0.6)))
        limb(m, b, top + Vector((math.cos(a) * 0.12, math.sin(a) * 0.12, -0.1)), tip, 0.07, 0.01, sides=3, steps=1)
    for i in range(6):
        a = i * 2.3 + 0.5
        z = 2.4 + i * 0.65
        start = Vector((0.1 + 0.03 * i, 0.02 * i, z))
        end = start + Vector((math.cos(a) * rng.uniform(1.2, 2.2), math.sin(a) * rng.uniform(1.2, 2.2), rng.uniform(0.5, 1.4)))
        limb(m, b, start, end, 0.11 - i * 0.008, 0.025, bend=(0, 0, 0.25), sides=4, steps=2)
        fork = end + Vector((math.cos(a + 0.8) * 0.7, math.sin(a + 0.8) * 0.7, 0.5))
        limb(m, b, end.lerp(start, 0.3), fork, 0.04, 0.012, sides=3, steps=1)


def fir(m):
    """A tall fir: a straight bole under tiers of drooping, needled
    branches, broad at the foot and narrowing to a spire."""
    b = bark("dark")
    trunk(m, b, [(0, 0, 0), (0, 0, 4.0), (0, 0, 9.5)], [0.28, 0.2, 0.06], sides=6, roots=4, root_len=0.6)
    rng = m.rng
    branches = tex("Leaf_Fir", FIR_BRANCH[0], (0.8, 0.95, 1.0))
    mesh = m.mesh(branches)
    tiers = 8
    for t in range(tiers):
        f = t / (tiers - 1)
        z = 1.8 + 8.0 * f ** 0.9
        reach = 3.0 * (1 - f) + 0.45
        droop = 0.9 * (1 - f) + 0.35
        count = max(5, int(11 * (1 - f) + 5))
        lo, hi = Vector((0, 0, z)), None
        for k in range(count):
            a = 2 * math.pi * (k + 0.5 * (t % 2)) / count + rng.uniform(-0.12, 0.12)
            out = Vector((math.cos(a), math.sin(a), 0))
            side = Vector((-out.y, out.x, 0))
            base = lo + out * 0.1 + UP * 0.25
            tip = lo + out * reach * rng.uniform(0.85, 1.1) - UP * droop
            w0, w1 = reach * 0.35, reach * 0.55
            mid = base.lerp(tip, 0.55) + UP * 0.12
            n = ((tip - base).cross(side)).normalized()
            if n.z < 0:
                n = -n
            shade = (out + UP * 0.8).normalized()
            pts = [base - side * w0 / 2, base + side * w0 / 2, mid + side * w1 / 2, mid - side * w1 / 2]
            uvs = [uv_in(FIR_BRANCH[1], 0, 0), uv_in(FIR_BRANCH[1], 1, 0), uv_in(FIR_BRANCH[1], 1, 0.55),
                   uv_in(FIR_BRANCH[1], 0, 0.55)]
            mesh.face(pts, uvs, [shade] * 4)
            pts = [mid - side * w1 / 2, mid + side * w1 / 2, tip + side * w1 * 0.15, tip - side * w1 * 0.15]
            uvs = [uv_in(FIR_BRANCH[1], 0, 0.55), uv_in(FIR_BRANCH[1], 1, 0.55), uv_in(FIR_BRANCH[1], 0.65, 1),
                   uv_in(FIR_BRANCH[1], 0.35, 1)]
            mesh.face(pts, uvs, [shade] * 4)
        # A dark cone inside each tier, so it reads solid from below.
        lathe(m, core("fir"), [(0.0, z - droop * 0.7), (reach * 0.62, z - droop * 0.7), (0.0, z + 0.45)], segs=7)


# --- Roots, stumps, and logs ---------------------------------------------------


def root(m, material, a, length, r0, rise, rng, sides=5):
    d = Vector((math.cos(a), math.sin(a), 0))
    side = Vector((-d.y, d.x, 0)) * rng.uniform(-0.25, 0.25)
    pts = [
        d * 0.15 + UP * rise,
        d * length * 0.3 + side + UP * rise * 0.7,
        d * length * 0.65 - side * 0.5 + UP * rise * 0.25,
        d * length + side * 0.3 - UP * 0.12,
    ]
    tube(m, material, pts, [r0, r0 * 0.75, r0 * 0.5, r0 * 0.2], sides=sides)


def roots_spread(m):
    """Exposed roots, radiating from where a trunk meets the ground."""
    b = bark("brown")
    rng = m.rng
    for i in range(6):
        a = 2 * math.pi * i / 6 + rng.uniform(-0.25, 0.25)
        root(m, b, a, rng.uniform(1.4, 2.4), rng.uniform(0.12, 0.2), rng.uniform(0.12, 0.25), rng, sides=4)


def root_arch(m):
    """A tangle of old roots arching out of a bank: a low root mass with loops
    a fox could run under."""
    b = bark("dark")
    rng = m.rng
    for i in range(6):
        x0 = -1.6 + i * 0.62 + rng.uniform(-0.1, 0.1)
        h = rng.uniform(0.8, 1.6)
        span = rng.uniform(1.4, 2.2)
        y0 = rng.uniform(-0.4, 0.4)
        pts = [Vector((x0, y0 - span / 2, -0.15))]
        for k in range(1, 6):
            t = k / 6
            pts.append(Vector((x0 + rng.uniform(-0.15, 0.15), y0 - span / 2 + span * t, math.sin(math.pi * t) * h)))
        pts.append(Vector((x0, y0 + span / 2, -0.15)))
        r = rng.uniform(0.09, 0.16)
        tube(m, b, pts, [r * 1.3, r * 1.1, r, r * 0.9, r, r * 1.1, r * 1.3], sides=5)
    blob(m, tex("Moss_Bank", ROCKS, (0.55, 0.75, 0.45)), (0, 0.9, 0), (2.2, 0.9, 0.55), subdiv=1, jitter=0.2,
         squash_base=True)
    for i in range(5):
        root(m, b, rng.uniform(-math.pi, 0), rng.uniform(0.8, 1.5), 0.07, 0.08, rng)


def end_grain():
    return flat("Wood_EndGrain", (0.42, 0.27, 0.12))


def moss():
    return flat("Moss_Cap", (0.09, 0.2, 0.03), 1.0)


def stump_mossy(m):
    """A wide, low stump with a mossy crown, root feet, and bracket fungi."""
    b = bark("brown")
    rng = m.rng
    tube(m, b, [(0, 0, -0.2), (0, 0, 0.2), (0, 0, 0.55)], [0.7, 0.55, 0.5], sides=9, jitter=0.06)
    lathe(m, end_grain(), [(0.5, 0.55), (0.44, 0.57), (0.0, 0.59)], segs=9)
    blob(m, moss(), (0.12, 0.05, 0.6), (0.36, 0.3, 0.09), subdiv=1, jitter=0.2)
    for i in range(5):
        root(m, b, 2 * math.pi * i / 5 + rng.uniform(-0.2, 0.2), rng.uniform(0.9, 1.3), 0.16, 0.25, rng)
    shelf = flat("Fungus_Shelf", (0.62, 0.42, 0.18))
    for i, z in enumerate((0.22, 0.34, 0.3)):
        a = 0.6 + i * 0.5
        lathe(m, shelf, [(0.0, -0.03), (0.16, 0.0), (0.12, 0.04), (0.0, 0.05)],
              center=(math.cos(a) * 0.55, math.sin(a) * 0.55, z), segs=6)


def stump_broken(m):
    """A tall stump, snapped off, with a jagged, splintered top."""
    b = bark("dark")
    rng = m.rng
    tube(m, b, [(0, 0, -0.2), (0, 0, 0.3), (0, 0, 1.1)], [0.55, 0.42, 0.4], sides=8, jitter=0.05)
    grain = end_grain()
    sides = 8
    for k in range(sides):
        a0, a1 = 2 * math.pi * k / sides, 2 * math.pi * (k + 1) / sides
        h0 = 1.1 + rng.uniform(0.05, 0.75) * (1 if k % 3 else 0.3)
        p0 = Vector((math.cos(a0) * 0.4, math.sin(a0) * 0.4, 1.1))
        p1 = Vector((math.cos(a1) * 0.4, math.sin(a1) * 0.4, 1.1))
        tip = Vector((math.cos((a0 + a1) / 2) * 0.3, math.sin((a0 + a1) / 2) * 0.3, h0))
        m.mesh(grain).face([p0, p1, tip], [(0, 0), (1, 0), (0.5, 1)])
        m.mesh(grain).face([p1, p0, Vector((0, 0, 1.15))], [(0, 0), (1, 0), (0.5, 1)])
    for i in range(4):
        root(m, b, 2 * math.pi * i / 4 + 0.4, rng.uniform(0.8, 1.1), 0.13, 0.2, rng)
    blob(m, moss(), (-0.2, 0.25, 0.1), (0.5, 0.35, 0.18), subdiv=1, jitter=0.2)


def log_hollow(m):
    """A fallen, hollow log, open at both ends, with moss along its back."""
    b = bark("brown")
    inside = flat("Wood_Rot", (0.2, 0.11, 0.05))
    length, r = 4.2, 0.48
    sides = 9
    pts = [Vector((x, 0, r * 0.85)) for x in (-length / 2, -length / 6, length / 6, length / 2)]
    tube(m, b, pts, [r, r * 1.02, r * 0.98, r * 0.92], sides=sides, jitter=0.04)
    # The inner wall, facing in, and the rims.
    m_in = m.mesh(inside)
    for k in range(sides):
        a0, a1 = 2 * math.pi * k / sides, 2 * math.pi * (k + 1) / sides
        q = lambda x, a, rr: Vector((x, math.cos(a) * rr, r * 0.85 + math.sin(a) * rr))
        ri = r * 0.68
        m_in.face([q(-length / 2, a0, ri), q(-length / 2, a1, ri), q(length / 2, a1, ri), q(length / 2, a0, ri)],
                  [(0, 0), (1, 0), (1, 4), (0, 4)])
        for x, s in ((-length / 2, 1), (length / 2, -1)):
            ro = r * (1.0 if x < 0 else 0.92)
            pts = [q(x, a0, ro), q(x, a1, ro), q(x, a1, ri), q(x, a0, ri)]
            if s < 0:
                pts = pts[::-1]
            m.mesh(end_grain()).face(pts, [(0, 0), (1, 0), (1, 1), (0, 1)])
    blob(m, moss(), (0.3, 0.0, r * 1.7), (1.6, 0.32, 0.12), subdiv=1, jitter=0.25)
    blob(m, moss(), (-1.4, 0.1, r * 1.6), (0.6, 0.28, 0.1), subdiv=1, jitter=0.25)
    limb(m, b, (0.8, 0.3, r), (1.1, 1.1, r * 1.6), 0.07, 0.03, sides=4, steps=1)


def log_broken(m):
    """A trunk that fell and broke in two, its halves askew, with splintered
    ends and a stub of a limb."""
    b = bark("dark")
    grain = end_grain()
    rng = m.rng
    for (cx, cy, yaw, length, r) in ((-1.25, 0, 0.12, 2.3, 0.36), (1.35, 0.35, -0.35, 2.0, 0.33)):
        d = Vector((math.cos(yaw), math.sin(yaw), 0))
        c = Vector((cx, cy, r * 0.85))
        pts = [c - d * length / 2, c, c + d * length / 2]
        tube(m, b, pts, [r, r * 0.97, r * 0.94], sides=8, jitter=0.05)
        for end, sgn in ((pts[0], -1), (pts[-1], 1)):
            s, t = frame(d)
            ring = [end + (s * math.cos(2 * math.pi * k / 8) + t * math.sin(2 * math.pi * k / 8)) * r * 0.95
                    for k in range(8)]
            for k in range(8):
                splinter = end + d * sgn * rng.uniform(0.05, 0.35)
                tri = [ring[k], ring[(k + 1) % 8], splinter]
                if sgn < 0:
                    tri = tri[::-1]
                m.mesh(grain).face(tri, [(0, 0), (1, 0), (0.5, 1)])
    limb(m, b, (-1.0, 0, 0.6), (-0.7, -0.5, 1.2), 0.08, 0.03, sides=4, steps=1)
    blob(m, moss(), (-1.4, 0.05, 0.62), (0.7, 0.25, 0.1), subdiv=1, jitter=0.25)


# --- Bushes and shrubs ---------------------------------------------------------


def shrub(m, lobes, kind, density, size):
    canopy(m, lobes, kind, density=density, size=size, under=0.55, core_scale=0.7)


def shrub_mound(m):
    """A low, wide mound of a shrub."""
    shrub(m, [((0, 0, 0.55), (1.3, 1.1, 0.65)), ((0.7, 0.3, 0.45), (0.8, 0.7, 0.5)),
              ((-0.7, -0.2, 0.42), (0.75, 0.7, 0.45))], "mid", 7.0, (0.8, 1.1))


def shrub_tall(m):
    """An upright, oval shrub, taller than a person."""
    shrub(m, [((0, 0, 1.25), (0.8, 0.75, 1.15)), ((0.25, -0.2, 0.6), (0.65, 0.6, 0.6))], "dark", 7.0, (0.8, 1.1))


def shrub_flowering(m):
    """A rounded shrub in flower."""
    shrub(m, [((0, 0, 0.75), (1.0, 0.95, 0.75)), ((0.5, 0.4, 0.5), (0.6, 0.6, 0.5))], "light", 7.0, (0.75, 1.0))
    rng = m.rng
    petals = atlas("Bloom_Pink", FLOWER["pink"], (1.0, 0.9, 0.95))
    for d in sphere_dirs(26, rng, up_bias=0.6):
        if d.z < 0:
            continue
        p = Vector((0, 0, 0.75)) + Vector((d.x * 1.02, d.y * 0.97, d.z * 0.78))
        card(m, petals, FLOWER["pink"][1], p, d, 0.26, 0.26, spin=rng.uniform(0, 6.3))


def bramble(m):
    """A wild, sprawling bramble thicket with red berries."""
    rng = m.rng
    lobes = []
    for i in range(5):
        a = i * 1.3
        lobes.append(((math.cos(a) * 0.9, math.sin(a) * 0.6, 0.45 + 0.15 * (i % 2)),
                      (rng.uniform(0.6, 0.9), rng.uniform(0.55, 0.8), rng.uniform(0.4, 0.6))))
    shrub(m, lobes, "mid", 6.0, (0.7, 1.0))
    canes = bark("dark")
    for i in range(4):
        a = i * 1.6
        start = Vector((math.cos(a) * 0.3, math.sin(a) * 0.3, 0.0))
        end = Vector((math.cos(a) * 1.8, math.sin(a) * 1.3, 0.2))
        limb(m, canes, start, end, 0.025, 0.01, bend=(0, 0, 0.8), sides=3, steps=2)
    berry = flat("Berry_Red", (0.45, 0.02, 0.03), 0.5)
    for i in range(10):
        a = rng.uniform(0, 6.3)
        p = (math.cos(a) * rng.uniform(0.6, 1.4), math.sin(a) * rng.uniform(0.4, 0.9), rng.uniform(0.5, 1.0))
        box(m, berry, p, (0.08, 0.08, 0.08), yaw=a)


# --- Ground plants -------------------------------------------------------------


def frond(m, material, region, base, a, length, width, lift=0.55, droop=0.45):
    """An arching frond: a three-segment strip from `base` along heading `a`."""
    d = Vector((math.cos(a), math.sin(a), 0))
    side = Vector((-d.y, d.x, 0))
    pts = []
    for t in (0.0, 0.35, 0.7, 1.0):
        rise = math.sin(math.pi * min(t * 1.25, 1.0)) * length * lift - t * t * length * droop
        pts.append(Vector(base) + d * length * t + UP * (rise + 0.03))
    mesh = m.mesh(material)
    for i in range(3):
        t0, t1 = i / 3, (i + 1) / 3
        w0, w1 = width * (1 - 0.6 * t0), width * (1 - 0.6 * t1)
        a0, a1 = pts[i], pts[i + 1]
        n = (a1 - a0).cross(side).normalized()
        if n.z < 0:
            n = -n
        quad = [a0 - side * w0 / 2, a0 + side * w0 / 2, a1 + side * w1 / 2, a1 - side * w1 / 2]
        uvs = [uv_in(region, 0, t0), uv_in(region, 1, t0), uv_in(region, 1, t1), uv_in(region, 0, t1)]
        mesh.face(quad, uvs, [(n + UP).normalized()] * 4)


def fern_clump(m):
    """A clump of a dozen arching fern fronds."""
    rng = m.rng
    material = atlas("Fern_Frond", FERN, (0.75, 0.95, 0.7))
    for i in range(13):
        a = 2 * math.pi * i / 13 + rng.uniform(-0.2, 0.2)
        frond(m, material, FERN[1], (rng.uniform(-0.08, 0.08), rng.uniform(-0.08, 0.08), 0), a,
              rng.uniform(0.8, 1.2), rng.uniform(0.32, 0.42), lift=rng.uniform(0.55, 0.8))


def blades(m, material, region, count, radius, height, rng, center=(0, 0, 0)):
    mesh = m.mesh(material)
    c = Vector(center)
    for i in range(count):
        a = rng.uniform(0, 2 * math.pi)
        r = radius * math.sqrt(rng.uniform(0, 1))
        base = c + Vector((math.cos(a) * r, math.sin(a) * r, -0.02))
        h = height * rng.uniform(0.6, 1.1)
        lean = Vector((math.cos(a), math.sin(a), 0)) * h * rng.uniform(0.1, 0.4)
        yaw = rng.uniform(0, 2 * math.pi)
        side = Vector((math.cos(yaw), math.sin(yaw), 0)) * 0.035
        mid = base + lean * 0.4 + UP * h * 0.55
        tip = base + lean + UP * h
        n = side.cross(UP).normalized()
        shade = (UP * 0.8 + n * 0.2).normalized()
        mesh.face([base - side, base + side, mid + side * 0.6, mid - side * 0.6],
                  [uv_in(region, 0, 0), uv_in(region, 1, 0), uv_in(region, 1, 0.55), uv_in(region, 0, 0.55)],
                  [shade] * 4)
        mesh.face([mid - side * 0.6, mid + side * 0.6, tip],
                  [uv_in(region, 0, 0.55), uv_in(region, 1, 0.55), uv_in(region, 0.5, 1)], [shade] * 3)


def grass_tall(m):
    """A tuft of tall meadow grass, knee to waist high."""
    rng = m.rng
    blades(m, atlas("Grass_Blade", GRASS), GRASS[1], 20, 0.5, 0.95, rng)
    blades(m, atlas("Grass_BladeLight", GRASS_LIGHT), GRASS_LIGHT[1], 8, 0.45, 1.1, rng)


def wildflower_clump(m):
    """Tall grass with wildflowers on stems among it."""
    rng = m.rng
    blades(m, atlas("Grass_Blade", GRASS), GRASS[1], 14, 0.45, 0.75, rng)
    stem = flat("Stem_Green", (0.08, 0.2, 0.03))
    for i, color in enumerate(["yellow", "violet", "cream", "red", "yellow", "pink", "violet"]):
        a = rng.uniform(0, 6.3)
        r = rng.uniform(0.05, 0.5)
        base = Vector((math.cos(a) * r, math.sin(a) * r, 0))
        h = rng.uniform(0.45, 0.85)
        top = base + Vector((rng.uniform(-0.08, 0.08), rng.uniform(-0.08, 0.08), h))
        limb(m, stem, base, top, 0.012, 0.008, sides=3, steps=1)
        region = FLOWER[color]
        facing = (UP + Vector((rng.uniform(-0.6, 0.6), rng.uniform(-0.6, 0.6), 0))).normalized()
        card(m, atlas("Bloom_" + color.capitalize(), region), region[1], top, facing, 0.2, 0.2, spin=rng.uniform(0, 6.3))


def mushroom(m, at, scale, cap, stem_mat, rng, tall=1.0):
    x, y = at
    h = 0.14 * scale * tall
    lathe(m, stem_mat, [(0.032 * scale, 0.0), (0.028 * scale, h)], center=(x, y, 0), segs=4)
    lathe(m, cap, [(0.0, h - 0.01 * scale), (0.1 * scale, h - 0.005 * scale), (0.085 * scale, h + 0.04 * scale),
                   (0.0, h + 0.07 * scale)], center=(x, y, 0), segs=5)


def mushroom_ring(m):
    """A fairy ring: a circle of pale caps in the grass, with a few red ones."""
    rng = m.rng
    stem_mat = flat("Mushroom_Stem", (0.78, 0.72, 0.6))
    caps = [flat("Mushroom_Cream", (0.75, 0.62, 0.45)), flat("Mushroom_Tan", (0.48, 0.3, 0.15)),
            flat("Mushroom_Red", (0.62, 0.06, 0.03))]
    n = 17
    for i in range(n):
        a = 2 * math.pi * i / n + rng.uniform(-0.08, 0.08)
        r = 1.6 + rng.uniform(-0.12, 0.12)
        cap = caps[2] if i % 6 == 0 else caps[i % 2]
        mushroom(m, (math.cos(a) * r, math.sin(a) * r), rng.uniform(0.8, 1.5), cap, stem_mat, rng)
        if i % 3 == 0:
            a2 = a + 0.08
            mushroom(m, (math.cos(a2) * (r + 0.12), math.sin(a2) * (r + 0.12)), 0.6, caps[i % 2], stem_mat, rng)
    clover = atlas("Clover_Leaf", CLOVER)
    for i in range(10):
        a = rng.uniform(0, 6.3)
        r = rng.uniform(0.2, 1.2)
        card(m, clover, CLOVER[1], (math.cos(a) * r, math.sin(a) * r, 0.04), UP, 0.28, 0.28, spin=rng.uniform(0, 6.3))


# --- Walls, fences, and gardens ------------------------------------------------


def wall_leaves(m, material, width, height, count, rng, top_jag=0.5, depth=0.12, size=(0.45, 0.65), base_z=0.0,
                ragged=True):
    """Leaf cards lying on a wall plane at y=0, facing -Y, with a ragged top."""
    lo = Vector((0, -1, 0))

    for i in range(count):
        x = rng.uniform(-width / 2, width / 2)
        # Thicker low down, thinning to a ragged top.
        edge = height - (top_jag * (0.5 + 0.5 * math.sin(x * 2.7 + 1.3)) if ragged else 0)
        edge *= 1 - 0.35 * (abs(x) / (width / 2)) ** 2
        z = base_z + edge * (1 - rng.random() ** 1.6)
        p = Vector((x, -rng.uniform(0.03, depth), z))
        facing = (lo + Vector((rng.uniform(-0.4, 0.4), 0, rng.uniform(-0.1, 0.5)))).normalized()
        s = rng.uniform(*size)
        card(m, material, CLUSTER[1], p, facing, s, s, spin=rng.uniform(0, 6.3),
             shade=(lo + UP * 0.45).normalized())


def ivy_wall(m):
    """Ivy climbing a wall: a ragged sheet of leaves 2.6 m wide and up to 3 m
    high on the wall plane, with its stems. Its origin is a kit wall piece's:
    on the wall's center line at its base, the outside face 0.09 m toward +Z,
    so the town places it with the wall and it breaks with it."""
    rng = m.rng
    m.offset = Vector((0, -0.11, 0))
    stem = bark("dark")
    for i in range(5):
        x = -1.0 + i * 0.5 + rng.uniform(-0.1, 0.1)
        pts = [Vector((x + rng.uniform(-0.15, 0.15), -0.03, z)) for z in (0.0, 0.8, 1.6, 2.3)]
        tube(m, stem, pts, [0.03, 0.025, 0.02, 0.012], sides=3)
    wall_leaves(m, leaf("ivy"), 2.6, 3.0, 80, rng, top_jag=0.9)


def ivy_low(m):
    """Ivy over a low wall or a fence: 2.4 m wide, 1.3 m high, with a few
    trails over the top."""
    rng = m.rng
    wall_leaves(m, leaf("ivy"), 2.4, 1.3, 40, rng, top_jag=0.35, size=(0.4, 0.55))
    for i in range(6):
        x = rng.uniform(-1.0, 1.0)
        card(m, leaf("ivy"), CLUSTER[1], (x, 0.1, 1.25), (0, -0.2, 1), 0.5, 0.45, spin=rng.uniform(0, 6.3))


def wood():
    return flat("Trellis_Wood", (0.36, 0.22, 0.1), 0.85)


def rose_trellis(m):
    """A timber trellis with climbing roses, 1.6 m wide and 2.4 m tall, for a
    wall. Its front faces +Z; its origin is a kit wall piece's, as
    `ivy_wall`'s, with the trellis 0.15 m off the wall's face."""
    rng = m.rng
    m.offset = Vector((0, -0.24, 0))
    w = wood()
    for x in (-0.8, 0.8):
        box(m, w, (x, 0, 1.2), (0.08, 0.08, 2.4))
    box(m, w, (0, 0, 2.38), (1.76, 0.1, 0.08))
    for k in range(-3, 4):
        x = k * 0.22
        box(m, w, (x, 0.02, 1.2), (0.03, 0.03, 2.3), tilt=(0, 0.35))
        box(m, w, (x, 0.04, 1.2), (0.03, 0.03, 2.3), tilt=(0, -0.35))
    leaves = leaf("dark")
    wall_leaves(m, leaves, 1.7, 2.5, 34, rng, top_jag=0.4, depth=0.1, size=(0.4, 0.55))
    for color, n in (("red", 9), ("pink", 7)):
        material = atlas("Bloom_" + color.capitalize(), FLOWER[color])
        for i in range(n):
            x = rng.uniform(-0.75, 0.75)
            z = rng.uniform(0.4, 2.3)
            card(m, material, FLOWER[color][1], (x, -rng.uniform(0.12, 0.18), z),
                 (rng.uniform(-0.3, 0.3), -1, rng.uniform(0, 0.4)), 0.2, 0.2, spin=rng.uniform(0, 6.3))


def hedge_block(m, cx, length, height, depth, rng, count_scale=1.0):
    core_mat = core("hedge")
    box(m, core_mat, (cx, 0, height / 2), (length - 0.12, depth - 0.12, height - 0.08))
    leaves = leaf("hedge")
    faces = [
        (Vector((0, -1, 0)), length, height, lambda u, v: Vector((cx + u, -depth / 2, v * height))),
        (Vector((0, 1, 0)), length, height, lambda u, v: Vector((cx + u, depth / 2, v * height))),
        (Vector((0, 0, 1)), length, depth, lambda u, v: Vector((cx + u, (v - 0.5) * depth, height))),
        (Vector((-1, 0, 0)), depth, height, lambda u, v: Vector((cx - length / 2, u * depth / length, v * height))),
        (Vector((1, 0, 0)), depth, height, lambda u, v: Vector((cx + length / 2, u * depth / length, v * height))),
    ]
    for normal, a, b, at in faces:
        n = max(3, int(a * b * 9.0 * count_scale))
        for i in range(n):
            u = rng.uniform(-length / 2, length / 2) if normal.x == 0 else rng.uniform(-length / 2, length / 2)
            v = rng.uniform(0.08, 0.98)
            p = at(u, v) + normal * rng.uniform(0.0, 0.06)
            facing = (normal + Vector((rng.uniform(-0.3, 0.3), rng.uniform(-0.3, 0.3), rng.uniform(-0.1, 0.3)))).normalized()
            s = rng.uniform(0.6, 0.8)
            card(m, leaves, CLUSTER[1], p, facing, s, s, spin=rng.uniform(0, 6.3), shade=(normal + UP * 0.5).normalized())


def hedge_long(m):
    """A clipped hedge, 4 m long, 1.4 m tall, and 0.9 m deep, along X."""
    hedge_block(m, 0.0, 4.0, 1.4, 0.9, m.rng)


def hedge_gate(m):
    """A hedge with a gateway: two 1.6 m hedges either side of a 2.6 m gap
    under a clipped arch, and a pair of timber gates swung open into the
    garden behind. 5.8 m long; walking passes the gap's middle 2.6 m."""
    rng = m.rng
    for cx in (-2.1, 2.1):
        hedge_block(m, cx, 1.6, 1.5, 0.9, rng)
    # The arch: leaf cards over a half ring above the gap.
    leaves = leaf("hedge")
    core_mat = core("hedge")
    radius = 1.55
    for k in range(11):
        a = math.pi * k / 10
        p = Vector((math.cos(a) * radius, 0, 1.5 + math.sin(a) * radius * 0.8))
        box(m, core_mat, p, (0.36, 0.6, 0.36), tilt=(0, -a + math.pi / 2))
        out = Vector((math.cos(a), 0, math.sin(a)))
        for d in (Vector((0, -1, 0)), Vector((0, 1, 0)), out):
            facing = (d + out * 0.4).normalized()
            card(m, leaves, CLUSTER[1], p + d * 0.3 + out * 0.08, facing, 0.65, 0.65, spin=rng.uniform(0, 6.3),
                 shade=(d + UP * 0.5).normalized())
    # The gates, each 1.25 m, swung open into the garden.
    w = wood()
    for side in (-1, 1):
        hinge = Vector((side * 1.3, 0.1, 0))
        along = Vector((side * 0.26, 0.97, 0)).normalized()
        yaw = math.atan2(along.y, along.x)
        for z in (0.25, 0.85):
            box(m, w, hinge + along * 0.62 + UP * z, (1.25, 0.05, 0.1), yaw=yaw)
        for k in range(5):
            box(m, w, hinge + along * (0.08 + k * 0.26) + UP * 0.55, (0.08, 0.04, 1.0), yaw=yaw)


def planter_overflow(m):
    """A timber planter, 1.4 m long, overflowing with leaves, trailing
    stems, and flowers."""
    rng = m.rng
    w = wood()
    box(m, w, (0, 0, 0.24), (1.4, 0.55, 0.48))
    box(m, flat("Soil_Dark", (0.1, 0.06, 0.03), 1.0), (0, 0, 0.47), (1.3, 0.45, 0.04))
    shrub_l = [((-0.35, 0, 0.62), (0.45, 0.3, 0.28)), ((0.35, 0, 0.6), (0.45, 0.3, 0.26))]
    canopy(m, shrub_l, "mid", density=10.0, size=(0.4, 0.55), under=0.6, core_scale=0.6)
    trail = leaf("ivy")
    for side in (-1, 1):
        for i in range(5):
            x = -0.6 + i * 0.3 + rng.uniform(-0.05, 0.05)
            length = rng.uniform(0.45, 0.6)
            card(m, trail, CLUSTER[1], (x, side * 0.32, 0.56 - length / 2), (0, side, 0.15), 0.26, length,
                 shade=Vector((0, side, 0.6)).normalized())
    for color in ("red", "yellow", "violet", "cream", "pink", "red", "yellow"):
        material = atlas("Bloom_" + color.capitalize(), FLOWER[color])
        p = (rng.uniform(-0.6, 0.6), rng.uniform(-0.2, 0.2), rng.uniform(0.75, 0.9))
        card(m, material, FLOWER[color][1], p, (rng.uniform(-0.4, 0.4), rng.uniform(-0.4, 0.4), 1), 0.2, 0.2,
             spin=rng.uniform(0, 6.3))


def window_box(m):
    """A window box under a sill: 1.1 m of flowers with stems trailing down
    the wall. Its origin is a kit wall piece's, as `ivy_wall`'s: the box
    hangs under a window's sill, 0.92 m up."""
    rng = m.rng
    m.offset = Vector((0, -0.11, 0.72))
    w = wood()
    box(m, w, (0, -0.13, 0.1), (1.1, 0.24, 0.2))
    canopy(m, [((0, -0.14, 0.26), (0.5, 0.14, 0.14))], "mid", density=14.0, size=(0.3, 0.4), under=0.8,
           core_scale=0.7)
    trail = leaf("ivy")
    for i in range(6):
        x = -0.45 + i * 0.18 + rng.uniform(-0.04, 0.04)
        length = rng.uniform(0.35, 0.7)
        card(m, trail, CLUSTER[1], (x, -0.27, 0.12 - length / 2), (0, -1, 0.1), 0.22, length,
             shade=Vector((0, -1, 0.5)).normalized())
    for color in ("red", "pink", "cream", "red", "violet", "yellow", "pink", "red"):
        material = atlas("Bloom_" + color.capitalize(), FLOWER[color])
        p = (rng.uniform(-0.5, 0.5), -0.14 + rng.uniform(-0.1, 0.1), rng.uniform(0.3, 0.42))
        card(m, material, FLOWER[color][1], p, (rng.uniform(-0.3, 0.3), -0.6, 1), 0.17, 0.17, spin=rng.uniform(0, 6.3))


# --- Stones and rocks ----------------------------------------------------------


def stone():
    return tex("Stone_Rock", ROCKS, (0.95, 0.95, 0.9))


def lichen():
    return tex("Stone_Moss", ROCKS, (0.5, 0.75, 0.4))


def standing_stone(m):
    """A tall, weathered standing stone, a little out of true."""
    rng = m.rng
    mesh = m.mesh(stone())
    # A tapered slab with irregular corners.
    ring = lambda z, w, d, j: [Vector((x * w + rng.uniform(-j, j), y * d + rng.uniform(-j, j), z))
                               for x, y in ((-1, -1), (0, -1.15), (1, -1), (1.1, 0), (1, 1), (0, 1.1), (-1, 1), (-1.1, 0))]
    levels = [ring(-0.3, 0.55, 0.32, 0.03), ring(0.9, 0.5, 0.3, 0.06), ring(1.9, 0.44, 0.26, 0.06),
              ring(2.5, 0.3, 0.18, 0.05)]
    lean = Matrix.Rotation(0.06, 3, "X") @ Matrix.Rotation(-0.04, 3, "Y")
    levels = [[lean @ p for p in lv] for lv in levels]
    for a, b in zip(levels, levels[1:]):
        for k in range(8):
            j = (k + 1) % 8
            pts = [a[k], a[j], b[j], b[k]]
            n = (pts[1] - pts[0]).cross(pts[2] - pts[0]).normalized()
            ax = 0 if abs(n.x) > abs(n.y) else 1
            uvs = [((p.y if ax == 0 else p.x) / 2.0, p.z / 2.0) for p in pts]
            target = lichen() if (b is levels[-1] and k in (1, 2)) or (a is levels[0] and k % 3 == 0) else stone()
            m.mesh(target).face(pts, uvs)
    top = levels[-1]
    c = sum(top, Vector()) / 8 + UP * 0.12
    for k in range(8):
        m.mesh(lichen()).face([top[k], top[(k + 1) % 8], c], [(0, 0), (1, 0), (0.5, 1)])


def standing_stone_squat(m):
    """A shorter, broader standing stone, leaning, mossy on its back."""
    rock(m, stone(), (0, 0, 0.75), (0.6, 0.42, 0.95), subdiv=2, jitter=0.12, moss=lichen())


def cliff_rock(m):
    """A craggy outcrop, 4 m across and 3 m high, with ledges and moss."""
    rock(m, stone(), (0, 0, 1.2), (2.0, 1.6, 2.1), subdiv=3, jitter=0.22, moss=lichen())
    rock(m, stone(), (1.5, -0.7, 0.5), (1.0, 0.9, 0.8), subdiv=2, jitter=0.2, moss=lichen())
    rock(m, stone(), (-1.4, 0.6, 0.4), (0.9, 1.0, 0.7), subdiv=2, jitter=0.2, moss=lichen())


def boulder_cluster(m):
    """Three or four boulders lying together, with a fern between them."""
    rng = m.rng
    for (x, y, s) in ((0, 0, 0.9), (1.1, 0.4, 0.6), (-0.8, 0.6, 0.55), (0.3, -0.9, 0.4)):
        rock(m, stone(), (x, y, s * 0.55), (s, s * rng.uniform(0.8, 1.1), s * 0.75), subdiv=2, jitter=0.2,
             moss=lichen())
    material = atlas("Fern_Frond", FERN, (0.75, 0.95, 0.7))
    for i in range(6):
        frond(m, material, FERN[1], (0.4, 0.2, 0.0), i * 1.05, 0.7, 0.3)


# --- Clearings and water -------------------------------------------------------


def campfire(m):
    """A campfire in a ring of stones, with a teepee of logs over its embers
    and three log seats around it."""
    rng = m.rng
    for i in range(10):
        a = 2 * math.pi * i / 10
        rock(m, stone(), (math.cos(a) * 0.62, math.sin(a) * 0.62, 0.09), (0.16, 0.13, 0.13), subdiv=0, jitter=0.15)
    lathe(m, flat("Ash", (0.08, 0.07, 0.065), 1.0), [(0.55, 0.01), (0.0, 0.03)], segs=8)
    embers = flat("Embers_Glow", (1.0, 0.42, 0.08), 0.4, emit=(1.0, 0.42, 0.08))
    blob(m, embers, (0, 0, 0.06), (0.28, 0.28, 0.08), subdiv=1, jitter=0.2)
    b = bark("dark")
    for i in range(5):
        a = 2 * math.pi * i / 5 + 0.3
        foot = Vector((math.cos(a) * 0.45, math.sin(a) * 0.45, 0.02))
        limb(m, b, foot, (0, 0, 0.62), 0.055, 0.03, sides=5, steps=1)
    flame = flat("Flame_Glow", (1.0, 0.7, 0.2), 0.3, emit=(1.0, 0.65, 0.2))
    for i in range(3):
        a = i * 2.1
        lathe(m, flame, [(0.12, 0.02), (0.07, 0.25), (0.0, 0.45 + 0.1 * i)],
              center=(math.cos(a) * 0.08, math.sin(a) * 0.08, 0.02), segs=5)
    for i in range(3):
        a = 2 * math.pi * i / 3 + 0.5
        c = Vector((math.cos(a) * 2.3, math.sin(a) * 2.3, 0.22))
        d = Vector((-math.sin(a), math.cos(a), 0))
        tube(m, b, [c - d * 0.8, c + d * 0.8], [0.22, 0.2], sides=7, cap=True)


def cascade(m):
    """A low cascade where a stream spills over a weir of stones: the water
    rises over the stones' lip, falls a hand's height in white streaks, and
    foams below. 5 m across the stream (X) and 3 m along it (Y, flowing
    toward -Y); the stream's own water lies at the ground."""
    rng = m.rng
    water = flat("Water_Run", (0.06, 0.13, 0.19), 0.1)
    white = flat("Water_Foam", (0.8, 0.86, 0.88), 0.4)
    # The weir: low stones across the bed, bigger ones on the banks.
    for i in range(11):
        x = -2.5 + i * 0.5
        bank = abs(x) > 1.2
        s = rng.uniform(0.3, 0.42) * (1.7 if bank else 1.0)
        rock(m, stone(), (x, rng.uniform(-0.1, 0.1), 0.12 if not bank else 0.3),
             (s, 0.4, s * (1.5 if bank else 0.7)), subdiv=2 if bank else 1, jitter=0.2, moss=lichen())
    # The water climbs gently to the lip, then falls.
    sheet = m.mesh(water)
    lip = 0.26
    sheet.face([Vector((-1.3, 0.05, lip)), Vector((1.3, 0.05, lip)), Vector((1.3, 1.6, 0.02)), Vector((-1.3, 1.6, 0.02))],
               [(0, 0), (1, 0), (1, 1), (0, 1)], [UP] * 4)
    for k in range(7):
        x0, x1 = -1.3 + k * 2.6 / 7, -1.3 + (k + 1) * 2.6 / 7
        face = [Vector((x0, 0.05, lip)), Vector((x1, 0.05, lip)), Vector((x1, -0.3, 0.02)), Vector((x0, -0.3, 0.02))]
        m.mesh(white if k % 2 == 0 else water).face(face, [(0, 0), (1, 0), (1, 1), (0, 1)],
                                                   [Vector((0, -0.6, 0.8)).normalized()] * 4)
    # Foam spreading downstream: flat patches on the water.
    foam = m.mesh(white)
    for i in range(14):
        x = rng.uniform(-1.1, 1.1)
        y = -0.4 - rng.uniform(0, 1.2)
        r = rng.uniform(0.15, 0.32) * (1.2 - 0.4 * (-0.4 - y))
        pts = [Vector((x + math.cos(t) * r * rng.uniform(0.7, 1.2), y + math.sin(t) * r * 0.7, 0.03))
               for t in [2 * math.pi * k / 6 for k in range(6)]]
        foam.face(pts, [(0.5 + 0.5 * math.cos(2 * math.pi * k / 6), 0.5) for k in range(6)], [UP] * 6)
    material = atlas("Fern_Frond", FERN, (0.75, 0.95, 0.7))
    for x in (-2.3, 2.3):
        for i in range(5):
            frond(m, material, FERN[1], (x, 0.6, 0.3), i * 1.25, 0.6, 0.26)


MODELS = {
    "oak_forked": oak_forked,
    "beech_tall": beech_tall,
    "linden_broad": linden_broad,
    "willow_weeping": willow_weeping,
    "oak_old": oak_old,
    "snag": snag,
    "fir": fir,
    "roots_spread": roots_spread,
    "root_arch": root_arch,
    "stump_mossy": stump_mossy,
    "stump_broken": stump_broken,
    "log_hollow": log_hollow,
    "log_broken": log_broken,
    "shrub_mound": shrub_mound,
    "shrub_tall": shrub_tall,
    "shrub_flowering": shrub_flowering,
    "bramble": bramble,
    "fern_clump": fern_clump,
    "grass_tall": grass_tall,
    "wildflower_clump": wildflower_clump,
    "mushroom_ring": mushroom_ring,
    "ivy_wall": ivy_wall,
    "ivy_low": ivy_low,
    "rose_trellis": rose_trellis,
    "hedge_long": hedge_long,
    "hedge_gate": hedge_gate,
    "planter_overflow": planter_overflow,
    "window_box": window_box,
    "standing_stone": standing_stone,
    "standing_stone_squat": standing_stone_squat,
    "cliff_rock": cliff_rock,
    "boulder_cluster": boulder_cluster,
    "campfire": campfire,
    "cascade": cascade,
}


def main():
    a = kit.args()
    out = a[0] if a and not a[0] in MODELS else OUT
    names = [n for n in a if n in MODELS] or list(MODELS)
    for name in names:
        kit.reset()
        _MATS.clear()
        model = Model(name)
        MODELS[name](model)
        model.build()
        kit.export(os.path.join(out, name + ".glb"))


main()
