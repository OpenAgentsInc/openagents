"""Alice, Verse's original explorer-druid, on the Universal rig.

Run headless, one variant per run:
    Blender -b --factory-startup --python scripts/blender/alice.py -- \
        [OUT_DIR] [lod0|lod1|lod2|lod3] [--quick PREVIEW_DIR]

OUT_DIR defaults to assets/verse/characters/original/alice/build and the variant
to lod1. Each run writes `alice.<variant>.glb` and prints one `MODEL` line.
`--quick` skips the bake and export and renders the built scene from four
sides into PREVIEW_DIR, for fast iteration.

Alice is an NPC, a character the world places; a later female player
character reuses her base body (`build_body`) and head (`ubc_head`,
`build_head`) with an outfit of its own (`build_outfit` is Alice's).

Modes: Reference for the body, hair, clothing, and gear, made here from
primitives and procedural shading; Compose for the head and eyes, cut from
the CC0 Universal Base Characters female body (Quaternius,
`Superhero_Female_FullBody.gltf`) and reshaped by `reshape`, so the face has
real structure. `alice_paint.py` paints her face, eyes, and hair strips. The same file gives the skeleton: Alice has the Universal 65
joints with their names, parents, and rest transforms, and every Universal
Animation Library clip plays on her.

Design (docs/verse/female-character.md): an explorer-druid in a forest-green,
knee-length open coat over a cream linen tunic, dark leggings, tall warm-brown
boots, wrapped leather bracers, a copper-red sash, a cross-body satchel, a
staff on her back, and a collar. Long, softly wavy auburn hair with a center
part and locks that frame her face, as alpha-tested cards over a scalp cap.
A stylized, hand-painted face in the look of our world. She avoids every Echo signature the
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
  a soft top light, so folds read without a normal map. Its bottom band
  holds the painted hair strips every card samples, with their alpha.
"""

import json
import math
import os
import sys
from math import cos, exp, pi, sin

import bmesh
import bpy
from mathutils import Matrix, Vector

sys.path.insert(0, os.path.dirname(__file__))
import alice_paint  # noqa: E402
import kit  # noqa: E402

BASE = os.path.join(
    kit.REPO, "assets", "verse", "characters", "quaternius", "base", "Superhero_Female_FullBody.gltf"
)
OUT = os.path.join(kit.REPO, "assets", "verse", "characters", "original", "alice", "build")

# Ring segments, hair grid, finger detail, station stride, atlas edge, and
# the triangle budget per variant (docs/verse/female-character.md).
LODS = {
    # Near levels are built coarse and subdivided once (`subdiv`), so every
    # surface is smooth; far levels are built at their final density. The
    # hair cards (`cards`: top layer, under layer, and wisps a side;
    # `card_steps`: rows over the skull and down the fall) are never
    # subdivided.
    "lod0": dict(limb=16, torso=32, head=(32, 22), hair=(40, 10), finger=6, fingers=True, stride=1,
                 tex=2048, budget=100000, samples=128, subdiv=1, head_subdiv=1,
                 cards=(24, 16, 4), card_steps=(7, 22), card_across=2),
    "lod1": dict(limb=8, torso=16, head=(24, 16), hair=(28, 6), finger=4, fingers=True, stride=1,
                 tex=1024, budget=46000, samples=96, subdiv=1,
                 cards=(20, 12, 3), card_steps=(6, 16), card_across=2),
    "lod2": dict(limb=10, torso=16, head=(18, 12), hair=(24, 6), finger=4, fingers=False, stride=2,
                 tex=256, budget=10000, samples=64, head_ratio=0.45,
                 cards=(10, 4, 0), card_steps=(3, 7), card_across=1),
    "lod3": dict(limb=6, torso=10, head=(10, 8), hair=(14, 4), finger=3, fingers=False, stride=3,
                 tex=256, budget=3000, samples=48, lite=True, head_ratio=0.12,
                 cards=(6, 0, 0), card_steps=(2, 4), card_across=1),
}


def linear(h):
    """sRGB hex to linear RGB, which Blender's color inputs take."""
    h = h.lstrip("#")
    c = [int(h[i:i + 2], 16) / 255 for i in (0, 2, 4)]
    return tuple(x / 12.92 if x <= 0.04045 else ((x + 0.055) / 1.055) ** 2.4 for x in c)


