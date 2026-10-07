"""The Grid robot: an original black-and-white robot on the Universal rig.

Run headless, once per variant:

    Blender -b --factory-startup --python scripts/blender/grid_robot.py -- lod0
    Blender -b --factory-startup --python scripts/blender/grid_robot.py -- lod1

Writes `assets/verse/characters/original/grid-robot/build/grid-robot.<variant>.glb`
(not committed); `scripts/blender/grid_robot_admit.py` admits it. `lod0` is
the near model and `lod1` the far level of detail.

Mode: Reference. Every part is built here from primitives. GarderX's "Low
Poly Sci-fi Robot" (Fab, standard license) was studied for proportions, its
breakdown into rigid parts, and its design language only; none of its
geometry, UVs, or materials were copied, edited, or exported.

Style rules, from the Grid and the study:

- The Grid draws near-black faces with white lines on their edges and white
  for what glows, so the robot is near-black faceted armor (`Armor`), darker
  graphite joints and mechanics (`Trim`), and two glow materials,
  `EmitWhite` and `EmitPale`, for its accents. Verse draws the panel edges
  as lines from the faceted geometry, so every panel is a few flat facets.
- Rigid parts, each bound to one bone of the Universal rig at full weight,
  with ball joints where limbs meet, so the Universal Animation Library
  plays on it with no deformation.
- Its own identity: a wrap-around chevron visor (not a T), a hollow-diamond
  chest emblem, a crest fin on the helmet, angular pauldrons with a pale
  edge, ear pods, and glowing bands around the elbows and knees.

Frame: 1 unit = 1 m. The rig is the CC0 Universal Base Characters male rig
(`Superhero_Male_FullBody.gltf`), standing in its T pose with its feet on the
ground, facing -Y in Blender (+Z after export). Parts are written in glTF's
frame (x left, y up, z forward) through `g()`.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Vector

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

BASE = os.path.join(kit.REPO, "assets", "verse", "characters", "quaternius", "base", "Superhero_Male_FullBody.gltf")
OUT = os.path.join(kit.REPO, "assets", "verse", "characters", "original", "grid-robot", "build")
# Triangle budgets, checked here and again at admission.
BUDGETS = {"lod0": 7000, "lod1": 1500}
MATERIALS = ["Armor", "Trim", "EmitWhite", "EmitPale"]
ARMOR, TRIM, GLOW, PALE = range(4)


def g(x, y, z):
    """A point in glTF's frame as a Blender vector."""
    return Vector((x, -z, y))


class Builder:
    """Accumulates rigid parts into one mesh: faces with a material, every
    vertex bound to one bone."""

    def __init__(self, bones):
        self.bm = bmesh.new()
        self.deform = self.bm.verts.layers.deform.verify()
        self.bones = bones
        self.group = {b: i for i, b in enumerate(bones)}
        self.parts = []

    def solid(self, name, bone, material, rings, cap0=True, cap1=True, loop=False):
        """A closed loft through `rings` (lists of glTF points of equal
        length), capped at both ends, or joined last to first when `loop`."""
        bm = self.bm
        start = len(bm.faces)
        verts = [[bm.verts.new(g(*p)) for p in ring] for ring in rings]
        for v in (v for ring in verts for v in ring):
            v[self.deform][self.group[bone]] = 1.0
        n = len(rings[0])
        faces = []
        for a, b in zip(verts, verts[1:] + (verts[:1] if loop else [])):
            for k in range(n):
                quad = [a[k], a[(k + 1) % n], b[(k + 1) % n], b[k]]
                if len({id(v) for v in quad}) == 4:
                    faces.append(bm.faces.new(quad))
        if cap0:
            faces.append(bm.faces.new(list(reversed(verts[0]))))
        if cap1:
            faces.append(bm.faces.new(verts[-1]))
        for f in faces:
            f.material_index = material
        bmesh.ops.recalc_face_normals(bm, faces=faces)
        self.parts.append((name, bone, len(bm.faces) - start))

    def finish(self, name, arm):
        mesh = bpy.data.meshes.new(name)
        self.bm.to_mesh(mesh)
        self.bm.free()
        obj = bpy.data.objects.new(name, mesh)
        bpy.context.scene.collection.objects.link(obj)
        for b in self.bones:
            obj.vertex_groups.new(name=b)
        for m in MATERIALS:
            obj.data.materials.append(bpy.data.materials[m])
        mesh.uv_layers.new(name="UVMap")
        for p in mesh.polygons:
            p.use_smooth = False
        kit.skin(arm, obj)
        return obj


