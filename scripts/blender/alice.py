"""Alice, Verse's original explorer-druid, on the Universal rig.

Run headless, one variant per run:
    Blender -b --factory-startup --python scripts/blender/alice.py -- \
        [OUT_DIR] [lod0|lod1|lod2|lod3] [--outfit coat|light|summer|base] [--quick PREVIEW_DIR]

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
                 cards=(28, 16, 4), card_steps=(7, 22), card_across=2),
    "lod1": dict(limb=8, torso=16, head=(24, 16), hair=(28, 6), finger=4, fingers=True, stride=1,
                 tex=1024, budget=46000, samples=96, subdiv=1,
                 cards=(24, 12, 3), card_steps=(6, 16), card_across=2),
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
    "dress": "#7D9466",
    "dress_trim": "#5C7349",
    "nail": "#F2C3B4",
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
        # Per-face outfits, a bit each (`wardrobe`); empty for one outfit.
        self.mask = []
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

    def compact(self):
        """Drops vertices no face uses."""
        used = sorted({i for f in self.f for i in f})
        index = {old: new for new, old in enumerate(used)}
        self.v = [self.v[i] for i in used]
        self.w = [self.w[i] for i in used]
        self.f = [tuple(index[i] for i in f) for f in self.f]

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
HAIR_BAND = 0.18
STRIPS_PREVIEW = (1024, 256)


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


# --- Shapes of the body -------------------------------------------------------
#
# Alice's base body is complete on its own: a torso from the neck to the
# crotch, arms from inside the shoulder to the wrist, legs from inside the
# hip to the ankle, and feet. Its curves follow a study of a stylized
# reference figure (docs/verse/female-character.md, "Lessons from a body
# reference"), scaled to the Universal rig: a waist-to-hip girth of about
# 0.68, a bust about 1.3 times the waist, a bust that stands about 4 cm past
# the underbust, glutes about 5 cm past the small of the back, and legs that
# narrow to slim knees and ankles. Clothing is built as layers over it
# (`Layer`), each a surface offset from the one under it with a tension
# envelope, so fabric bridges hollows (between the breasts, under the
# bust) where cloth would, and follows the waist where it is tailored.

# The torso at each height: z, half width, half depth, center y, superellipse
# exponent. Front is -y.
BODY = [
    (0.825, 0.050, 0.055, 0.024, 2.0),
    (0.845, 0.100, 0.075, 0.022, 2.1),
    (0.87, 0.148, 0.090, 0.020, 2.2),
    (0.90, 0.172, 0.098, 0.019, 2.3),
    (0.935, 0.178, 0.100, 0.017, 2.3),
    (0.97, 0.164, 0.096, 0.013, 2.3),
    (1.01, 0.134, 0.087, 0.009, 2.3),
    (1.045, 0.115, 0.084, 0.005, 2.3),
    (1.075, 0.112, 0.086, 0.003, 2.3),
    (1.11, 0.113, 0.087, 0.001, 2.3),
    (1.15, 0.114, 0.087, -0.001, 2.35),
    (1.19, 0.118, 0.086, -0.002, 2.4),
    (1.23, 0.124, 0.088, -0.002, 2.4),
    (1.27, 0.129, 0.089, -0.001, 2.45),
    (1.31, 0.135, 0.089, 0.001, 2.5),
    (1.35, 0.143, 0.087, 0.006, 2.6),
    (1.39, 0.143, 0.080, 0.012, 2.6),
    (1.42, 0.128, 0.070, 0.016, 2.5),
    (1.445, 0.100, 0.060, 0.019, 2.3),
    (1.465, 0.066, 0.051, 0.020, 2.1),
    (1.49, 0.049, 0.046, 0.020, 2.0),
    (1.52, 0.045, 0.043, 0.018, 2.0),
]


def body_at(z):
    """The torso's section at height z: (half width, half depth, y, n)."""
    rows = BODY
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


def lobe(x, z, cx, cz, rx, rz_up, rz_down, power=1.6):
    """A soft super-Gaussian lobe at (cx, cz), fuller above than below when
    rz_up > rz_down, so it sits on a crease underneath."""
    rz = rz_up if z >= cz else rz_down
    r2 = ((x - cx) / rx) ** 2 + ((z - cz) / rz) ** 2
    return exp(-(r2 ** power))


def body_shape(d, p):
    """What the torso's superellipse leaves out, along the radial direction
    d at the point p: a round bust with a crease under it, collarbones,
    shoulder blades, a groove down the spine, the small of the back, and
    round glutes with a crease under them."""
    out = 0.0
    front, back = max(0.0, -d.y), max(0.0, d.y)
    if front > 0:
        for s in (-1, 1):
            out += 0.047 * lobe(p.x, p.z, s * 0.068, 1.252, 0.058, 0.080, 0.044) * front ** 0.6
        # Collarbones: a fine ridge from the notch out toward each shoulder,
        # with a hollow under it.
        line = 1.452 + 0.018 * min(1.0, max(0.0, abs(p.x) - 0.018) / 0.10)
        ridge = exp(-(((p.z - line) / 0.0045) ** 2)) * smoothstep(0.012, 0.03, abs(p.x)) * smoothstep(0.135, 0.10, abs(p.x))
        hollow = exp(-(((p.z - line + 0.014) / 0.008) ** 2)) * smoothstep(0.03, 0.06, abs(p.x)) * smoothstep(0.13, 0.09, abs(p.x))
        out += (0.0035 * ridge - 0.0025 * hollow) * front
        # A soft belly below the navel.
        out += 0.004 * exp(-((p.x / 0.07) ** 2)) * exp(-(((p.z - 0.985) / 0.03) ** 2)) * front
    if back > 0:
        for s in (-1, 1):
            out += 0.040 * lobe(p.x, p.z, s * 0.064, 0.905, 0.066, 0.075, 0.042, 1.4) * back ** 0.7
            out += 0.006 * lobe(p.x, p.z, s * 0.075, 1.345, 0.05, 0.06, 0.06) * back
        out -= 0.004 * exp(-((p.x / 0.010) ** 2)) * smoothstep(1.0, 1.08, p.z) * smoothstep(1.46, 1.40, p.z) * back
        out -= 0.010 * exp(-(((p.z - 1.06) / 0.05) ** 2)) * exp(-((p.x / 0.08) ** 2)) * back
    return out


def body_point(z, theta, sizes=None):
    """The base body's torso surface at height z and angle theta (from +x,
    counterclockwise seen from above; the front is -pi/2)."""
    a, b, yc, n = sizes or body_at(z)
    ct, st = cos(theta), sin(theta)
    ex = 2.0 / n
    x = math.copysign(abs(ct) ** ex, ct) * a
    y = math.copysign(abs(st) ** ex, st) * b
    p = Vector((x, yc + y, z))
    d = Vector((ct, st, 0)).normalized()
    return p + d * body_shape(d, p)


