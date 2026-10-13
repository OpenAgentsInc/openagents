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
    ("chin/chin-width-decr", 0.25),
    ("chin/chin-prominent-incr", 0.15),
    ("chin/chin-height-decr", 0.15),
    ("nose/nose-scale-horiz-decr", 0.25),
    ("nose/nose-point-width-decr", 0.2),
    ("nose/nose-nostrils-width-decr", 0.15),
    ("nose/nose-point-up", 0.25),
    ("nose/nose-hump-decr", 0.4),
    ("nose/nose-scale-vert-decr", 0.1),
    ("mouth/mouth-upperlip-volume-incr", 0.55),
    ("mouth/mouth-lowerlip-volume-incr", 0.6),
    ("mouth/mouth-cupidsbow-incr", 0.5),
    ("mouth/mouth-scale-horiz-incr", 0.05),
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
    ("expression/units/caucasian/eye-left-closure", 0.33),
    ("expression/units/caucasian/eye-right-closure", 0.33),
]

# Expressions: named weights over the CC0 expression units (FACS-like), the
# way a DNA rig maps a GUI control to raw controls. `jaw` opens the jaw bone
# (radians) for the teeth and tongue, which the units don't move.
EXPRESSIONS = {
    "neutral": {},
    "speak": {"mouth-open": 0.25, "mouth-parling": 0.2, "eyebrows-left-inner-up": 0.15, "eyebrows-right-inner-up": 0.15, "jaw": 0.06},
    "surprise": {
        "mouth-open": 0.55,
        "eyebrows-left-up": 0.6,
        "eyebrows-right-up": 0.6,
        "eye-left-opened-up": 0.35,
        "eye-right-opened-up": 0.35,
        "jaw": 0.13,
    },
    "closed": {"eye-left-closure": 0.67, "eye-right-closure": 0.67},
    "smile": {"mouth-corner-puller": 0.45, "mouth-elevation": 0.08, "eye-left-slit": 0.15, "eye-right-slit": 0.15},
}

SCALE = 0.1  # MakeHuman decimetres to metres
ARM_DROP = math.radians(36)  # from the base's A pose to arms hanging


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


def pose(base, expr):
    rig = Rig(base)
    # Arms down: the shoulder takes a little, the arm chain the rest.
    for side, sign in (("L", 1), ("R", -1)):
        pivot = rig.head(f"upperarm01.{side}")
        chain = rig.descendants(f"upperarm01.{side}")
        rotate_about(base.co, pivot, (0, 0, 1), -sign * ARM_DROP, rig.weight_of(chain))
        elbow = rig.head(f"lowerarm01.{side}")
        fore = rig.descendants(f"lowerarm01.{side}")
        # A relaxed elbow: the forearm swings a little forward.
        rotate_about(base.co, elbow, (1, 0, 0), -0.25, rig.weight_of(fore))
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
        if unit != "jaw":
            base.apply(f"expression/units/caucasian/{unit}", w)
    rig = pose(base, expr)
    return base, rig


# ------------------------------------------------------------- the meshes ---