def ring(center, u, v, ru, rv, n, phase=0.5):
    """`n` points around `center` in the plane of unit axes `u` and `v`, an
    ellipse of radii `ru` and `rv`; `phase` 0.5 puts a flat facet at +v."""
    c, u, v = Vector(center), Vector(u), Vector(v)
    out = []
    for k in range(n):
        t = math.pi / 2 + 2 * math.pi * (k + phase) / n
        out.append(tuple(c + u * (ru * math.cos(t)) + v * (rv * math.sin(t))))
    return out


def frame(axis):
    """Two unit axes perpendicular to `axis`, the second toward +z (or +y
    for a part along z)."""
    a = Vector(axis).normalized()
    up = Vector((0, 0, 1)) if abs(a.z) < 0.9 else Vector((0, 1, 0))
    u = up.cross(a).normalized()
    v = a.cross(u).normalized()
    return u, v


def prism(b, name, bone, material, p0, p1, r0, r1, n, chamfer=0.25, flat=None, phase=0.5):
    """A faceted limb from `p0` to `p1`: `n` sides, radii `r0` and `r1`
    (numbers, or (across, along v) pairs), its ends chamfered by `chamfer`
    of the radius."""
    p0, p1 = Vector(p0), Vector(p1)
    axis = p1 - p0
    u, v = frame(axis)
    if flat is not None:
        u, v = flat
    r0 = r0 if isinstance(r0, tuple) else (r0, r0)
    r1 = r1 if isinstance(r1, tuple) else (r1, r1)
    d = axis.normalized()
    c0 = min(r0) * chamfer
    c1 = min(r1) * chamfer
    rings = []
    if chamfer:
        rings.append(ring(p0, u, v, r0[0] * (1 - chamfer), r0[1] * (1 - chamfer), n, phase))
    rings.append(ring(p0 + d * c0, u, v, r0[0], r0[1], n, phase))
    rings.append(ring(p1 - d * c1, u, v, r1[0], r1[1], n, phase))
    if chamfer:
        rings.append(ring(p1, u, v, r1[0] * (1 - chamfer), r1[1] * (1 - chamfer), n, phase))
    b.solid(name, bone, material, rings)


def stack(b, name, bone, material, sections, n, axis=(0, 1, 0), center=(0, 0, 0), phase=0.5):
    """A loft along `axis` through `(height, half width, half depth, z
    offset)` sections, centered on `center`."""
    a = Vector(axis).normalized()
    u, v = Vector((1, 0, 0)), Vector((0, 0, 1))
    rings = []
    for h, hw, hd, dz in sections:
        c = Vector(center) + a * h + v * dz
        rings.append(ring(c, u, v, hw, hd, n, phase))
    b.solid(name, bone, material, rings)


def band(b, name, bone, material, p0, p1, r, n, depth=0.012):
    """A glowing ring band around a limb between `p0` and `p1`: an outer
    shell of radius `r` and an inner one `depth` smaller."""
    p0, p1 = Vector(p0), Vector(p1)
    u, v = frame(p1 - p0)
    outer = [ring(p0, u, v, r, r, n), ring(p1, u, v, r, r, n)]
    inner = [ring(p1, u, v, r - depth, r - depth, n), ring(p0, u, v, r - depth, r - depth, n)]
    b.solid(name, bone, material, outer + inner, cap0=False, cap1=False, loop=True)