# Arms (x along the T-posed arm), legs (z down the leg), and their skin
# sections: x or z, then the half extents (front to back, up and down for an
# arm; side to side, front to back for a leg).
ARM = [
    (0.085, 0.050, 0.054), (0.12, 0.054, 0.058), (0.16, 0.052, 0.055), (0.20, 0.046, 0.048),
    (0.25, 0.041, 0.042), (0.30, 0.038, 0.038), (0.35, 0.035, 0.034), (0.39, 0.032, 0.031),
    (0.42, 0.034, 0.032), (0.47, 0.035, 0.032), (0.52, 0.032, 0.028), (0.57, 0.028, 0.024),
    (0.61, 0.025, 0.020), (0.632, 0.026, 0.019),
]
LEG = [
    (1.00, 0.040, 0.048), (0.96, 0.066, 0.074), (0.92, 0.079, 0.085), (0.88, 0.080, 0.085), (0.84, 0.077, 0.082),
    (0.78, 0.074, 0.079), (0.72, 0.068, 0.073), (0.66, 0.062, 0.067), (0.61, 0.056, 0.060),
    (0.575, 0.052, 0.055), (0.545, 0.050, 0.053), (0.515, 0.049, 0.052), (0.48, 0.050, 0.056),
    (0.44, 0.052, 0.060), (0.40, 0.051, 0.060), (0.35, 0.047, 0.055), (0.29, 0.041, 0.047),
    (0.23, 0.035, 0.039), (0.17, 0.031, 0.034), (0.12, 0.029, 0.031), (0.085, 0.029, 0.032),
]


def table_at(rows, t, descending=False):
    """Linear interpolation of a section table at t."""
    if descending:
        rows = rows[::-1]
    if t <= rows[0][0]:
        return rows[0][1:]
    for a, b in zip(rows, rows[1:]):
        if a[0] <= t <= b[0]:
            u = (t - a[0]) / (b[0] - a[0])
            return tuple(lerp(a[k], b[k], u) for k in range(1, len(a)))
    return rows[-1][1:]


def leg_shape(d, z):
    """A leg's muscle: the calf's swell behind, the kneecap in front, and the
    inner thigh's softness."""
    out = 0.006 * max(0.0, d.y) * exp(-(((z - 0.42) / 0.06) ** 2))
    out += 0.004 * max(0.0, -d.y) * exp(-(((z - 0.54) / 0.02) ** 2))
    out += 0.004 * max(0.0, -d.x) * exp(-(((z - 0.86) / 0.07) ** 2))
    return out


def arm_shape(d, x):
    """An arm's deltoid over the shoulder and the forearm's swell."""
    out = 0.006 * max(0.0, d.z) * exp(-(((x - 0.14) / 0.04) ** 2))
    out += 0.003 * max(0.0, -d.y) * exp(-(((x - 0.46) / 0.04) ** 2))
    return out


def envelope(values, coords, k, wrap=None):
    """Each value raised to the tension envelope max_j(v_j - k |c_i - c_j|):
    cloth under tension k bridges hollows narrower than its sag. With
    `wrap`, coordinates are angles and distances wrap around a circle of
    that radius."""
    if k is None:
        return list(values)
    out = []
    for ci in coords:
        best = -1e9
        for v, cj in zip(values, coords):
            dc = abs(ci - cj)
            if wrap:
                dc = min(dc, 2 * pi - dc) * wrap
            best = max(best, v - k * dc)
        out.append(best)
    return out


# The outfits the Everglade level carries, on one atlas, the default first;
# and that level.
WARDROBE = ("coat", "light", "summer")
WARDROBE_LEVEL = "lod1"
# The outfits Alice can wear over her base body, and which of the body's
# parts each one hides everywhere: a part's faces whose every vertex lies in
# (low, high) along the axis ("z" up, "x" out along the arm) are removed.
OUTFITS = {
    # The tailored coat over a shirt and trousers: her default.
    "coat": {"covers": {"skin_torso": (0.86, 1.455, "z"), "skin_arm": (0.11, 0.615, "x"),
                        "skin_leg": (0.10, 1.01, "z")}},
    # The coat off: a V-necked shirt with its sleeves rolled to the elbow,
    # tucked into fitted trousers, and the sash cinching the waist.
    "light": {"covers": {"skin_torso": (0.86, 1.37, "z"), "skin_arm": (0.11, 0.345, "x"),
                         "skin_leg": (0.10, 1.01, "z")}},
    # A fitted summer dress to the knee, with ankle boots.
    "summer": {"covers": {"skin_torso": (0.92, 1.36, "z"), "skin_leg": (-1.0, 0.15, "z")}},
    # The base body alone, in a plain bandeau and briefs, for review.
    "base": {"covers": {}},
}


# --- The build ----------------------------------------------------------------


