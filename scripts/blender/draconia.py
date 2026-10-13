"""Draconia, a photoreal original lead for the Verse wizard-school film.

Run headless:
    Blender -b --factory-startup --python scripts/blender/draconia.py -- \
        [--out DIR] [--expr NAME[,NAME...]] [--tex 4096] [--preview DIR] [--skip-paint]

Builds her head and bust from the CC0 MakeHuman base mesh (draconia_mh.py):
the base's macro targets make a young woman, our own face targets give an
original face (not a copy of any person), and the 34 CC0 expression-unit
targets are the face's FACS-like blend shapes (see
docs/verse/draconia/metahuman-study.md for the architecture this adapts).
`draconia_paint.py` paints her skin and eyes from baked position maps,
`draconia_hair.py` grows the updo, and `draconia_cloth.py` cuts the uniform.

Writes `draconia.<expr>.glb` to `--out` (default
~/work/film-out/draconia/build). Materials carry `verse_shading` in their
glTF extras (skin, eye, cornea, hair, cloth), which Verse's imported
renderer reads as the shading model. `--preview DIR` renders a quick Cycles
check (an intermediate check, never a comparison render).

The frame: Blender +Z up, she faces -Y, her left is +X, 1 unit = 1 m, soles
at z = 0. The glTF exporter turns it into +Y up, facing +Z.
"""

import json
import math
import os
import random
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector
from mathutils.bvhtree import BVHTree

sys.path.insert(0, os.path.dirname(__file__))
import draconia_mh as mh  # noqa: E402

OUT = os.path.expanduser("~/work/film-out/draconia/build")

# Macro targets: female, caucasian, mostly "young" (25) with a little of
# "child" (11) for a late-teen face, average muscle, a little under average
# weight (MakeHuman's macro blend, our weights).
MACROS = [
    ("macrodetails/caucasian-female-young", 0.88),
    ("macrodetails/caucasian-female-child", 0.12),
    ("macrodetails/universal-female-young-averagemuscle-averageweight", 0.62),
    ("macrodetails/universal-female-young-averagemuscle-minweight", 0.26),
    ("macrodetails/universal-female-child-averagemuscle-averageweight", 0.12),
    ("macrodetails/height/female-young-averagemuscle-averageweight-maxheight", 0.25),
]

# Our face: an oval face with high cheekbones, a straight nose with a soft
# rounded tip, full lips with a defined Cupid's bow, large almond eyes, a
# soft small chin. Original weights, chosen by eye in preview; no person's
# likeness is the target.
FACE = [
    ("head/head-oval", 0.55),
    ("head/head-invertedtriangular", 0.25),
    ("head/head-age-decr", 0.35),
    ("head/head-fat-decr", 0.25),
    ("cheek/l-cheek-bones-incr", 0.45),
    ("cheek/r-cheek-bones-incr", 0.45),
    ("cheek/l-cheek-inner-decr", 0.25),
    ("cheek/r-cheek-inner-decr", 0.25),
    ("chin/chin-width-decr", 0.0),
    ("chin/chin-prominent-incr", 0.15),
    ("chin/chin-height-decr", 0.45),
    ("nose/nose-scale-horiz-decr", 0.5),
    ("nose/nose-point-width-decr", 0.1),
    ("nose/nose-nostrils-width-decr", 0.15),
    ("nose/nose-point-up", 0.25),
    ("nose/nose-hump-decr", 0.4),
    ("nose/nose-scale-vert-decr", 0.25),
    ("mouth/mouth-trans-up", 0.15),
    ("cheek/l-cheek-volume-incr", 0.25),
    ("cheek/r-cheek-volume-incr", 0.25),
    # Fitted to the reference's neutral frame (0:24): a broader, softer
    # lower face, high cheekbones, a defined bridge and a softer tip.
    ("nose/nose-scale-depth-incr", 0.2),
    ("cheek/l-cheek-bones-incr", 0.2),
    ("cheek/r-cheek-bones-incr", 0.2),
    ("forehead/forehead-scale-vert-decr", 0.3),
    ("eyebrows/eyebrows-trans-forward", 0.3),
    # Heavier upper lids: the fold sits lower over the eye.
    ("eyes/l-eye-eyefold-down", 0.45),
    ("eyes/r-eye-eyefold-down", 0.45),
    ("mouth/mouth-upperlip-volume-incr", 0.3),
    ("mouth/mouth-lowerlip-volume-incr", 0.4),
    ("mouth/mouth-cupidsbow-incr", 0.5),
    ("mouth/mouth-scale-horiz-incr", 0.18),
    ("mouth/mouth-angles-down", 0.1),
    ("eyes/l-eye-scale-incr", 0.25),
    ("eyes/r-eye-scale-incr", 0.25),
    ("eyes/l-eye-height2-incr", 0.15),
    ("eyes/r-eye-height2-incr", 0.15),
    ("eyes/l-eye-corner1-up", 0.15),
    ("eyes/r-eye-corner1-up", 0.15),
    ("eyes/l-eye-push1-in", 0.1),
    ("eyes/r-eye-push1-in", 0.1),
    ("neck/neck-scale-horiz-decr", 0.25),
    # Relaxed lids: the upper lid rests over the top of the iris.
    ("expression/units/caucasian/eye-left-closure", 0.2),
    ("expression/units/caucasian/eye-right-closure", 0.2),
]

# Expressions: named weights over the CC0 expression units (FACS-like), the
# way a DNA rig maps a GUI control to raw controls. `jaw` opens the jaw bone
# (radians) for the teeth and tongue, which the units don't move.
EXPRESSIONS = {
    "neutral": {},
    # Speaking, lips just parted (the reference barely opens her mouth).
    "speak": {"mouth-open": 0.09, "mouth-parling": 0.35, "eyebrows-left-inner-up": 0.12, "eyebrows-right-inner-up": 0.12, "jaw": 0.02},
    "surprise": {
        "mouth-open": 0.55,
        "eyebrows-left-up": 0.6,
        "eyebrows-right-up": 0.6,
        "eye-left-opened-up": 0.35,
        "eye-right-opened-up": 0.35,
        "jaw": 0.13,
    },
    "closed": {"eye-left-closure": 0.67, "eye-right-closure": 0.67},
    # A determined glare: brows lowered and drawn in (inner ends down,
    # outer a little up), upper lids tightened over the iris, the lower
    # lids raised a touch, nostrils flared slightly.
    # Lips just parted, breathing hard (D1's opening close-up).
    "parted": {"mouth-open": 0.16, "mouth-parling": 0.35, "jaw": 0.04, "eyebrows-left-inner-up": 0.15, "eyebrows-right-inner-up": 0.15},
    # A determined glare, at the strength the owner chose (peacock-open
    # v1): brows lowered with the outer ends lifted a little, lids
    # tightened but most of the iris showing, nostrils flared slightly.
    "glare": {
        "eyebrows-left-down": 1.15,
        "eyebrows-right-down": 1.15,
        "eyebrows-left-extern-up": 0.35,
        "eyebrows-right-extern-up": 0.35,
        "eye-left-slit": 0.3,
        "eye-right-slit": 0.3,
        "eye-left-closure": -0.3,
        "eye-right-closure": -0.3,
        "nose-left-dilatation": 0.25,
        "nose-right-dilatation": 0.25,
        "mouth-compression": 0.2,
    },
    # A gentle closed-mouth smile: corners drawn up and back, lips pressed
    # a little, cheeks raised (the lower lids lift), dimples.
    "smile": {
        "mouth-corner-puller": 0.6,
        "mouth-compression": 0.15,
        "mouth-elevation": 0.05,
        "eye-left-slit": 0.22,
        "eye-right-slit": 0.22,
        "mouth/mouth-dimples-in": 0.6,
        "cheek/l-cheek-volume-incr": 0.2,
        "cheek/r-cheek-volume-incr": 0.2,
    },
}

