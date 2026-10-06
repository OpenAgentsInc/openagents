"""Alice, Verse's original explorer-druid, on the Universal rig.

Run headless, one variant per run:
    Blender -b --factory-startup --python scripts/blender/alice.py -- \
        [OUT_DIR] [lod0|lod1|lod2|lod3] [--quick PREVIEW_DIR]

OUT_DIR defaults to assets/verse/characters/original/alice and the variant
to lod1. Each run writes `alice.<variant>.glb` and prints one `MODEL` line.
`--quick` skips the bake and export and renders the built scene from four
sides into PREVIEW_DIR, for fast iteration.

Alice is an NPC, a character the world places; a later female player
character reuses her base body (`build_body`) and head (`ubc_head`,
`build_head`) with an outfit of its own (`build_outfit` is Alice's).

Modes: Reference for the body, hair, clothing, and gear, made here from
primitives and procedural shading; Compose for the head, eyes, and brows,
cut from the CC0 Universal Base Characters female body (Quaternius,
`Superhero_Female_FullBody.gltf`) and reshaped by `reshape`, so the face has
real structure. The same file gives the skeleton: Alice has the Universal 65
joints with their names, parents, and rest transforms, and every Universal
Animation Library clip plays on her.

Design (docs/verse/female-character.md): an explorer-druid in a forest-green,
knee-length open coat over a cream linen tunic, dark leggings, tall warm-brown
boots, wrapped leather bracers, a copper-red sash, a cross-body satchel, a
staff on her back, and a hood worn down. Dark auburn hair in a chin-length
layered bob with a side part and a swept fringe. A stylized face in the
Quaternius-adjacent look of our world. She avoids every Echo signature the
spec lists: no updo, buns, braids, or ponytail, no scarf, no skirt, no lone
shoulder pad, no canteen, and no gold.

The frame is the rig's: Blender +Z up, the character faces -Y (+Z after
export), her left is +X, 1 unit = 1 m, standing in a T-pose with the soles
at z = 0.

How it is built:
- Every generated part is a loft of rings along a bone chain, or a surface
  fitted to the head by casting rays (the hair), written vertex by vertex, so
  the script, not an editor session, is the source.
- Weights come from the chain each part was lofted along: a vertex takes the
  bone of the segment it projects onto, blending to the neighbor with a
  smoothstep over about a quarter of the shorter segment around each joint.
  Special zones blend the shoulder (spine_03, clavicle, upperarm), the hip
  (pelvis and thigh), and the coat tails (pelvis toward each thigh). Limbs are
  built on the left and mirrored, so both sides match exactly. The head keeps
  the base's own weights. At most four
  influences per vertex, normalized.
- One base-color atlas per variant, baked deterministically in Cycles:
  the diffuse color of the procedural materials, times ambient occlusion and
  a soft top light, so folds read without a normal map.
"""

import json
import math
import os
import sys
from math import cos, exp, pi, sin

import bmesh
import bpy
from mathutils import Vector

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

BASE = os.path.join(
    kit.REPO, "assets", "verse", "characters", "quaternius", "base", "Superhero_Female_FullBody.gltf"
)
OUT = os.path.join(kit.REPO, "assets", "verse", "characters", "original", "alice")

# Ring segments, hair grid, finger detail, station stride, atlas edge, and
# the triangle budget per variant (docs/verse/female-character.md).
LODS = {
    "lod0": dict(limb=16, torso=28, head=(32, 22), hair=(48, 16), finger=6, fingers=True, stride=1,
                 tex=1024, budget=24000, samples=128),
    "lod1": dict(limb=12, torso=22, head=(24, 16), hair=(36, 12), finger=5, fingers=True, stride=1,
                 tex=512, budget=16000, samples=96),
    "lod2": dict(limb=10, torso=16, head=(18, 12), hair=(26, 9), finger=4, fingers=False, stride=2,
                 tex=256, budget=10000, samples=64, head_ratio=0.45),
    "lod3": dict(limb=6, torso=10, head=(10, 8), hair=(14, 6), finger=3, fingers=False, stride=3,
                 tex=256, budget=3000, samples=48, lite=True, head_ratio=0.12),
}


def linear(h):
    """sRGB hex to linear RGB, which Blender's color inputs take."""
    h = h.lstrip("#")
    c = [int(h[i:i + 2], 16) / 255 for i in (0, 2, 4)]
    return tuple(x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4 for x in c)


# The palette: forest green, warm brown leather, and cream linen, with copper
# accents; dark auburn hair. No gold anywhere.
PALETTE = {
    "skin": "#E2AE8C",
    "blush": "#FFD2C4",
    "lips": "#F0A49C",
    "glint": "#FFFFFF",
    "brow": "#5E2D1C",
    "hair": "#5E2418",
    "hair_inner": "#3A160F",
    "linen": "#E9DDC2",
    "linen_shade": "#CDBE9C",
    "coat": "#2F5B35",
    "coat_inner": "#27482B",
    "coat_trim": "#1F3B25",
    "lining": "#C8B38A",
    "leggings": "#3B3A30",
    "leather": "#7B4C2B",
    "leather_dark": "#55331E",
    "sole": "#2B1E16",
    "sash": "#B0532F",
    "copper": "#C3773F",
    "wood": "#6A4A2E",
    "leaf": "#4F7A35",
}


def smoothstep(a, b, x):
    if a == b:
        return 1.0 if x >= b else 0.0
    t = min(1.0, max(0.0, (x - a) / (b - a)))
    return t * t * (3 - 2 * t)


def lerp(a, b, t):
    return a + (b - a) * t


def side_name(name, s):
    """`name` with the side suffix for side `s` (+1 left, -1 right)."""
    return name + ("_l" if s > 0 else "_r")


def mirror_name(name):
    if name.endswith("_l"):
        return name[:-2] + "_r"
    if name.endswith("_r"):
        return name[:-2] + "_l"
    return name


def mix(a, b, t):
    """Blend two weight maps: (1 - t) a + t b."""
    out = {}
    for k, w in a.items():
        out[k] = out.get(k, 0.0) + w * (1 - t)
    for k, w in b.items():
        out[k] = out.get(k, 0.0) + w * t
    return {k: w for k, w in out.items() if w > 1e-5}


def rigid(bone):
    return {bone: 1.0}


# --- Mesh accumulation --------------------------------------------------------


class Mesh:
    """Vertices with bone weights, and faces with a material and a part tag."""

    def __init__(self):
        self.v, self.w, self.f, self.m, self.tag = [], [], [], [], []
        self.part = "body"

    def vert(self, p, w):
        self.v.append(Vector(p))
        self.w.append(dict(w))
        return len(self.v) - 1

    def face(self, idx, mat):
        self.f.append(tuple(idx))
        self.m.append(mat)
        self.tag.append(self.part)

    def extend_mirrored(self, other):
        """Append `other` mirrored across x = 0, sides swapped, faces flipped."""
        base = len(self.v)
        for p, w in zip(other.v, other.w):
            self.v.append(Vector((-p.x, p.y, p.z)))
            self.w.append({mirror_name(k): x for k, x in w.items()})
        for f, m, t in zip(other.f, other.m, other.tag):
            self.f.append(tuple(base + i for i in reversed(f)))
            self.m.append(m)
            self.tag.append(t)

    def extend(self, other):
        base = len(self.v)
        self.v += other.v
        self.w += other.w
        for f, m, t in zip(other.f, other.m, other.tag):
            self.f.append(tuple(base + i for i in f))
            self.m.append(m)
            self.tag.append(t)


def grid(M, P, W, mat, wrap=False, flip=False, matfn=None):
    """Quads between rows of points. With columns counterclockwise about the
    row direction, the faces face outward."""
    rows, cols = len(P), len(P[0])
    idx = [[M.vert(P[i][j], W[i][j]) for j in range(cols)] for i in range(rows)]
    ncol = cols if wrap else cols - 1
    for i in range(rows - 1):
        for j in range(ncol):
            j2 = (j + 1) % cols
            q = (idx[i][j], idx[i][j2], idx[i + 1][j2], idx[i + 1][j])
            if flip:
                q = q[::-1]
            M.face(q, matfn(i, j) if matfn else mat)
    return idx


def fan(M, ring, center, w, mat, outward):
    """Close `ring` with a fan to `center`, facing `outward`."""
    c = M.vert(center, w)
    n = len(ring)
    a, b = M.v[ring[0]] - M.v[c], M.v[ring[1]] - M.v[c]
    keep = a.cross(b).dot(outward) > 0
    for j in range(n):
        tri = (c, ring[j], ring[(j + 1) % n])
        M.face(tri if keep else tri[::-1], mat)


def frame(axis, ref):
    a = Vector(axis).normalized()
    u = Vector(ref) - a * Vector(ref).dot(a)
    if u.length < 1e-6:
        u = a.orthogonal()
    u.normalize()
    return a, u, a.cross(u)


def ring_points(c, axis, ref, rx, ry, segs, n=2.0, shape=None, phase=0.0):
    a, u, v = frame(axis, ref)
    out = []
    ex = 2.0 / n
    for j in range(segs):
        th = phase + 2 * pi * j / segs
        ct, st = cos(th), sin(th)
        cx = math.copysign(abs(ct) ** ex, ct)
        cy = math.copysign(abs(st) ** ex, st)
        p = c + u * (rx * cx) + v * (ry * cy)
        if shape:
            d = (u * ct + v * st).normalized()
            p = p + d * shape(d, p)
        out.append(p)
    return out


def loft(M, rings, segs, mat, wfn, cap0=False, cap1=False, matfn=None, post=None, dome=0.0):
    """Loft closed rings. Each ring: dict(c, axis, ref, rx, ry, n, shape)."""
    P = []
    for r in rings:
        pts = ring_points(r["c"], r["axis"], r.get("ref", (0, 0, 1)), r["rx"], r["ry"], segs,
                          r.get("n", 2.0), r.get("shape"), r.get("phase", 0.0))
        if post:
            pts = [post(p) for p in pts]
        P.append(pts)
    W = [[wfn(p) for p in row] for row in P]
    idx = grid(M, P, W, mat, wrap=True, matfn=matfn)
    if cap0:
        a = Vector(rings[0]["axis"]).normalized()
        c = sum(P[0], Vector()) / segs - a * dome
        fan(M, idx[0], c, wfn(c), mat, -a)
    if cap1:
        a = Vector(rings[-1]["axis"]).normalized()
        c = sum(P[-1], Vector()) / segs + a * dome
        fan(M, idx[-1], c, wfn(c), mat, a)
    return idx