def ball(b, name, bone, material, center, r, segs=8, rings_=5):
    """A low-poly ball joint."""
    c = Vector(center)
    rings = []
    for i in range(1, rings_):
        t = math.pi * i / rings_
        y = -math.cos(t) * r
        rr = math.sin(t) * r
        rings.append(ring(c + Vector((0, y, 0)), (1, 0, 0), (0, 0, 1), rr, rr, segs))
    b.solid(name, bone, material, rings)


def slab(b, name, bone, material, outline, normal, thickness):
    """A flat plate: the polygon `outline` (glTF points, counterclockwise
    seen from `normal`) extruded `thickness` back along `normal`."""
    nrm = Vector(normal).normalized()
    front = [tuple(Vector(p)) for p in outline]
    back = [tuple(Vector(p) - nrm * thickness) for p in outline]
    b.solid(name, bone, material, [back, front])


def box(b, name, bone, material, center, size, chamfer=0.0):
    """An axis-aligned box in glTF's frame, its vertical edges chamfered."""
    cx, cy, cz = center
    sx, sy, sz = (s / 2 for s in size)
    if chamfer:
        c = min(sx, sz) * chamfer
        pts = [(sx, sz - c), (sx - c, sz), (-sx + c, sz), (-sx, sz - c), (-sx, -sz + c), (-sx + c, -sz), (sx - c, -sz), (sx, -sz + c)]
    else:
        pts = [(sx, sz), (-sx, sz), (-sx, -sz), (sx, -sz)]
    rings = [[(cx + x, cy + y, cz + z) for x, z in pts] for y in (-sy, sy)]
    b.solid(name, bone, material, rings)


def wedge(b, name, bone, material, profile, x0, x1, taper=0.0):
    """A solid extruded across x from `x0` to `x1` from a side `profile` of
    (z, y) points; `taper` narrows the top edge inward."""
    rings = []
    for x in (x0, x1):
        rings.append([(x, y, z) for z, y in profile])
    b.solid(name, bone, material, rings)