SCALE = 0.1  # MakeHuman decimetres to metres
# `updo` (the reference's loose messy updo) or `long` (waist-length, loose,
# for the battle and the peacock guardian).
HAIR_STYLE = "updo"
# Small gold hoops (the battle and peacock look); `--earrings`.
EARRINGS = False


def to_blender(c):
    return Vector((c[0] * SCALE, -c[2] * SCALE, c[1] * SCALE))


# ---------------------------------------------------------------- the rig ---


class Rig:
    """The CC0 default skeleton: joint positions are means of base-mesh
    vertices, and the CC0 weights bind it."""

    def __init__(self, base):
        with open(os.path.join(mh.MH_DATA, "rigs", "default.mhskel")) as f:
            self.skel = json.load(f)
        with open(os.path.join(mh.MH_DATA, "rigs", "default_weights.mhw")) as f:
            self.weights = json.load(f)["weights"]
        self.base = base

    def joint(self, name):
        vs = self.skel["joints"][name]
        co = self.base.co
        return [sum(co[i][k] for i in vs) / len(vs) for k in range(3)]

    def head(self, bone):
        return self.joint(bone + "____head")

    def tail(self, bone):
        return self.joint(bone + "____tail")

    def descendants(self, bone):
        out = {bone}
        changed = True
        while changed:
            changed = False
            for name, b in self.skel["bones"].items():
                if b["parent"] in out and name not in out:
                    out.add(name)
                    changed = True
        return out

    def weight_of(self, bones):
        w = {}
        for b in bones:
            for i, x in self.weights.get(b, []):
                w[i] = w.get(i, 0.0) + x
        return w


def rotate_about(co, pivot, axis, angle, weights):
    """Rotate vertices about `pivot` by `angle` x their weight (linear blend
    between rest and the full rotation, as skinning does)."""
    R = Matrix.Rotation(angle, 3, Vector(axis))
    p = Vector(pivot)
    for i, w in weights.items():
        if w <= 0:
            continue
        v = Vector(co[i])
        r = R @ (v - p) + p
        co[i] = list(v.lerp(r, min(w, 1.0)))


def from_blender(v):
    return [v.x / SCALE, v.z / SCALE, -v.y / SCALE]


def pose(base, expr):
    """Her rest pose: limbs turned to the Universal rig's rest directions
    (draconia_rig.to_rest), so every Universal clip plays on her; the jaw
    opens for the open-mouth expressions."""
    import draconia_rig

    rig = Rig(base)
    draconia_rig.to_rest(base, rig, to_blender, from_blender)
    jaw = EXPRESSIONS[expr].get("jaw", 0.0)
    if jaw:
        pivot = rig.head("jaw")
        w = rig.weight_of(rig.descendants("jaw"))
        rotate_about(base.co, pivot, (1, 0, 0), jaw, w)
    return rig


def build_base(expr):
    base = mh.Base()
    for rel, w in MACROS + FACE:
        base.apply(rel, w)
    for unit, w in EXPRESSIONS[expr].items():
        if unit == "jaw":
            continue
        # A unit name is a CC0 expression unit; a path is a shape target.
        base.apply(unit if "/" in unit else f"expression/units/caucasian/{unit}", w)
    rig = pose(base, expr)
    return base, rig


# ------------------------------------------------------------- the meshes ---


def mesh_from_faces(base, name, faces, shift, uv=True, weights=None):
    """A mesh of the base's `faces`, with its UVs and, given per-vertex
    `weights` ((joint, weight) lists), a vertex group per joint."""
    used = sorted({i for vs, _ in faces for i in vs})
    remap = {old: new for new, old in enumerate(used)}
    me = bpy.data.meshes.new(name)
    me.from_pydata(
        [to_blender(base.co[i]) + shift for i in used],
        [],
        [[remap[i] for i in vs] for vs, _ in faces],
    )
    if uv:
        layer = me.uv_layers.new(name="UVMap")
        k = 0
        for vs, ts in faces:
            for t in ts:
                layer.data[k].uv = base.uv[t] if t >= 0 else (0.0, 0.0)
                k += 1
    me.validate()
    obj = bpy.data.objects.new(name, me)
    bpy.context.collection.objects.link(obj)
    for p in me.polygons:
        p.use_smooth = True
    if weights is not None:
        groups = {}
        for old, new in remap.items():
            for joint, w in weights[old]:
                if joint not in groups:
                    groups[joint] = obj.vertex_groups.new(name=joint)
                groups[joint].add([new], w, "REPLACE")
    return obj, remap