def shell(M, P, W, thick, mat, inner_mat, rim_mat, matfn=None, wrap=False, rims=(True, True, True, True)):
    """A surface grid with thickness: the outer faces, the inner faces
    `thick` behind them, and rims on the open boundaries (first row, last
    row, first column, last column)."""
    rows, cols = len(P), len(P[0])

    def at(i, j):
        if wrap:
            j %= cols
        return P[max(0, min(rows - 1, i))][max(0, min(cols - 1, j))]

    N = []
    for i in range(rows):
        row = []
        for j in range(cols):
            dc = at(i, j + 1) - at(i, j - 1)
            dr = at(i + 1, j) - at(i - 1, j)
            n = dc.cross(dr)
            row.append(n.normalized() if n.length > 1e-9 else Vector((0, 0, 1)))
        N.append(row)
    Q = [[P[i][j] - N[i][j] * thick for j in range(cols)] for i in range(rows)]
    out = grid(M, P, W, mat, wrap=wrap, matfn=matfn)
    inn = grid(M, Q, W, inner_mat, wrap=wrap, flip=True)

    def rim(a, b, a2, b2, interior):
        q = (a, b, b2, a2)
        n = (M.v[b] - M.v[a]).cross(M.v[a2] - M.v[a])
        if n.dot(interior) > 0:
            q = q[::-1]
        M.face(q, rim_mat)

    if rims[0] or rims[1]:
        for i, nb in ((0, 1), (rows - 1, rows - 2)):
            if not rims[0 if i == 0 else 1]:
                continue
            for j in range(cols if wrap else cols - 1):
                j2 = (j + 1) % cols
                interior = P[nb][j] - P[i][j]
                rim(out[i][j], out[i][j2], inn[i][j], inn[i][j2], interior)
    if not wrap:
        for j, nb, flag in ((0, 1, rims[2]), (cols - 1, cols - 2, rims[3])):
            if not flag:
                continue
            for i in range(rows - 1):
                interior = P[i][nb] - P[i][j]
                rim(out[i][j], out[i + 1][j], inn[i][j], inn[i + 1][j], interior)
    return out, inn


# --- The rig and its chains ---------------------------------------------------


def load_rig():
    kit.reset()
    bpy.ops.import_scene.gltf(filepath=BASE)
    arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    keep = {"Superhero_Female", "Eyes", "Eyebrows"}
    for o in list(bpy.data.objects):
        if o.type != "ARMATURE" and o.name not in keep:
            bpy.data.objects.remove(o, do_unlink=True)
    arm.name = "Armature"
    joints = {b.name: (arm.matrix_world @ b.head_local, arm.matrix_world @ b.tail_local)
              for b in arm.data.bones}
    return arm, joints


def gauss(x, w):
    return exp(-((x / w) ** 2))


def reshape(p):
    """Turns the Universal female head into Alice's: a little larger, for
    the stylized proportions of our world; a softer, narrower jaw and a
    rounder chin; a shorter, slightly upturned nose; fuller lips and cheeks;
    and a softer brow ridge. Masks read the original position, so the
    changes don't compound."""
    q = p.copy()
    k = smoothstep(1.50, 1.57, p.z)
    center = Vector((0.0, -0.005, 1.60))
    q = center + (q - center) * (1 + 0.05 * k)
    front = smoothstep(0.02, -0.045, p.y)
    lower = smoothstep(1.62, 1.545, p.z)
    q.x *= 1 - 0.075 * lower * front
    chin = gauss(p.x, 0.03) * gauss(p.z - 1.55, 0.02) * front
    q.y += 0.003 * chin
    q.z += 0.002 * chin
    nose = gauss(p.x, 0.02) * gauss(p.z - 1.614, 0.016) * smoothstep(-0.09, -0.108, p.y)
    q.y += 0.0075 * nose
    q.z += 0.0035 * nose
    lips = gauss(p.x, 0.021) * gauss(p.z - 1.591, 0.009) * smoothstep(-0.08, -0.092, p.y)
    q.y -= 0.0018 * lips
    cheek = gauss(abs(p.x) - 0.048, 0.016) * gauss(p.z - 1.618, 0.022) * smoothstep(-0.02, -0.06, p.y)
    q.x += math.copysign(0.0045 * cheek, p.x)
    q.y -= 0.003 * cheek
    brow = gauss(p.z - 1.682, 0.01) * smoothstep(-0.075, -0.09, p.y)
    q.y += 0.0025 * brow
    # Ears laid closer to the head, so the bob falls over them.
    ear = smoothstep(0.066, 0.078, abs(p.x)) * gauss(p.z - 1.64, 0.05)
    if ear > 0:
        out = abs(q.x) - 0.066
        q.x = math.copysign(0.066 + out * (1 - 0.6 * ear), q.x)
    return q


def ubc_head(lod):
    """The head, eyes, and brows, cut from the CC0 Universal female base
    body (Quaternius, Compose mode), reshaped into Alice, and kept on the
    rig with their own weights. Returns the head and eyes objects; the
    brows are joined to the head."""
    body = bpy.data.objects["Superhero_Female"]
    eyes = bpy.data.objects["Eyes"]
    brows = bpy.data.objects["Eyebrows"]
    groups = {g.index: g.name for g in body.vertex_groups}
    me = body.data

    def headish(v):
        w = sum(g.weight for g in v.groups if groups[g.group] in ("Head", "neck_01"))
        return w > 0.5 and v.co.z > 1.45

    bm = bmesh.new()
    bm.from_mesh(me)
    bm.verts.ensure_lookup_table()
    drop = [f for f in bm.faces if not all(headish(me.vertices[v.index]) for v in f.verts)]
    bmesh.ops.delete(bm, geom=drop, context="FACES")
    bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
    for v in bm.verts:
        v.co = reshape(v.co)
    bm.to_mesh(me)
    bm.free()
    # Eyes move with their sockets as rigid balls.
    for side in (1, -1):
        vs = [v for v in eyes.data.vertices if v.co.x * side > 0]
        c = sum((v.co for v in vs), Vector()) / len(vs)
        delta = reshape(c) - c
        for v in vs:
            v.co += delta
    # Brows follow the brow ridge, a little slimmer and lower.
    for side in (1, -1):
        vs = [v for v in brows.data.vertices if v.co.x * side > 0]
        mz = sum(v.co.z for v in vs) / len(vs)
        for v in vs:
            co = v.co.copy()
            # Slimmer, a little higher, and the inner ends lifted most, so
            # her resting look is open rather than stern.
            inner = smoothstep(0.045, 0.012, abs(co.x))
            co.z = mz + (co.z - mz) * 0.5 + 0.0015 + 0.0035 * inner
            v.co = reshape(co)
    for o in (body, eyes, brows):
        uv = o.data.uv_layers
        while len(uv) > 1:
            uv.remove(uv[-1])
        uv[0].name = "UVMap"
    ratio = lod.get("head_ratio", 1.0)
    if ratio < 1.0:
        for o, r in ((body, ratio), (eyes, max(0.12, ratio)), (brows, ratio * 0.5)):
            mod = o.modifiers.new("Decimate", "DECIMATE")
            mod.ratio = r
            bpy.context.view_layer.objects.active = o
            bpy.ops.object.modifier_apply(modifier=mod.name)
    # Materials: Alice's skin over the base's painted lips and shading, the
    # base's eye texture, and auburn brows.
    face_material(body)
    brows.data.materials.clear()
    brows.data.materials.append(material("brow"))
    eye_material(eyes)
    # Warmth on the cheeks, the nose, and the lips, for the bake.
    attr = body.data.color_attributes.new("warm", "FLOAT_COLOR", "POINT")
    for v in body.data.vertices:
        p = v.co
        w = 0.0
        if p.y < -0.03:
            w += 0.9 * gauss(abs(p.x) - 0.045, 0.018) * gauss(p.z - 1.615, 0.016)
            w += 0.7 * gauss(p.x, 0.012) * gauss(p.z - 1.618, 0.012) * smoothstep(-0.09, -0.105, p.y)
        lip = gauss(p.x, 0.022) * gauss(p.z - 1.591, 0.0075) if p.y < -0.075 else 0.0
        attr.data[v.index].color = (min(1.0, w), min(1.0, lip), 0, 1)
    bpy.context.view_layer.objects.active = brows
    bpy.ops.object.select_all(action="DESELECT")
    body.select_set(True)
    brows.select_set(True)
    bpy.context.view_layer.objects.active = body
    bpy.ops.object.join()
    body.name = "AliceHead"
    return body, eyes


class Chain:
    """A polyline of bones; weights blend over `widths` around each joint."""

    def __init__(self, points, names, widths):
        self.p = [Vector(x) for x in points]
        self.n = names
        self.L = [(self.p[i + 1] - self.p[i]).length for i in range(len(names))]
        self.S = [0.0]
        for seg in self.L:
            self.S.append(self.S[-1] + seg)
        self.wid = widths

    def param(self, q):
        best = None
        for i in range(len(self.n)):
            a, b = self.p[i], self.p[i + 1]
            ab = b - a
            t = max(0.0, min(1.0, (q - a).dot(ab) / max(ab.length_squared, 1e-12)))
            d = (a + ab * t - q).length
            if best is None or d < best[0] - 1e-9:
                best = (d, self.S[i] + t * self.L[i])
        return best[1]

    def weights(self, q):
        s = self.param(q)
        f = [smoothstep(self.S[k] - w, self.S[k] + w, s) for k, w in zip(range(1, len(self.n)), self.wid)]
        out = {}
        for i, name in enumerate(self.n):
            lo = f[i - 1] if i > 0 else 1.0
            hi = f[i] if i < len(f) else 0.0
            if lo - hi > 1e-5:
                out[name] = out.get(name, 0.0) + (lo - hi)
        return out