def mesh_from_faces(base, name, faces, shift, uv=True):
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
            length = (0.0055 + 0.0035 * outer) * rng.uniform(0.75, 1.1)
        else:
            length = (0.0025 + 0.0025 * outer) * rng.uniform(0.7, 1.1)
        radial = (root - centre)
        radial.y = 0
        radial = radial.normalized()
        fwd = Vector((0, -1, 0))
        up = Vector((0, 0, 1 if upper else -1))
        segs = 6
        prev = None
        width0 = 0.00007 * rng.uniform(0.8, 1.2)
        tilt = rng.uniform(-0.25, 0.25)
        for k in range(segs + 1):
            t = k / segs
            # Out of the lid, then curling up (or down) and out.
            d = fwd * (0.75 - 0.55 * t) + up * (0.35 + 0.9 * t) + radial * (0.25 + tilt)
            p = root + d.normalized() * 0  # placeholder for clarity
            p = root + (fwd * (0.75 * t - 0.25 * t * t) + up * (0.25 * t + 0.6 * t * t) + radial * (0.3 + tilt) * t) * length
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

    base, rig = build_base(expr)
    feet = min(c[1] for c in base.co) * SCALE
    shift = Vector((0, 0, -feet))
    marks = Landmarks(base, rig, shift)

    # The full posed body, unsubdivided: the cloth fits over it.
    body_faces = [(vs, ts) for vs, ts, g in base.faces if g == "body"]
    body, _ = mesh_from_faces(base, "body_fit", body_faces, shift)

    skin, _ = mesh_from_faces(base, "skin", keep_skin(base, rig, marks, shift), shift)
    unwrap(skin)
    subdivide(skin, 2)

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
        ball, cornea = eyeball(f"eye_{side}", centre, r, look)
        eyes[side] = (ball, cornea, centre)
        objs += [ball, cornea]
        print("EYE", side, "seat offset mm", round(hi * 1000, 2))
    lash_objs = []
    for side, sign in (("l", 1), ("r", -1)):
        centre = eyes[side][2]
        for upper, count, seed in ((True, 110, 3 if sign > 0 else 4), (False, 28, 5 if sign > 0 else 6)):
            o = lashes(f"lash_{side}_{'u' if upper else 'd'}", openings[side], centre, sign, upper, count, seed)
            if o:
                lash_objs.append(o)

    # Teeth and tongue from the CC0 helpers, for the open-mouth expressions.
    mouth = []
    for g in ("helper-upper-teeth", "helper-lower-teeth", "helper-tongue"):
        faces = [(vs, ts) for vs, ts, gg in base.faces if gg == g]
        o, _ = mesh_from_faces(base, g.replace("helper-", ""), faces, shift)
        subdivide(o, 1)
        mouth.append(o)

    hair_objs = draconia_hair.grow(skin, marks, seed=7)
    cloth_objs = draconia_cloth.build(body, marks)

    marks.profile(skin)
    if not skip_paint:
        maps = draconia_paint.paint(skin, eyes, marks, tex, out)
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
    eye_mat = material("draconia_eye", "eye", color=(0.8, 0.8, 0.8, 1), rough=0.3, images=maps.get("eye", {}))
    cornea_mat = material("draconia_cornea", "cornea", color=(1, 1, 1, 1), rough=0.05, alpha="BLEND")
    # Clear in Blender's own preview; Verse draws a cornea as specular only.
    cornea_mat.node_tree.nodes["Principled BSDF"].inputs["Alpha"].default_value = 0.0
    for side in ("l", "r"):
        eyes[side][0].data.materials.append(eye_mat)
        eyes[side][1].data.materials.append(cornea_mat)
    lash_mat = material("draconia_lash", "hair", color=(0.035, 0.028, 0.022, 1), rough=0.45)
    for o in lash_objs:
        o.data.materials.append(lash_mat)
    teeth_mat = material("draconia_teeth", "standard", color=(0.78, 0.74, 0.66, 1), rough=0.25)
    tongue_mat = material("draconia_tongue", "skin", color=(0.55, 0.22, 0.22, 1), rough=0.35)
    mouth[0].data.materials.append(teeth_mat)
    mouth[1].data.materials.append(teeth_mat)
    mouth[2].data.materials.append(tongue_mat)
    for o in hair_objs + cloth_objs:
        pass  # their modules assign materials
    bpy.data.objects.remove(body)
    parts = objs + lash_objs + mouth + hair_objs + cloth_objs
    with open(os.path.join(out, "draconia.landmarks.json"), "w") as f:
        json.dump(marks.as_dict(), f, indent=1)
    return parts, marks


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
    os.makedirs(out, exist_ok=True)
    for expr in exprs:
        bpy.ops.wm.read_factory_settings(use_empty=True)
        parts, marks = build(expr, tex, out, skip)
        if prev:
            preview(os.path.join(prev, expr), parts, marks)
        export(os.path.join(out, f"draconia.{expr}.glb"), parts)


if __name__ == "__main__":
    main()