def build(variant, joints):
    """Every part of the robot; `joints` maps bone names to rest positions
    in glTF's frame. `lod1` is the far level: fewer sides, mitten hands, and
    only the accents that read at a distance."""
    far = variant == "lod1"
    j = joints
    b = Builder(list(j))
    n_head = 6 if far else 10
    n_body = 6 if far else 8
    n_limb = 4 if far else 6

    # Head: a faceted helmet, a chevron visor, a crest fin, and ear pods.
    hz = 0.012
    stack(b, "helmet", "Head", ARMOR, [
        (1.598, 0.07, 0.08, hz - 0.012),
        (1.625, 0.098, 0.108, hz - 0.004),
        (1.655, 0.112, 0.122, hz),
        (1.735, 0.124, 0.134, hz),
        (1.805, 0.112, 0.122, hz - 0.005),
        (1.850, 0.070, 0.082, hz - 0.012),
        (1.866, 0.030, 0.040, hz - 0.015),
    ], n_head)
    # The visor follows the helmet's front facets at eye height, dipping to
    # a shallow chevron at the center.
    arc = 7 if not far else 5
    span = math.radians(118)
    top, low = [], []
    for k in range(arc):
        t = math.pi / 2 - span / 2 + span * k / (arc - 1)
        dip = 0.022 * (1 - abs(k - (arc - 1) / 2) / ((arc - 1) / 2))
        x, z = 0.129 * math.cos(t), hz + 0.139 * math.sin(t)
        top.append((x, 1.756 - dip, z))
        low.append((x, 1.731 - dip, z))
    inset = [(x * 0.93, y, hz + (z - hz) * 0.93) for x, y, z in top]
    inset_low = [(x * 0.93, y, hz + (z - hz) * 0.93) for x, y, z in low]
    # A curved strip lofted along the arc, one quad section per facet edge.
    b.solid("visor", "Head", GLOW, [[top[k], low[k], inset_low[k], inset[k]] for k in range(arc)])
    if not far:
        crest = [(0.0, 1.874, 0.06), (0.0, 1.886, 0.0), (0.0, 1.874, -0.09), (0.0, 1.835, -0.135), (0.0, 1.85, -0.06), (0.0, 1.862, 0.02)]
        b.solid("crest", "Head", ARMOR, [[(x - 0.011, y, z) for x, y, z in crest], [(x + 0.011, y, z) for x, y, z in crest]])
        for side in (1, -1):
            c = Vector((side * 0.122, 1.725, hz - 0.01))
            axis = Vector((side, 0, 0))
            prism(b, f"ear_{side}", "Head", TRIM, c, c + axis * 0.03, 0.042, 0.036, 6, chamfer=0.2)
            prism(b, f"ear_glow_{side}", "Head", PALE, c + axis * 0.028, c + axis * 0.034, 0.02, 0.018, 6, chamfer=0)
        # A jaw grille: three dark bars below the visor.
        for k, x in enumerate((-0.03, 0.0, 0.03)):
            box(b, f"grille_{k}", "Head", TRIM, (x, 1.655, hz + 0.118), (0.012, 0.03, 0.012))

    # Neck: a graphite core with two ribs.
    neck = j["neck_01"]
    prism(b, "neck", "neck_01", TRIM, (0, 1.515, neck.z + 0.01), (0, 1.635, j["Head"].z), 0.046, 0.042, n_limb, chamfer=0)
    if not far:
        for k, y in enumerate((1.555, 1.59)):
            prism(b, f"neck_rib_{k}", "neck_01", TRIM, (0, y, neck.z + 0.012), (0, y + 0.012, neck.z + 0.012), 0.058, 0.058, 8, chamfer=0)

    # Chest (spine_03): a tapered plate that stays narrow below the
    # shoulders so the arms swing clear.
    stack(b, "chest", "spine_03", ARMOR, [
        (1.290, 0.112, 0.092, 0.005),
        (1.350, 0.140, 0.118, 0.014),
        (1.430, 0.150, 0.128, 0.018),
        (1.495, 0.142, 0.112, 0.004),
        (1.548, 0.08, 0.068, -0.014),
    ], n_body)
    # The chest emblem: a hollow diamond with a solid core, on the front facet.
    front = 0.128 * math.cos(math.pi / n_body) + 0.018
    ez, ey = front + 0.004, 1.425
    if far:
        slab(b, "emblem", "spine_03", GLOW, [(0, ey - 0.05, ez), (0.036, ey, ez), (0, ey + 0.05, ez), (-0.036, ey, ez)], (0, 0, 1), 0.02)
    else:
        w, h, t = 0.046, 0.062, 0.012
        outer = [(0, ey - h), (w, ey), (0, ey + h), (-w, ey)]
        inner = [(0, ey - h + t * 1.6), (w - t * 1.2, ey), (0, ey + h - t * 1.6), (-w + t * 1.2, ey)]
        for k in range(4):
            a0, a1 = outer[k], outer[(k + 1) % 4]
            i0, i1 = inner[k], inner[(k + 1) % 4]
            quad = [(a0[0], a0[1], ez), (a1[0], a1[1], ez), (i1[0], i1[1], ez), (i0[0], i0[1], ez)]
            slab(b, f"emblem_{k}", "spine_03", GLOW, quad, (0, 0, 1), 0.016)
        slab(b, "emblem_core", "spine_03", GLOW, [(0, ey - 0.02, ez), (0.015, ey, ez), (0, ey + 0.02, ez), (-0.015, ey, ez)], (0, 0, 1), 0.016)
        # The back: a raised plate with a pale bar.
        back = -(0.128 * math.cos(math.pi / n_body) - 0.018)
        slab(b, "back_plate", "spine_03", ARMOR, [(-0.07, 1.36, back - 0.014), (-0.08, 1.47, back - 0.012), (0.08, 1.47, back - 0.012), (0.07, 1.36, back - 0.014)], (0, 0, -1), 0.02)
        slab(b, "back_bar", "spine_03", PALE, [(-0.045, 1.44, back - 0.017), (-0.045, 1.452, back - 0.017), (0.045, 1.452, back - 0.017), (0.045, 1.44, back - 0.017)], (0, 0, -1), 0.006)

    # Abdomen (spine_02, spine_01): a graphite core and stacked ribs.
    prism(b, "abdomen", "spine_02", TRIM, (0, 1.16, j["spine_02"].z), (0, 1.30, j["spine_02"].z + 0.004), 0.085, 0.09, n_body, chamfer=0)
    prism(b, "waist", "spine_01", TRIM, (0, 1.05, j["spine_01"].z), (0, 1.17, j["spine_01"].z), 0.09, 0.08, n_body, chamfer=0)
    ribs = [(1.205, "spine_02"), (1.255, "spine_02")] if not far else [(1.23, "spine_02")]
    for k, (y, bone) in enumerate(ribs):
        stack(b, f"rib_{k}", bone, ARMOR, [(y - 0.016, 0.105, 0.085, 0.006), (y, 0.118, 0.098, 0.008), (y + 0.016, 0.105, 0.085, 0.006)], n_body)

    # Pelvis: a belt plate above the hips, a narrow front and back plate
    # between the thighs, and a pale belt light.
    pz = j["pelvis"].z
    stack(b, "belt", "pelvis", ARMOR, [
        (0.960, 0.135, 0.098, pz + 0.035),
        (0.990, 0.158, 0.112, pz + 0.038),
        (1.040, 0.150, 0.106, pz + 0.036),
        (1.065, 0.115, 0.088, pz + 0.032),
    ], n_body)
    if not far:
        slab(b, "fauld", "pelvis", ARMOR, [(-0.034, 0.965, pz + 0.135), (-0.026, 0.885, pz + 0.118), (0.026, 0.885, pz + 0.118), (0.034, 0.965, pz + 0.135)], (0, 0, 1), 0.05)
        slab(b, "fauld_back", "pelvis", ARMOR, [(0.034, 0.965, pz - 0.045), (0.026, 0.9, pz - 0.035), (-0.026, 0.9, pz - 0.035), (-0.034, 0.965, pz - 0.045)], (0, 0, -1), 0.04)
    slab(b, "belt_light", "pelvis", PALE, [(-0.05, 1.004, pz + 0.152), (-0.05, 1.018, pz + 0.152), (0.05, 1.018, pz + 0.152), (0.05, 1.004, pz + 0.152)], (0, 0, 1), 0.006)

    for s, side in ((1, "l"), (-1, "r")):
        clav, sh, el, wr = j[f"clavicle_{side}"], j[f"upperarm_{side}"], j[f"lowerarm_{side}"], j[f"hand_{side}"]
        x = Vector((s, 0, 0))
        # Pauldron (clavicle): an angular cap over the shoulder ball, open
        # below, with a pale edge along its outer rim.
        cap = [(0.125, 1.548), (0.215, 1.572), (0.282, 1.532), (0.292, 1.47), (0.262, 1.47), (0.255, 1.518), (0.21, 1.54), (0.13, 1.52)]
        zf, zb = sh.z + 0.085, sh.z - 0.095
        rings = []
        for z, shrink in ((zb, 0.75), (zb + 0.03, 1.0), (zf - 0.03, 1.0), (zf, 0.75)):
            cx, cy = 0.215, 1.515
            rings.append([(s * (cx + (px - cx) * shrink), cy + (py - cy) * shrink, z) for px, py in cap])
        if s < 0:
            rings = [list(reversed(r)) for r in rings]
        b.solid(f"pauldron_{side}", f"clavicle_{side}", ARMOR, rings)
        if not far:
            rim = [(s * 0.293, 1.47, zf - 0.03), (s * 0.293, 1.47, zb + 0.03), (s * 0.293, 1.482, zb + 0.03), (s * 0.293, 1.482, zf - 0.03)]
            if s < 0:
                rim = list(reversed(rim))
            slab(b, f"pauldron_rim_{side}", f"clavicle_{side}", PALE, rim, (s, 0, 0), 0.008)
        # Shoulder ball, upper arm, elbow ball and band, forearm.
        ball(b, f"shoulder_{side}", f"upperarm_{side}", TRIM, sh, 0.054, 6 if far else 8, 4 if far else 5)
        prism(b, f"upperarm_{side}", f"upperarm_{side}", ARMOR, sh + x * 0.05, el - x * 0.035, (0.05, 0.052), (0.043, 0.045), n_limb)
        ball(b, f"elbow_{side}", f"lowerarm_{side}", TRIM, el, 0.04, 6 if far else 8, 4)
        band(b, f"elbow_band_{side}", f"lowerarm_{side}", GLOW, el + x * 0.036, el + x * 0.05, 0.05, 8 if not far else 6)
        prism(b, f"forearm_{side}", f"lowerarm_{side}", ARMOR, el + x * 0.052, wr - x * 0.02, (0.052, 0.054), (0.04, 0.042), n_limb)
        # Hand: a palm and, near, five two-part fingers on their bones.
        palm = wr + x * 0.05 + Vector((0, -0.004, 0.003))
        if far:
            box(b, f"hand_{side}", f"hand_{side}", TRIM, (palm.x + s * 0.04, palm.y, palm.z), (0.2, 0.04, 0.1))
        else:
            box(b, f"palm_{side}", f"hand_{side}", TRIM, tuple(palm), (0.088, 0.042, 0.084), chamfer=0.25)
            for f in ("index", "middle", "ring", "pinky", "thumb"):
                a, m, leaf = j[f"{f}_01_{side}"], j[f"{f}_02_{side}"], j[f"{f}_04_leaf_{side}"]
                if f == "thumb":
                    leaf = m.lerp(leaf, 0.7)
                w = 0.02 if f != "thumb" else 0.018
                prism(b, f"{f}_1_{side}", f"{f}_01_{side}", TRIM, a, m, w / 2 + 0.002, w / 2, 4, chamfer=0.15, phase=0.0)
                prism(b, f"{f}_2_{side}", f"{f}_02_{side}", ARMOR, m, leaf, w / 2, w / 2 - 0.002, 4, chamfer=0.3, phase=0.0)

        hip, knee, ankle, toe = j[f"thigh_{side}"], j[f"calf_{side}"], j[f"foot_{side}"], j[f"ball_{side}"]
        y = Vector((0, 1, 0))
        # Hip ball, thigh, knee ball and band, shin with its guard, ankle.
        ball(b, f"hip_{side}", f"thigh_{side}", TRIM, hip, 0.062, 6 if far else 8, 4 if far else 5)
        prism(b, f"thigh_{side}", f"thigh_{side}", ARMOR, hip - y * 0.07, knee + y * 0.058, (0.07, 0.074), (0.052, 0.055), n_body if not far else n_limb)
        ball(b, f"knee_{side}", f"calf_{side}", TRIM, knee, 0.05, 6 if far else 8, 4)
        band(b, f"knee_band_{side}", f"calf_{side}", GLOW, knee - y * 0.056, knee - y * 0.07, 0.058, 8 if not far else 6)
        prism(b, f"shin_{side}", f"calf_{side}", ARMOR, knee - y * 0.074, ankle + y * 0.068, (0.058, 0.06), (0.046, 0.048), n_body if not far else n_limb)
        if not far:
            # A shin guard on the front facet with a pale chevron.
            gz = knee.z + 0.062
            slab(b, f"shin_guard_{side}", f"calf_{side}", ARMOR, [(knee.x - 0.04, 0.44, gz), (knee.x, 0.47, gz + 0.006), (knee.x + 0.04, 0.44, gz), (knee.x + 0.032, 0.25, gz - 0.008), (knee.x - 0.032, 0.25, gz - 0.008)][::-1], (0, 0, 1), 0.03)
            for k, yy in enumerate((0.40, 0.37)):
                chev = [(knee.x - 0.026, yy - 0.012, gz + 0.004), (knee.x, yy + 0.004, gz + 0.008), (knee.x + 0.026, yy - 0.012, gz + 0.004), (knee.x + 0.026, yy - 0.004, gz + 0.004), (knee.x, yy + 0.012, gz + 0.008), (knee.x - 0.026, yy - 0.004, gz + 0.004)]
                slab(b, f"chevron_{k}_{side}", f"calf_{side}", PALE, chev[::-1], (0, 0, 1), 0.006)
            band(b, f"ankle_band_{side}", f"calf_{side}", PALE, ankle + y * 0.07, ankle + y * 0.08, 0.05, 8)
        ball(b, f"ankle_{side}", f"foot_{side}", TRIM, ankle, 0.042, 6 if far else 8, 4)
        # Boot: a wedge from heel to ball, the sole on the ground, and a toe
        # cap on the ball bone.
        heel, front_z = ankle.z - 0.085, toe.z + 0.004
        boot = [(heel + 0.025, 0.0), (front_z, 0.0), (front_z, 0.05), (toe.z - 0.03, 0.08), (ankle.z + 0.03, 0.105), (heel + 0.02, 0.1), (heel, 0.06), (heel, 0.022)]
        wedge(b, f"boot_{side}", f"foot_{side}", ARMOR, boot if s > 0 else boot, hip.x - 0.058, hip.x + 0.058)
        tip = j[f"ball_leaf_{side}"].z + 0.012
        cap_ = [(front_z + 0.005, 0.0), (tip - 0.014, 0.0), (tip, 0.014), (tip, 0.024), (tip - 0.03, 0.05), (front_z + 0.005, 0.048)]
        wedge(b, f"toe_{side}", f"ball_{side}", ARMOR, cap_, hip.x - 0.052, hip.x + 0.052)
    return b