class Rig:
    def __init__(self, joints):
        self.J = joints

    def h(self, name):
        return self.J[name][0].copy()

    def t(self, name):
        return self.J[name][1].copy()

    def torso(self):
        h = self.h
        return Chain(
            [(0, 0.03, 0.70), h("spine_01"), h("spine_02"), h("spine_03"), h("neck_01"), h("Head"),
             (0, 0, 1.85)],
            ["pelvis", "spine_01", "spine_02", "spine_03", "neck_01", "Head"],
            [0.05, 0.04, 0.045, 0.035, 0.025],
        )

    def arm(self, s):
        h = self.h
        n = lambda b: side_name(b, s)  # noqa: E731
        return Chain(
            [(0, 0.02, 1.40), h(n("clavicle")), h(n("upperarm")), h(n("lowerarm")), h(n("hand")),
             h(n("middle_01"))],
            ["spine_03", n("clavicle"), n("upperarm"), n("lowerarm"), n("hand")],
            [0.02, 0.045, 0.04, 0.022],
        )

    def leg(self, s):
        h, t = self.h, self.t
        n = lambda b: side_name(b, s)  # noqa: E731
        hip = h(n("thigh"))
        return Chain(
            [(hip.x * 0.8, 0.03, 1.08), hip, h(n("calf")), h(n("foot")), h(n("ball")), t(n("ball_leaf"))],
            ["pelvis", n("thigh"), n("calf"), n("foot"), n("ball")],
            [0.07, 0.05, 0.03, 0.02],
        )

    def finger(self, s, name):
        h, t = self.h, self.t
        n = lambda b: side_name(b, s)  # noqa: E731
        return Chain(
            [h(n("hand")), h(n(name + "_01")), h(n(name + "_02")), h(n(name + "_03")),
             h(n(name + "_04_leaf")), t(n(name + "_04_leaf"))],
            [n("hand"), n(name + "_01"), n(name + "_02"), n(name + "_03"), n(name + "_03")],
            [0.012, 0.006, 0.005, 0.0005],
        )


# --- Shapes of the body and clothing -------------------------------------------

# The torso at each height: z, half width, half depth, center y, superellipse
# exponent. Front is -y.
TORSO = [
    (0.80, 0.188, 0.128, 0.010, 2.2, True),
    (0.86, 0.180, 0.122, 0.010, 2.2, False),
    (0.93, 0.172, 0.116, 0.012, 2.3, True),
    (1.00, 0.150, 0.106, 0.010, 2.3, False),
    (1.06, 0.134, 0.098, 0.006, 2.3, True),
    (1.12, 0.139, 0.100, 0.000, 2.3, False),
    (1.19, 0.148, 0.106, -0.004, 2.4, True),
    (1.26, 0.156, 0.112, -0.004, 2.4, False),
    (1.32, 0.164, 0.110, 0.000, 2.5, True),
    (1.38, 0.170, 0.101, 0.010, 2.6, False),
    (1.43, 0.160, 0.090, 0.015, 2.6, True),
    (1.47, 0.112, 0.074, 0.020, 2.4, False),
    (1.50, 0.052, 0.050, 0.022, 2.0, True),
]


def torso_at(z):
    """Interpolated torso section at height z: (half width, half depth, y, n)."""
    rows = TORSO
    if z <= rows[0][0]:
        r = rows[0]
        return r[1], r[2], r[3], r[4]
    for a, b in zip(rows, rows[1:]):
        if a[0] <= z <= b[0]:
            t = (z - a[0]) / (b[0] - a[0])
            t = t * t * (3 - 2 * t)
            return tuple(lerp(a[k], b[k], t) for k in (1, 2, 3, 4))
    r = rows[-1]
    return r[1], r[2], r[3], r[4]


def bust(d, p):
    """The torso's modest bust and shoulder blades."""
    out = 0.0
    if d.y < 0:
        for sx in (-1, 1):
            out += 0.022 * exp(-(((p.x - sx * 0.072) / 0.055) ** 2)) * exp(-(((p.z - 1.265) / 0.065) ** 2)) * (-d.y)
    if d.y > 0:
        out += 0.006 * exp(-(((abs(p.x) - 0.08) / 0.05) ** 2)) * exp(-(((p.z - 1.36) / 0.06) ** 2)) * d.y
    return out


def section_point(z, theta, grow=0.0, sizes=None):
    a, b, yc, n = sizes or torso_at(z)
    ct, st = cos(theta), sin(theta)
    ex = 2.0 / n
    x = math.copysign(abs(ct) ** ex, ct) * (a + grow)
    y = math.copysign(abs(st) ** ex, st) * (b + grow)
    p = Vector((x, yc + y, z))
    d = Vector((ct, st, 0)).normalized()
    return p + d * bust(d, p)


# --- The build ----------------------------------------------------------------