class Landmarks:
    """Points on her face, in Blender metres, for painting and fitting."""

    def __init__(self, base, rig, shift):
        def J(name):
            return to_blender(base.centre(name)) + shift

        self.eye_l = J("helper-l-eye")
        self.eye_r = J("helper-r-eye")
        self.mouth = J("joint-mouth")
        self.head_top = to_blender(rig.tail("head")) + shift
        self.head_base = to_blender(rig.head("head")) + shift
        self.jaw = to_blender(rig.head("jaw")) + shift
        self.neck = to_blender(rig.head("neck01")) + shift
        self.chest = to_blender(rig.head("spine02")) + shift
        r = []
        for side in ("l", "r"):
            vs = base.group_vertices(f"helper-{side}-eye")
            pts = [to_blender(base.co[i]) + shift for i in vs]
            c = sum(pts, Vector()) / len(pts)
            r.append(sum((p - c).length for p in pts) / len(pts))
        self.eye_radius = sum(r) / 2

    def profile(self, skin):
        """The face's midline profile from the built skin: the nose tip
        (the frontmost point below the eyes) and the lips' meeting line
        (the deepest point between the two lip peaks), which the painting
        aims at."""
        eye_z = (self.eye_l.z + self.eye_r.z) / 2
        bins = {}
        for v in skin.data.vertices:
            c = v.co
            if abs(c.x) < 0.003 and eye_z - 0.11 < c.z < eye_z - 0.01 and c.y < self.eye_l.y + 0.02:
                k = round((c.z - eye_z) * 1000)
                if k not in bins or c.y < bins[k].y:
                    bins[k] = c.copy()
        ks = sorted(bins)
        tip = min((bins[k] for k in ks if -65 <= k <= -25), key=lambda c: c.y)
        below = [k for k in ks if (tip.z - eye_z) * 1000 - 50 <= k <= (tip.z - eye_z) * 1000 - 10]
        # Lip peaks: local frontmost points; the stomion: the deepest between.
        ys = [bins[k].y for k in below]
        peaks = [i for i in range(1, len(ys) - 1) if ys[i] <= ys[i - 1] and ys[i] <= ys[i + 1]]
        if len(peaks) >= 2:
            lo, hi = sorted(sorted(peaks, key=lambda i: ys[i])[:2])
            mid = max(range(lo, hi + 1), key=lambda i: ys[i])
            self.mouth = bins[below[mid]].copy()
        self.nose_tip = tip.copy()
        print("PROFILE nose", tuple(round(x, 4) for x in tip), "mouth", tuple(round(x, 4) for x in self.mouth))

    def as_dict(self):
        return {k: list(v) if isinstance(v, Vector) else v for k, v in self.__dict__.items()}


def keep_skin(base, rig, landmarks, shift):
    """The skin faces she shows in the uniform: the head, neck, the top of
    the chest inside the collar, and the hands. Everything the uniform hides
    is cut, which costs nothing and keeps the subdivided mesh small."""
    head_w = rig.weight_of(rig.descendants("neck01"))
    hand_w = {}
    for side in ("L", "R"):
        for i, w in rig.weight_of(rig.descendants(f"wrist.{side}")).items():
            hand_w[i] = hand_w.get(i, 0) + w
    collar_z = landmarks.neck.z - 0.07
    faces = []
    for vs, ts, g in base.faces:
        if g != "body":
            continue
        zs = [to_blender(base.co[i]).z + shift.z for i in vs]
        hw = sum(head_w.get(i, 0) for i in vs) / len(vs)
        aw = sum(hand_w.get(i, 0) for i in vs) / len(vs)
        if hw > 0.02 or aw > 0.02 or min(zs) > collar_z:
            faces.append((vs, ts))
    return faces


def lip_mask(obj, remap):
    """The vermilion of the lips as a per-vertex attribute `lipmask`: how
    far the CC0 lip-volume targets move each vertex, normalized, so the
    lips' colour sits exactly on the lips' shape."""
    reach = {}
    for rel in ("mouth/mouth-upperlip-volume-incr", "mouth/mouth-lowerlip-volume-incr"):
        for i, d in mh.read_target(rel):
            m = (d[0] ** 2 + d[1] ** 2 + d[2] ** 2) ** 0.5
            reach[i] = max(reach.get(i, 0.0), m)
    top = max(reach.values()) if reach else 1.0
    attr = obj.data.attributes.new("lipmask", "FLOAT_COLOR", "POINT")
    for old, new in remap.items():
        v = min(reach.get(old, 0.0) / top * 1.6, 1.0)
        attr.data[new].color = (v, v, v, 1.0)


def subdivide(obj, levels):
    mod = obj.modifiers.new("sub", "SUBSURF")
    mod.levels = levels
    mod.render_levels = levels
    mod.uv_smooth = "PRESERVE_BOUNDARIES"
    bpy.context.view_layer.objects.active = obj
    bpy.ops.object.modifier_apply(modifier=mod.name)


def unwrap(obj, margin=0.004):
    """A fresh UV layout for the visible skin, so the face gets most of the
    texels: the head and neck by angle-based unwrapping with seams the
    script places, then everything packed."""
    bpy.context.view_layer.objects.active = obj
    for o in bpy.context.selected_objects:
        o.select_set(False)
    obj.select_set(True)
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.uv.smart_project(angle_limit=math.radians(66), island_margin=margin, area_weight=0.0, scale_to_bounds=False)
    bpy.ops.object.mode_set(mode="OBJECT")


# ------------------------------------------------------------------ eyes ---


def eyeball(name, centre, radius, look):
    """An eyeball (sclera and iris on one sphere) and a cornea shell.

    The eyeball's UV is an azimuthal projection around its front pole, so
    the iris is a disc at UV (0.5, 0.5) with radius 0.5 x (iris angle / pi).
    The cornea is a clear dome over the iris, bulging 2.6 mm past the
    sclera, the way a real cornea does; the renderer refracts through it.
    """
    bm = bmesh.new()
    rings, segs = 48, 64
    front = Vector(look).normalized()
    basis = front.to_track_quat("Y", "Z").to_matrix()  # local +Y = front
    verts = []
    for i in range(rings + 1):
        theta = math.pi * i / rings  # 0 at the front pole
        row = []
        for j in range(segs):
            phi = 2 * math.pi * j / segs
            local = Vector((math.sin(theta) * math.cos(phi), math.cos(theta), math.sin(theta) * math.sin(phi)))
            v = bm.verts.new(centre + basis @ (local * radius))
            row.append((v, theta, phi))
        verts.append(row)
    uv = bm.loops.layers.uv.new("UVMap")
    for i in range(rings):
        for j in range(segs):
            a, b = verts[i][j], verts[i][(j + 1) % segs]
            c, d = verts[i + 1][(j + 1) % segs], verts[i + 1][j]
            quad = [a, d, c, b]
            try:
                f = bm.faces.new([q[0] for q in quad])
            except ValueError:
                continue
            for loop, (v, theta, phi) in zip(f.loops, quad):
                r = 0.5 * theta / math.pi
                loop[uv].uv = (0.5 + r * math.cos(phi), 0.5 + r * math.sin(phi))
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-7)
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    for p in me.polygons:
        p.use_smooth = True
    obj = bpy.data.objects.new(name, me)
    bpy.context.collection.objects.link(obj)

    # Cornea: a dome over the front, its edge on the sclera at the limbus
    # (iris radius 5.9 mm), bulging 2.6 mm.
    bm = bmesh.new()
    limbus = 0.0059
    edge_theta = math.asin(min(limbus / radius, 0.99))
    rows, segs = 14, 64
    uvl = bm.loops.layers.uv.new("UVMap")
    grid = []
    for i in range(rows + 1):
        t = i / rows  # 0 apex, 1 edge
        theta = edge_theta * t
        row = []
        for j in range(segs):
            phi = 2 * math.pi * j / segs
            local = Vector((math.sin(theta) * math.cos(phi), math.cos(theta), math.sin(theta) * math.sin(phi))) * radius
            bulge = 0.0026 * (1 - t * t) + 0.00012
            local += Vector((0, 1, 0)) * bulge
            row.append(bm.verts.new(centre + basis @ local))
        grid.append(row)
    for i in range(rows):
        for j in range(segs):
            q = [grid[i][j], grid[i + 1][j], grid[i + 1][(j + 1) % segs], grid[i][(j + 1) % segs]]
            try:
                f = bm.faces.new(q)
            except ValueError:
                continue
            for loop in f.loops:
                loc = basis.inverted() @ (loop.vert.co - centre)
                loop[uvl].uv = (0.5 + loc.x / (2 * limbus), 0.5 + loc.z / (2 * limbus))
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-7)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    me = bpy.data.meshes.new(name + "_cornea")
    bm.to_mesh(me)
    bm.free()
    for p in me.polygons:
        p.use_smooth = True
    cornea = bpy.data.objects.new(name + "_cornea", me)
    bpy.context.collection.objects.link(cornea)
    return obj, cornea