class Alice:
    def __init__(self, rig, lod, outfit="coat"):
        self.R = rig
        self.L = lod
        self.outfit = outfit
        self.outer = None
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

    def w_skirt(self, p):
        """A skirt's weights: like the coat's, but the front and back follow
        the thighs too, so a stride doesn't push a knee through it."""
        w = self.w_torso(p, hem=False)
        if p.z < 1.0:
            s = 1 if p.x >= 0 else -1
            th = 0.62 * smoothstep(0.98, 0.70, p.z) * smoothstep(0.0, 0.07, abs(p.x))
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
        horizontal direction d (a unit vector), and that axis: the outermost
        layer below the shoulders, the collar and neck above."""
        if z <= 1.468:
            outer = self.outer
            th = math.atan2(d.y, d.x)
            zz = min(max(z, outer.zs[0]), outer.zs[-1])
            zs = outer.zs
            fz = (zz - zs[0]) / (zs[-1] - zs[0]) * (len(zs) - 1)
            i = min(int(fz), len(zs) - 2)
            c = outer.C[i].lerp(outer.C[i + 1], fz - i)
            return outer.radius(zz, th), Vector((c.x, c.y, 0))
        # The collar's roll, then the slim neck above it.
        r = lerp(0.104, 0.046, smoothstep(1.47, 1.56, z))
        return r, Vector((0, lerp(0.028, 0.012, smoothstep(1.47, 1.56, z)), 0))

    def drape(self, p, gap):
        """`p` pushed out, horizontally, to `gap` beyond her clothed body."""
        _, _, yc, _ = body_at(min(p.z, 1.5))
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
        else:
            # Behind the shoulders, onto the back, not over their tops.
            a_t = lerp(2.35, 3.0, (a_e - 1.42) / (pi - 1.42))
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
        # Hair falls in a line: where the body pushes it out (the shoulder
        # blades, the bust), the hair above leans out to meet it gradually
        # rather than kinking.
        axis = Vector((0, self.HC.y, 0))
        for i in range(len(pts) - 2, n_skull, -1):
            p, q = pts[i], pts[i + 1]
            hp = Vector((p.x - axis.x, p.y - axis.y, 0))
            hq = Vector((q.x - axis.x, q.y - axis.y, 0))
            want = hq.length - 0.6 * (p.z - q.z)
            if hp.length < want and hp.length > 1e-6:
                pts[i] = Vector((axis.x, axis.y, p.z)) + hp * (want / hp.length)
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
            lane = {0: index % 2, 1: 2 + (index * 7) % 5, 2: lanes - 1}[spec["layer"]]
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
            nr = len(rings)

            def nail(i, j, nr=nr):
                # The back of the last joint: in the T-pose the hand's back
                # faces up, the rings' first axis.
                return "nail" if i >= nr - 3 and cos(2 * pi * (j + 0.5) / fsegs) > 0.35 else "skin"

            loft(M, rings, fsegs, "skin", ch.weights, cap1=True, dome=0.004, matfn=nail)

    # Legs ----------------------------------------------------------------------------------

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

    FOOT = [
        # A boot of a woman's size: 27 cm heel to toe, narrow, with a toe
        # that rises rather than a flat paddle.
        (0.102, 0.022, 0.060, True), (0.096, 0.032, 0.092, False), (0.080, 0.036, 0.116, True),
        (0.050, 0.038, 0.116, False), (0.015, 0.039, 0.106, True), (-0.020, 0.040, 0.090, False),
        (-0.055, 0.041, 0.074, True), (-0.090, 0.040, 0.064, False), (-0.122, 0.037, 0.058, True),
        (-0.150, 0.032, 0.052, False), (-0.170, 0.023, 0.044, True),
    ]

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

    # The base body and the layers over it ------------------------------------------
    #
    # `build_body` makes the complete skin body; `build_outfit` dresses it in
    # one of OUTFITS, and `cover` then removes the body's faces that an
    # opaque garment hides everywhere, so they cost no triangles and cannot
    # show through. A garment is a `Layer`: a surface offset from the body
    # (or from the garment under it) with a tension envelope, sampled on a
    # fine grid and read back at any height and angle.

    def rows_of(self, zs):
        """`zs` thinned for the far levels."""
        stride = self.L["stride"]
        if stride == 1 or len(zs) <= 3:
            return list(zs)
        inner = zs[1:-1][::stride]
        return [zs[0]] + inner + [zs[-1]]

    def mesh_rows(self, M, P, wfn, mat, matfn=None, wrap=True, axis=Vector((0, 0, 1)), cap0=None, cap1=None):
        """Faces between rows of points (each row a ring about the axis),
        oriented to face away from each row's center, with optional fans at
        the first and last rows to the points `cap0` and `cap1`."""
        W = [[wfn(p) for p in row] for row in P]
        c0 = sum(P[0], Vector()) / len(P[0])
        a, b, d = P[0][0], P[0][1 % len(P[0])], P[1][0]
        flip = (b - a).cross(d - a).dot(a - c0) < 0
        idx = grid(M, P, W, mat, wrap=wrap, flip=flip, matfn=matfn)
        for cap, row, out in ((cap0, 0, -1), (cap1, -1, 1)):
            if cap is not None:
                fan(M, idx[row], cap, wfn(cap), mat, axis * out)
        return idx

    def torso_rows(self, zs, segs, fn):
        """Rings of `segs` points at heights `zs`: fn(z, theta) -> point,
        theta counterclockwise from the front."""
        return [[fn(z, -pi / 2 + 2 * pi * j / segs) for j in range(segs)] for z in zs]

    def limb_ring(self, c, axis, ref, rx, ry, segs, shape=None, gap=0.0):
        a, u, v = frame(axis, ref)
        out = []
        for j in range(segs):
            th = 2 * pi * j / segs
            dvec = (u * cos(th) * rx + v * sin(th) * ry)
            dn = dvec.normalized()
            out.append(c + dvec + dn * ((shape(dn) if shape else 0.0) + gap))
        return out

    def torso_skin(self):
        """The torso from the neck's base to the crotch, closed between the
        legs."""
        M = self.M
        M.part = "skin_torso"
        zs = self.rows_of([r[0] for r in BODY])
        P = self.torso_rows(zs, self.L["torso"], body_point)
        self.mesh_rows(M, P, lambda p: self.w_torso(p, hem=True), "skin",
                       cap0=Vector((0, 0.024, zs[0] - 0.008)))

    def arm_skin(self):
        """The left arm, from inside the shoulder to the wrist; mirrored."""
        L = Mesh()
        L.part = "skin_arm"
        chain = self.R.arm(1)
        segs = self.L["limb"]
        xs = self.rows_of([r[0] for r in ARM])
        P = [self.limb_ring(self.arm_center(1, x), (1, 0, 0), (0, 1, 0), *table_at(ARM, x), segs,
                            shape=lambda d, x=x: arm_shape(d, x)) for x in xs]
        self.mesh_rows(L, P, chain.weights, "skin", axis=Vector((1, 0, 0)))
        self.hand_left(L)
        return L

    def leg_skin(self):
        """The left leg, from inside the hip to the ankle; mirrored."""
        L = Mesh()
        L.part = "skin_leg"
        chain = self.R.leg(1)
        segs = self.L["limb"]
        zs = self.rows_of([r[0] for r in LEG])
        P = [self.limb_ring(self.leg_center(1, z), (0, 0, -1), (1, 0, 0), *table_at(LEG, z, descending=True), segs,
                            shape=lambda d, z=z: leg_shape(d, z)) for z in zs]
        self.mesh_rows(L, P, chain.weights, "skin", axis=Vector((0, 0, -1)))
        return L

    # Layers ---------------------------------------------------------------------------

    class Layer:
        """A surface about the torso's axis: a radius at each height and
        angle on a fine grid, read back by `at`."""

        def __init__(self, z0, z1, radius, kz=None, kth=None, step=0.01, nth=72):
            n = max(2, int(round((z1 - z0) / step)) + 1)
            self.zs = [z0 + (z1 - z0) * i / (n - 1) for i in range(n)]
            self.ths = [-pi / 2 + 2 * pi * j / nth for j in range(nth)]
            self.nth = nth
            R, D, C = [], [], []
            for z in self.zs:
                row_r, row_d = [], []
                c = None
                for th in self.ths:
                    r, d, c = radius(z, th)
                    row_r.append(r)
                    row_d.append(d)
                R.append(row_r)
                D.append(row_d)
                C.append(c)
            # Tension down each column, then around each row.
            for j in range(nth):
                col = envelope([R[i][j] for i in range(n)], self.zs, kz)
                for i in range(n):
                    R[i][j] = col[i]
            if kth is not None:
                for i in range(n):
                    mean = sum(R[i]) / nth
                    R[i] = envelope(R[i], self.ths, kth, wrap=mean)
            self.R, self.D, self.C = R, D, C

        def at(self, z, th, extra=0.0):
            """The point at height z and angle th, `extra` farther out."""
            zs = self.zs
            fz = (min(max(z, zs[0]), zs[-1]) - zs[0]) / (zs[-1] - zs[0]) * (len(zs) - 1)
            i = min(int(fz), len(zs) - 2)
            tz = fz - i
            fj = ((th + pi / 2) % (2 * pi)) / (2 * pi) * self.nth
            j = int(fj) % self.nth
            j2 = (j + 1) % self.nth
            tj = fj - int(fj)

            def bil(G):
                return (G[i][j] * (1 - tj) + G[i][j2] * tj) * (1 - tz) + (G[i + 1][j] * (1 - tj) + G[i + 1][j2] * tj) * tz

            r = bil(self.R) + extra
            d = bil(self.D)
            d = Vector((d.x, d.y, 0)).normalized()
            c = self.C[i].lerp(self.C[i + 1], tz)
            return Vector((c.x, c.y, z)) + d * r

        def radius(self, z, th):
            p = self.at(z, th)
            zs = self.zs
            fz = (min(max(z, zs[0]), zs[-1]) - zs[0]) / (zs[-1] - zs[0]) * (len(zs) - 1)
            i = min(int(fz), len(zs) - 2)
            c = self.C[i].lerp(self.C[i + 1], fz - i)
            return math.hypot(p.x - c.x, p.y - c.y)

    def body_radius_fn(self, gap, hull=False):
        """The base body's radius about its axis, `gap` out (a number or
        fn(z, theta)); with `hull`, below the hips the hips' hull, as a
        skirt or a coat's tails hang."""
        def radius(z, th):
            zz = max(z, 0.89) if hull else max(z, 0.825)
            a, b, yc, n = body_at(zz)
            p = body_point(zz, th, sizes=(a, b, yc, n))
            c = Vector((0, yc, z))
            d = Vector((p.x, p.y - yc, 0))
            r = d.length
            g = gap(z, th) if callable(gap) else gap
            return r + g, d.normalized(), c
        return radius

    def over(self, under, gap):
        """A radius function `gap` out from the layer `under`."""
        def radius(z, th):
            p = under.at(z, th)
            i = 0
            c = None
            zs = under.zs
            fz = (min(max(z, zs[0]), zs[-1]) - zs[0]) / (zs[-1] - zs[0]) * (len(zs) - 1)
            i = min(int(fz), len(zs) - 2)
            c = under.C[i].lerp(under.C[i + 1], fz - i)
            d = Vector((p.x - c.x, p.y - c.y, 0))
            g = gap(z, th) if callable(gap) else gap
            return d.length + g, d.normalized(), Vector((c.x, c.y, z))
        return radius

    def layer_mesh(self, layer, zs, mat, wfn, matfn=None, ths=None, cap0=None, drop=None):
        """Faces over `layer` at heights `zs`, all the way around (or over
        the angles `ths`), skipping faces where drop(center) holds."""
        M = self.M
        segs = self.L["torso"]
        ths = ths or [-pi / 2 + 2 * pi * j / segs for j in range(segs)]
        P = [[layer.at(z, th) for th in ths] for z in zs]
        start = len(M.f)
        self.mesh_rows(M, P, wfn, mat, matfn=matfn, cap0=cap0)
        if drop:
            keep = [k for k in range(start, len(M.f))
                    if not drop(sum((M.v[i] for i in M.f[k]), Vector()) / len(M.f[k]))]
            M.f[start:] = [M.f[k] for k in keep]
            M.m[start:] = [M.m[k] for k in keep]
            M.tag[start:] = [M.tag[k] for k in keep]

    def limb_layer(self, L, rings_at, ts, gap, mat, wfn, matfn=None, axis=Vector((1, 0, 0))):
        """A sleeve or a trouser leg over a limb: rings_at(t, gap) at each
        station t, `gap` (a number or fn(t)) out."""
        P = [rings_at(t, gap(t) if callable(gap) else gap) for t in ts]
        return self.mesh_rows(L, P, wfn, mat, matfn=matfn, axis=axis)

    def arm_ring(self, x, gap, segs=None):
        return self.limb_ring(self.arm_center(1, x), (1, 0, 0), (0, 1, 0), *table_at(ARM, min(x, 0.632)),
                              segs or self.L["limb"], shape=lambda d: arm_shape(d, x), gap=gap)

    def leg_ring(self, z, gap, segs=None):
        return self.limb_ring(self.leg_center(1, z), (0, 0, -1), (1, 0, 0), *table_at(LEG, max(z, 0.085), descending=True),
                              segs or self.L["limb"], shape=lambda d: leg_shape(d, z), gap=gap)

    # Garments ---------------------------------------------------------------------------

    def shirt(self, sleeve_end, neckline):
        """The cream linen shirt, tucked in at the waist: it follows the
        body closely but bridges the hollow under the bust and between the
        breasts as cloth does. `neckline`: "crew" or "v"."""
        M = self.M
        M.part = "shirt"
        self.shirt_layer = layer = self.Layer(0.95, 1.48, self.body_radius_fn(0.006), kz=0.55, kth=0.7)
        zs = self.rows_of([0.97, 1.0, 1.04, 1.075, 1.11, 1.15, 1.19, 1.23, 1.27, 1.31, 1.35, 1.39, 1.42, 1.445,
                           1.465, 1.478])
        drop = None
        if neckline == "v":
            def drop(c):
                depth = 1.475 - 0.085 * smoothstep(0.075, 0.0, abs(c.x))
                return c.y < 0.0 and c.z > depth and abs(c.x) < 0.09
        self.layer_mesh(layer, zs, "linen", self.w_torso, drop=drop)
        L = Mesh()
        L.part = "shirt"
        chain = self.R.arm(1)
        xs = [x for x in (0.10, 0.14, 0.18, 0.23, 0.28, 0.33, 0.38, 0.43, 0.48, 0.53, 0.58, 0.625) if x < sleeve_end]
        xs = self.rows_of(xs + [sleeve_end])
        rolled = sleeve_end < 0.5

        def gap(x):
            if rolled:
                return 0.007 + 0.008 * smoothstep(sleeve_end - 0.035, sleeve_end - 0.02, x)
            return 0.007

        def matfn(i, j):
            return "linen_shade" if rolled and xs[min(i + 1, len(xs) - 1)] > sleeve_end - 0.03 else "linen"

        self.limb_layer(L, self.arm_ring, xs, gap, "linen", chain.weights, matfn=matfn)
        return L

    def trousers(self, knee_boot):
        """Close-fitting dark trousers with a waistband, into the boots."""
        M = self.M
        M.part = "trousers"
        layer = self.Layer(0.82, 1.05, self.body_radius_fn(lambda z, th: 0.006 + 0.003 * smoothstep(1.0, 1.01, z)),
                           kz=0.6, kth=0.9)
        zs = self.rows_of([0.825, 0.845, 0.87, 0.90, 0.935, 0.97, 1.0, 1.01, 1.04])

        def matfn(i, j):
            return "leather_dark" if zs[i] >= 1.0 else "leggings"

        self.layer_mesh(layer, zs, "leggings", lambda p: self.w_torso(p, hem=True), matfn=matfn,
                        cap0=Vector((0, 0.024, 0.825 - 0.012)))
        L = Mesh()
        L.part = "trousers"
        chain = self.R.leg(1)
        bottom = 0.30 if knee_boot else 0.10
        zs = self.rows_of([z for z in (0.98, 0.95, 0.90, 0.84, 0.78, 0.72, 0.66, 0.61, 0.575, 0.545, 0.515, 0.48,
                                       0.44, 0.40, 0.35, 0.30, 0.23, 0.17, 0.12, 0.10) if z >= bottom])
        self.limb_layer(L, self.leg_ring, zs, 0.006, "leggings", chain.weights, axis=Vector((0, 0, -1)))
        return L

    def boots(self, top):
        """Boots shaped to the leg: a fitted shaft to `top` with a folded
        cuff, a narrow ankle, a low heel, and a toe that rises."""
        L = Mesh()
        L.part = "boots"
        chain = self.R.leg(1)
        zs = [top, top - 0.005, top - 0.025, top - 0.035]
        zs += [z for z in (0.40, 0.35, 0.29, 0.23, 0.17, 0.13, 0.10, 0.085) if z < top - 0.04]
        zs = self.rows_of(zs)
        under = 0.010 if top > 0.3 else 0.006

        def gap(z):
            cuff = smoothstep(top - 0.04, top - 0.03, z)
            # The cuff's top edge turns back in to the leg.
            lip = smoothstep(top - 0.006, top, z)
            return under + 0.006 * cuff - (under + 0.004) * lip + 0.004 * smoothstep(0.17, 0.10, z)

        def mat(i, j):
            return "leather_dark" if zs[i] > top - 0.036 else "leather"

        self.limb_layer(L, self.leg_ring, zs, gap, "leather", chain.weights, matfn=mat, axis=Vector((0, 0, -1)))
        sole = -0.004
        x0 = self.R.h("foot_l").x
        rings = []
        for y, hw, top_z, key in self.stations(self.FOOT):
            zc = (top_z + sole) / 2
            hz = (top_z - sole) / 2 + 0.008
            rings.append(dict(c=Vector((x0 + 0.004 * smoothstep(0.0, -0.2, y), y, zc)), axis=(0, -1, 0),
                              ref=(0, 0, 1), rx=hz, ry=hw, n=2.8))

        def flat(p):
            return Vector((p.x, p.y, max(p.z, sole)))

        def w_foot(p):
            w = chain.weights(Vector((x0, p.y, min(p.z, 0.05))))
            if p.z > 0.085:
                w = mix(w, rigid("calf_l"), 0.4 * smoothstep(0.085, 0.12, p.z))
            return w

        start = len(L.f)
        loft(L, rings, self.L["limb"], "leather", w_foot, cap0=True, cap1=True, post=flat, dome=0.004)
        for k in range(start, len(L.f)):
            zz = [L.v[i].z for i in L.f[k]]
            if max(zz) < sole + 0.012:
                L.m[k] = "sole"
            elif min(L.v[i].y for i in L.f[k]) > 0.075 and max(zz) < 0.04:
                L.m[k] = "sole"
        return L

    def bracer(self, under_gap):
        """Wrapped leather from mid forearm to the wrist."""
        L = Mesh()
        L.part = "gear"
        chain = self.R.arm(1)
        xs = self.rows_of([0.47, 0.485, 0.53, 0.58, 0.61, 0.628, 0.64])
        last = len(xs)

        def gap(x):
            return under_gap + 0.003 + 0.004 * smoothstep(0.47, 0.49, x) * smoothstep(0.645, 0.62, x)

        def mat(i, j):
            return "leather_dark" if i in (0, last - 2) else "leather"

        self.limb_layer(L, self.arm_ring, xs, gap, "leather", chain.weights, matfn=mat)
        return L

    def coat(self):
        """The tailored coat: it follows the bust, the waist, and the hips
        over the shirt, then flares from the hips to the knee, open in
        front, with side vents and turned-back cuffs at the elbow."""
        M = self.M
        M.part = "coat"
        base = self.Layer(0.40, 1.48, self.body_radius_fn(0.006, hull=True), kz=0.55, kth=0.7)

        def gap(z, th):
            return 0.011 + 0.006 * smoothstep(1.0, 0.9, z)

        hip = 0.95

        def radius(z, th):
            r, d, c = self.over(base, gap)(z, th)
            if z < hip:
                rh, _, _ = self.over(base, gap)(hip, th)
                r = max(r, rh + (hip - z) * 0.30)
            return r, d, c

        self.coat_layer = layer = self.Layer(0.47, 1.47, radius, kz=0.9, kth=0.8)
        self.outer = layer
        rows = self.rows_of([r[0] for r in self.COAT])
        cols = max(3, self.L["torso"] // 4 + 1)
        for s in (1, -1):
            for piece in ("front", "back"):
                P = []
                for z in rows:
                    op = math.radians(self.opening(z))
                    lo, hi = (op, pi / 2 - self.vent(z)) if piece == "front" else (pi / 2 + self.vent(z), pi)
                    row = [layer.at(z, -pi / 2 + s * (lo + (hi - lo) * j / (cols - 1))) for j in range(cols)]
                    P.append(row if s > 0 else row[::-1])
                W = [[self.w_coat(p) for p in row] for row in P]
                edge = 0 if s > 0 else cols - 2

                def matfn(i, j, edge=edge, piece=piece):
                    if i == 0 or (piece == "front" and j == edge):
                        return "coat_trim"
                    return "coat"

                first, last = (piece == "front", True) if s > 0 else (True, piece == "front")
                shell(M, P, W, 0.007, "coat", "coat_inner", "coat_trim", matfn=matfn, rims=(True, True, first, last))
        # Sleeves to the elbow with a turned-back cuff, over the shirt.
        L = Mesh()
        L.part = "coat"
        chain = self.R.arm(1)
        xs = self.rows_of([0.10, 0.16, 0.22, 0.28, 0.33, 0.355, 0.395, 0.41])

        def sleeve_gap(x):
            return 0.020 + 0.012 * smoothstep(0.35, 0.36, x) * smoothstep(0.41, 0.40, x) - 0.012 * smoothstep(0.40, 0.41, x)

        nr = len(xs)

        def cuff(i, j):
            return "coat_trim" if i >= nr - 3 else "coat"

        self.limb_layer(L, self.arm_ring, xs, sleeve_gap, "coat", chain.weights, matfn=cuff)
        M.extend(L)
        M.extend_mirrored(L)

    # The coat's front opening's half angle and the side vents, by height.
    COAT = [
        (0.47, 55), (0.52, 53), (0.60, 49), (0.70, 44), (0.80, 38), (0.88, 33), (0.94, 30), (1.00, 27),
        (1.06, 26), (1.12, 25), (1.19, 23), (1.26, 21), (1.32, 19), (1.38, 17), (1.43, 15), (1.468, 14),
    ]

    def opening(self, z):
        return table_at(self.COAT, z)[0]

    @staticmethod
    def vent(z):
        """Half the side vent's opening at height z, radians: closed above the
        hip, opening toward the hem, so the arms hang clear of the skirt."""
        return math.radians(11.0) * smoothstep(0.98, 0.72, z)

    def dress(self):
        """A fitted summer dress in sage linen: a bodice that follows the
        bust and the waist with a straight neckline and narrow straps, and a
        skirt that flares from the hips to the knee in soft folds."""
        M = self.M
        M.part = "dress"
        top_front, top_back = 1.385, 1.405
        hip = 0.93

        body = self.body_radius_fn(0.005, hull=True)

        def radius(z, th):
            r, d, c = body(z, th)
            if z < hip:
                rh, _, _ = body(hip, th)
                fold = 0.010 * smoothstep(hip, 0.55, z) * sin(11 * th + 0.6)
                r = max(r, rh + (hip - z) * 0.33 + fold)
            return r, d, c

        self.dress_layer = layer = self.Layer(0.50, 1.42, radius, kz=1.2, kth=0.5)
        self.outer = layer
        zs = self.rows_of([0.50, 0.53, 0.58, 0.64, 0.70, 0.76, 0.82, 0.87, 0.91, 0.95, 1.0, 1.04, 1.075, 1.11, 1.15,
                           1.19, 1.23, 1.27, 1.31, 1.35, 1.38, 1.405])

        def drop(c):
            top = lerp(top_front, top_back, smoothstep(-0.05, 0.05, c.y))
            return c.z > top

        def matfn(i, j):
            return "dress_trim" if i == 0 else "dress"

        self.layer_mesh(layer, zs, "dress", self.w_skirt, matfn=matfn, drop=drop)
        # The straps, over each shoulder.
        for s in (1, -1):
            path = [layer.at(top_front - 0.004, -pi / 2 + s * 0.62, 0.002),
                    Vector((s * 0.098, -0.03, 1.452)), Vector((s * 0.102, 0.02, 1.468)),
                    Vector((s * 0.098, 0.07, 1.446)), layer.at(top_back - 0.004, pi / 2 - s * 0.62, 0.002)]
            for k in (1, 2, 3):
                p = path[k]
                th = math.atan2(p.y - 0.01, p.x)
                q = self.body_radius_fn(0.004)(p.z, th)
                path[k] = Vector((q[2].x, q[2].y, p.z)) + q[1] * q[0]
            rings = []
            for k, c in enumerate(path):
                t = (path[min(k + 1, 4)] - path[max(k - 1, 0)]).normalized()
                out = Vector((c.x, c.y - 0.01, 0)).normalized()
                rings.append(dict(c=c + out * 0.002, axis=t, ref=out, rx=0.0015, ry=0.007))
            loft(M, rings, 4, "dress_trim", self.w_torso)

    def underwear(self):
        """A plain bandeau and briefs, for looking at the base body."""
        M = self.M
        M.part = "underwear"
        bra = self.Layer(1.17, 1.33, self.body_radius_fn(0.004), kz=0.8, kth=0.45)
        self.layer_mesh(bra, self.rows_of([1.18, 1.21, 1.25, 1.29, 1.32]), "linen", self.w_torso)
        briefs = self.Layer(0.82, 1.0, self.body_radius_fn(0.003), kz=0.8, kth=0.9)
        # The leg openings are where the thighs cut the gusset.
        self.layer_mesh(briefs, self.rows_of([0.875, 0.90, 0.935, 0.96]), "linen",
                        lambda p: self.w_torso(p, hem=True), cap0=Vector((0, 0.024, 0.818)))
        self.outer = self.Layer(0.6, 1.48, self.body_radius_fn(0.004), kz=0.8, kth=0.6)

    def feet(self):
        """Bare feet, for the base body only."""
        L = Mesh()
        L.part = "skin_foot"
        chain = self.R.leg(1)
        x0 = self.R.h("foot_l").x
        rings = []
        for y, hw, top, key in self.stations(self.FOOT):
            rings.append(dict(c=Vector((x0, y, top * 0.42)), axis=(0, -1, 0), ref=(0, 0, 1),
                              rx=top * 0.42 + 0.004, ry=hw * 0.82, n=2.6))

        def w_foot(p):
            return chain.weights(Vector((x0, p.y, min(p.z, 0.05))))

        loft(L, rings, self.L["limb"], "skin", w_foot, cap0=True, cap1=True, dome=0.004)
        return L

    def sash(self):
        """A rust sash cinching the waist over the outermost layer, knotted
        at her left hip with two hanging ends."""
        M = self.M
        M.part = "gear"
        segs = self.L["torso"]
        outer = self.outer
        zs = (0.985, 0.998, 1.012, 1.024, 1.038, 1.052, 1.068)
        gaps = (0.003, 0.007, 0.009, 0.005, 0.009, 0.007, 0.003)
        P = [[outer.at(z, -pi / 2 + 2 * pi * j / segs, g) for j in range(segs)] for z, g in zip(zs, gaps)]
        self.mesh_rows(M, P, self.w_coat, "sash")
        knot = outer.at(1.025, -pi / 2 + 0.95, 0.012)
        kd = Vector((knot.x, knot.y - 0.01, 0)).normalized()
        kr = [dict(c=knot + kd * (0.006 * k - 0.004), axis=kd, ref=(0, 0, 1), rx=r, ry=r * 0.8)
              for k, r in enumerate((0.01, 0.022, 0.024, 0.012))]
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

    def strap(self):
        """The satchel's strap, from her left shoulder across her chest to
        her right hip and back up over her shoulder blade, laid on the
        outermost layer."""
        M = self.M
        M.part = "gear"
        steps = max(16, self.L["torso"])
        outer = self.outer
        top = Vector((0.095, 0.022, 1.47))
        guide = [(top, Vector((-0.17, -0.11, 0.985))), (Vector((-0.17, 0.12, 1.0)), top)]
        pts = []
        half = steps // 2
        for (a, b) in guide:
            for k in range(half):
                g = a.lerp(b, k / half)
                z = min(max(g.z, 0.97), 1.47)
                th = math.atan2(g.y - 0.005, g.x)
                # Across the coat's open front it lies on the shirt.
                front = abs(math.atan2(math.sin(th + pi / 2), math.cos(th + pi / 2)))
                layer = outer
                if self.outfit == "coat" and front < math.radians(self.opening(z)):
                    layer = self.shirt_layer
                p = layer.at(z, th, 0.005)
                if z > 1.40:
                    # Over the shoulder: no lower than the body there.
                    lift = smoothstep(1.40, 1.47, z) * 0.012
                    p = p + Vector((0, 0, lift))
                pts.append(p)
        # Smooth the path (the layers' fine grid shows as kinks otherwise),
        # then lay it back on the surface where smoothing sank it.
        for _ in range(3):
            pts = [pts[k] * 0.5 + (pts[k - 1] + pts[(k + 1) % len(pts)]) * 0.25 for k in range(len(pts))]
        for k, p in enumerate(pts):
            z = min(max(p.z, 0.97), 1.47)
            th = math.atan2(p.y - 0.005, p.x)
            front = abs(math.atan2(math.sin(th + pi / 2), math.cos(th + pi / 2)))
            layer = outer
            if self.outfit == "coat" and front < math.radians(self.opening(z)):
                layer = self.shirt_layer
            floor = layer.at(z, th, 0.006)
            c = Vector((0, 0.005, 0))
            hp, hf = Vector((p.x, p.y - c.y, 0)), Vector((floor.x, floor.y - c.y, 0))
            if hp.length < hf.length and hp.length > 1e-6:
                pts[k] = Vector((c.x, c.y, p.z)) + hp * (hf.length / hp.length)
        n = (guide[0][1] - top).cross(guide[1][0] - top).normalized()
        w, t = 0.018, 0.005
        P = []
        for k, p in enumerate(pts):
            out = Vector((p.x, p.y - 0.01, 0)).normalized()
            tangent = (pts[(k + 1) % len(pts)] - pts[k - 1]).normalized()
            side = tangent.cross(out).normalized()
            if side.dot(n) < 0:
                side = -side
            P.append([p + side * w, p + side * w + out * t, p - side * w + out * t, p - side * w])
        steps = len(pts)
        rows = [[P[k][c] for k in range(steps)] for c in range(4)]
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
        side = self.outer.at(0.935, pi, 0.0)
        x0 = side.x - 0.034
        c0 = Vector((x0, 0.005, 0.935))
        w = self.w_coat(Vector((x0 + 0.04, 0.0, 0.94)))
        segs = max(8, self.L["limb"])
        rings = []
        for dy, sc in ((-0.100, 0.70), (-0.094, 0.92), (-0.080, 1.0), (0.080, 1.0), (0.094, 0.92), (0.100, 0.70)):
            rings.append(dict(c=c0 + Vector((0, dy, 0)), axis=(0, 1, 0), ref=(1, 0, 0), rx=0.032 * sc,
                              ry=0.072 * sc, n=3.2))
        loft(M, rings, segs, "leather", lambda p: w, cap0=True, cap1=True)
        P = []
        for i, z in enumerate((0.985, 0.955, 0.925, 0.900)):
            row = [Vector((x0 - 0.035 - 0.004 * (1 - i / 3), y + 0.005, z)) for y in (-0.096, -0.04, 0.04, 0.096)]
            P.append(row)
        P.reverse()
        W = [[w for _ in r] for r in P]
        shell(M, [r[::-1] for r in P], W, 0.004, "leather_dark", "leather_dark", "leather_dark")
        bk = Vector((x0 - 0.040, 0.005, 0.905))
        rings = [dict(c=bk + Vector((d, 0, 0)), axis=(-1, 0, 0), ref=(0, 0, 1), rx=0.013, ry=0.011)
                 for d in (0.0, -0.005)]
        loft(M, rings, 6, "copper", lambda p: w, cap1=True)

    def clasps(self):
        """Two copper clasps on the coat's front edges at the chest, and one at
        the collar."""
        M = self.M
        M.part = "gear"
        for z in (1.22, 1.31):
            op = math.radians(self.opening(z) + 1)
            for s in (1, -1):
                th = -pi / 2 + s * op
                p = self.coat_layer.at(z, th, 0.007)
                d = Vector((cos(th), sin(th), 0)).normalized()
                rings = [dict(c=p + d * k, axis=d, ref=(0, 0, 1), rx=0.011, ry=0.011) for k in (0.0, 0.006)]
                loft(M, rings, 6, "copper", lambda q, p=p: self.w_coat(p), cap1=True)
        p = Vector((0, -0.058, 1.468))
        rings = [dict(c=p + Vector((0, -k, 0)), axis=(0, -1, 0), ref=(0, 0, 1), rx=0.012, ry=0.012) for k in (0, 0.007)]
        loft(M, rings, 6, "copper", lambda q: self.torso_chain.weights(p), cap1=True)

    def covered(self, tag, pts):
        """Whether the outfit hides a body face of part `tag` with the corners
        `pts` everywhere: a margin inside each garment's edges keeps the skin
        under its hems."""
        rule = OUTFITS[self.outfit]["covers"].get(tag)
        if rule is None:
            return False
        lo, hi, axis = rule
        return all(lo <= (abs(p.x) if axis == "x" else p.z) <= hi for p in pts)

    # The build, in three reusable parts --------------------------------------------------
    #
    # A later female player character reuses the base body and the head; the
    # outfit is Alice's own.

    def build_body(self):
        """The complete base body: torso, arms with hands, and legs, in skin."""
        self.torso_skin()
        arm = self.arm_skin()
        self.M.extend(arm)
        self.M.extend_mirrored(arm)
        leg = self.leg_skin()
        self.M.extend(leg)
        self.M.extend_mirrored(leg)

    def build_head(self, head, eyes):
        """The hair, fitted to the reshaped head `head` (a Blender object from
        `ubc_head`)."""
        from mathutils.bvhtree import BVHTree

        dg = bpy.context.evaluated_depsgraph_get()
        self.bvh = BVHTree.FromObject(head, dg)
        self.hair()

    def build_outfit(self):
        """One of OUTFITS over the base body, then the hidden skin removed."""
        outfit = self.outfit
        M = self.M
        if outfit == "base":
            self.underwear()
            feet = self.feet()
            M.extend(feet)
            M.extend_mirrored(feet)
            return
        if outfit == "summer":
            self.dress()
            boots = self.boots(0.17)
            M.extend(boots)
            M.extend_mirrored(boots)
            self.sash()
        else:
            arm = self.shirt(0.632 if outfit == "coat" else 0.37, "crew" if outfit == "coat" else "v")
            arm.extend(self.bracer(0.007 if outfit == "coat" else 0.0))
            M.extend(arm)
            M.extend_mirrored(arm)
            leg = self.trousers(True)
            leg.extend(self.boots(0.46))
            M.extend(leg)
            M.extend_mirrored(leg)
            if outfit == "coat":
                self.coat()
                self.hood()
            else:
                self.outer = self.Layer(0.85, 1.48, self.over(self.shirt_layer, 0.003), kz=0.9, kth=0.8)
            self.sash()
            if not self.L.get("lite"):
                self.strap()
                if outfit == "coat":
                    self.clasps()
            self.satchel()
        self.staff()

    def build(self, head, eyes):
        self.build_body()
        # The hair drapes over the outermost layer, so the outfit comes first.
        self.build_outfit()
        self.build_head(head, eyes)
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
        color = node_scale(nt, color, node_math(nt, "MULTIPLY_ADD", weave, 0.06, 0.97))
        wr = node_value(nt, "ShaderNodeTexWave", pos, 22.0, wave_type="BANDS", bands_direction="Z",
                        Distortion=5.0, Detail=3.0)
        color = node_scale(nt, color, node_math(nt, "MULTIPLY_ADD", wr, 0.10, 0.95))
    elif base in ("coat", "coat_inner", "coat_trim"):
        fuzz = node_value(nt, "ShaderNodeTexNoise", pos, 320.0, Detail=2.0)
        color = node_scale(nt, color, node_math(nt, "MULTIPLY_ADD", fuzz, 0.05, 0.975))
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
        color = node_scale(nt, color, node_math(nt, "MULTIPLY_ADD", grain, 0.06, 0.97))
        crease = node_value(nt, "ShaderNodeTexWave", pos, 70.0, wave_type="BANDS", bands_direction="Z",
                            Distortion=5.0, Detail=2.0)
        lines = node_math(nt, "SUBTRACT", 1.0, node_smooth(nt, crease, 0.0, 0.08))
        color = node_scale(nt, color, node_math(nt, "MULTIPLY_ADD", lines, -0.16, 1.0))
        color = node_toward(nt, color, node_math(nt, "MULTIPLY", edges, 0.45), linear("#C08A5C"))
    elif base == "leggings":
        knit = node_value(nt, "ShaderNodeTexWave", pos, 260.0, wave_type="BANDS", bands_direction="X")
        color = node_scale(nt, color, node_math(nt, "MULTIPLY_ADD", knit, 0.06, 0.97))
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
        "dress": ("noise", 0.05), "dress_trim": ("noise", 0.05), "skin": ("noise", 0.035),
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


def wardrobe(builds):
    """One mesh holding several outfits' meshes, (bit, mesh, covered): faces
    shared by outfits (the body, the trousers and boots the coat and the
    light outfit both wear) appear once, with every outfit's bit in its mask,
    so they share the atlas's texels; a body face an outfit covers isn't
    in that outfit."""
    U = Mesh()
    vertices, faces = {}, {}
    for bit, M, covered in builds:
        for k, f in enumerate(M.f):
            if covered and covered(M.tag[k], [M.v[i] for i in f]):
                continue
            ids = []
            for i in f:
                key = (round(M.v[i].x, 6), round(M.v[i].y, 6), round(M.v[i].z, 6))
                j = vertices.get(key)
                if j is None:
                    j = len(U.v)
                    U.v.append(M.v[i].copy())
                    U.w.append(dict(M.w[i]))
                    vertices[key] = j
                if j not in ids:
                    ids.append(j)
            if len(ids) < 3:
                # A sliver whose corners merged.
                continue
            key = tuple(sorted(ids))
            if key in faces:
                U.mask[faces[key]] |= bit
                continue
            faces[key] = len(U.f)
            U.f.append(tuple(ids))
            U.m.append(M.m[k])
            U.tag.append(M.tag[k])
            U.mask.append(bit)
            if M.uv:
                U.uv.append(M.uv[k][: len(ids)])
    return U


def outfit_copy(obj, bit):
    """A copy of `obj` with only the faces outfit `bit` wears (and the faces
    every outfit wears, whose mask is 0)."""
    o = obj.copy()
    o.data = obj.data.copy()
    bpy.context.scene.collection.objects.link(o)
    bm = bmesh.new()
    bm.from_mesh(o.data)
    layer = bm.faces.layers.int.get("outfits")
    if layer is not None:
        gone = [f for f in bm.faces if f[layer] and not (f[layer] & bit)]
        bmesh.ops.delete(bm, geom=gone, context="FACES")
        bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
    bm.to_mesh(o.data)
    bm.free()
    if "outfits" in o.data.attributes:
        o.data.attributes.remove(o.data.attributes["outfits"])
    return o


def to_object(M, arm):
    me = bpy.data.meshes.new("Alice")
    me.from_pydata([tuple(v) for v in M.v], [], [list(f) for f in M.f])
    me.update()
    assert not me.validate(verbose=False), "the mesh had invalid geometry"
    if M.uv:
        layer = me.uv_layers.new(name="hairuv")
        for poly, uv in zip(me.polygons, M.uv):
            for li, t in zip(poly.loop_indices, uv):
                layer.data[li].uv = t
    if M.mask and len(set(M.mask)) > 1:
        outfits = me.attributes.new("outfits", "INT", "FACE")
        outfits.data.foreach_set("value", M.mask)
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


def bake(obj, lod, bits=(1,)):
    """Bake color, ambient occlusion, and object-space normals; return the
    combined base-color image. With several outfits (`bits`), occlusion is
    baked for each outfit alone and each texel keeps the lightest of the
    outfits that wear it, so one outfit's garments don't shade another's
    skin."""
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
    passes = [("color", dict(type="DIFFUSE", pass_filter={"COLOR"})),
              ("normal", dict(type="NORMAL", normal_space="OBJECT")), ("mask", dict(type="EMIT"))]
    if len(bits) == 1:
        passes.insert(1, ("ao", dict(type="AO")))
    for kind, args in passes:
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
    px = {k: np.array(img.pixels[:], dtype=np.float32).reshape(size, size, 4) for k, img in images.items()
          if k != "ao" or len(bits) == 1}
    if len(bits) > 1:
        px["ao"] = outfit_occlusion(obj, bits, images["ao"], size)
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


def outfit_occlusion(obj, bits, image, size):
    """Occlusion baked for each outfit alone, each texel the lightest of the
    outfits that wear it (texels no outfit bakes stay open)."""
    import numpy as np

    seen = bpy.data.images.new("alice_seen", size, size, float_buffer=True)
    seen.colorspace_settings.name = "Non-Color"
    best = np.zeros((size, size, 4), dtype=np.float32)
    any_seen = np.zeros((size, size, 1), dtype=bool)
    obj.hide_render = True
    for bit in bits:
        part = outfit_copy(obj, bit)
        part.hide_render = False
        for kind, img, args in (("ao", image, dict(type="AO")), ("seen", seen, dict(type="EMIT"))):
            for slot in part.material_slots:
                nt = slot.material.node_tree
                bsdf = nt.nodes["Principled BSDF"]
                if kind == "seen":
                    bsdf.inputs["Emission Color"].default_value = (1, 1, 1, 1)
                    bsdf.inputs["Emission Strength"].default_value = 1.0
                node = nt.nodes.get("bake") or nt.nodes.new("ShaderNodeTexImage")
                node.name = "bake"
                node.image = img
                nt.nodes.active = node
            bpy.ops.object.select_all(action="DESELECT")
            part.select_set(True)
            bpy.context.view_layer.objects.active = part
            bpy.ops.object.bake(margin=max(2, size // 128), use_clear=True, **args)
        ao = np.array(image.pixels[:], dtype=np.float32).reshape(size, size, 4)
        mask = np.array(seen.pixels[:], dtype=np.float32).reshape(size, size, 4)[..., :1] > 0.5
        best = np.where(mask, np.maximum(best, ao), best)
        any_seen |= mask
        bpy.data.objects.remove(part, do_unlink=True)
    obj.hide_render = False
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    return np.where(any_seen, best, 1.0)


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
    outfit = None
    if "--outfit" in argv:
        i = argv.index("--outfit")
        outfit = argv[i + 1]
        argv = argv[:i] + argv[i + 2:]
    out_dir = argv[0] if argv and not argv[0].startswith("lod") else OUT
    variant = next((a for a in argv if a.startswith("lod")), "lod1")
    lod = LODS[variant]
    # The Everglade level carries every outfit on one atlas; the other
    # levels, a quick look, and `--outfit` build one.
    if outfit is None:
        outfits = list(WARDROBE) if variant == WARDROBE_LEVEL and not quick else ["coat"]
    else:
        assert outfit in OUTFITS, outfit
        outfits = [outfit]
    arm, joints = load_rig()
    head, eyes = ubc_head(lod)
    builds = []
    for o in outfits:
        alice = Alice(Rig(joints), lod, o)
        alice.build(head, eyes)
        builds.append(alice)
    bits = {o: 1 << k for k, o in enumerate(outfits)}
    M = wardrobe([(bits[a.outfit], a.M, a.covered) for a in builds])
    C = wardrobe([(bits[a.outfit], a.C, None) for a in builds])
    drop_ears(head)
    obj = to_object(M, arm)
    cards = to_object(C, arm)
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
    # Each outfit's triangles: its own faces and the ones every outfit wears.
    mask = [0] * len(obj.data.polygons)
    if "outfits" in obj.data.attributes:
        obj.data.attributes["outfits"].data.foreach_get("value", mask)
    tris = {o: sum(len(p.vertices) - 2 for p, m in zip(obj.data.polygons, mask) if not m or m & bits[o])
            for o in outfits}
    print("PARTS", variant, json.dumps(tris), json.dumps(parts))
    for o, n in tris.items():
        assert n <= lod["budget"], f"{variant} {o}: {n} triangles exceeds {lod['budget']}"
    if quick:
        stem = stem_of(outfits[0])
        quick_views(quick, f"{stem}.{variant}")
        bpy.ops.object.select_all(action="SELECT")
        bpy.ops.export_scene.gltf(filepath=os.path.join(quick, f"{stem}.{variant}.glb"), export_format="GLB",
                                  export_yup=True, export_apply=True, export_animations=False, export_skins=False,
                                  export_materials="NONE")
        print("QUICK", variant, tris[outfits[0]])
        return
    unwrap(obj, lod)
    image = bake(obj, lod, [bits[o] for o in outfits])
    finish(obj, image)
    os.makedirs(out_dir, exist_ok=True)
    for o in outfits:
        part = outfit_copy(obj, bits[o]) if len(outfits) > 1 else obj
        out = os.path.join(out_dir, f"{stem_of(o)}.{variant}.glb")
        bpy.ops.object.select_all(action="DESELECT")
        arm.select_set(True)
        part.select_set(True)
        bpy.ops.export_scene.gltf(filepath=out, export_format="GLB", export_yup=True, export_apply=True,
                                  use_selection=True, export_animations=False, export_skins=True,
                                  export_image_format="AUTO", export_texcoords=True, export_normals=True)
        if part is not obj:
            bpy.data.objects.remove(part, do_unlink=True)
        info = {"out": os.path.relpath(out, kit.REPO), "variant": variant, "outfit": o, "triangles": tris[o],
                "budget": lod["budget"], "texture": lod["tex"], "parts": parts,
                "bytes": os.path.getsize(out), "blender": bpy.app.version_string}
        print("MODEL", json.dumps(info))


def stem_of(outfit):
    """The file stem of an outfit's model: `alice` for the coat."""
    return "alice" if outfit == "coat" else f"alice-{outfit}"


main()