class Alice:
    def __init__(self, rig, lod):
        self.R = rig
        self.L = lod
        self.M = Mesh()
        self.torso_chain = rig.torso()
        self.bvh = None
        self.skull_cache = {}
        self.eyes = []

    # Weights -------------------------------------------------------------------

    def w_torso(self, p, hem=True):
        w = self.torso_chain.weights(p)
        ax = abs(p.x)
        s = 1 if p.x >= 0 else -1
        clav = 0.62 * smoothstep(0.095, 0.175, ax) * smoothstep(1.33, 1.43, p.z)
        upper = 0.25 * smoothstep(0.15, 0.20, ax) * smoothstep(1.36, 1.43, p.z)
        if clav + upper > 0:
            extra = {side_name("clavicle", s): clav}
            if upper > 0:
                extra[side_name("upperarm", s)] = upper
            w = {k: v * (1 - clav - upper) for k, v in w.items()}
            for k, v in extra.items():
                w[k] = w.get(k, 0) + v
        if hem and p.z < 0.96:
            th = 0.45 * smoothstep(0.96, 0.80, p.z) * smoothstep(0.02, 0.12, ax)
            if th > 0:
                w = mix(w, {side_name("thigh", s): 1.0}, th)
        return w

    def w_coat(self, p):
        w = self.w_torso(p, hem=False)
        if p.z < 1.0:
            s = 1 if p.x >= 0 else -1
            # Enough of the thigh that a stride swings the tail, little
            # enough that a tucked knee lifts it as cloth, not as a board.
            th = 0.62 * smoothstep(1.0, 0.78, p.z) * smoothstep(0.0, 0.16, abs(p.x))
            w = mix(w, {side_name("thigh", s): 1.0}, th)
        return w

    def stations(self, rows):
        """Every station for lod0 and lod1; key stations only beyond."""
        stride = self.L["stride"]
        if stride == 1:
            return rows
        out = [r for k, r in enumerate(rows) if r[-1] or k == 0 or k == len(rows) - 1]
        if stride >= 3:
            keep = [out[0]] + out[1:-1][::2] + [out[-1]]
            out = keep
        return out

    # Torso, neck, pelvis -----------------------------------------------------------

    def tunic(self):
        M = self.M
        M.part = "tunic"
        segs = self.L["torso"]
        rings = []
        hem = torso_at(0.905)
        rows = [(0.905, hem[0] + 0.008, hem[1] + 0.006, hem[2], hem[3], True)]
        rows += [r for r in TORSO if r[0] > 0.92]
        for z, a, b, yc, n, key in self.stations(rows):
            rings.append(dict(c=Vector((0, yc, z)), axis=(0, 0, 1), ref=(1, 0, 0), rx=a, ry=b, n=n,
                              shape=bust))
        nrow = len(rings)

        def matfn(i, j):
            return "linen_shade" if i == 0 else "linen"

        loft(M, rings, segs, "linen", self.w_torso, matfn=matfn)
        # Briefs under the hem, closing the body between the legs.
        M.part = "body"
        br = []
        for z, a, b in ((0.86, 0.150, 0.100), (0.90, 0.162, 0.108), (0.96, 0.160, 0.106), (1.01, 0.145, 0.100)):
            br.append(dict(c=Vector((0, 0.012, z)), axis=(0, 0, 1), ref=(1, 0, 0), rx=a, ry=b, n=2.2))
        loft(M, br, max(8, segs // 2), "leggings", lambda p: self.w_torso(p, hem=True), cap0=True,
             dome=0.025)
        return nrow

    # Head ------------------------------------------------------------------------------
    #
    # The head is the CC0 Universal Base Characters female head, reshaped into
    # Alice by `reshape` (see `ubc_head`), so it has real facial structure:
    # edge loops around the eyes and mouth, a brow ridge, a nose with
    # nostrils, lips with an upper and a lower volume, and ears. The hair is
    # fitted to it by casting rays at the reshaped skull.

    HC = Vector((0.0, -0.004, 1.656))

    def head_point(self, th, ph):
        """The skull's surface in direction (th, ph) from the head's center:
        th the azimuth from +x (front is -pi/2), ph the angle from the top."""
        return self.skull(th, ph)[0]

    def head_normal(self, th, ph):
        return self.skull(th, ph)[1]

    def skull(self, th, ph):
        key = (round(th, 6), round(ph, 6))
        hit = self.skull_cache.get(key)
        if hit:
            return hit
        d = Vector((sin(ph) * cos(th), sin(ph) * sin(th), cos(ph)))
        found = self.bvh.ray_cast(self.HC + d * 0.4, -d) if self.bvh else (None, None, None, None)
        if found[0] is None:
            p = self.HC + Vector((d.x * 0.085, d.y * 0.10, d.z * 0.112))
            n = d
        else:
            p = found[0]
            # A rounder normal than the faceted face's, for smooth hair.
            n = (found[1].normalized() * 0.4 + (p - self.HC).normalized() * 0.6).normalized()
        self.skull_cache[key] = (p, n)
        return p, n

    # Hair --------------------------------------------------------------------------------

    def hair(self):
        """A chin-length layered bob with a side part on her left and a fringe
        swept across her right brow: one shell with thickness."""
        M = self.M
        M.part = "hair"
        cols, nrows = self.L["hair"]
        out_off = 0.011
        thick = 0.010
        ph_eq = 1.62
        P, W = [], []
        columns = []
        for j in range(cols):
            th = -pi / 2 + 2 * pi * j / cols
            al = math.atan2(math.sin(th + pi / 2), math.cos(th + pi / 2))
            aa = abs(al)
            # Where the column leaves the skull: the hairline in front, the
            # equator at the sides and back.
            hairline = 0.62 + 0.30 * min(1.0, aa / 1.25) ** 2
            fringe = 0.48 * exp(-(((al + 0.10) / 0.62) ** 2)) * smoothstep(0.95, 0.30, al)
            # Tapered locks at the fringe's edge, not one cut line.
            locks = 0.05 * abs(((al * 11 / pi) % 2) - 1) * smoothstep(0.05, 0.25, fringe)
            ph_face = min(1.20, hairline + fringe + locks)
            k = smoothstep(1.02, 1.38, aa)
            ph_end = lerp(ph_face, ph_eq, k)
            z_end = 1.532 + 0.016 * (aa / pi) + 0.007 * abs(((al * 9 / pi) % 2) - 1)
            columns.append((th, al, ph_end, k, z_end))
        for th, al, ph_end, k, z_end in columns:
            p_eq = self.head_point(th, ph_end)
            n_eq = self.head_normal(th, ph_end)
            skull = (ph_end - 0.06) * 0.10
            drop = max(0.0, (p_eq.z - z_end)) * k
            total = skull + drop
            col = []
            for i in range(nrows + 1):
                d = total * i / nrows
                if d <= skull + 1e-9 or drop <= 0:
                    ph = 0.06 + (ph_end - 0.06) * min(1.0, d / skull)
                    # The fringe sweeps toward her right as it falls.
                    tt = th
                    p = self.head_point(tt, ph) + self.head_normal(tt, ph) * out_off
                    groove = -0.004 * exp(-(((al - 0.45) / 0.07) ** 2)) * smoothstep(0.95, 0.4, ph)
                    ridge = 0.0022 * sin(al * 17) * smoothstep(0.35, 0.9, ph)
                    p = p + self.head_normal(tt, ph) * (groove + ridge)
                else:
                    f = (d - skull) / drop
                    radial = Vector((p_eq.x - self.HC.x, p_eq.y - self.HC.y, 0)).normalized()
                    p = p_eq + n_eq * out_off
                    p = p + Vector((0, 0, -(p_eq.z - z_end) * f * k / max(k, 1e-6) if k > 0 else 0))
                    p = p + radial * (0.014 * f - 0.012 * smoothstep(0.75, 1.0, f) + 0.003 * sin(al * 17))
                col.append(p)
            P.append(col)
        # Smooth over the ears: the rays that fit the skull catch them, and
        # hair falls over them, not around them.
        side = [c[3] > 0.6 for c in columns]
        for _ in range(3):
            Q = [list(col) for col in P]
            for j in range(cols):
                a, b = (j - 1) % cols, (j + 1) % cols
                if not (side[j] and side[a] and side[b]):
                    continue
                for i in range(1, nrows + 1):
                    Q[j][i] = P[j][i] * 0.5 + (P[a][i] + P[b][i]) * 0.25
            P = Q
        # Rows bottom to top, columns counterclockwise: faces point out.
        rows = [[P[j][nrows - i] for j in range(cols)] for i in range(nrows + 1)]
        W = [[rigid("Head") for _ in r] for r in rows]
        out, inn = shell(M, rows, W, thick, "hair", "hair_inner", "hair_inner", wrap=True,
                         rims=(True, False, False, False))
        top_out = out[-1]
        fan(M, top_out, self.head_point(0, 0) + Vector((0, 0, out_off + 0.002)), rigid("Head"), "hair",
            Vector((0, 0, 1)))

    # Arms and hands -------------------------------------------------------------------------

    def arm_center(self, s, x):
        R = self.R
        ua, el, wr = R.h(side_name("upperarm", s)), R.h(side_name("lowerarm", s)), R.h(side_name("hand", s))
        ax = abs(x)
        if ax <= ua.x * s:
            t = smoothstep(0.08, ua.x * s, ax)
            return Vector((x, lerp(0.034, ua.y, t), lerp(1.402, ua.z, t)))
        pts = [ua, el, wr, R.h(side_name("middle_01", s))]
        for a, b in zip(pts, pts[1:]):
            if abs(a.x) <= ax <= abs(b.x):
                t = (ax - abs(a.x)) / (abs(b.x) - abs(a.x))
                return a.lerp(b, t)
        return pts[-1].copy()

    SLEEVE = [
        (0.085, 0.060, 0.064, True), (0.12, 0.060, 0.066, False), (0.16, 0.058, 0.062, True),
        (0.20, 0.055, 0.058, False), (0.25, 0.051, 0.053, True), (0.30, 0.048, 0.049, False),
        (0.35, 0.046, 0.046, True), (0.375, 0.045, 0.044, False), (0.392, 0.044, 0.043, True),
        (0.41, 0.044, 0.043, False), (0.44, 0.045, 0.045, True), (0.50, 0.043, 0.043, False),
        (0.56, 0.040, 0.039, True), (0.61, 0.036, 0.035, False), (0.641, 0.034, 0.033, True),
    ]

    def sleeve_radius(self, x):
        rows = self.SLEEVE
        if x <= rows[0][0]:
            return rows[0][1], rows[0][2]
        for a, b in zip(rows, rows[1:]):
            if a[0] <= x <= b[0]:
                t = (x - a[0]) / (b[0] - a[0])
                return lerp(a[1], b[1], t), lerp(a[2], b[2], t)
        return rows[-1][1], rows[-1][2]

    def arm_left(self):
        """The left arm's tunic sleeve, bracer, and hand, in a mesh of its own."""
        M = Mesh()
        s = 1
        chain = self.R.arm(s)
        segs = self.L["limb"]
        M.part = "tunic"
        rings = []
        for x, ry, rz, key in self.stations(self.SLEEVE):
            rings.append(dict(c=self.arm_center(s, x), axis=(1, 0, 0), ref=(0, 1, 0), rx=ry, ry=rz))
        loft(M, rings, segs, "linen", chain.weights)
        # The bracer: wrapped leather from mid forearm to the wrist, tapering
        # onto the wrist.
        M.part = "gear"
        br = []
        for x, g, key in ((0.47, 0.004, True), (0.485, 0.008, False), (0.53, 0.009, True), (0.58, 0.009, False),
                          (0.62, 0.009, True), (0.638, 0.007, False), (0.650, 0.000, True)):
            ry, rz = self.sleeve_radius(min(x, 0.641))
            if x >= 0.65:
                ry, rz = 0.034, 0.026
            br.append((x, ry + g, rz + g, key))
        rings = [dict(c=self.arm_center(s, x), axis=(1, 0, 0), ref=(0, 1, 0), rx=ry, ry=rz)
                 for x, ry, rz, key in self.stations(br)]
        nb = len(rings)

        def bracer_mat(i, j):
            return "leather_dark" if i in (1, nb - 3) else "leather"

        loft(M, rings, segs, "leather", chain.weights, matfn=bracer_mat)
        self.hand_left(M)
        return M

    def hand_left(self, M):
        R = self.R
        s = 1
        M.part = "hands"
        hand = R.arm(s)
        fsegs = self.L["finger"]
        segs = max(6, self.L["limb"])
        wrist = R.h("hand_l")
        cy = 0.050
        palm = [(0.630, 0.030, 0.019), (0.655, 0.037, 0.021), (0.682, 0.043, 0.019), (0.708, 0.047, 0.017),
                (0.728, 0.046, 0.0145)]

        def thenar(d, p):
            return 0.006 * max(0.0, -d.y) * max(0.0, -d.z) * exp(-(((p.x - 0.665) / 0.025) ** 2))

        fingers = self.L["fingers"]
        if not fingers:
            palm += [(0.75, 0.044, 0.012), (0.79, 0.040, 0.011), (0.815, 0.032, 0.009)]
        rings = []
        for x, ry, rz in palm:
            c = Vector((x, lerp(wrist.y, cy, smoothstep(0.63, 0.70, x)), lerp(wrist.z, 1.4145, smoothstep(0.63, 0.72, x))))
            rings.append(dict(c=c, axis=(1, 0, 0), ref=(0, 1, 0), rx=ry, ry=rz, n=2.6, shape=thenar))
        mid = R.finger(s, "middle")

        def w_palm(p):
            if fingers or p.x < 0.725:
                return hand.weights(p)
            return mid.weights(Vector((p.x, R.h("middle_01_l").y, p.z)))

        loft(M, rings, segs, "skin", w_palm, cap1=True, dome=0.008)
        names = ["index", "middle", "ring", "pinky"] if fingers else []
        radii = {"index": 0.0112, "middle": 0.0116, "ring": 0.0110, "pinky": 0.0098, "thumb": 0.0128}
        for name in names + ["thumb"]:
            ch = R.finger(s, name)
            pts = [R.h(f"{name}_01_l"), R.h(f"{name}_02_l"), R.h(f"{name}_03_l"), R.h(f"{name}_04_leaf_l"),
                   R.t(f"{name}_04_leaf_l")]
            start = pts[0] - (pts[1] - pts[0]).normalized() * 0.014
            stations = [(start, 1.0)]
            for a, b in zip(pts, pts[1:4]):
                stations.append((a, 1.0))
                if self.L["stride"] == 1:
                    stations.append((a.lerp(b, 0.5), 0.97))
            tip = pts[4]
            stations.append((pts[3].lerp(tip, 0.55), 0.86))
            stations.append((pts[3].lerp(tip, 0.85), 0.68))
            r0 = radii[name]
            rings = []
            for k, (c, sc) in enumerate(stations):
                nxt = stations[min(k + 1, len(stations) - 1)][0]
                prv = stations[max(k - 1, 0)][0]
                axis = (nxt - prv).normalized()
                taper = 1.0 - 0.15 * k / len(stations)
                rings.append(dict(c=c, axis=axis, ref=(0, 0, 1), rx=r0 * sc * taper * 0.92, ry=r0 * sc * taper))
            loft(M, rings, fsegs, "skin", ch.weights, cap1=True, dome=0.004)

    # Legs and boots -----------------------------------------------------------------------

    def leg_center(self, s, z):
        R = self.R
        hip, knee, ankle = R.h(side_name("thigh", s)), R.h(side_name("calf", s)), R.h(side_name("foot", s))
        if z >= hip.z:
            return Vector((hip.x * lerp(0.94, 0.86, smoothstep(hip.z, 1.02, z)), hip.y - 0.022, z))
        if z >= 0.80:
            t = smoothstep(0.80, hip.z, z)
            base = hip.lerp(knee, (hip.z - z) / (hip.z - knee.z))
            return Vector((base.x * lerp(1.0, 0.94, t), base.y - 0.022 * t, z))
        for a, b in ((hip, knee), (knee, ankle)):
            if b.z <= z <= a.z:
                t = (a.z - z) / (a.z - b.z)
                return a.lerp(b, t)
        return Vector((ankle.x, ankle.y, z))

    LEGGINGS = [
        (1.00, 0.072, 0.076, True), (0.95, 0.074, 0.079, False), (0.88, 0.073, 0.079, True),
        (0.80, 0.070, 0.076, False), (0.72, 0.066, 0.071, True), (0.64, 0.060, 0.065, False),
        (0.575, 0.055, 0.059, True), (0.555, 0.053, 0.056, False), (0.535, 0.052, 0.055, True),
        (0.515, 0.052, 0.055, False), (0.49, 0.052, 0.056, True), (0.44, 0.053, 0.060, False),
        (0.40, 0.050, 0.058, True),
    ]

    BOOT = [
        (0.452, 0.058, 0.064, True), (0.447, 0.071, 0.077, False), (0.43, 0.073, 0.079, True),
        (0.405, 0.069, 0.075, False), (0.398, 0.061, 0.068, True), (0.34, 0.059, 0.067, False),
        (0.27, 0.053, 0.059, True), (0.20, 0.046, 0.051, False), (0.14, 0.042, 0.047, True),
        (0.10, 0.041, 0.047, False), (0.075, 0.040, 0.044, True),
    ]

    FOOT = [
        (0.122, 0.024, 0.062, True), (0.114, 0.036, 0.094, False), (0.094, 0.041, 0.118, True),
        (0.060, 0.043, 0.118, False), (0.020, 0.045, 0.112, True), (-0.020, 0.047, 0.094, False),
        (-0.060, 0.049, 0.072, True), (-0.100, 0.048, 0.060, False), (-0.140, 0.045, 0.052, True),
        (-0.175, 0.039, 0.045, False), (-0.200, 0.029, 0.038, True),
    ]

    def leg_left(self):
        M = Mesh()
        s = 1
        chain = self.R.leg(s)
        segs = self.L["limb"]
        M.part = "legs"

        def knee(d, p):
            return 0.007 * max(0.0, -d.y) * exp(-(((p.z - 0.53) / 0.035) ** 2))

        rings = [dict(c=self.leg_center(s, z), axis=(0, 0, -1), ref=(1, 0, 0), rx=rx, ry=ry, shape=knee)
                 for z, rx, ry, key in self.stations(self.LEGGINGS)]
        loft(M, rings, segs, "leggings", chain.weights)
        M.part = "boots"

        def calf(d, p):
            return 0.006 * max(0.0, d.y) * exp(-(((p.z - 0.33) / 0.06) ** 2))

        rows = self.stations(self.BOOT)
        rings = [dict(c=self.leg_center(s, z), axis=(0, 0, -1), ref=(1, 0, 0), rx=rx, ry=ry, shape=calf)
                 for z, rx, ry, key in rows]
        cuff_rows = sum(1 for r in rows if r[0] >= 0.398) - 1

        def boot_mat(i, j):
            return "leather_dark" if i < cuff_rows else "leather"

        loft(M, rings, segs, "leather", chain.weights, matfn=boot_mat)
        # The foot: heel to toe along -y, its sole flat at the ground.
        sole = -0.004
        x0 = self.R.h("foot_l").x
        rings = []
        for y, hw, top, key in self.stations(self.FOOT):
            zc = (top + sole) / 2
            hz = (top - sole) / 2 + 0.008
            rings.append(dict(c=Vector((x0 + 0.004 * smoothstep(0.0, -0.2, y), y, zc)), axis=(0, -1, 0),
                              ref=(0, 0, 1), rx=hz, ry=hw, n=2.8))

        def flat(p):
            return Vector((p.x, p.y, max(p.z, sole)))

        def w_foot(p):
            w = chain.weights(Vector((x0, p.y, min(p.z, 0.05))))
            if p.z > 0.085:
                w = mix(w, rigid("calf_l"), 0.4 * smoothstep(0.085, 0.12, p.z))
            return w

        start = len(M.f)
        loft(M, rings, segs, "leather", w_foot, cap0=True, cap1=True, post=flat, dome=0.004)
        for k in range(start, len(M.f)):
            zs = [M.v[i].z for i in M.f[k]]
            if max(zs) < sole + 0.012:
                M.m[k] = "sole"
        return M

    # Coat, hood, sash, satchel, staff ------------------------------------------------------

    COAT = [
        (0.47, 0.250, 0.200, 55, True), (0.52, 0.243, 0.193, 53, False), (0.60, 0.230, 0.180, 49, True),
        (0.70, 0.220, 0.165, 44, False), (0.80, 0.210, 0.154, 38, True), (0.88, 0.204, 0.146, 33, False),
        (0.94, 0.198, 0.142, 30, True), (1.00, 0.174, 0.128, 27, False), (1.06, 0.152, 0.116, 26, True),
        (1.12, 0.156, 0.117, 25, False), (1.19, 0.164, 0.122, 23, True), (1.26, 0.172, 0.128, 21, False),
        (1.32, 0.180, 0.126, 19, True), (1.38, 0.186, 0.117, 17, False), (1.43, 0.177, 0.106, 15, True),
        (1.468, 0.126, 0.088, 14, True),
    ]

    def coat(self):
        M = self.M
        M.part = "coat"
        rows = self.stations(self.COAT)
        half = self.L["torso"] // 2 + 1
        for s in (1, -1):
            P = []
            for z, a, b, opening, key in rows:
                _, _, yc, n = torso_at(z)
                op = math.radians(opening)
                row = []
                for j in range(half):
                    t = j / (half - 1)
                    if s > 0:
                        th = -pi / 2 + op + t * (pi - op)
                    else:
                        th = pi / 2 + t * (pi - op)
                    row.append(section_point(z, th, sizes=(a, b, yc + 0.006, 2.3 if z < 1.0 else n)))
                P.append(row)
            W = [[self.w_coat(p) for p in row] for row in P]
            nrow = len(P)
            front_col = 0 if s > 0 else half - 1

            def matfn(i, j, nrow=nrow, front_col=front_col):
                if i == 0:
                    return "coat_trim"
                if j == front_col or (front_col == half - 1 and j == half - 2):
                    return "coat_trim"
                return "coat"

            vent_rows = sum(1 for r in rows if r[0] < 0.86)
            shell(M, P, W, 0.007, "coat", "coat_inner", "coat_trim", matfn=matfn,
                  rims=(True, True, s > 0, s < 0))
            del vent_rows
        # Sleeves to the elbow with a turned-back cuff.
        L = Mesh()
        L.part = "coat"
        chain = self.R.arm(1)
        rows = []
        for x, g, key in ((0.10, 0.013, True), (0.16, 0.014, False), (0.22, 0.014, True), (0.28, 0.014, False),
                          (0.33, 0.015, True), (0.355, 0.024, True), (0.395, 0.026, False), (0.41, 0.010, True)):
            ry, rz = self.sleeve_radius(x)
            rows.append((x, ry + g, rz + g, key))
        rings = [dict(c=self.arm_center(1, x), axis=(1, 0, 0), ref=(0, 1, 0), rx=ry, ry=rz)
                 for x, ry, rz, key in self.stations(rows)]
        nr = len(rings)

        def cuff(i, j):
            return "coat_trim" if i >= nr - 3 else "coat"

        loft(L, rings, self.L["limb"], "coat", chain.weights, matfn=cuff)
        M.extend(L)
        M.extend_mirrored(L)

    def hood(self):
        """The hood, worn down: a collar about the neck and the hood's folds
        lying on her upper back, lined in linen."""
        M = self.M
        M.part = "coat"
        segs = max(8, self.L["limb"])
        # The collar: a soft roll around the neck.
        rows = []
        tube = max(4, segs // 3)
        for k in range(segs):
            t = 2 * pi * k / segs
            c = Vector((0.083 * cos(t), 0.028 + 0.078 * sin(t), 1.488 + 0.018 * sin(t)))
            rows.append(c)
        P = []
        for i in range(tube + 1):
            u = 2 * pi * i / tube
            row = []
            for k in range(segs):
                c = rows[k]
                radial = Vector((c.x, c.y - 0.028, 0)).normalized()
                p = c + radial * (0.018 * cos(u)) + Vector((0, 0, 0.016 * sin(u)))
                row.append(p)
            P.append(row)
        W = [[self.torso_chain.weights(p) for p in row] for row in P]
        grid(M, P, W, "coat", wrap=True)
        # The hood's body on the back: a soft, flattened dome.
        c0 = Vector((0, 0.112, 1.425))
        rings = []
        n = max(5, self.L["torso"] // 4)
        for i in range(n + 1):
            v = -pi / 2 + pi * i / n
            rings.append(dict(c=c0 + Vector((0, 0.008 * cos(v), 0.066 * sin(v))), axis=(0, 0, 1), ref=(1, 0, 0),
                              rx=max(0.008, 0.100 * cos(v)), ry=max(0.006, 0.032 * cos(v)), n=2.2,
                              shape=lambda d, p: 0.012 * max(0.0, -d.y) * 0))
        start = len(M.f)

        def w_hood(p):
            return mix(rigid("spine_03"), rigid("neck_01"), 0.25 * smoothstep(1.43, 1.49, p.z))

        loft(M, rings, max(8, self.L["torso"] // 2), "coat", w_hood, cap0=True, cap1=True)
        # The lining shows where the hood folds open toward her neck.
        for k in range(start, len(M.f)):
            cz = sum(M.v[i].z for i in M.f[k]) / len(M.f[k])
            cy = sum(M.v[i].y for i in M.f[k]) / len(M.f[k])
            if cz > 1.46 and cy < 0.112:
                M.m[k] = "lining"

    def sash(self):
        M = self.M
        M.part = "gear"
        segs = self.L["torso"]
        rings = []
        for z, g in ((0.985, 0.010), (1.005, 0.015), (1.045, 0.015), (1.068, 0.010)):
            a, b, yc, n = (lerp(0.170, 0.152, (z - 1.0) / 0.06), lerp(0.124, 0.116, (z - 1.0) / 0.06),
                           torso_at(z)[2] + 0.006, 2.3)
            rings.append(dict(c=Vector((0, yc, z)), axis=(0, 0, 1), ref=(1, 0, 0), rx=a + g, ry=b + g, n=n))
        loft(M, rings, segs, "sash", self.w_coat)
        # The knot at her left hip and its two hanging ends.
        knot = Vector((0.150, -0.075, 1.025))
        kr = [dict(c=knot + Vector((0, 0, dz)), axis=(0.5, -0.8, 0), ref=(0, 0, 1), rx=r, ry=r * 0.8)
              for dz, r in ((0.0, 0.01), (0.0, 0.022), (0.0, 0.024), (0.0, 0.012))]
        for k, r in enumerate(kr):
            r["c"] = knot + Vector((0.5, -0.8, 0)).normalized() * (0.006 * k - 0.004)
        loft(M, kr, max(6, segs // 3), "sash", self.w_coat, cap0=True, cap1=True)
        for dx, length, ang in ((0.0, 0.20, 0.10), (0.03, 0.15, 0.28)):
            P = []
            steps = 4 if self.L["stride"] == 1 else 2
            for i in range(steps + 1):
                t = i / steps
                c = knot + Vector((dx + sin(ang) * length * t, -0.012 - 0.01 * t, -length * t))
                w = 0.026 * (1 - 0.2 * t)
                tangent = Vector((cos(ang), 0.15, 0)).normalized()
                P.append([c + tangent * w, c - tangent * w])
            P.reverse()
            W = [[self.w_coat(p) for p in row] for row in P]
            shell(M, [[r[1], r[0]] for r in P], W, 0.004, "sash", "sash", "sash")

    def strap_path(self, steps):
        """Points where a plane from her left shoulder to her right hip cuts
        the coat, offset out from it."""
        A = Vector((0.098, 0.020, 1.478))
        B = Vector((-0.18, -0.115, 0.985))
        C = Vector((-0.18, 0.135, 1.005))
        n = (B - A).cross(C - A).normalized()
        pts = []
        for k in range(steps):
            t = 2 * pi * k / steps
            z = 1.2
            for _ in range(12):
                a, b, yc, nn = self.coat_size(z)
                p = section_point(z, t, sizes=(a + 0.010, b + 0.010, yc, nn))
                # Move z so p lies on the plane.
                dz = -n.dot(p - A) / n.z if abs(n.z) > 1e-6 else 0
                z = max(0.95, min(1.48, z + dz * 0.8))
            pts.append(p)
        return pts, n

    def coat_size(self, z):
        rows = self.COAT
        if z <= rows[0][0]:
            r = rows[0]
            return r[1], r[2], torso_at(z)[2] + 0.006, 2.3
        for a, b in zip(rows, rows[1:]):
            if a[0] <= z <= b[0]:
                t = (z - a[0]) / (b[0] - a[0])
                t = t * t * (3 - 2 * t)
                return lerp(a[1], b[1], t), lerp(a[2], b[2], t), torso_at(z)[2] + 0.006, torso_at(z)[3]
        r = rows[-1]
        return r[1], r[2], torso_at(z)[2] + 0.006, torso_at(z)[3]

    def strap(self):
        M = self.M
        M.part = "gear"
        steps = max(12, self.L["torso"])
        pts, n = self.strap_path(steps)
        w, t = 0.018, 0.005
        P = []
        for k, p in enumerate(pts):
            q = pts[(k + 1) % steps] - pts[k - 1]
            out = Vector((p.x, p.y - 0.01, 0)).normalized()
            side = n
            P.append([p + side * w, p + side * w + out * t, p - side * w + out * t, p - side * w])
        # Rings around the band's path: four corners each.
        rows = [[P[k][c] for k in range(steps)] for c in range(4)]
        rows.append(rows[0])
        W = [[self.w_coat(p) for p in row] for row in rows]
        idx = [[M.vert(rows[i][j], W[i][j]) for j in range(steps)] for i in range(4)]
        for i in range(4):
            i2 = (i + 1) % 4
            for j in range(steps):
                j2 = (j + 1) % steps
                q = (idx[i][j], idx[i][j2], idx[i2][j2], idx[i2][j])
                mid = (M.v[q[0]] + M.v[q[2]]) / 2
                nrm = (M.v[q[1]] - M.v[q[0]]).cross(M.v[q[3]] - M.v[q[0]])
                core = (P[j][0] + P[j][1] + P[j][2] + P[j][3]) / 4
                M.face(q if nrm.dot(mid - core) > 0 else q[::-1], "leather")

    def satchel(self):
        M = self.M
        M.part = "gear"
        c0 = Vector((-0.238, 0.005, 0.935))
        w = self.w_coat(Vector((-0.20, 0.0, 0.94)))
        segs = max(8, self.L["limb"])
        rings = []
        for dy, sc in ((-0.100, 0.70), (-0.094, 0.92), (-0.080, 1.0), (0.080, 1.0), (0.094, 0.92), (0.100, 0.70)):
            rings.append(dict(c=c0 + Vector((0, dy, 0)), axis=(0, 1, 0), ref=(1, 0, 0), rx=0.032 * sc,
                              ry=0.072 * sc, n=3.2))
        loft(M, rings, segs, "leather", lambda p: w, cap0=True, cap1=True)
        # The flap over its outer face, and a copper buckle.
        P = []
        for i, z in enumerate((0.985, 0.955, 0.925, 0.900)):
            row = []
            for y in (-0.096, -0.04, 0.04, 0.096):
                row.append(Vector((-0.238 - 0.035 - 0.004 * (1 - i / 3), y + 0.005, z)))
            P.append(row)
        P.reverse()
        W = [[w for _ in r] for r in P]
        shell(M, [r[::-1] for r in P], W, 0.004, "leather_dark", "leather_dark", "leather_dark")
        bk = Vector((-0.278, 0.005, 0.905))
        rings = [dict(c=bk + Vector((d, 0, 0)), axis=(-1, 0, 0), ref=(0, 0, 1), rx=0.013, ry=0.011)
                 for d in (0.0, -0.005)]
        loft(M, rings, 6, "copper", lambda p: w, cap1=True)

    def staff(self):
        """A staff carried across her back, rigid to the upper spine."""
        M = self.M
        M.part = "gear"
        a = Vector((-0.20, 0.205, 0.56))
        b = Vector((0.25, 0.180, 1.93))
        axis = (b - a).normalized()
        segs = 5 if self.L.get("lite") else max(6, self.L["limb"] // 2)
        rings = []
        n = 6 if self.L["stride"] == 1 else 3
        for i in range(n + 1):
            t = i / n
            c = a.lerp(b, t) + Vector((0.006 * sin(t * 9), 0, 0))
            rings.append(dict(c=c, axis=axis, ref=(0, 1, 0), rx=0.015 - 0.003 * t, ry=0.015 - 0.003 * t))
        loft(M, rings, segs, "wood", lambda p: rigid("spine_03"), cap0=True, cap1=True)
        # A copper ring under the head, and a knot of leaves.
        rc = a.lerp(b, 0.90)
        rings = [dict(c=rc + axis * d, axis=axis, ref=(0, 1, 0), rx=0.019, ry=0.019) for d in (-0.02, 0.02)]
        loft(M, rings, segs, "copper", lambda p: rigid("spine_03"))
        if not self.L.get("lite"):
            for k, (ang, tilt) in enumerate(((0.6, 0.5), (2.6, 0.7), (4.4, 0.45))):
                base = b - axis * 0.02
                d = Vector((cos(ang), sin(ang) * 0.4, 0.6 + tilt)).normalized()
                side = d.cross(Vector((0, 1, 0))).normalized()
                tip = base + d * 0.10
                mid = base + d * 0.05
                quad = [base, mid + side * 0.022, tip, mid - side * 0.022]
                ids = [M.vert(p, rigid("spine_03")) for p in quad]
                M.face(ids, "leaf")
                M.face(ids[::-1], "leaf")

    def clasps(self):
        """Two copper clasps on the coat's front edges at the chest, and one at
        the collar."""
        M = self.M
        M.part = "gear"
        for z in (1.22, 1.31):
            a, b, yc, n = self.coat_size(z)
            op = math.radians(23 if z < 1.25 else 20)
            for s in (1, -1):
                th = -pi / 2 + s * op
                p = section_point(z, th, sizes=(a, b, yc, n))
                d = Vector((cos(th), sin(th), 0)).normalized()
                rings = [dict(c=p + d * k, axis=d, ref=(0, 0, 1), rx=0.011, ry=0.011) for k in (0.0, 0.006)]
                loft(M, rings, 6, "copper", lambda q: self.w_coat(p), cap1=True)
        p = Vector((0, -0.054, 1.468))
        rings = [dict(c=p + Vector((0, -k, 0)), axis=(0, -1, 0), ref=(0, 0, 1), rx=0.012, ry=0.012) for k in (0, 0.007)]
        loft(M, rings, 6, "copper", lambda q: self.torso_chain.weights(p), cap1=True)

    # All of her -----------------------------------------------------------------------------

    # The build, in three reusable parts --------------------------------------------------
    #
    # A later female player character reuses the base body and the head; the
    # outfit is Alice's own.

    def build_body(self):
        """The base body: the skin and underlayer that show (hands, legs,
        and feet), lofted on the Universal rig."""
        arm = self.arm_left()
        self.M.extend(arm)
        self.M.extend_mirrored(arm)
        leg = self.leg_left()
        self.M.extend(leg)
        self.M.extend_mirrored(leg)

    def build_head(self, head, eyes):
        """The hair and the eyes' highlights, fitted to the reshaped head
        `head` and eyes `eyes` (Blender objects from `ubc_head`)."""
        from mathutils.bvhtree import BVHTree

        dg = bpy.context.evaluated_depsgraph_get()
        self.bvh = BVHTree.FromObject(head, dg)
        self.hair()
        self.glints(eyes)

    def glints(self, eyes):
        """A small highlight on each cornea, up and toward her outer side."""
        M = self.M
        M.part = "face"
        pts = [v.co.copy() for v in eyes.data.vertices]
        for s in (1, -1):
            side = [p for p in pts if p.x * s > 0]
            front = min(p.y for p in side)
            cornea = [p for p in side if p.y < front + 0.004]
            c = sum(cornea, Vector()) / len(cornea)
            at = Vector((c.x + s * 0.0035, front - 0.0006, c.z + 0.0035))
            n = 6 if self.L.get("lite") else 8
            ring = [M.vert(at + Vector((0.0016 * cos(2 * pi * k / n), 0, 0.0016 * sin(2 * pi * k / n))),
                           rigid("Head")) for k in range(n)]
            fan(M, ring, at + Vector((0, -0.0002, 0)), rigid("Head"), "glint", Vector((0, -1, 0)))

    def build_outfit(self):
        """Alice's own clothes and gear."""
        self.tunic()
        self.coat()
        self.hood()
        self.sash()
        if not self.L.get("lite"):
            self.strap()
            self.clasps()
        self.satchel()
        self.staff()

    def build(self, head, eyes):
        self.build_body()
        self.build_head(head, eyes)
        self.build_outfit()
        return self.M


# --- Blender objects, materials, bake, export -------------------------------------------------


def material(name):
    m = bpy.data.materials.get(name)
    if m:
        return m
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    bsdf.inputs["Roughness"].default_value = 0.8
    col = linear(PALETTE[name])
    m.diffuse_color = (*col, 1)
    # Procedural variation: woven linen, mottled wool and leather, and hair
    # strands, all in object space so every variant bakes the same look.
    variation = {
        "linen": ("noise", 0.05), "linen_shade": ("noise", 0.05), "coat": ("noise", 0.08),
        "coat_inner": ("noise", 0.06), "coat_trim": ("noise", 0.06), "leather": ("noise", 0.12),
        "leather_dark": ("noise", 0.10), "leggings": ("noise", 0.05), "sash": ("noise", 0.06),
        "hair": ("strands", 0.30), "hair_inner": ("strands", 0.20), "wood": ("grain", 0.25),
    }.get(name)
    if not variation:
        bsdf.inputs["Base Color"].default_value = (*col, 1)
        return m
    kind, amount = variation
    coord = nt.nodes.new("ShaderNodeTexCoord")
    ramp_in = None
    if kind == "noise":
        tex = nt.nodes.new("ShaderNodeTexNoise")
        tex.inputs["Scale"].default_value = 14.0
        tex.inputs["Detail"].default_value = 3.0
        nt.links.new(coord.outputs["Object"], tex.inputs["Vector"])
        ramp_in = tex.outputs["Fac"]
    elif kind == "weave":
        w1 = nt.nodes.new("ShaderNodeTexWave")
        w1.wave_type = "BANDS"
        w1.bands_direction = "X"
        w1.inputs["Scale"].default_value = 160.0
        w2 = nt.nodes.new("ShaderNodeTexWave")
        w2.wave_type = "BANDS"
        w2.bands_direction = "Z"
        w2.inputs["Scale"].default_value = 160.0
        for w in (w1, w2):
            nt.links.new(coord.outputs["Object"], w.inputs["Vector"])
        add = nt.nodes.new("ShaderNodeMath")
        add.operation = "MULTIPLY"
        nt.links.new(w1.outputs["Fac"], add.inputs[0])
        nt.links.new(w2.outputs["Fac"], add.inputs[1])
        ramp_in = add.outputs[0]
    elif kind == "strands":
        geo = nt.nodes.new("ShaderNodeNewGeometry")
        sep = nt.nodes.new("ShaderNodeSeparateXYZ")
        nt.links.new(geo.outputs["Position"], sep.inputs[0])
        sub = nt.nodes.new("ShaderNodeMath")
        sub.operation = "SUBTRACT"
        nt.links.new(sep.outputs["Y"], sub.inputs[0])
        sub.inputs[1].default_value = -0.006
        at = nt.nodes.new("ShaderNodeMath")
        at.operation = "ARCTAN2"
        nt.links.new(sub.outputs[0], at.inputs[0])
        nt.links.new(sep.outputs["X"], at.inputs[1])
        noise = nt.nodes.new("ShaderNodeTexNoise")
        noise.inputs["Scale"].default_value = 9.0
        nt.links.new(coord.outputs["Object"], noise.inputs["Vector"])
        mul = nt.nodes.new("ShaderNodeMath")
        mul.operation = "MULTIPLY_ADD"
        nt.links.new(at.outputs[0], mul.inputs[0])
        mul.inputs[1].default_value = 34.0
        nt.links.new(noise.outputs["Fac"], mul.inputs[2])
        sn = nt.nodes.new("ShaderNodeMath")
        sn.operation = "SINE"
        nt.links.new(mul.outputs[0], sn.inputs[0])
        half = nt.nodes.new("ShaderNodeMath")
        half.operation = "MULTIPLY_ADD"
        nt.links.new(sn.outputs[0], half.inputs[0])
        half.inputs[1].default_value = 0.5
        half.inputs[2].default_value = 0.5
        # A sheen band near the crown.
        band = nt.nodes.new("ShaderNodeMapRange")
        nt.links.new(sep.outputs["Z"], band.inputs["Value"])
        band.inputs["From Min"].default_value = 1.66
        band.inputs["From Max"].default_value = 1.74
        mx = nt.nodes.new("ShaderNodeMath")
        mx.operation = "ADD"
        nt.links.new(half.outputs[0], mx.inputs[0])
        sheen = nt.nodes.new("ShaderNodeMath")
        sheen.operation = "MULTIPLY"
        nt.links.new(band.outputs["Result"], sheen.inputs[0])
        sheen.inputs[1].default_value = 0.35
        nt.links.new(sheen.outputs[0], mx.inputs[1])
        ramp_in = mx.outputs[0]
    elif kind == "grain":
        w = nt.nodes.new("ShaderNodeTexWave")
        w.wave_type = "RINGS"
        w.inputs["Scale"].default_value = 30.0
        w.inputs["Distortion"].default_value = 6.0
        nt.links.new(coord.outputs["Object"], w.inputs["Vector"])
        ramp_in = w.outputs["Fac"]
    mixn = nt.nodes.new("ShaderNodeMix")
    mixn.data_type = "RGBA"
    nt.links.new(ramp_in, mixn.inputs["Factor"])
    lo = tuple(c * (1 - amount) for c in col)
    hi = tuple(min(1.0, c * (1 + amount)) for c in col)
    mixn.inputs["A"].default_value = (*lo, 1)
    mixn.inputs["B"].default_value = (*hi, 1)
    nt.links.new(mixn.outputs["Result"], bsdf.inputs["Base Color"])
    return m


def base_image(obj):
    """The base-color image of `obj`'s first material, from the CC0 base."""
    nt = obj.data.materials[0].node_tree
    link = nt.nodes["Principled BSDF"].inputs["Base Color"].links[0]
    node = link.from_node
    while node.type != "TEX_IMAGE":
        node = next(i.links[0].from_node for i in node.inputs if i.links)
    return node.image


def textured(name, image, uv="UVMap"):
    """A material sampling `image` through UV map `uv`; returns the
    material, its node tree, and the image's color output."""
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    uvn = nt.nodes.new("ShaderNodeUVMap")
    uvn.uv_map = uv
    tex = nt.nodes.new("ShaderNodeTexImage")
    tex.image = image
    nt.links.new(uvn.outputs["UV"], tex.inputs["Vector"])
    nt.nodes["Principled BSDF"].inputs["Roughness"].default_value = 0.6
    return m, nt, tex.outputs["Color"]


def face_material(obj):
    """Alice's skin over the base's painted face: its lips, brows' shadow,
    and shading, scaled to her tone, warmed on the cheeks, nose, and lips."""
    import numpy as np

    image = base_image(obj)
    px = np.array(image.pixels[:], dtype=np.float32).reshape(-1, 4)[:, :3]
    # The pixels are stored sRGB-encoded; the shader samples them linear.
    px = np.where(px <= 0.04045, px / 12.92, ((px + 0.055) / 1.055) ** 2.4)
    # The base's typical skin, from its brighter half.
    lum = px.mean(axis=1)
    typical = px[lum > np.median(lum)].mean(axis=0)
    target = linear(PALETTE["skin"])
    factor = tuple(min(6.0, t / max(c, 1e-4)) for t, c in zip(target, typical))
    m, nt, color = textured("face_skin", image)
    # A third of the way toward the typical tone: the base's painted
    # contours are a superhero's; Alice's are softer.
    soft = nt.nodes.new("ShaderNodeMix")
    soft.data_type = "RGBA"
    soft.inputs["Factor"].default_value = 0.35
    nt.links.new(color, soft.inputs["A"])
    soft.inputs["B"].default_value = (*typical.tolist(), 1)
    mul = nt.nodes.new("ShaderNodeMix")
    mul.data_type = "RGBA"
    mul.blend_type = "MULTIPLY"
    mul.inputs["Factor"].default_value = 1.0
    nt.links.new(soft.outputs["Result"], mul.inputs["A"])
    mul.inputs["B"].default_value = (*factor, 1)
    attr = nt.nodes.new("ShaderNodeAttribute")
    attr.attribute_name = "warm"
    sep = nt.nodes.new("ShaderNodeSeparateColor")
    nt.links.new(attr.outputs["Color"], sep.inputs["Color"])
    scale = nt.nodes.new("ShaderNodeMath")
    scale.operation = "MULTIPLY"
    nt.links.new(sep.outputs[0], scale.inputs[0])
    scale.inputs[1].default_value = 0.30
    warm = nt.nodes.new("ShaderNodeMix")
    warm.data_type = "RGBA"
    warm.blend_type = "MULTIPLY"
    nt.links.new(scale.outputs[0], warm.inputs["Factor"])
    nt.links.new(mul.outputs["Result"], warm.inputs["A"])
    warm.inputs["B"].default_value = (*linear(PALETTE["blush"]), 1)
    # The lips: a soft rose over the base's own lip shading.
    lips = nt.nodes.new("ShaderNodeMath")
    lips.operation = "MULTIPLY"
    nt.links.new(sep.outputs[1], lips.inputs[0])
    lips.inputs[1].default_value = 0.55
    rose = nt.nodes.new("ShaderNodeMix")
    rose.data_type = "RGBA"
    rose.blend_type = "MULTIPLY"
    nt.links.new(lips.outputs[0], rose.inputs["Factor"])
    nt.links.new(warm.outputs["Result"], rose.inputs["A"])
    rose.inputs["B"].default_value = (*linear(PALETTE["lips"]), 1)
    nt.links.new(rose.outputs["Result"], nt.nodes["Principled BSDF"].inputs["Base Color"])
    obj.data.materials.clear()
    obj.data.materials.append(m)


def eye_material(obj):
    """The base's eye texture: sclera, a brown iris, and a pupil."""
    m, nt, color = textured("eyes", base_image(obj))
    nt.links.new(color, nt.nodes["Principled BSDF"].inputs["Base Color"])
    obj.data.materials.clear()
    obj.data.materials.append(m)


def to_object(M, arm):
    me = bpy.data.meshes.new("Alice")
    me.from_pydata([tuple(v) for v in M.v], [], [list(f) for f in M.f])
    me.update()
    obj = bpy.data.objects.new("Alice", me)
    bpy.context.scene.collection.objects.link(obj)
    names = sorted(set(M.m))
    for n in names:
        me.materials.append(material(n))
    slot = {n: i for i, n in enumerate(names)}
    for poly, m in zip(me.polygons, M.m):
        poly.material_index = slot[m]
        poly.use_smooth = True
    bones = {b.name for b in arm.data.bones}
    groups = {}
    for i, w in enumerate(M.w):
        # Keep the four largest influences and normalize.
        top = sorted(w.items(), key=lambda kv: -kv[1])[:4]
        total = sum(x for _, x in top)
        for name, x in top:
            assert name in bones, name
            if name not in groups:
                groups[name] = obj.vertex_groups.new(name=name)
            groups[name].add([i], x / total, "REPLACE")
    obj.parent = arm
    mod = obj.modifiers.new("Armature", "ARMATURE")
    mod.object = arm
    # Smooth by angle, so hems and caps keep a crease.
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    bpy.ops.object.shade_smooth_by_angle(angle=math.radians(55))
    obj.select_set(False)
    return obj


def join_head(obj, head, eyes):
    """Joins the head and eyes into Alice's object, at most four influences
    a vertex, normalized."""
    bpy.ops.object.select_all(action="DESELECT")
    for o in (obj, head, eyes):
        o.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.join()
    for p in obj.data.polygons:
        p.use_smooth = True
    bpy.ops.object.vertex_group_limit_total(group_select_mode="ALL", limit=4)
    bpy.ops.object.vertex_group_normalize_all(group_select_mode="ALL", lock_active=False)
    # One armature modifier.
    mods = [m for m in obj.modifiers if m.type == "ARMATURE"]
    for m in mods[1:]:
        obj.modifiers.remove(m)
    return obj


def check_weights(obj):
    """Every vertex has one to four influences summing to one."""
    bad = 0
    for v in obj.data.vertices:
        ws = [g.weight for g in v.groups if g.weight > 0]
        if not (1 <= len(ws) <= 4) or abs(sum(ws) - 1) > 1e-3:
            bad += 1
    assert bad == 0, f"{bad} vertices with bad weights"


def unwrap(obj, lod):
    """Smart-project UVs; the face gets three times the texel density."""
    me = obj.data
    head = {i for i, p in enumerate(me.polygons) if obj.material_slots[p.material_index].name in
            ("face_skin", "eyes", "glint", "brow")
            and sum(me.vertices[k].co.z for k in p.vertices) / len(p.vertices) > 1.50}
    verts = {k for i in head for k in me.polygons[i].vertices}
    saved = {k: me.vertices[k].co.copy() for k in verts}
    center = Vector((0, -0.006, 1.646))
    for k in verts:
        me.vertices[k].co = center + (me.vertices[k].co - center) * 2.2
    atlas = me.uv_layers.new(name="atlas")
    me.uv_layers.active = atlas
    atlas.active_render = True
    bpy.ops.object.select_all(action="DESELECT")
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    margin = 2.5 / lod["tex"]
    bpy.ops.uv.smart_project(angle_limit=math.radians(60), island_margin=margin, area_weight=0.0,
                             scale_to_bounds=False)
    bpy.ops.uv.pack_islands(margin=margin, rotate=True)
    bpy.ops.object.mode_set(mode="OBJECT")
    for k, co in saved.items():
        me.vertices[k].co = co


def bake(obj, lod):
    """Bake color, ambient occlusion, and object-space normals; return the
    combined base-color image."""
    import numpy as np

    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.device = "CPU"
    scene.cycles.samples = lod["samples"]
    scene.cycles.seed = 7
    scene.cycles.use_denoising = False
    if scene.world is None:
        scene.world = bpy.data.worlds.new("World")
    scene.world.light_settings.distance = 0.12
    size = lod["tex"]
    images = {}
    for kind in ("color", "ao", "normal"):
        img = bpy.data.images.new(f"alice_{kind}", size, size, float_buffer=True)
        img.colorspace_settings.name = "Non-Color" if kind != "color" else "Linear Rec.709"
        images[kind] = img
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    for kind, args in (("color", dict(type="DIFFUSE", pass_filter={"COLOR"})), ("ao", dict(type="AO")),
                       ("normal", dict(type="NORMAL", normal_space="OBJECT"))):
        for slot in obj.material_slots:
            nt = slot.material.node_tree
            node = nt.nodes.get("bake") or nt.nodes.new("ShaderNodeTexImage")
            node.name = "bake"
            node.image = images[kind]
            nt.nodes.active = node
        bpy.ops.object.bake(margin=max(2, size // 128), use_clear=True, **args)
    px = {k: np.array(img.pixels[:], dtype=np.float32).reshape(size, size, 4) for k, img in images.items()}
    color = px["color"][..., :3]
    ao = px["ao"][..., :1]
    # Soften the occlusion's sampling noise with a small blur.
    k = np.array([1, 4, 6, 4, 1], dtype=np.float32)
    k /= k.sum()
    for axis in (0, 1):
        ao = sum(np.roll(ao, s - 2, axis=axis) * k[s] for s in range(5))
    up = px["normal"][..., 2:3] * 2 - 1
    shade = (0.52 + 0.48 * np.clip(ao, 0, 1) ** 0.85) * (0.90 + 0.10 * np.clip(up, -1, 1))
    lin = np.clip(color * shade, 0, 1)
    srgb = np.where(lin <= 0.0031308, lin * 12.92, 1.055 * np.power(lin, 1 / 2.4) - 0.055)
    # Six bits a channel: soft shading shows no banding at a character's
    # size on screen, and the atlas compresses to about half.
    srgb = np.round(np.clip(srgb, 0, 1) * 63) / 63
    out = np.concatenate([srgb, np.ones_like(srgb[..., :1])], axis=-1)
    final = bpy.data.images.new("Alice_BaseColor", size, size, alpha=False)
    final.colorspace_settings.name = "sRGB"
    final.pixels[:] = out.reshape(-1).tolist()
    final.pack()
    return final


def finish(obj, image):
    """One material sampling the baked atlas."""
    m = bpy.data.materials.new("alice")
    m.use_nodes = True
    nt = m.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    bsdf.inputs["Roughness"].default_value = 0.85
    tex = nt.nodes.new("ShaderNodeTexImage")
    tex.image = image
    nt.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    obj.data.materials.clear()
    obj.data.materials.append(m)
    for p in obj.data.polygons:
        p.material_index = 0
    # Only the atlas ships; the base's own UV map sampled its textures.
    for layer in list(obj.data.uv_layers):
        if layer.name != "atlas":
            obj.data.uv_layers.remove(layer)
    for attr in list(obj.data.color_attributes):
        obj.data.color_attributes.remove(attr)


def quick_views(out_dir, name):
    """Render the built scene from four sides, before any bake."""
    os.makedirs(out_dir, exist_ok=True)
    s = bpy.context.scene
    s.render.engine = "BLENDER_EEVEE"
    s.render.resolution_x, s.render.resolution_y = 600, 900
    s.world = bpy.data.worlds.new("w")
    s.world.color = (0.55, 0.6, 0.65)
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    sun.rotation_euler = (0.7, 0.2, -0.5)
    sun.data.energy = 3.0
    s.collection.objects.link(sun)
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    cam.data.lens = 85
    s.collection.objects.link(cam)
    s.camera = cam
    target = Vector((0, 0, 0.95))
    for label, ang in (("front", 0), ("three_quarter", 35), ("side", 90), ("back", 180)):
        a = math.radians(ang)
        cam.location = target + Vector((sin(a) * 6.2, -cos(a) * 6.2, 0.35))
        cam.rotation_euler = (target - cam.location).to_track_quat("-Z", "Y").to_euler()
        s.render.filepath = os.path.join(out_dir, f"{name}_{label}.png")
        bpy.ops.render.render(write_still=True)
    s.render.resolution_x, s.render.resolution_y = 700, 700
    for label, at, offset in (("face", (0, 0, 1.62), (0.25, -1.2, 0.05)),
                              ("hand", (0.72, 0.05, 1.41), (0.15, -0.55, 0.30)),
                              ("boot", (0.11, -0.02, 0.15), (0.45, -0.75, 0.25)),
                              ("back_detail", (0, 0.1, 1.25), (0.4, 2.2, 0.3))):
        at = Vector(at)
        cam.location = at + Vector(offset)
        cam.rotation_euler = (at - cam.location).to_track_quat("-Z", "Y").to_euler()
        sun.rotation_euler = (0.7, 0.2, -0.5) if label != "back_detail" else (-0.7, 0.2, 0.5)
        s.render.filepath = os.path.join(out_dir, f"{name}_{label}.png")
        bpy.ops.render.render(write_still=True)


def main():
    argv = kit.args()
    quick = None
    if "--quick" in argv:
        i = argv.index("--quick")
        quick = argv[i + 1]
        argv = argv[:i] + argv[i + 2:]
    out_dir = argv[0] if argv and not argv[0].startswith("lod") else OUT
    variant = next((a for a in argv if a.startswith("lod")), "lod1")
    lod = LODS[variant]
    arm, joints = load_rig()
    head, eyes = ubc_head(lod)
    alice = Alice(Rig(joints), lod)
    M = alice.build(head, eyes)
    obj = to_object(M, arm)
    parts = {}
    for f, tag in zip(M.f, M.tag):
        parts[tag] = parts.get(tag, 0) + len(f) - 2
    parts["head"] = sum(len(p.vertices) - 2 for p in head.data.polygons)
    parts["eyes"] = sum(len(p.vertices) - 2 for p in eyes.data.polygons)
    obj = join_head(obj, head, eyes)
    check_weights(obj)
    tris = sum(parts.values())
    print("PARTS", variant, json.dumps(parts))
    assert tris <= lod["budget"], f"{variant}: {tris} triangles exceeds {lod['budget']}"
    if quick:
        quick_views(quick, variant)
        print("QUICK", variant, tris)
        return
    unwrap(obj, lod)
    image = bake(obj, lod)
    finish(obj, image)
    out = os.path.join(out_dir, f"alice.{variant}.glb")
    os.makedirs(out_dir, exist_ok=True)
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.export_scene.gltf(filepath=out, export_format="GLB", export_yup=True, export_apply=True,
                              export_animations=False, export_skins=True, export_image_format="AUTO",
                              export_texcoords=True, export_normals=True)
    info = {"out": os.path.relpath(out, kit.REPO), "variant": variant, "triangles": tris,
            "budget": lod["budget"], "texture": lod["tex"], "vertices": len(M.v), "parts": parts,
            "bytes": os.path.getsize(out), "blender": bpy.app.version_string}
    print("MODEL", json.dumps(info))


main()