def eye_openings(skin, base, shift, landmarks):
    """The lid margins of each eye, from the CC0 lash helper shells: each
    shell is a strip whose edge nearest the eye is the margin. Points are
    snapped onto the (subdivided) skin and ordered around the eye."""
    bvh = BVHTree.FromObject(skin, bpy.context.evaluated_depsgraph_get())
    out = {}
    for side, c in (("l", landmarks.eye_l), ("r", landmarks.eye_r)):
        pts = []
        for shell in (1, 2):
            vs = base.group_vertices(f"helper-{side}-eyelashes-{shell}")
            ps = [to_blender(base.co[i]) + shift for i in vs]
            ps.sort(key=lambda p: (p - c).length)
            pts += ps[: len(ps) // 2]
        snapped = []
        for p in pts:
            loc, _, _, _ = bvh.find_nearest(p)
            snapped.append(loc if loc is not None else p)
        out[side] = sorted(snapped, key=lambda p: math.atan2(p.z - c.z, p.x - c.x))
    return out


def lashes(name, opening, centre, side_sign, upper=True, count=90, seed=1):
    """Eyelashes as tapered curved ribbons from the lid margin: upper lashes
    longer (8-11 mm), denser and curling up; lower ones short and sparse."""
    rng = random.Random(seed)
    lid = [p for p in opening if (p.z > centre.z) == upper]
    lid.sort(key=lambda p: p.x)
    if len(lid) < 3:
        return None
    # Arc-length parametrisation of the lid margin.
    acc = [0.0]
    for a, b in zip(lid, lid[1:]):
        acc.append(acc[-1] + (b - a).length)
    total = acc[-1]

    def at(s):
        s *= total
        for k in range(len(lid) - 1):
            if acc[k + 1] >= s:
                t = (s - acc[k]) / max(acc[k + 1] - acc[k], 1e-9)
                return lid[k].lerp(lid[k + 1], t)
        return lid[-1]

    bm = bmesh.new()
    uv = bm.loops.layers.uv.new("UVMap")
    for n in range(count):
        s = rng.uniform(0.04, 0.97)
        root = at(s)
        # Outer third longest (toward her temple: +x on her left).
        outer = s if (side_sign > 0) else 1 - s
        if upper:
            # Heavy upper lashes whose outer third sweeps out into a wing.
            length = (0.0058 + 0.0048 * outer**1.5) * rng.uniform(0.75, 1.1)
        else:
            length = (0.0018 + 0.0018 * outer) * rng.uniform(0.7, 1.1)
        radial = (root - centre)
        radial.y = 0
        radial = radial.normalized()
        fwd = Vector((0, -1, 0))
        up = Vector((0, 0, 1 if upper else -1))
        segs = 6
        prev = None
        width0 = (0.00009 if upper else 0.00006) * rng.uniform(0.8, 1.2)
        tilt = rng.uniform(-0.25, 0.25)
        for k in range(segs + 1):
            t = k / segs
            # Out of the lid, then curling up (or down) and out.
            d = fwd * (0.75 - 0.55 * t) + up * (0.35 + 0.9 * t) + radial * (0.25 + tilt)
            p = root + d.normalized() * 0  # placeholder for clarity
            wing = 0.9 * outer**2 if upper else 0.0
            p = root + (fwd * (0.75 * t - 0.25 * t * t) + up * (0.25 * t + 0.6 * t * t) + radial * (0.3 + tilt + wing) * t) * length
            side = fwd.cross(up).normalized() if abs(fwd.cross(up).length) > 0 else Vector((1, 0, 0))
            w = width0 * (1 - 0.85 * t)
            a = bm.verts.new(p - side * w)
            b = bm.verts.new(p + side * w)
            if prev:
                f = bm.faces.new([prev[0], prev[1], b, a])
                for loop, (u, v) in zip(f.loops, [(0, t - 1 / segs), (1, t - 1 / segs), (1, t), (0, t)]):
                    loop[uv].uv = (u, v)
            prev = (a, b)
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    obj = bpy.data.objects.new(name, me)
    bpy.context.collection.objects.link(obj)
    return obj


def brows(skin, marks, eyes, count=650, rest=None):
    import draconia_paint

    if draconia_paint.BROWS == "pale":
        count = 380
    """Each brow as ~650 short strands (5-9 mm, 0.06 mm wide, tapering)
    rooted on the skin and lying close along it, rising at the inner head
    and combed outward along the arch, darker ash brown (hair shading),
    over the painted brow that fills between them."""
    rest = rest or skin
    bvh = BVHTree.FromObject(rest, bpy.context.evaluated_depsgraph_get())
    # Strands are placed on the rest face, then carried by each root's
    # nearest vertex to where the expression moved it.
    from mathutils.kdtree import KDTree

    tree = KDTree(len(rest.data.vertices))
    for v in rest.data.vertices:
        tree.insert(v.co, v.index)
    tree.balance()
    moved = [sv.co - rv.co for sv, rv in zip(skin.data.vertices, rest.data.vertices)]
    rng = random.Random(17)
    bm = bmesh.new()
    uvl = bm.loops.layers.uv.new("UVMap")
    for side, sign in (("l", 1), ("r", -1)):
        e = eyes[side][2]
        inner = e + Vector((-sign * 0.016, -0.02, 0.016))
        for n in range(count):
            s = rng.random() ** 0.9
            bx = inner.x + sign * 0.044 * s
            bz = inner.z + 0.009 * math.sin(min(s / 0.68, 1.0) * math.pi / 2) - 0.007 * max(s - 0.68, 0) / 0.32
            thick = 0.0062 * (1 - 0.6 * s) + 0.0016
            bz += rng.uniform(-0.5, 0.5) * thick
            loc, nrm, _, _ = bvh.ray_cast(Vector((bx, e.y - 0.05, bz)), Vector((0, 1, 0)))
            if loc is None:
                continue
            shift_v = moved[tree.find(loc)[1]]
            ang = math.radians(75 - 65 * min(s / 0.45, 1)) if s < 0.45 else math.radians(10 - 30 * (s - 0.45))
            ang += rng.uniform(-0.25, 0.25)
            d = Vector((sign * math.cos(ang), 0, math.sin(ang)))
            d = (d - nrm * d.dot(nrm)).normalized()
            length = rng.uniform(0.005, 0.009) * (1.1 - 0.3 * s)
            prev = None
            segs = 4
            for k in range(segs + 1):
                t = k / segs
                p = loc + nrm * (0.00015 + 0.0005 * t) + d * length * t
                q, qn, _, _ = bvh.find_nearest(p)
                if q is not None and (p - q).dot(qn) < 0.00012:
                    p = q + qn * 0.00012
                side_v = d.cross(nrm).normalized() * 0.00003 * (1 - 0.7 * t)
                p = p + shift_v
                a, b = bm.verts.new(p - side_v), bm.verts.new(p + side_v)
                if prev:
                    f = bm.faces.new([prev[0], prev[1], b, a])
                    for loop, uv in zip(f.loops, [(0, t), (1, t), (1, t), (0, t)]):
                        loop[uvl].uv = uv
                prev = (a, b)
    me = bpy.data.meshes.new("brows")
    bm.to_mesh(me)
    bm.free()
    o = bpy.data.objects.new("brows", me)
    bpy.context.collection.objects.link(o)
    import draconia_paint

    col = (0.30, 0.26, 0.19, 1) if draconia_paint.BROWS == "pale" else (0.12, 0.09, 0.065, 1)
    o.data.materials.append(material("draconia_brow", "hair", color=col, rough=0.5))
    return [o]


def mouth_cavity(marks):
    """An ellipsoid shell facing inward behind the lips: a dark, reddish
    oral cavity (MetaHuman's mouth interior, much simplified)."""
    m = marks.mouth
    bpy.ops.mesh.primitive_uv_sphere_add(radius=1.0, segments=32, ring_count=16, location=m + Vector((0, 0.028, -0.004)))
    o = bpy.context.object
    o.name = "mouth_cavity"
    o.scale = (0.022, 0.026, 0.016)
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    bpy.ops.object.mode_set(mode="EDIT")
    bpy.ops.mesh.select_all(action="SELECT")
    bpy.ops.mesh.flip_normals()
    bpy.ops.object.mode_set(mode="OBJECT")
    for p in o.data.polygons:
        p.use_smooth = True
    o.data.materials.append(material("draconia_mouth", "skin", color=(0.09, 0.025, 0.025, 1), rough=0.5))
    return o


def earrings(skin, marks):
    """Small gold hoops (17 mm across, a 2.2 mm wire) through each earlobe:
    the lowest point of the ear's outer rim."""
    eye_z = (marks.eye_l.z + marks.eye_r.z) / 2
    eye_y = marks.eye_l.y
    out = []
    gold = bpy.data.materials.get("draconia_gold") or material("draconia_gold", "standard", color=(1.0, 0.72, 0.32, 1), rough=0.22)
    # Half metallic: Verse has no environment reflections yet, so a fully
    # metallic hoop would show only its highlights.
    gold.node_tree.nodes["Principled BSDF"].inputs["Metallic"].default_value = 0.55
    for side in (1, -1):
        ear = [v.co for v in skin.data.vertices if v.co.x * side > 0.05 and eye_z - 0.08 < v.co.z < eye_z + 0.02 and eye_y + 0.04 < v.co.y < eye_y + 0.16]
        if not ear:
            continue
        xmax = max(abs(p.x) for p in ear)
        rim = [p for p in ear if abs(p.x) > xmax - 0.012]
        lobe = min(rim, key=lambda p: p.z)
        bpy.ops.mesh.primitive_torus_add(major_radius=0.0085, minor_radius=0.0011, major_segments=48, minor_segments=12, location=lobe + Vector((side * 0.0015, 0.0, -0.0075)), rotation=(0, math.pi / 2, 0))
        hoop = bpy.context.object
        hoop.name = f"earring_{'l' if side > 0 else 'r'}"
        bpy.ops.object.transform_apply(location=False, rotation=True, scale=True)
        for p in hoop.data.polygons:
            p.use_smooth = True
        hoop.data.materials.append(gold)
        out.append(hoop)
    return out


def wet_eye(side, sign, opening, centre):
    """A tear line, a 0.7 mm wet strip lying on the lower lid's margin
    against the eye (drawn as a cornea: specular only, so it reads as a
    glint of water), and the caruncle, a small pink, glossy mound in the
    inner corner."""
    lower = sorted([p for p in opening if p.z < centre.z + 0.0005], key=lambda p: p.x)
    out = []
    if len(lower) >= 3:
        bm = bmesh.new()
        uvl = bm.loops.layers.uv.new("UVMap")
        prev = None
        for i, p in enumerate(lower):
            toward = (centre - p).normalized()
            a = bm.verts.new(p + toward * 0.0003)
            b = bm.verts.new(p + toward * 0.0003 + Vector((0, 0, 0.0007)) - toward * 0.0002)
            if prev:
                f = bm.faces.new([prev[0], prev[1], b, a])
                for loop, uv in zip(f.loops, [(0.5, 0.45), (0.5, 0.45), (0.5, 0.45), (0.5, 0.45)]):
                    loop[uvl].uv = uv
            prev = (a, b)
        me = bpy.data.meshes.new(f"tearline_{side}")
        bm.to_mesh(me)
        bm.free()
        for poly in me.polygons:
            poly.use_smooth = True
        o = bpy.data.objects.new(f"tearline_{side}", me)
        bpy.context.collection.objects.link(o)
        o.data.materials.append(bpy.data.materials.get("draconia_cornea") or material("draconia_cornea", "cornea", color=(1, 1, 1, 1), rough=1.0, alpha="BLEND"))
        out.append(o)
    # The inner corner: the margin point nearest the nose.
    inner = min(opening, key=lambda p: abs(p.x))
    c = inner + (centre - inner).normalized() * 0.0012 + Vector((0, 0, -0.0003))
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.0012, segments=16, ring_count=10, location=c)
    car = bpy.context.object
    car.name = f"caruncle_{side}"
    car.scale = (0.7, 0.5, 1.2)
    bpy.ops.object.transform_apply(scale=True)
    for p in car.data.polygons:
        p.use_smooth = True
    m = bpy.data.materials.get("draconia_caruncle") or material("draconia_caruncle", "skin", color=(0.60, 0.40, 0.38, 1), rough=0.25)
    car.data.materials.append(m)
    out.append(car)
    return out