def main():
    variant = (kit.args() or ["lod0"])[0]
    assert variant in BUDGETS, f"unknown variant {variant}"
    kit.reset()
    kit.mat("Armor", (0.022, 0.023, 0.026), rough=0.45, metal=0.4)
    kit.mat("Trim", (0.06, 0.062, 0.068), rough=0.6, metal=0.6)
    kit.mat("EmitWhite", (1.0, 1.0, 1.0), rough=0.3, emit=(1.0, 1.0, 1.0), strength=6.0)
    kit.mat("EmitPale", (0.8, 0.84, 0.9), rough=0.3, emit=(0.8, 0.86, 0.95), strength=2.5)
    bpy.ops.import_scene.gltf(filepath=BASE)
    arm = next(o for o in bpy.data.objects if o.type == "ARMATURE")
    for o in [o for o in bpy.data.objects if o.type == "MESH"]:
        bpy.data.objects.remove(o)
    arm.name = "Universal"
    joints = {}
    for bone in arm.data.bones:
        p = arm.matrix_world @ bone.head_local
        joints[bone.name] = Vector((p.x, p.z, -p.y))
    b = build(variant, joints)
    obj = b.finish("GridRobot", arm)
    tris = kit.triangles([obj])
    assert tris <= BUDGETS[variant], f"{variant}: {tris} triangles over {BUDGETS[variant]}"
    # Rigid: every vertex on exactly one bone at full weight.
    for v in obj.data.vertices:
        assert len(v.groups) == 1 and v.groups[0].weight == 1.0
    lo = min((obj.matrix_world @ v.co).z for v in obj.data.vertices)
    assert abs(lo) < 1e-4, f"soles at {lo}"
    os.makedirs(OUT, exist_ok=True)
    out = os.path.join(OUT, f"grid-robot.{variant}.glb")
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.export_scene.gltf(filepath=out, export_format="GLB", export_yup=True, export_skins=True,
                              export_animations=False, export_apply=False, export_materials="EXPORT")
    parts = {}
    for name, bone, faces in b.parts:
        parts[bone] = parts.get(bone, 0) + 1
    print("MODEL", variant, tris, "parts", len(b.parts), "bones", len(parts), out)


main()