# The palette: forest green, warm brown leather, and cream linen, with copper
# accents; dark auburn hair. No gold anywhere.
PALETTE = {
    "skin": "#E9AF92",
    "hair": "#5E2418",
    "hair_base": "#4E1D10",
    "hair_inner": "#3C150C",
    "hair_dark": "#2A0D07",
    "hair_light": "#A8502A",
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
        # Per-face UVs, for meshes whose faces carry their own (the cards).
        self.uv = []
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


# The base head's left eye center, and how much larger Alice's eyes are.
EYE_CENTER = Vector((0.0315, -0.06, 1.656))
EYE_GROW = 0.04
# How far each eye moves toward the nose, m.
EYE_IN = 0.0022
# An iris's diameter as a share of the eye opening's width.
IRIS_SHARE = 0.205
# Levels a channel in the baked atlas (seven bits).
LEVELS = 127
# How much more of the atlas, by length, the face takes than the rest of
# her.
FACE_TEXELS = 6.5
# The atlas's bottom band, a share of its height, holds the hair strips
# every card samples; STRIPS_PREVIEW is their size in the cards' material.
HAIR_BAND = 0.16
STRIPS_PREVIEW = (1024, 192)
# Her arms are this much slimmer than the lofts' first measurements.
ARM_SLIM = 0.88


def reshape(p):
    """Turns the Universal female head into Alice's: a little larger, for
    the stylized proportions of our world; a softer, narrower jaw and a
    rounder chin; a shorter, slightly upturned nose; fuller lips and cheeks;
    and a softer brow ridge. Masks read the original position, so the
    changes don't compound."""
    q = p.copy()
    # A slimmer neck.
    neck = smoothstep(1.565, 1.53, p.z)
    q.x *= 1 - 0.10 * neck
    q.y = 0.02 + (q.y - 0.02) * (1 - 0.06 * neck)
    k = smoothstep(1.50, 1.57, p.z)
    center = Vector((0.0, -0.005, 1.60))
    q = center + (q - center) * (1 + 0.05 * k)
    front = smoothstep(0.02, -0.045, p.y)
    lower = smoothstep(1.62, 1.545, p.z)
    # An oval face: the jaw narrows a little, toward a small, round chin.
    q.x *= 1 - 0.065 * lower * front
    chin = gauss(p.x, 0.03) * gauss(p.z - 1.55, 0.02) * front
    q.y += 0.004 * chin
    q.z += 0.002 * chin
    # A shorter nose that sits back in profile, with a soft, rounded tip.
    nose = gauss(p.x, 0.028) * gauss(p.z - 1.614, 0.017) * smoothstep(-0.088, -0.108, p.y)
    q.y += 0.0085 * nose
    q.z += 0.003 * nose
    tip = gauss(p.x, 0.012) * gauss(p.z - 1.612, 0.008) * smoothstep(-0.095, -0.11, p.y)
    q.x += 0.06 * p.x * tip
    lips = gauss(p.x, 0.021) * gauss(p.z - 1.591, 0.009) * smoothstep(-0.08, -0.092, p.y)
    q.y -= 0.0018 * lips
    # The mouth's corners turn up into a slight smile.
    corner = gauss(abs(p.x) - 0.021, 0.006) * gauss(p.z - 1.590, 0.006) * front
    q.z += 0.0018 * corner
    # Fuller, higher cheeks.
    cheek = gauss(abs(p.x) - 0.046, 0.017) * gauss(p.z - 1.622, 0.022) * smoothstep(-0.02, -0.06, p.y)
    q.x += math.copysign(0.006 * cheek, p.x)
    q.y -= 0.005 * cheek
    # A soft brow ridge that shades the eyes, as a game face needs.
    brow = gauss(p.z - 1.676, 0.008) * gauss(abs(p.x) - 0.03, 0.025) * smoothstep(-0.075, -0.09, p.y)
    q.y -= 0.0012 * brow
    # Eyes a little larger, and a little closer together: the base's are
    # set wider than a painted reference's, whose eye centers are about
    # 0.52 of the face's width apart.
    for s in (1, -1):
        c = EYE_CENTER.copy()
        c.x *= s
        q.x -= s * EYE_IN * gauss(math.hypot(p.x - c.x, p.z - c.z), 0.024) * smoothstep(0.0, -0.05, p.y)
        r = math.hypot(p.x - c.x, (p.z - c.z) * 1.3)
        w = smoothstep(0.03, 0.012, r) * smoothstep(0.0, -0.05, p.y)
        if w > 0:
            q.x += (p.x - c.x) * EYE_GROW * w
            q.z += (p.z - c.z) * EYE_GROW * w
    # Nostrils closed to a soft underside: open nostrils render as dark
    # holes at a game's distance.
    nostril = smoothstep(0.018, 0.012, abs(p.x)) * smoothstep(1.596, 1.600, p.z) * smoothstep(1.614, 1.609, p.z)
    nostril *= smoothstep(-0.084, -0.090, p.y)
    if nostril > 0:
        q.z = lerp(q.z, max(q.z, 1.6045), nostril)
    # Ears laid closer to the head, so the bob falls over them.
    ear = smoothstep(0.062, 0.074, abs(p.x)) * gauss(p.z - 1.64, 0.06)
    if ear > 0:
        out = abs(q.x) - 0.066
        q.x = math.copysign(0.066 + out * (1 - 0.92 * ear), q.x)
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
        moved = reshape(c)
        # Turned up a little, so her gaze meets yours rather than resting
        # under heavy lids.
        up = Matrix.Rotation(math.radians(-6), 3, "X")
        for v in vs:
            v.co = moved + up @ ((v.co - c) * (1 + EYE_GROW + 0.035))
    # The painted brows replace the base's brow mesh.
    bpy.data.objects.remove(brows, do_unlink=True)
    for o in (body, eyes):
        uv = o.data.uv_layers
        while len(uv) > 1:
            uv.remove(uv[-1])
        uv[0].name = "UVMap"
    marks = face_marks(body, eyes)
    ratio = lod.get("head_ratio", 1.0)
    if ratio < 1.0:
        for o, r in ((body, ratio), (eyes, max(0.12, ratio))):
            mod = o.modifiers.new("Decimate", "DECIMATE")
            mod.ratio = r
            bpy.context.view_layer.objects.active = o
            bpy.ops.object.modifier_apply(modifier=mod.name)
    # Materials: her hand-painted face and eyes, projected from the front.
    size = 2048 if lod["tex"] >= 1024 else 1024
    face_material(body, alice_paint.face(marks, size))
    eye_material(eyes, alice_paint.eyes(marks, size))
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
    # A woman's figure: hips wider than the waist and as wide as the
    # shoulders, a waist the sash cinches, a bust, and narrower shoulders.
    (0.80, 0.190, 0.130, 0.012, 2.2, True),
    (0.86, 0.188, 0.126, 0.014, 2.2, False),
    (0.93, 0.182, 0.120, 0.016, 2.3, True),
    (1.00, 0.150, 0.104, 0.010, 2.3, False),
    (1.06, 0.120, 0.090, 0.006, 2.3, True),
    (1.12, 0.124, 0.092, 0.000, 2.3, False),
    (1.19, 0.136, 0.100, -0.004, 2.4, True),
    (1.225, 0.141, 0.104, -0.004, 2.4, False),
    (1.26, 0.146, 0.108, -0.004, 2.4, False),
    (1.29, 0.149, 0.107, -0.002, 2.45, False),
    (1.32, 0.152, 0.106, 0.000, 2.5, True),
    (1.38, 0.156, 0.098, 0.010, 2.6, False),
    (1.43, 0.146, 0.086, 0.015, 2.6, True),
    (1.47, 0.104, 0.070, 0.020, 2.4, False),
    (1.50, 0.046, 0.044, 0.022, 2.0, True),
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
    """The torso's bust, a round, natural dome on each side rather than a
    point, and the shoulder blades."""
    out = 0.0
    if d.y < 0:
        for sx in (-1, 1):
            r2 = ((p.x - sx * 0.068) / 0.062) ** 2 + ((p.z - 1.244) / 0.066) ** 2
            # A super-Gaussian: a full, rounded front that falls off softly.
            out += 0.056 * exp(-(r2 ** 1.6)) * (-d.y) ** 0.6
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
        # The hair cards: alpha-tested, never subdivided.
        self.C = Mesh()
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
            th = 0.5 * smoothstep(1.0, 0.78, p.z) * smoothstep(0.0, 0.16, abs(p.x))
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

    def body_radius(self, z, d):
        """How far her clothed body reaches from its axis at height z in the
        horizontal direction d (a unit vector), and that axis: the coat
        below the shoulders, the collar and neck above."""
        if z <= 1.468:
            a, b, yc, n = self.coat_size(z)
            c = Vector((0, yc, 0))
            phi = math.atan2(d.y, d.x)
            th = phi
            for _ in range(4):
                p = section_point(z, th, sizes=(a, b, yc, n))
                got = math.atan2(p.y - yc, p.x)
                th += math.atan2(math.sin(phi - got), math.cos(phi - got))
            p = section_point(z, th, sizes=(a, b, yc, n))
            return math.hypot(p.x, p.y - yc), c
        # The collar's roll, then the slim neck above it.
        r = lerp(0.104, 0.046, smoothstep(1.47, 1.56, z))
        return r, Vector((0, lerp(0.028, 0.012, smoothstep(1.47, 1.56, z)), 0))

    def drape(self, p, gap):
        """`p` pushed out, horizontally, to `gap` beyond her clothed body."""
        _, _, yc, _ = torso_at(min(p.z, 1.5))
        c = Vector((0, yc, 0))
        h = Vector((p.x - c.x, p.y - c.y, 0))
        if h.length < 1e-6:
            return p
        d = h.normalized()
        need, axis = self.body_radius(p.z, d)
        h = Vector((p.x - axis.x, p.y - axis.y, 0))
        r = h.length
        want = need + gap
        # A soft maximum, so the hair rolls over the shoulder smoothly.
        k = 0.012
        soft = 0.5 * (r + want + math.sqrt((r - want) ** 2 + k * k))
        if r < 1e-6:
            return p
        return Vector((axis.x, axis.y, p.z)) + Vector((h.x, h.y, 0)) * (soft / r)

    @staticmethod
    def w_hair(z):
        """Hair rides the head above the jaw, then the neck, then, where it
        lies on the shoulders and back, the upper spine."""
        a = smoothstep(1.60, 1.53, z)
        b = smoothstep(1.53, 1.44, z)
        w = {"Head": (1 - a)}
        if a * (1 - b) > 0:
            w["neck_01"] = a * (1 - b)
        if a * b > 0:
            w["spine_03"] = a * b
        return {k: v for k, v in w.items() if v > 1e-4}

    def hairline(self, aa):
        """The hairline's angle from the crown at azimuth `aa` from the
        front: a rounded line high on the forehead, lower at the temples,
        and at the nape behind."""
        return 0.66 + 0.34 * min(1.0, aa / 1.2) ** 2 + 0.75 * smoothstep(1.05, 1.6, aa) + 0.2 * smoothstep(1.8, 2.6, aa)

    def hair(self):
        """The hair's opaque base: a scalp cap from the hairline over the
        skull, darker at the roots, with the center part, and behind it a
        sheet from the nape down her back. The cards (`cards`) lie over it,
        so it shows only in the shadow between them."""
        M = self.M
        M.part = "hair"
        cols, nrows = self.L["hair"]
        out_off = 0.009
        columns = []
        for j in range(cols):
            th = -pi / 2 + 2 * pi * j / cols
            al = math.atan2(math.sin(th + pi / 2), math.cos(th + pi / 2))
            aa = abs(al)
            ph_end = self.hairline(aa)
            k = smoothstep(1.35, 2.0, aa)
            # Down her back to the shoulder blades, hidden under the cards.
            z_end = lerp(1.50, 1.34, smoothstep(1.6, 2.6, aa))
            columns.append((th, al, ph_end, k, z_end))
        P = []
        for th, al, ph_end, k, z_end in columns:
            p_eq = self.head_point(th, ph_end)
            n_eq = self.head_normal(th, ph_end)
            skull = (ph_end - 0.06) * 0.10
            drop = max(0.0, (p_eq.z - z_end)) * k
            col = []
            for i in range(nrows + 1):
                u = i / nrows
                d = (skull + drop) * u
                if d <= skull + 1e-9 or drop <= 0:
                    ph = 0.06 + (ph_end - 0.06) * min(1.0, d / skull)
                    # Close to the skin at the hairline in front, so no step
                    # shows between the hair and the forehead.
                    edge = 1 - 0.8 * smoothstep(ph_end - 0.3, ph_end, ph) * (1 - k)
                    p = self.head_point(th, ph) + self.head_normal(th, ph) * out_off * edge
                else:
                    f = (d - skull) / drop
                    p = p_eq + n_eq * out_off + Vector((0, 0, -(p_eq.z - z_end) * f))
                    p = self.drape(p, 0.006)
                col.append(p)
            P.append(col)
        rows = [[P[j][nrows - i] for j in range(cols)] for i in range(nrows + 1)]
        W = [[self.w_hair(p.z) for p in r] for r in rows]
        out, inn = shell(M, rows, W, 0.002, "hair_base", "hair_inner", "hair_inner", wrap=True,
                         rims=(True, False, False, False))
        fan(M, out[-1], self.head_point(0, 0) + Vector((0, 0, out_off + 0.001)), rigid("Head"), "hair_base",
            Vector((0, 0, 1)))
        self.cards()

    def skull_dir(self, d, off):
        """The point `off` out from the skull in direction `d` from the
        head's center, and the skull's normal there."""
        d = d.normalized()
        ph = math.acos(max(-1.0, min(1.0, d.z)))
        th = math.atan2(d.y, d.x)
        p, n = self.skull(th, ph)
        return p + n * off, n

    def clear_head(self, p, gap):
        """`p` moved out of the head to at least `gap` from its surface."""
        if self.bvh is None or p.z < 1.47:
            return p
        hit = self.bvh.find_nearest(p)
        if hit[0] is None:
            return p
        q, n = hit[0], hit[1].normalized()
        if (p - q).dot(n) < gap:
            return q + n * gap
        return p

    def meet_hairline(self, q):
        """`q` brought down onto the skin where it nears the hairline in
        front, so the hair meets the forehead rather than floating over it:
        from below, a gap there shows the cards' shaded undersides as a dark
        band."""
        d = q - self.HC
        if d.y > 0.02 or d.length < 1e-6:
            return q
        ph = math.acos(max(-1.0, min(1.0, d.normalized().z)))
        al = abs(math.atan2(d.y, d.x) + pi / 2)
        al = min(al, 2 * pi - al)
        if al > 1.7:
            return q
        line = self.hairline(al)
        w = smoothstep(line - 0.30, line - 0.04, ph)
        if w <= 0:
            return q
        p, n = self.skull_dir(d, 0.0)
        off = (q - p).dot(n)
        return p + n * lerp(off, 0.0025, w) + (q - p - n * off)

    def card_specs(self):
        """Each card's side, exit azimuth, layer, and length, deterministic.
        Front locks leave the part over the temples and fall in front of the
        shoulders; the rest sweep from the center part and the crown down
        the sides and back."""
        import random

        rng = random.Random(7)
        top, under, wisps = self.L["cards"]
        out = []
        for s in (1, -1):
            for k in range(top):
                t = (k + 0.5) / top
                a_e = lerp(1.02, pi - 0.04, t ** 0.92) + rng.uniform(-0.04, 0.04)
                out.append(dict(s=s, a_e=a_e, layer=1, width=rng.uniform(0.030, 0.042), seed=rng.random()))
            for k in range(under):
                t = (k + 0.5) / under
                a_e = lerp(1.10, pi - 0.08, t) + rng.uniform(-0.05, 0.05)
                out.append(dict(s=s, a_e=a_e, layer=0, width=rng.uniform(0.040, 0.052), seed=rng.random()))
            out.append(dict(s=s, a_e=pi - 0.01, layer=1, width=0.05, seed=rng.random()))
            for k in range(wisps):
                a_e = lerp(1.05, 2.4, (k + 0.5) / max(1, wisps)) + rng.uniform(-0.1, 0.1)
                out.append(dict(s=s, a_e=a_e, layer=2, width=rng.uniform(0.008, 0.012), seed=rng.random()))
        return out

    def card_path(self, spec, n_skull, n_fall):
        """A card's center line from its root on the part (or the crown)
        over the skull to where it leaves the head, then down in loosening
        S-waves to its tip, with the outward normal at each point."""
        import random

        rng = random.Random(int(spec["seed"] * 1e6))
        s, a_e, layer = spec["s"], spec["a_e"], spec["layer"]
        off = {0: 0.012, 1: 0.018, 2: 0.022}[layer]
        # The root: on the center part, a little to her side of it, from
        # the front hairline back to the crown.
        back = smoothstep(1.05, 2.7, a_e)
        root_ph = lerp(0.60, 0.10, back)
        root_al = lerp(0.035, 0.6, smoothstep(2.3, pi, a_e))
        exit_ph = lerp(1.30, 1.95, smoothstep(1.1, 2.3, a_e))

        def direction(al, ph):
            th = -pi / 2 + s * al
            return Vector((sin(ph) * cos(th), sin(ph) * sin(th), cos(ph)))

        d0 = direction(root_al, root_ph)
        d1 = direction(a_e, exit_ph)
        pts, nrm = [], []
        for i in range(n_skull + 1):
            t = i / n_skull
            d = d0.slerp(d1, t)
            if a_e < 1.6:
                # The front cards sweep from the part along the hairline to
                # the temples, so no scalp shows between the hair and the
                # forehead.
                ph = math.acos(max(-1.0, min(1.0, d.z)))
                al = abs(math.atan2(d.y, d.x) + pi / 2)
                al = min(al, 2 * pi - al)
                edge = self.hairline(al) - 0.10 - 0.12 * smoothstep(1.02, 1.6, a_e)
                d = direction(al, lerp(max(ph, edge), ph, smoothstep(0.75, 1.0, t)))
            # Volume: the hair lifts off the crown and settles at the sides.
            lift = 0.014 * sin(pi * min(1.0, t * 1.2)) * (1.0 - 0.4 * back) + 0.006 * t
            settle = 1 - 0.55 * smoothstep(0.55, 1.0, t) * (1 - back)
            p, n = self.skull_dir(d, (off + lift) * settle)
            pts.append(p)
            nrm.append(n)
        E = pts[-1]
        r_e = math.hypot(E.x, E.y - self.HC.y)
        # Where it ends: front locks over the chest, the sides past the
        # shoulder blades, the back longest at the middle, as a soft V.
        lock = a_e < 1.42
        if lock:
            z_end = 1.19 + rng.uniform(-0.02, 0.025)
        else:
            z_end = lerp(1.30, 1.245, smoothstep(1.7, pi, a_e)) + rng.uniform(-0.035, 0.025)
        if layer == 0:
            z_end += 0.03
        # Below the jaw the fall turns in front of the shoulder (the locks)
        # or behind it.
        if lock:
            a_t = lerp(0.62, 0.95, (a_e - 1.02) / 0.4)
        elif a_e < 1.75:
            a_t = 1.95
        else:
            a_t = max(a_e, 2.05)
        phase = 2 * pi * (0.35 * a_e * s + 0.15 * rng.random())
        length = E.z - z_end
        for i in range(1, n_fall + 1):
            f = i / n_fall
            z = E.z - length * f
            turn = smoothstep(1.56, 1.40, z)
            al = lerp(a_e, a_t, turn)
            th = -pi / 2 + s * al
            d = Vector((cos(th), sin(th), 0))
            r = r_e * (1 + 0.10 * smoothstep(0.0, 0.4, f))
            p = Vector((0, self.HC.y, z)) + d * r
            # S-waves that loosen and grow toward the ends.
            fall = E.z - z
            period = 0.075 + 0.05 * f
            amp = 0.003 + 0.013 * smoothstep(0.1, 0.9, f)
            tangent = Vector((-d.y, d.x, 0))
            w = sin(2 * pi * fall / period + phase)
            p = p + tangent * (amp * w) + d * (0.004 * cos(2 * pi * fall / period + phase))
            p = self.drape(p, 0.010 + 0.006 * layer + 0.004 * f)
            p = self.clear_head(p, off + 0.004)
            pts.append(p)
            nrm.append(Vector((p.x, p.y - self.HC.y, 0)).normalized())
        # The tips curl in a little.
        return pts, nrm, n_skull

    def cards(self):
        """Alpha-tested hair cards: clumps that leave the center part,
        lift at the crown, frame the face, and fall in loosening S-waves to
        tapered, strand-cut tips. Each card's UV runs across it (u) and from
        root to tip (v), so the strand texture follows the hair."""
        C = self.C
        C.part = "hair"
        n_skull, n_fall = self.L["card_steps"]
        across = self.L.get("card_across", 2)
        lanes = alice_paint.HAIR_LANES
        for index, spec in enumerate(self.card_specs()):
            pts, nrm, ns = self.card_path(spec, n_skull, n_fall)
            # The under layer's lanes are darker; the wisps have their own.
            lane = {0: index % 2, 1: 2 + index % 3, 2: lanes - 1}[spec["layer"]]
            n = len(pts)
            rows, uvs = [], []
            for i, (p, nm) in enumerate(zip(pts, nrm)):
                nxt = pts[min(i + 1, n - 1)]
                prv = pts[max(i - 1, 0)]
                t = (nxt - prv).normalized()
                side = t.cross(nm).normalized()
                v = i / (n - 1)
                # Narrow at the root, full through the fall, tapering to the
                # tip.
                w = spec["width"] * (0.75 + 0.25 * smoothstep(0.0, 0.25, v)) * (1.0 - 0.62 * smoothstep(0.62, 1.0, v))
                if spec["layer"] == 2:
                    w = spec["width"] * (1 - 0.7 * v)
                row = []
                for k in range(across + 1):
                    u = k / across
                    x = (u - 0.5) * w
                    # A rounded clump: the middle stands a little proud.
                    bulge = 0.18 * w * (1 - (2 * u - 1) ** 2)
                    q = p + side * x + nm * bulge
                    if i <= ns:
                        q = self.meet_hairline(q)
                    row.append(q)
                rows.append(row)
                # Strip space: along the strip root to tip, across its lane.
                uvs.append([(0.002 + 0.996 * v, (lane + 0.04 + 0.92 * k / across) / lanes)
                            for k in range(across + 1)])
            idx = [[C.vert(q, self.w_hair(q.z)) for q in row] for row in rows]
            for i in range(n - 1):
                for k in range(across):
                    q = (idx[i][k], idx[i][k + 1], idx[i + 1][k + 1], idx[i + 1][k])
                    uv = (uvs[i][k], uvs[i][k + 1], uvs[i + 1][k + 1], uvs[i + 1][k])
                    a, b, c = C.v[q[0]], C.v[q[1]], C.v[q[3]]
                    # Faces point out from the head.
                    if (b - a).cross(c - a).dot(nrm[i]) < 0:
                        q, uv = q[::-1], uv[::-1]
                    C.face(q, "hair_card")
                    C.uv.append(uv)

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
            return rows[0][1] * ARM_SLIM, rows[0][2] * ARM_SLIM
        for a, b in zip(rows, rows[1:]):
            if a[0] <= x <= b[0]:
                t = (x - a[0]) / (b[0] - a[0])
                return lerp(a[1], b[1], t) * ARM_SLIM, lerp(a[2], b[2], t) * ARM_SLIM
        return rows[-1][1] * ARM_SLIM, rows[-1][2] * ARM_SLIM

    def arm_left(self):
        """The left arm's tunic sleeve, bracer, and hand, in a mesh of its own."""
        M = Mesh()
        s = 1
        chain = self.R.arm(s)
        segs = self.L["limb"]
        M.part = "tunic"
        rings = []
        for x, ry, rz, key in self.stations(self.SLEEVE):
            rings.append(dict(c=self.arm_center(s, x), axis=(1, 0, 0), ref=(0, 1, 0), rx=ry * ARM_SLIM, ry=rz * ARM_SLIM))
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
        (0.452, 0.054, 0.060, True), (0.447, 0.064, 0.070, False), (0.43, 0.066, 0.072, True),
        (0.405, 0.063, 0.069, False), (0.398, 0.056, 0.063, True), (0.34, 0.055, 0.062, False),
        (0.27, 0.049, 0.054, True), (0.20, 0.042, 0.047, False), (0.14, 0.038, 0.043, True),
        (0.10, 0.037, 0.042, False), (0.075, 0.036, 0.040, True),
    ]

    FOOT = [
        # A boot of a woman's size: 27 cm heel to toe, narrow, with a toe
        # that rises rather than a flat paddle.
        (0.102, 0.022, 0.060, True), (0.096, 0.032, 0.092, False), (0.080, 0.036, 0.116, True),
        (0.050, 0.038, 0.116, False), (0.015, 0.039, 0.106, True), (-0.020, 0.040, 0.090, False),
        (-0.055, 0.041, 0.074, True), (-0.090, 0.040, 0.064, False), (-0.122, 0.037, 0.058, True),
        (-0.150, 0.032, 0.052, False), (-0.170, 0.023, 0.044, True),
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

    # The coat follows her figure: fitted at the bust and the waist, flaring
    # over the hips to the knee. Columns: height, half width, half depth,
    # the front opening's half angle (degrees), and whether the station is
    # a key one.
    COAT = [
        (0.47, 0.236, 0.186, 55, True), (0.52, 0.231, 0.181, 53, False), (0.60, 0.224, 0.172, 49, True),
        (0.70, 0.216, 0.162, 44, False), (0.80, 0.210, 0.154, 38, True), (0.88, 0.206, 0.148, 33, False),
        (0.94, 0.200, 0.140, 30, True), (1.00, 0.168, 0.120, 27, False), (1.06, 0.136, 0.104, 26, True),
        (1.12, 0.140, 0.106, 25, False), (1.19, 0.152, 0.114, 23, True), (1.225, 0.157, 0.118, 22, False),
        (1.26, 0.162, 0.122, 21, False), (1.29, 0.165, 0.121, 20, False), (1.32, 0.168, 0.120, 19, True), (1.38, 0.172, 0.113, 17, False), (1.43, 0.162, 0.101, 15, True),
        (1.468, 0.118, 0.084, 14, True),
    ]

    @staticmethod
    def vent(z):
        """Half the side vent's opening at height z, radians: closed above the
        hip, opening toward the hem, so the arms hang clear of the skirt."""
        return math.radians(11.0) * smoothstep(0.98, 0.72, z)

    def coat(self):
        M = self.M
        M.part = "coat"
        rows = self.stations(self.COAT)
        cols = max(3, self.L["torso"] // 4 + 1)
        for s in (1, -1):
            for piece in ("front", "back"):
                P = []
                for z, a, b, opening, key in rows:
                    _, _, yc, n = torso_at(z)
                    op = math.radians(opening)
                    lo, hi = (op, pi / 2 - self.vent(z)) if piece == "front" else (pi / 2 + self.vent(z), pi)
                    row = []
                    for j in range(cols):
                        al = lo + (hi - lo) * j / (cols - 1)
                        th = -pi / 2 + s * al
                        row.append(section_point(z, th, sizes=(a, b, yc + 0.006, 2.3 if z < 1.0 else n)))
                    # Columns counterclockwise about +z, so faces point out.
                    P.append(row if s > 0 else row[::-1])
                W = [[self.w_coat(p) for p in row] for row in P]
                edge = 0 if s > 0 else cols - 2

                def matfn(i, j, edge=edge, piece=piece):
                    if i == 0 or (piece == "front" and j == edge):
                        return "coat_trim"
                    return "coat"

                # Rims on the hem, the shoulder, and the open edges: the
                # front opening and the side vents.
                first, last = (piece == "front", True) if s > 0 else (True, piece == "front")
                shell(M, P, W, 0.007, "coat", "coat_inner", "coat_trim", matfn=matfn,
                      rims=(True, True, first, last))
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
        """The coat's collar: a soft roll about her neck. (Her long hair now
        lies where a hood worn down would.)"""
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

    def sash(self):
        M = self.M
        M.part = "gear"
        segs = self.L["torso"]
        rings = []
        # A soft band with two folds across it.
        for z, g in ((0.985, 0.010), (0.998, 0.015), (1.012, 0.017), (1.024, 0.013), (1.038, 0.017),
                     (1.052, 0.015), (1.068, 0.010)):
            a, b, yc, _ = self.coat_size(z)
            n = 2.3
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

    def build_outfit(self):
        """Alice's own clothes and gear."""
        self.tunic()
        self.coat()
        self.hood()  # the collar
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


def node_math(nt, op, a, b=None, c=None):
    """A Math node computing `op` of sockets or numbers; returns its output."""
    n = nt.nodes.new("ShaderNodeMath")
    n.operation = op
    for i, x in enumerate((a, b, c)):
        if x is None:
            continue
        if isinstance(x, (int, float)):
            n.inputs[i].default_value = x
        else:
            nt.links.new(x, n.inputs[i])
    return n.outputs[0]


def node_smooth(nt, value, lo, hi):
    """smoothstep(lo, hi, value) as a Map Range node."""
    n = nt.nodes.new("ShaderNodeMapRange")
    n.interpolation_type = "SMOOTHSTEP"
    nt.links.new(value, n.inputs["Value"])
    n.inputs["From Min"].default_value = lo
    n.inputs["From Max"].default_value = hi
    return n.outputs["Result"]


def card_material():
    """The hair cards' material: the painted strips
    (`alice_paint.hair_strips`), read through each card's `hairuv`, which
    places the card on its lane root to tip. The strips' alpha cuts the
    tips into strands and frays the edges; the bake copies the strips into
    the atlas's band (`HAIR_BAND`)."""
    import numpy as np

    strips = alice_paint.hair_strips(*STRIPS_PREVIEW)
    img = bpy.data.images.new("hair_strips", STRIPS_PREVIEW[0], STRIPS_PREVIEW[1], alpha=True, float_buffer=True)
    img.colorspace_settings.name = "Linear Rec.709"
    img.pixels.foreach_set(np.ascontiguousarray(strips, dtype=np.float32).ravel())
    m = bpy.data.materials.new("hair_card")
    m.use_nodes = True
    nt = m.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    bsdf.inputs["Roughness"].default_value = 0.6
    uvn = nt.nodes.new("ShaderNodeUVMap")
    uvn.uv_map = "hairuv"
    tex = nt.nodes.new("ShaderNodeTexImage")
    tex.image = img
    tex.extension = "EXTEND"
    nt.links.new(uvn.outputs["UV"], tex.inputs["Vector"])
    nt.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
    nt.links.new(tex.outputs["Alpha"], bsdf.inputs["Alpha"])
    m["alpha"] = True
    return m


def node_value(nt, kind, vector, scale, **settings):
    """A texture node's factor output over `vector` at `scale`."""
    n = nt.nodes.new(kind)
    n.inputs["Scale"].default_value = scale
    for key, value in settings.items():
        if key in n.inputs:
            n.inputs[key].default_value = value
        else:
            setattr(n, key, value)
    nt.links.new(vector, n.inputs["Vector"])
    return n.outputs["Fac"]


def node_scale(nt, color, factor):
    """`color` times the scalar `factor`."""
    n = nt.nodes.new("ShaderNodeMix")
    n.data_type = "RGBA"
    n.blend_type = "MULTIPLY"
    n.inputs["Factor"].default_value = 1.0
    nt.links.new(color, n.inputs["A"])
    comb = nt.nodes.new("ShaderNodeCombineColor")
    for i in range(3):
        nt.links.new(factor, comb.inputs[i])
    nt.links.new(comb.outputs[0], n.inputs["B"])
    return n.outputs["Result"]


def node_toward(nt, color, factor, target):
    """`color` blended toward the linear color `target` by `factor`."""
    n = nt.nodes.new("ShaderNodeMix")
    n.data_type = "RGBA"
    nt.links.new(factor, n.inputs["Factor"])
    nt.links.new(color, n.inputs["A"])
    n.inputs["B"].default_value = (*target, 1)
    return n.outputs["Result"]


def surface_detail(nt, name, color):
    """Painted surface detail a base-color-only renderer can show, after a
    high-fidelity reference whose texture carries its seams, stitching,
    creases, and wear: a weave in the linen, fuzz and long folds in the
    wool coat with princess seams and their stitching, grain, creases, and
    worn edges on the leather, fold streaks in the sash, polished edges on
    the copper, and warmth at the knuckles and fingertips."""
    geo = nt.nodes.new("ShaderNodeNewGeometry")
    pos = geo.outputs["Position"]
    sep = nt.nodes.new("ShaderNodeSeparateXYZ")
    nt.links.new(pos, sep.inputs[0])
    x, z = sep.outputs["X"], sep.outputs["Z"]
    edges = node_smooth(nt, geo.outputs["Pointiness"], 0.52, 0.62)
    base = name.split(".")[0]
    if base in ("linen", "linen_shade"):
        # The weave: crossed threads about 3 mm apart, and soft wrinkles.
        a = node_value(nt, "ShaderNodeTexWave", pos, 210.0, wave_type="BANDS", bands_direction="X")
        b = node_value(nt, "ShaderNodeTexWave", pos, 210.0, wave_type="BANDS", bands_direction="Z")
        weave = node_math(nt, "MULTIPLY", node_math(nt, "ADD", a, b), 0.5)
        color = node_scale(nt, color, node_math(nt, "MULTIPLY_ADD", weave, 0.08, 0.96))
        wr = node_value(nt, "ShaderNodeTexWave", pos, 22.0, wave_type="BANDS", bands_direction="Z",
                        Distortion=5.0, Detail=3.0)
        color = node_scale(nt, color, node_math(nt, "MULTIPLY_ADD", wr, 0.10, 0.95))
    elif base in ("coat", "coat_inner", "coat_trim"):
        fuzz = node_value(nt, "ShaderNodeTexNoise", pos, 320.0, Detail=2.0)
        color = node_scale(nt, color, node_math(nt, "MULTIPLY_ADD", fuzz, 0.10, 0.95))
        folds = node_value(nt, "ShaderNodeTexWave", pos, 16.0, wave_type="BANDS", bands_direction="X",
                           Distortion=3.0, Detail=2.0)
        color = node_scale(nt, color, node_math(nt, "MULTIPLY_ADD", folds, 0.16, 0.91))
        if base == "coat":
            # Princess seams, front and back, with a row of stitches beside
            # each.
            d = node_math(nt, "ABSOLUTE", node_math(nt, "SUBTRACT", node_math(nt, "ABSOLUTE", x), 0.085))
            seam = node_math(nt, "SUBTRACT", 1.0, node_smooth(nt, d, 0.0007, 0.0018))
            color = node_scale(nt, color, node_math(nt, "MULTIPLY_ADD", seam, -0.40, 1.0))
            row = node_math(nt, "SUBTRACT", 1.0, node_smooth(nt, node_math(nt, "ABSOLUTE",
                            node_math(nt, "SUBTRACT", d, 0.0042)), 0.0004, 0.0009))
            dash = node_smooth(nt, node_math(nt, "SINE", node_math(nt, "MULTIPLY", z, 2 * pi / 0.007)), 0.1, 0.4)
            color = node_toward(nt, color, node_math(nt, "MULTIPLY", node_math(nt, "MULTIPLY", row, dash), 0.45),
                                linear("#8FA57E"))
        if base == "coat_trim":
            color = node_toward(nt, color, node_math(nt, "MULTIPLY", edges, 0.35), linear("#5E7D57"))
    elif base in ("leather", "leather_dark", "sole"):
        grain = node_value(nt, "ShaderNodeTexNoise", pos, 420.0, Detail=2.0)
        color = node_scale(nt, color, node_math(nt, "MULTIPLY_ADD", grain, 0.12, 0.94))
        crease = node_value(nt, "ShaderNodeTexWave", pos, 55.0, wave_type="BANDS", bands_direction="Z",
                            Distortion=7.0, Detail=3.0)
        lines = node_math(nt, "SUBTRACT", 1.0, node_smooth(nt, crease, 0.0, 0.10))
        color = node_scale(nt, color, node_math(nt, "MULTIPLY_ADD", lines, -0.28, 1.0))
        color = node_toward(nt, color, node_math(nt, "MULTIPLY", edges, 0.45), linear("#C08A5C"))
    elif base == "leggings":
        knit = node_value(nt, "ShaderNodeTexWave", pos, 260.0, wave_type="BANDS", bands_direction="X")
        color = node_scale(nt, color, node_math(nt, "MULTIPLY_ADD", knit, 0.10, 0.95))
    elif base == "sash":
        a = node_value(nt, "ShaderNodeTexWave", pos, 230.0, wave_type="BANDS", bands_direction="X")
        color = node_scale(nt, color, node_math(nt, "MULTIPLY_ADD", a, 0.08, 0.96))
        fold = node_value(nt, "ShaderNodeTexWave", pos, 38.0, wave_type="BANDS", bands_direction="Z",
                          Distortion=4.0, Detail=2.0)
        color = node_scale(nt, color, node_math(nt, "MULTIPLY_ADD", fold, 0.24, 0.86))
    elif base == "copper":
        spots = node_value(nt, "ShaderNodeTexNoise", pos, 60.0, Detail=3.0)
        color = node_scale(nt, color, node_math(nt, "MULTIPLY_ADD", spots, 0.30, 0.82))
        color = node_toward(nt, color, node_math(nt, "MULTIPLY", edges, 0.6), linear("#F2B47A"))
    elif base == "skin":
        color = node_toward(nt, color, node_math(nt, "MULTIPLY", edges, 0.5), linear("#E08A78"))
    return color


def material(name):
    m = bpy.data.materials.get(name)
    if m:
        return m
    if name == "hair_card":
        return card_material()
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
        "hair": ("strands", 0.38), "hair_base": ("strands", 0.30), "hair_inner": ("strands", 0.20), "wood": ("grain", 0.25),
    }.get(name)
    if not variation:
        rgb = nt.nodes.new("ShaderNodeRGB")
        rgb.outputs[0].default_value = (*col, 1)
        nt.links.new(surface_detail(nt, name, rgb.outputs[0]), bsdf.inputs["Base Color"])
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
        mul.inputs[1].default_value = 64.0
        nt.links.new(noise.outputs["Fac"], mul.inputs[2])
        sn = nt.nodes.new("ShaderNodeMath")
        sn.operation = "SINE"
        nt.links.new(mul.outputs[0], sn.inputs[0])
        half = nt.nodes.new("ShaderNodeMath")
        half.operation = "MULTIPLY_ADD"
        nt.links.new(sn.outputs[0], half.inputs[0])
        half.inputs[1].default_value = 0.3
        half.inputs[2].default_value = 0.35
        # Darker at the roots, lighter toward the ends.
        length = nt.nodes.new("ShaderNodeMapRange")
        nt.links.new(sep.outputs["Z"], length.inputs["Value"])
        length.inputs["From Min"].default_value = 1.76
        length.inputs["From Max"].default_value = 1.32
        length.inputs["To Min"].default_value = -0.25
        length.inputs["To Max"].default_value = 0.40
        ends = nt.nodes.new("ShaderNodeMath")
        ends.operation = "ADD"
        nt.links.new(half.outputs[0], ends.inputs[0])
        nt.links.new(length.outputs["Result"], ends.inputs[1])
        half = ends
        # A soft sheen band across the crown.
        band = nt.nodes.new("ShaderNodeMapRange")
        nt.links.new(sep.outputs["Z"], band.inputs["Value"])
        band.data_type = "FLOAT"
        band.interpolation_type = "SMOOTHERSTEP"
        band.inputs["From Min"].default_value = 1.68
        band.inputs["From Max"].default_value = 1.73
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
    nt.links.new(surface_detail(nt, name, mixn.outputs["Result"]), bsdf.inputs["Base Color"])
    return m


def face_marks(head, eyes):
    """Where the painting goes, measured on the reshaped head: each eye
    opening's boundary, the mouth's line, the nose's tip, and each iris's
    center and radius in the front view (x, z)."""
    bm = bmesh.new()
    bm.from_mesh(head.data)
    openings, mouth = [], []
    centers = []
    for side in (1, -1):
        vs = [v.co for v in eyes.data.vertices if v.co.x * side > 0]
        centers.append(sum(vs, Vector()) / len(vs))
    # The lids close over the eyeballs without a hole, so an opening is
    # where, seen from in front, the eyeball is nearer than the skin; its
    # boundary is the lids' edge.
    from mathutils.bvhtree import BVHTree

    dg = bpy.context.evaluated_depsgraph_get()
    skin, ball = BVHTree.FromObject(head, dg), BVHTree.FromObject(eyes, dg)
    step = 0.0004
    for side, c in zip((1, -1), centers):
        n = int(0.032 / step)
        seen = set()
        for i in range(-n, n + 1):
            for k in range(-n // 2, n // 2 + 1):
                x, z = c.x + i * step, c.z + k * step
                start = Vector((x, -0.4, z))
                e = ball.ray_cast(start, Vector((0, 1, 0)))
                if e[0] is None:
                    continue
                h = skin.ray_cast(start, Vector((0, 1, 0)))
                if h[0] is None or e[3] < h[3]:
                    seen.add((i, k))
        pts = [(c.x + i * step, c.z + k * step) for i, k in seen
               if any((i + di, k + dk) not in seen for di, dk in ((1, 0), (-1, 0), (0, 1), (0, -1)))]
        openings.append(pts)
    for e in bm.edges:
        m = (e.verts[0].co + e.verts[1].co) / 2
        if e.is_boundary and m.y < -0.05 and abs(m.x) < 0.04 and 1.56 < m.z < 1.60:
            mouth += [(v.co.x, v.co.z) for v in e.verts]
    tip = min((v.co for v in bm.verts if abs(v.co.x) < 0.003 and 1.60 < v.co.z < 1.64), key=lambda p: p.y)
    bm.free()
    # The irises face along each eye's gaze, turned up as the eyes are.
    gaze = Matrix.Rotation(math.radians(-6), 3, "X") @ Vector((0, -1, 0))
    irises = []
    for side, c in zip((1, -1), centers):
        vs = [v.co for v in eyes.data.vertices if v.co.x * side > 0]
        vs.sort(key=lambda p: -(p - c).dot(gaze))
        front = vs[: max(3, len(vs) // 40)]
        f = sum(front, Vector()) / len(front)
        irises.append((f.x, f.z))
    width = max(max(abs(x) for x, _ in o) - min(abs(x) for x, _ in o) for o in openings)
    return {"openings": openings, "mouth": mouth, "nose_tip": tip.z, "irises": irises,
            "iris_radius": IRIS_SHARE * width}


def projected(name, pixels, rect):
    """A material that paints `pixels` (RGBA, linear, rows bottom to top)
    over each surface point's (x, z) in `rect`, as seen from in front.
    Returns the material, its node tree, the painted color, and the
    position's components."""
    import numpy as np

    h, w = pixels.shape[:2]
    img = bpy.data.images.new(name, w, h, alpha=True, float_buffer=True)
    img.colorspace_settings.name = "Linear Rec.709"
    img.pixels.foreach_set(np.ascontiguousarray(pixels, dtype=np.float32).ravel())
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    geo = nt.nodes.new("ShaderNodeNewGeometry")
    sep = nt.nodes.new("ShaderNodeSeparateXYZ")
    nt.links.new(geo.outputs["Position"], sep.inputs[0])
    x0, x1, z0, z1 = rect
    comb = nt.nodes.new("ShaderNodeCombineXYZ")
    for axis, lo, hi, slot in (("X", x0, x1, 0), ("Z", z0, z1, 1)):
        mr = nt.nodes.new("ShaderNodeMapRange")
        mr.clamp = False
        nt.links.new(sep.outputs[axis], mr.inputs["Value"])
        mr.inputs["From Min"].default_value = lo
        mr.inputs["From Max"].default_value = hi
        nt.links.new(mr.outputs["Result"], comb.inputs[slot])
    tex = nt.nodes.new("ShaderNodeTexImage")
    tex.image = img
    tex.extension = "EXTEND"
    tex.interpolation = "Cubic"
    nt.links.new(comb.outputs[0], tex.inputs["Vector"])
    bsdf = nt.nodes["Principled BSDF"]
    bsdf.inputs["Roughness"].default_value = 0.6
    return m, nt, tex.outputs["Color"], sep


def face_material(obj, pixels):
    """Alice's hand-painted face (`alice_paint.face`), projected from the
    front onto the face; the back and sides of the head and neck take the
    side skin tone, so the front's features don't show through."""
    m, nt, color, sep = projected("face_skin", pixels, alice_paint.FACE_RECT)
    front = nt.nodes.new("ShaderNodeMapRange")
    front.interpolation_type = "SMOOTHSTEP"
    nt.links.new(sep.outputs["Y"], front.inputs["Value"])
    front.inputs["From Min"].default_value = 0.005
    front.inputs["From Max"].default_value = -0.035
    mix = nt.nodes.new("ShaderNodeMix")
    mix.data_type = "RGBA"
    nt.links.new(front.outputs["Result"], mix.inputs["Factor"])
    mix.inputs["A"].default_value = (*linear(alice_paint.PALETTE["skin_side"]), 1)
    nt.links.new(color, mix.inputs["B"])
    nt.links.new(mix.outputs["Result"], nt.nodes["Principled BSDF"].inputs["Base Color"])
    obj.data.materials.clear()
    obj.data.materials.append(m)


def eye_material(obj, pixels):
    """Alice's painted eyes (`alice_paint.eyes`), projected from the front."""
    m, nt, color, _ = projected("eyes", pixels, alice_paint.EYE_RECT)
    nt.links.new(color, nt.nodes["Principled BSDF"].inputs["Base Color"])
    obj.data.materials.clear()
    obj.data.materials.append(m)


def to_object(M, arm):
    me = bpy.data.meshes.new("Alice")
    me.from_pydata([tuple(v) for v in M.v], [], [list(f) for f in M.f])
    me.update()
    if M.uv:
        layer = me.uv_layers.new(name="hairuv")
        for poly, uv in zip(me.polygons, M.uv):
            for li, t in zip(poly.loop_indices, uv):
                layer.data[li].uv = t
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


def join_head(obj, head, eyes, cards):
    """Joins the head, eyes, and hair cards into Alice's object, at most
    four influences a vertex, normalized."""
    bpy.ops.object.select_all(action="DESELECT")
    for o in (obj, head, eyes, cards):
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


def drop_ears(head):
    """Presses the ears flat once the hair is fitted (her long hair covers
    them, and they would only poke through), and removes the inside of the
    mouth."""
    bm = bmesh.new()
    bm.from_mesh(head.data)

    def ear(c):
        return abs(c.x) > 0.064 and 1.585 < c.z < 1.715 and c.y > -0.03

    # Pressed flat to the skull rather than removed, so no hole shows
    # between the locks.
    for v in bm.verts:
        if ear(v.co):
            v.co.x = math.copysign(min(abs(v.co.x), 0.060), v.co.x)
    # The inside of the mouth, which the closed lips hide from every side:
    # it would only take atlas space.
    from mathutils.bvhtree import BVHTree

    tree = BVHTree.FromBMesh(bm)
    looks = [Vector(d).normalized() for d in ((0, 1, 0), (0.7, 1, 0), (-0.7, 1, 0), (0, 1, 0.7), (0, 1, -0.7))]

    def hidden(f):
        c = f.calc_center_median()
        if not (abs(c.x) < 0.035 and 1.55 < c.z < 1.62 and c.y > -0.105):
            return False
        for d in looks:
            hit = tree.ray_cast(c - d * 0.3, d)
            if hit[0] is None or (hit[0] - c).length < 0.0015:
                return False
        return True

    inside = [f for f in bm.faces if hidden(f)]
    bmesh.ops.delete(bm, geom=inside, context="FACES")
    bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
    bm.to_mesh(head.data)
    bm.free()


def subdivide(obj, levels):
    """Subdivides `obj` `levels` times, keeping its weights."""
    if not levels:
        return
    bpy.ops.object.select_all(action="DESELECT")
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    mod = obj.modifiers.new("Subdivision", "SUBSURF")
    mod.levels = levels
    mod.render_levels = levels
    mod.quality = 3
    mod.uv_smooth = "PRESERVE_BOUNDARIES"
    bpy.ops.object.modifier_move_to_index(modifier=mod.name, index=0)
    bpy.ops.object.modifier_apply(modifier=mod.name)


def smooth(obj):
    """Four influences a vertex at most, none below a 255th, normalized;
    then smooth shading with weighted normals, keeping creases sharper than
    60 degrees, such as the coat's hems and the boots' soles."""
    bpy.ops.object.select_all(action="DESELECT")
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    bpy.ops.object.vertex_group_limit_total(group_select_mode="ALL", limit=4)
    bpy.ops.object.vertex_group_normalize_all(group_select_mode="ALL", lock_active=False)
    bpy.ops.object.vertex_group_clean(group_select_mode="ALL", limit=0.004)
    bpy.ops.object.vertex_group_normalize_all(group_select_mode="ALL", lock_active=False)
    bpy.ops.object.shade_smooth_by_angle(angle=math.radians(60))
    weighted = obj.modifiers.new("WeightedNormal", "WEIGHTED_NORMAL")
    weighted.keep_sharp = True
    weighted.weight = 50
    bpy.ops.object.modifier_move_to_index(modifier=weighted.name, index=0)
    bpy.ops.object.modifier_apply(modifier=weighted.name)


def check_weights(obj):
    """Every vertex has one to four influences summing to one."""
    bad = 0
    for v in obj.data.vertices:
        ws = [g.weight for g in v.groups if g.weight > 0]
        if not (1 <= len(ws) <= 4) or abs(sum(ws) - 1) > 1e-3:
            bad += 1
    assert bad == 0, f"{bad} vertices with bad weights"


def unwrap(obj, lod):
    """Smart-project UVs for everything but the hair cards, the face at
    FACE_TEXELS times the texel density, packed above the atlas's bottom
    band (`HAIR_BAND`); the cards map onto the hair strips painted in that
    band, so they share its texels rather than each taking its own."""
    me = obj.data
    slot = {i: s.material.name for i, s in enumerate(obj.material_slots)}
    card_ids = {p.index for p in me.polygons if slot[p.material_index] == "hair_card"}
    saved = {}
    center = Vector((0, -0.006, 1.646))
    # The face takes the most, the rest of the head (under the hair) little,
    # blended smoothly so the islands don't tear between them.
    verts = {k for p in me.polygons if slot[p.material_index] in ("face_skin", "eyes") for k in p.vertices}
    for k in verts:
        co = me.vertices[k].co
        if co.z < 1.50:
            continue
        face = smoothstep(-0.01, -0.045, co.y) * smoothstep(1.52, 1.56, co.z)
        scale = lerp(0.8, FACE_TEXELS, face)
        saved[k] = co.copy()
        me.vertices[k].co = center + (co - center) * scale
    atlas = me.uv_layers.new(name="atlas")
    me.uv_layers.active = atlas
    atlas.active_render = True
    # Select everything but the cards (vertices too, which the selection
    # follows in edit mode).
    for v in me.vertices:
        v.select = False
    for e in me.edges:
        e.select = False
    for p in me.polygons:
        p.select = p.index not in card_ids
        if p.select:
            for k in p.vertices:
                me.vertices[k].select = True
    bpy.ops.object.select_all(action="DESELECT")
    bpy.context.view_layer.objects.active = obj
    obj.select_set(True)
    bpy.ops.object.mode_set(mode="EDIT")
    margin = 4.0 / lod["tex"]
    bpy.ops.uv.smart_project(angle_limit=math.radians(60), island_margin=margin, area_weight=0.0,
                             scale_to_bounds=False)
    bpy.ops.object.mode_set(mode="OBJECT")
    for k, co in saved.items():
        me.vertices[k].co = co
    band = HAIR_BAND
    # The mode switches replace the layers' data; look them up again.
    atlas = me.uv_layers["atlas"]
    hair = me.uv_layers["hairuv"]
    # Stretch the projected islands over the space above the band, then
    # pack them into it.
    us = [d.uv[0] for p in me.polygons if p.index not in card_ids for d in (atlas.data[li] for li in p.loop_indices)]
    vs = [d.uv[1] for p in me.polygons if p.index not in card_ids for d in (atlas.data[li] for li in p.loop_indices)]
    u0, u1, v0, v1 = min(us), max(us), min(vs), max(vs)
    for p in me.polygons:
        for li in p.loop_indices:
            if p.index in card_ids:
                x, y = hair.data[li].uv
                atlas.data[li].uv = (x, y * band)
            else:
                u, v = atlas.data[li].uv
                atlas.data[li].uv = ((u - u0) / (u1 - u0), band + (1 - band) * (v - v0) / (v1 - v0))
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.uv.pack_islands(udim_source="ORIGINAL_AABB", margin=margin, rotate=True)
    bpy.ops.object.mode_set(mode="OBJECT")


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
    scene.world.light_settings.distance = 0.07
    size = lod["tex"]
    images = {}
    for kind in ("color", "ao", "normal", "mask"):
        img = bpy.data.images.new(f"alice_{kind}", size, size, float_buffer=True)
        img.colorspace_settings.name = "Non-Color" if kind != "color" else "Linear Rec.709"
        images[kind] = img
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    # The cards bake opaque; their texels are replaced by the strips.
    for slot in obj.material_slots:
        nt = slot.material.node_tree
        bsdf = nt.nodes["Principled BSDF"]
        if bsdf.inputs["Alpha"].links:
            nt.links.remove(bsdf.inputs["Alpha"].links[0])
            bsdf.inputs["Alpha"].default_value = 1.0
    for kind, args in (("color", dict(type="DIFFUSE", pass_filter={"COLOR"})), ("ao", dict(type="AO")),
                       ("normal", dict(type="NORMAL", normal_space="OBJECT")),
                       ("mask", dict(type="EMIT"))):
        for slot in obj.material_slots:
            nt = slot.material.node_tree
            if kind == "mask":
                # The face's mask: its materials glow white for this pass.
                face = slot.material.name.split(".")[0] in ("face_skin", "eyes")
                bsdf = nt.nodes["Principled BSDF"]
                bsdf.inputs["Emission Color"].default_value = (1, 1, 1, 1) if face else (0, 0, 0, 1)
                bsdf.inputs["Emission Strength"].default_value = 1.0
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
    shade = (0.60 + 0.40 * np.clip(ao, 0, 1) ** 0.85) * (0.92 + 0.10 * np.clip(up, -1, 1))
    # The face takes little occlusion: deep sockets, nostrils, and the
    # mouth's corners otherwise bake to dark wedges at a game's distance.
    face = np.clip(px["mask"][..., :1], 0, 1)
    face_shade = 0.90 + 0.10 * np.clip(ao, 0, 1)
    shade = shade * (1 - face) + face_shade * face
    lin = np.clip(color * shade, 0, 1)
    srgb = np.where(lin <= 0.0031308, lin * 12.92, 1.055 * np.power(lin, 1 / 2.4) - 0.055)
    # Fewer levels a channel than eight bits: soft shading shows no banding
    # at a character's size on screen, and the atlas compresses far better.
    srgb = np.round(np.clip(srgb, 0, 1) * LEVELS) / LEVELS
    alpha = np.ones_like(srgb[..., :1])
    # The hair strips fill the bottom band, alpha and all.
    rows = int(round(HAIR_BAND * size))
    strips = alice_paint.hair_strips(size, rows)
    lin = np.clip(strips[..., :3], 0, 1)
    band = np.where(lin <= 0.0031308, lin * 12.92, 1.055 * np.power(lin, 1 / 2.4) - 0.055)
    srgb[:rows] = np.round(band * LEVELS) / LEVELS
    alpha[:rows, :, 0] = np.round(strips[..., 3] * LEVELS) / LEVELS
    out = np.concatenate([srgb, alpha], axis=-1)
    final = bpy.data.images.new("Alice_BaseColor", size, size, alpha=True)
    final.colorspace_settings.name = "sRGB"
    final.pixels[:] = out.reshape(-1).tolist()
    final.pack()
    return final


def finish(obj, image):
    """Two materials sampling the baked atlas: `alice`, opaque, and
    `alice_hair` for the cards, which the atlas's alpha cuts
    (`character_admit.py` writes it as an alpha-masked material)."""
    cards = {p.index for p in obj.data.polygons if obj.material_slots[p.material_index].name == "hair_card"}
    mats = []
    for name in ("alice", "alice_hair"):
        m = bpy.data.materials.new(name)
        m.use_nodes = True
        nt = m.node_tree
        bsdf = nt.nodes["Principled BSDF"]
        bsdf.inputs["Roughness"].default_value = 0.85
        tex = nt.nodes.new("ShaderNodeTexImage")
        tex.image = image
        nt.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
        # Both read the atlas's alpha, so the exporter writes one RGBA image.
        nt.links.new(tex.outputs["Alpha"], bsdf.inputs["Alpha"])
        mats.append(m)
    obj.data.materials.clear()
    for m in mats:
        obj.data.materials.append(m)
    for p in obj.data.polygons:
        p.material_index = 1 if p.index in cards else 0
    # Only the atlas ships; the other UV maps sampled the paintings.
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
    drop_ears(head)
    obj = to_object(M, arm)
    cards = to_object(alice.C, arm)
    parts = {}
    for f, tag in zip(M.f, M.tag):
        parts[tag] = parts.get(tag, 0) + len(f) - 2
    # The near levels are built coarse and subdivided, so nothing reads
    # faceted; lod0 subdivides the head too.
    subdivide(obj, lod.get("subdiv", 0))
    subdivide(head, lod.get("head_subdiv", 0))
    if lod.get("subdiv"):
        parts = {k: v * 4 ** lod["subdiv"] for k, v in parts.items()}
    parts["head"] = sum(len(p.vertices) - 2 for p in head.data.polygons)
    parts["eyes"] = sum(len(p.vertices) - 2 for p in eyes.data.polygons)
    parts["cards"] = sum(len(p.vertices) - 2 for p in cards.data.polygons)
    obj = join_head(obj, head, eyes, cards)
    smooth(obj)
    check_weights(obj)
    tris = sum(len(p.vertices) - 2 for p in obj.data.polygons)
    print("PARTS", variant, tris, json.dumps(parts))
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