# ---------------------------------------------------------------- export ---


def material(name, shading, color=(0.8, 0.8, 0.8, 1), rough=0.5, images=None, alpha=None, extras=None):
    """A glTF-exportable material whose extras name its Verse shading model."""
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    nt = m.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = color
    bsdf.inputs["Roughness"].default_value = rough
    images = images or {}
    if "base" in images:
        tex = nt.nodes.new("ShaderNodeTexImage")
        tex.image = images["base"]
        nt.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
        if alpha:
            nt.links.new(tex.outputs["Alpha"], bsdf.inputs["Alpha"])
    if "normal" in images:
        tex = nt.nodes.new("ShaderNodeTexImage")
        tex.image = images["normal"]
        tex.image.colorspace_settings.name = "Non-Color"
        nm = nt.nodes.new("ShaderNodeNormalMap")
        nt.links.new(tex.outputs["Color"], nm.inputs["Color"])
        nt.links.new(nm.outputs["Normal"], bsdf.inputs["Normal"])
    if "orm" in images:
        tex = nt.nodes.new("ShaderNodeTexImage")
        tex.image = images["orm"]
        tex.image.colorspace_settings.name = "Non-Color"
        sep = nt.nodes.new("ShaderNodeSeparateColor")
        nt.links.new(tex.outputs["Color"], sep.inputs["Color"])
        nt.links.new(sep.outputs["Green"], bsdf.inputs["Roughness"])
        nt.links.new(sep.outputs["Blue"], bsdf.inputs["Metallic"])
    if alpha == "MASK":
        m.blend_method = "CLIP" if hasattr(m, "blend_method") else None
        m.alpha_threshold = 0.5
    elif alpha == "BLEND":
        m.blend_method = "BLEND"
    if alpha == "MASK":
        try:
            m.surface_render_method = "DITHERED"
        except AttributeError:
            pass
    m["verse_shading"] = shading
    for k, v in (extras or {}).items():
        m[k] = v
    return m


def export(path, objects):
    # Every part's own transform goes into its vertices: the skin ignores
    # node transforms (draconia_rig.skin_glb).
    for o in bpy.context.scene.objects:
        o.select_set(False)
    for o in objects:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    for o in bpy.context.scene.objects:
        o.select_set(False)
    for o in objects:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    os.makedirs(os.path.dirname(path), exist_ok=True)
    bpy.ops.export_scene.gltf(
        filepath=path,
        export_format="GLB",
        use_selection=True,
        export_apply=True,
        export_extras=True,
        export_yup=True,
        export_image_format="AUTO",
        export_materials="EXPORT",
        export_animations=False,
        export_skins=False,
        export_attributes=True,
    )
    patch_glb(path)
    tris = sum(sum(len(p.vertices) - 2 for p in o.data.polygons) for o in objects if o.type == "MESH")
    print(f"MODEL {os.path.basename(path)} {tris}")


def patch_glb(path):
    """Settles each material's glTF alpha mode from its Verse shading, which
    Blender's exporter can't express: hair is a cutout (Verse makes its
    coverage stochastic), a cornea is opaque in glTF terms (Verse draws it
    additively, specular only), and `verse_sheen` becomes the cloth's
    shading parameters."""
    import struct

    with open(path, "rb") as f:
        data = f.read()
    n = struct.unpack("<I", data[12:16])[0]
    doc = json.loads(data[20 : 20 + n])
    rest = data[20 + n :]
    for m in doc.get("materials", []):
        ex = m.get("extras", {})
        kind = ex.get("verse_shading")
        pbr = m.setdefault("pbrMetallicRoughness", {})
        if kind == "hair":
            m["alphaMode"] = "MASK"
            m["alphaCutoff"] = 0.5
            m["doubleSided"] = True
        elif kind == "cornea":
            m.pop("alphaMode", None)
            m.pop("alphaCutoff", None)
            pbr["baseColorFactor"] = [1, 1, 1, 1]
        if kind in ("eye", "cornea") and "metallicRoughnessTexture" in pbr:
            m["occlusionTexture"] = {"index": pbr["metallicRoughnessTexture"]["index"], "strength": 1.0}
        if kind == "skin" and m.get("name") == "draconia_skin" and "metallicRoughnessTexture" in pbr:
            # The skin's ORM carries baked occlusion in R.
            m["occlusionTexture"] = {"index": pbr["metallicRoughnessTexture"]["index"], "strength": 1.0}
        if kind == "skin" and "verse_shading_params" not in ex and m.get("name") == "draconia_skin":
            # Skin: moderate scattering, lobe mix 0.85, a matte 0.3 specular
            # weight (an oily sheen, not wet plastic).
            ex["verse_shading_params"] = [0.7, 0.85, 0.3, 0.0]
        if kind == "cloth" and "verse_sheen" in ex:
            ex["verse_shading_params"] = [ex.pop("verse_sheen"), ex.pop("verse_sheen_roughness", 0.5), 0, 0]
    body = json.dumps(doc, separators=(",", ":")).encode()
    body += b" " * ((4 - len(body) % 4) % 4)
    chunk = struct.pack("<I", len(body)) + b"JSON" + body
    total = 12 + len(chunk) + len(rest)
    with open(path, "wb") as f:
        f.write(data[:8] + struct.pack("<I", total) + chunk + rest)


# ----------------------------------------------------------------- build ---


def build(expr, tex, out, skip_paint=False):
    import draconia_cloth
    import draconia_hair
    import draconia_paint

    import draconia_rig

    base, rig = build_base(expr)
    feet = min(c[1] for c in base.co) * SCALE
    shift = Vector((0, 0, -feet))
    marks = Landmarks(base, rig, shift)
    weights = draconia_rig.weights(base, rig)

    # The full posed body, unsubdivided: the cloth fits over it.
    body_faces = [(vs, ts) for vs, ts, g in base.faces if g == "body"]
    body, _ = mesh_from_faces(base, "body_fit", body_faces, shift, weights=weights)

    skin_faces = keep_skin(base, rig, marks, shift)
    skin, skin_map = mesh_from_faces(base, "skin", skin_faces, shift, weights=weights)
    lip_mask(skin, skin_map)
    unwrap(skin)
    subdivide(skin, 2)
    # The neutral face with this expression's UVs: skin maps are painted
    # where features sit at rest, so the paint (brows, freckles, creases)
    # moves with the skin when an expression pulls it.
    rest_skin = skin
    if expr != "neutral":
        nbase, _ = build_base("neutral")
        nskin, _ = mesh_from_faces(nbase, "skin_neutral", skin_faces, shift)
        subdivide(nskin, 2)
        rest_skin = skin.copy()
        rest_skin.data = skin.data.copy()
        rest_skin.name = "skin_rest"
        bpy.context.collection.objects.link(rest_skin)
        for v, nv in zip(rest_skin.data.vertices, nskin.data.vertices):
            v.co = nv.co
        bpy.data.objects.remove(nskin)

    # Eyes: centred on the helper spheres, an anatomical 12 mm radius, the
    # front of the cornea where the helper's front was, verging slightly.
    objs = [skin]
    eyes = {}
    openings = eye_openings(skin, base, shift, marks)
    for side, c, sign in (("l", marks.eye_l, 1), ("r", marks.eye_r, -1)):
        r = 0.0119
        # Seat the ball so the sclera clears the lid margin by 0.4 mm: the
        # nearest margin point sits exactly r + 0.4 mm from the centre.
        lo, hi = -0.01, 0.02
        for _ in range(40):
            mid_off = (lo + hi) / 2
            centre = c + Vector((0, mid_off, 0))
            near = min((b - centre).length for b in openings[side])
            if near < r + 0.0004:
                lo = mid_off
            else:
                hi = mid_off
        centre = c + Vector((0, hi, 0))
        look = Vector((-sign * 0.03, -1, -0.02))
        ball, cornea = eyeball(f"eyeball_{side}", centre, r, look)
        # Each eye turns on its own joint (`eye_l`, `eye_r`, children of
        # Head at the eyeball's centre), so a clip can aim her gaze.
        for o in (ball, cornea):
            g = o.vertex_groups.new(name=f"eye_{side}")
            g.add(list(range(len(o.data.vertices))), 1.0, "REPLACE")
        eyes[side] = (ball, cornea, centre)
        objs += [ball, cornea]
        print("EYE", side, "seat offset mm", round(hi * 1000, 2))
    lash_objs = []
    for side, sign in (("l", 1), ("r", -1)):
        centre = eyes[side][2]
        for upper, count, seed in ((True, 160, 3 if sign > 0 else 4), (False, 45, 5 if sign > 0 else 6)):
            o = lashes(f"lash_{side}_{'u' if upper else 'd'}", openings[side], centre, sign, upper, count, seed)
            if o:
                lash_objs.append(o)

    # Brows as hair: strands rooted in the skin, lying along the brow's
    # growth (rising at the head, then along the arch to the tail).
    objs += brows(skin, marks, eyes, rest=rest_skin)

    # The wet line along each lower lid and the caruncle in each inner
    # corner (MetaHuman's tear-line and caruncle pieces, as geometry).
    wet = []
    for side, sign in (("l", 1), ("r", -1)):
        centre = eyes[side][2]
        wet += wet_eye(side, sign, openings[side], centre)
    objs += wet

    # The mouth's interior: a dark, soft cavity behind the lips, so parted
    # lips show depth, not the head's inside.
    objs.append(mouth_cavity(marks))

    # Teeth and tongue from the CC0 helpers, for the open-mouth expressions.
    mouth = []
    # (The CC0 tongue helper sits forward of the lips once the jaw opens;
    # it is left out until the mouth has its own interior.)
    for g in ("helper-upper-teeth", "helper-lower-teeth"):
        faces = [(vs, ts) for vs, ts, gg in base.faces if gg == g]
        o, _ = mesh_from_faces(base, g.replace("helper-", ""), faces, shift, weights=weights)
        subdivide(o, 1)
        # The CC0 teeth sit a little proud of the lips at the corners: set
        # them 2.5 mm back and narrow the arch 8 %.
        for v in o.data.vertices:
            v.co.y += 0.0025
            v.co.x = marks.mouth.x + (v.co.x - marks.mouth.x) * 0.92
        mouth.append(o)

    hair_objs, hair_joints = draconia_hair.grow(skin, marks, seed=7, style=HAIR_STYLE)
    hair_joints = [(f"eye_{side}", "Head", eyes[side][2].copy()) for side in ("l", "r")] + hair_joints
    if EARRINGS:
        objs += earrings(skin, marks)
    cloth_objs = draconia_cloth.build(body, marks)
    # Shells carry the body's own weights; the pieces built from curves
    # (collar, tie, trim, hood) take the nearest body vertices'.
    transfer_weights(body, [o for o in cloth_objs if not o.vertex_groups])

    marks.profile(skin)
    draconia_paint.EXPRESSION = expr
    if not skip_paint:
        maps = draconia_paint.paint(rest_skin, eyes, marks, tex, out)
    else:
        maps = {}
    # Materials.
    skin_mat = material(
        "draconia_skin",
        "skin",
        color=(0.78, 0.6, 0.52, 1),
        rough=0.5,
        images=maps.get("skin", {}),
    )
    skin.data.materials.append(skin_mat)
    eye_mat = material("draconia_eye", "eye", color=(0.8, 0.8, 0.8, 1), rough=0.3, images={k: v for k, v in maps.get("eye", {}).items() if k in ("base", "orm")})
    cornea_images = {"orm": maps["eye"]["cornea_orm"]} if "eye" in maps else {}
    cornea_mat = material("draconia_cornea", "cornea", color=(1, 1, 1, 1), rough=1.0, alpha="BLEND", images=cornea_images)
    # Clear in Blender's own preview; Verse draws a cornea as specular only.
    cornea_mat.node_tree.nodes["Principled BSDF"].inputs["Alpha"].default_value = 0.0
    for side in ("l", "r"):
        eyes[side][0].data.materials.append(eye_mat)
        eyes[side][1].data.materials.append(cornea_mat)
    lash_mat = material("draconia_lash", "hair", color=(0.035, 0.028, 0.022, 1), rough=0.45)
    for o in lash_objs:
        o.data.materials.append(lash_mat)
    teeth_mat = material("draconia_teeth", "standard", color=(0.70, 0.66, 0.58, 1), rough=0.3)
    tongue_mat = material("draconia_tongue", "skin", color=(0.55, 0.22, 0.22, 1), rough=0.35)
    mouth[0].data.materials.append(teeth_mat)
    mouth[1].data.materials.append(teeth_mat)
    if len(mouth) > 2:
        mouth[2].data.materials.append(tongue_mat)
    for o in hair_objs + cloth_objs:
        pass  # their modules assign materials
    bpy.data.objects.remove(body)
    if rest_skin is not skin:
        bpy.data.objects.remove(rest_skin)
    parts = objs + lash_objs + mouth + hair_objs + cloth_objs
    # The skeleton: the Universal joints at her joints, then her extras.
    joint_pos = draconia_rig.joint_positions(rig, lambda c: to_blender(c) + shift)
    names = list(draconia_rig.Universal().names) + [j[0] for j in hair_joints]
    for o in parts:
        write_skin_attributes(o, names)
    with open(os.path.join(out, "draconia.landmarks.json"), "w") as f:
        json.dump(marks.as_dict(), f, indent=1)
    return parts, marks, joint_pos, hair_joints


def transfer_weights(body, objs, k=4):
    """Cloth takes the weights of the nearest body vertices (the mean of
    the `k` nearest), so it follows the limbs it covers; hands' joints
    become the forearm's, since sleeves don't curl with fingers."""
    from mathutils.kdtree import KDTree

    me = body.data
    tree = KDTree(len(me.vertices))
    for v in me.vertices:
        tree.insert(v.co, v.index)
    tree.balance()
    names = {g.index: g.name for g in body.vertex_groups}
    vw = [{names[g.group]: g.weight for g in v.groups} for v in me.vertices]

    def remap(j):
        for side in ("l", "r"):
            if j == f"hand_{side}" or j.endswith(f"_{side}") and j.split("_")[0] in ("thumb", "index", "middle", "ring", "pinky"):
                return f"lowerarm_{side}"
        return j

    for o in objs:
        groups = {}
        for v in o.data.vertices:
            acc = {}
            for co, idx, dist in tree.find_n(v.co, k):
                for j, w in vw[idx].items():
                    j = remap(j)
                    acc[j] = acc.get(j, 0.0) + w / k
            for j, w in sorted(acc.items(), key=lambda kv: -kv[1])[:4]:
                if j not in groups:
                    groups[j] = o.vertex_groups.new(name=j)
                groups[j].add([v.index], w, "REPLACE")


def write_skin_attributes(obj, names):
    """Per-vertex joints and weights as float attributes `_VJ`, `_VW`
    (indices into `names`), which the glTF exporter writes as custom
    attributes and draconia_rig.skin_glb turns into JOINTS_0 / WEIGHTS_0.
    A vertex in no group is bound to `Head`."""
    index = {n: i for i, n in enumerate(names)}
    head = index["Head"]
    me = obj.data
    gname = {g.index: g.name for g in obj.vertex_groups}
    vj = me.attributes.new("_VJ", "FLOAT_COLOR", "POINT")
    vw = me.attributes.new("_VW", "FLOAT_COLOR", "POINT")
    for v in me.vertices:
        pairs = sorted(((gname[g.group], g.weight) for g in v.groups if g.weight > 1e-4 and gname.get(g.group) in index), key=lambda p: -p[1])[:4]
        s = sum(w for _, w in pairs)
        if s <= 0:
            pairs, s = [("Head", 1.0)], 1.0
        js = [index[j] for j, _ in pairs] + [0] * (4 - len(pairs))
        ws = [w / s for _, w in pairs] + [0.0] * (4 - len(pairs))
        vj.data[v.index].color = js
        vw.data[v.index].color = ws


def preview(out_dir, objs, marks):
    os.makedirs(out_dir, exist_ok=True)
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    scene.cycles.samples = 48
    scene.render.resolution_x = 1000
    scene.render.resolution_y = 1000
    world = bpy.data.worlds.new("w")
    scene.world = world
    world.use_nodes = True
    world.node_tree.nodes["Background"].inputs[0].default_value = (0.03, 0.05, 0.05, 1)
    target = (marks.eye_l + marks.eye_r) / 2 + Vector((0, 0, -0.03))
    for loc, energy, size, col in [
        (Vector((-0.6, -0.9, 0.2)), 60, 0.3, (1.0, 0.75, 0.5)),
        (Vector((0.9, 0.3, 0.4)), 40, 1.0, (0.4, 0.9, 0.8)),
        (Vector((0.4, -1.0, 0.5)), 10, 1.5, (0.8, 0.9, 1.0)),
    ]:
        bpy.ops.object.light_add(type="AREA", location=target + loc)
        light = bpy.context.object
        light.data.energy = energy
        light.data.size = size
        light.data.color = col
        light.rotation_euler = (target - light.location).to_track_quat("-Z", "Y").to_euler()
    for name, offset, lens in [
        ("front", Vector((0, -0.62, 0.0)), 85),
        ("three", Vector((0.3, -0.55, 0.02)), 85),
        ("side", Vector((0.62, -0.02, 0.0)), 85),
        ("eyes", Vector((0.0, -0.3, 0.0)), 100),
        ("bust", Vector((0.15, -1.7, -0.15)), 50),
        ("back", Vector((0.35, 0.55, 0.05)), 60),
    ]:
        bpy.ops.object.camera_add(location=target + offset)
        cam = bpy.context.object
        cam.data.lens = lens
        cam.data.clip_start = 0.01
        cam.rotation_euler = (target - cam.location).to_track_quat("-Z", "Y").to_euler()
        scene.camera = cam
        scene.render.filepath = os.path.join(out_dir, f"{name}.png")
        bpy.ops.render.render(write_still=True)


def main():
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    out, exprs, prev, tex, skip = OUT, ["neutral"], None, 4096, False
    it = iter(argv)
    for a in it:
        if a == "--out":
            out = next(it)
        elif a == "--expr":
            exprs = next(it).split(",")
        elif a == "--preview":
            prev = next(it)
        elif a == "--tex":
            tex = int(next(it))
        elif a == "--skip-paint":
            skip = True
        elif a == "--hair":
            global HAIR_STYLE
            HAIR_STYLE = next(it)
        elif a == "--brows":
            import draconia_paint

            draconia_paint.BROWS = next(it)
        elif a == "--battle":
            import draconia_paint

            draconia_paint.BATTLE = True
        elif a == "--earrings":
            global EARRINGS
            EARRINGS = True
    os.makedirs(out, exist_ok=True)
    for expr in exprs:
        bpy.ops.wm.read_factory_settings(use_empty=True)
        parts, marks, joint_pos, hair_joints = build(expr, tex, out, skip)
        if prev:
            preview(os.path.join(prev, expr), parts, marks)
        path = os.path.join(out, f"draconia{'' if HAIR_STYLE == 'updo' else '-' + HAIR_STYLE}.{expr}.glb")
        export(path, parts)
        import draconia_rig

        names = draconia_rig.skin_glb(path, joint_pos, hair_joints)
        print("SKIN", os.path.basename(path), len(names), "joints")


if __name__ == "__main__":
    main()
