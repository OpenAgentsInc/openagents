"""Build the Grove's Shapechange dragon and write it as glTF.

Run headless:
    Blender -b --factory-startup --python scripts/blender/dragon.py -- [OUT_DIR] [--preview DIR [--views V,...]]

Writes `dragon.glb` (default folder `assets/verse/generated`): a stylized
low-poly young red dragon in the Quaternius look, a Reference model built
from primitives with no source asset. With `--preview DIR` it also renders
each clip at a few phases into DIR, for review before admission.

Style rules, from the Quaternius kits the zone already carries: chunky,
readable silhouettes; few segments, flat-shaded facets for scales; flat
saturated colors in a few tones (crimson hide, a gold belly, a wine-dark
wing membrane, ivory horns and claws, glowing amber eyes); no textures.

The body, neck, and tail are one lofted tube along the spine, skinned
smoothly across the spine's bones so the neck and tail bend; the head,
legs, horns, spikes, and wings are solids weighted to one bone each, as
`animals.py` builds its beasts. The wings are rigged spread (the bind
pose) and fold along the flanks in the ground clips.

Clips, in Verse's clip vocabulary: `idle` (breathing, the tail swaying),
`walk` (a four-beat walk), `fly` (wingbeats), `glide` (wings held, the
tail streaming), `bite`, `breath` (drawing in, then a breath held with the
jaws wide), `roar` (rearing, wings half open), and `sweep` (the tail
swung around). Every bone points up +Z from its pivot, so `rx` pitches,
`ry` yaws, and `rz` rolls about the forward axis. The dragon faces -Y in
Blender, which is +Z (glTF's front) after export; its feet stand at the
origin's height. 1 unit = 1 m: it stands about 5 m to the top of its head,
10 m from snout to tail, and 11 m across the spread wings.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Vector

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

TAU = 2 * math.pi
R = math.radians

# The spine from the tail's tip to the top of the neck: (y, z, half width,
# half height). The dragon faces -Y.
SPINE = [
    (7.3, 0.85, 0.05, 0.05),
    (6.5, 0.95, 0.12, 0.11),
    (5.6, 1.15, 0.2, 0.18),
    (4.7, 1.4, 0.28, 0.25),
    (3.8, 1.7, 0.37, 0.33),
    (2.9, 2.0, 0.5, 0.45),
    (2.0, 2.3, 0.7, 0.66),
    (1.1, 2.45, 0.88, 0.85),
    (0.2, 2.5, 1.0, 0.95),
    (-0.7, 2.6, 1.02, 0.98),
    (-1.45, 2.85, 0.82, 0.82),
    (-1.95, 3.3, 0.55, 0.58),
    (-2.3, 3.85, 0.42, 0.44),
    (-2.55, 4.4, 0.36, 0.38),
    (-2.72, 4.85, 0.33, 0.34),
]
SIDES = 8

# Bones: (name, parent, head). Spine bones sit on the spine; their weights
# along the loft blend between neighbors.
BONES = [
    ("body", None, (0, 0.2, 2.5)),
    ("chest", "body", (0, -0.9, 2.6)),
    ("neck1", "chest", (0, -1.8, 3.1)),
    ("neck2", "neck1", (0, -2.2, 3.7)),
    ("neck3", "neck2", (0, -2.5, 4.3)),
    ("head", "neck3", (0, -2.75, 4.9)),
    ("jaw", "head", (0, -3.14, 4.8)),
    ("tail1", "body", (0, 2.0, 2.3)),
    ("tail2", "tail1", (0, 3.4, 1.85)),
    ("tail3", "tail2", (0, 4.7, 1.4)),
    ("tail4", "tail3", (0, 5.8, 1.1)),
    ("tail5", "tail4", (0, 6.7, 0.92)),
]
# Each spine bone's place along the loft, as a y position, tail to neck;
# the loft's weights blend linearly between neighboring bones' places.
CHAIN = [
    ("tail5", 6.9),
    ("tail4", 5.95),
    ("tail3", 4.95),
    ("tail2", 3.85),
    ("tail1", 2.75),
    ("body", 1.0),
    ("chest", -0.9),
    ("neck1", -1.85),
    ("neck2", -2.25),
    ("neck3", -2.55),
    ("head", -2.8),
]

def _tail(y, k=0.85):
    """Shortens the tail: past the hips, positions draw in by `k`."""
    return 2.0 + (y - 2.0) * k if y > 2.0 else y


SPINE = [(_tail(y), z, w, h) for y, z, w, h in SPINE]
BONES = [(n, p, (h[0], _tail(h[1]), h[2])) for n, p, h in BONES]
CHAIN = [(n, _tail(y)) for n, y in CHAIN]
# The head is drawn this much larger than modeled, about the head bone.
HEAD_SCALE = 1.3
# The wings likewise, about the shoulder.
WING_SCALE = 1.3

LEGS = {
    # tag: (side x, hip or shoulder y, hip z, knee (y, z), ankle (y, z), toe y, thigh radius)
    "B": (0.78, 1.25, 2.25, (0.75, 1.25), (1.55, 0.42), 0.95, 0.42),
    "F": (0.8, -1.2, 2.25, (-0.95, 1.2), (-1.25, 0.38), -1.75, 0.33),
}


def materials():
    return dict(
        hide=kit.mat("Dragon_Hide", (0.56, 0.08, 0.06), 0.75),
        belly=kit.mat("Dragon_Belly", (0.93, 0.62, 0.24), 0.8),
        wing=kit.mat("Dragon_Membrane", (0.33, 0.05, 0.08), 0.85),
        horn=kit.mat("Dragon_Horn", (0.9, 0.84, 0.68), 0.6),
        dark=kit.mat("Dragon_Ridge", (0.2, 0.04, 0.05), 0.7),
        eye=kit.mat("Dragon_Eye", (1.0, 0.75, 0.15), 0.3, emit=(1.0, 0.6, 0.1), strength=6.0),
        mouth=kit.mat("Dragon_Mouth", (0.42, 0.06, 0.08), 0.9),
    )


def spine_weights(y):
    """Each spine bone's weight at loft position `y`: linear between the
    two neighboring bones' places, all on the end bone past either end."""
    places = CHAIN
    if y >= places[0][1]:
        return {places[0][0]: 1.0}
    if y <= places[-1][1]:
        return {places[-1][0]: 1.0}
    for (a, ya), (b, yb) in zip(places, places[1:]):
        if ya >= y >= yb:
            s = (ya - y) / (ya - yb)
            return {a: 1.0 - s, b: s}
    return {"body": 1.0}


def loft(m):
    """The body, neck, and tail as one tube along SPINE, flat-faceted,
    the downward faces in the belly's gold, weighted smoothly along the
    spine's bones."""
    bm = bmesh.new()
    rings = []
    for i, (y, z, w, h) in enumerate(SPINE):
        # Each ring turns to the spine's local direction (in y-z).
        a = SPINE[max(i - 1, 0)]
        b = SPINE[min(i + 1, len(SPINE) - 1)]
        d = Vector((0, b[0] - a[0], b[1] - a[1])).normalized()
        n = Vector((0, -d.z, d.y))  # the ring's "up" in the y-z plane
        if n.z < 0:
            n = -n
        ring = []
        for k in range(SIDES):
            # Rotated half a step so a flat face sits on top and on the belly.
            t = TAU * (k + 0.5) / SIDES
            x = math.cos(t) * w
            up = math.sin(t) * h
            # A keel: the top of the back peaks a little.
            if math.sin(t) > 0.9:
                up *= 1.08
            p = Vector((x, y, z)) + n * up
            ring.append(bm.verts.new(p))
        rings.append(ring)
    faces = []
    for r0, r1 in zip(rings, rings[1:]):
        for k in range(SIDES):
            j = (k + 1) % SIDES
            faces.append(bm.faces.new((r0[k], r0[j], r1[j], r1[k])))
    # Close the tail's tip and the neck's top.
    bm.faces.new(list(reversed(rings[0])))
    bm.faces.new(rings[-1])
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    me = bpy.data.meshes.new("Spine")
    bm.to_mesh(me)
    bm.free()
    o = bpy.data.objects.new("Spine", me)
    bpy.context.scene.collection.objects.link(o)
    o.data.materials.append(m["hide"])
    o.data.materials.append(m["belly"])
    for poly in o.data.polygons:
        if poly.normal.z < -0.45 or (poly.normal.z < -0.1 and poly.normal.y < -0.6):
            poly.material_index = 1
    # Smooth weights along the spine.
    groups = {}
    for v in o.data.vertices:
        for bone, wgt in spine_weights(v.co.y).items():
            if bone not in groups:
                groups[bone] = o.vertex_groups.new(name=bone)
            groups[bone].add([v.index], wgt, "REPLACE")
    return o


def bone_at(y):
    """The spine bone with the most weight at `y`."""
    w = spine_weights(y)
    return max(w, key=w.get)


def spikes(m):
    """Dark ridge spikes down the back from the neck to the tail, and a
    spade at the tail's tip."""
    parts = []
    for i, (y, z, w, h) in enumerate(SPINE[1:-1], start=1):
        size = min(h, 0.5) * 0.8
        if size < 0.08:
            continue
        for k, f in enumerate((0.0, 0.5)):
            if k == 1 and i >= len(SPINE) - 3:
                continue
            nxt = SPINE[i + 1]
            yy = y + (nxt[0] - y) * f
            zz = z + (nxt[1] - z) * f
            hh = h + (nxt[3] - h) * f
            s = size * (1.0 if k == 0 else 0.7)
            o = kit.cyl("Spike%d_%d" % (i, k), s * 0.42, s * 1.1, (0, yy + s * 0.2, zz + hh * 1.02 + s * 0.4),
                        m["dark"], verts=4, r2=0.01, rot=(R(-28), 0, 0))
            parts.append(kit.bind(o, bone_at(yy)))
    # The tail's spade: a flat diamond.
    o = kit.ball("Spade", 1.0, (0, _tail(7.6), 0.86), m["dark"], segs=4, rings=2, scale=(0.36, 0.62, 0.06))
    parts.append(kit.bind(o, "tail5"))
    return parts


def head(m):
    """The head on the `head` bone, the lower jaw on `jaw`: a wedge skull,
    a long snout, swept-back horns, a brow, glowing eyes, and teeth."""
    parts = []
    hide, horn, dark = m["hide"], m["horn"], m["dark"]
    # Skull and snout.
    parts.append(kit.bind(kit.ball("Skull", 0.5, (0, -2.98, 5.02), hide, segs=8, rings=5,
                                   scale=(0.82, 1.05, 0.72)), "head"))
    parts.append(kit.bind(kit.cyl("Snout", 0.3, 0.95, (0, -3.62, 4.95), hide, verts=6, r2=0.18,
                                  rot=(R(96), 0, 0)), "head"))
    parts.append(kit.bind(kit.box("Muzzle", (0.34, 0.36, 0.16), (0, -4.0, 4.98), hide,
                                  rot=(R(6), 0, 0)), "head"))
    # The roof of the mouth, seen when the jaws open.
    parts.append(kit.bind(kit.box("Palate", (0.3, 0.9, 0.04), (0, -3.6, 4.83), m["mouth"]), "head"))
    # Brow ridges over the eyes, and the eyes.
    for sx in (-1, 1):
        parts.append(kit.bind(kit.box("Brow%d" % sx, (0.22, 0.42, 0.1), (sx * 0.27, -3.22, 5.24), dark,
                                      rot=(R(-12), 0, sx * R(-14))), "head"))
        parts.append(kit.bind(kit.ball("Eye%d" % sx, 0.085, (sx * 0.33, -3.3, 5.12), m["eye"], segs=6,
                                       rings=4, scale=(0.7, 1.2, 0.8)), "head"))
        parts.append(kit.bind(kit.ball("Nostril%d" % sx, 0.05, (sx * 0.12, -4.17, 5.05), dark, segs=5,
                                       rings=3), "head"))
        # Horns: two long ones swept back, two short ones below them.
        parts.append(kit.bind(kit.cyl("Horn%d" % sx, 0.11, 0.95, (sx * 0.3, -2.62, 5.42), horn, verts=6,
                                      r2=0.015, rot=(R(-58), sx * R(-14), 0)), "head"))
        parts.append(kit.bind(kit.cyl("HornLow%d" % sx, 0.07, 0.5, (sx * 0.42, -2.72, 5.08), horn, verts=5,
                                      r2=0.01, rot=(R(-80), sx * R(-30), 0)), "head"))
        # A frill of cheek spikes.
        parts.append(kit.bind(kit.cyl("Cheek%d" % sx, 0.06, 0.4, (sx * 0.46, -2.82, 4.86), dark, verts=4,
                                      r2=0.008, rot=(R(-95), sx * R(-50), 0)), "head"))
        # Upper teeth along the snout's rim.
        for k in range(3):
            y = -3.55 - k * 0.22
            parts.append(kit.bind(kit.cyl("Tooth%d_%d" % (sx, k), 0.035, 0.12, (sx * 0.16, y, 4.78), horn,
                                          verts=4, r2=0.004, rot=(R(180), 0, 0)), "head"))
    # A nose horn.
    parts.append(kit.bind(kit.cyl("NoseHorn", 0.06, 0.22, (0, -4.05, 5.13), horn, verts=4, r2=0.006,
                                  rot=(R(-30), 0, 0)), "head"))
    # Lower jaw, hinged at the back of the skull.
    parts.append(kit.bind(kit.cyl("Jaw", 0.22, 1.05, (0, -3.55, 4.7), hide, verts=6, r2=0.12,
                                  rot=(R(94), 0, 0)), "jaw"))
    parts.append(kit.bind(kit.box("Tongue", (0.18, 0.7, 0.04), (0, -3.55, 4.82), m["mouth"]), "jaw"))
    for sx in (-1, 1):
        for k in range(2):
            y = -3.6 - k * 0.25
            parts.append(kit.bind(kit.cyl("LowTooth%d_%d" % (sx, k), 0.03, 0.1, (sx * 0.12, y, 4.86), horn,
                                          verts=4, r2=0.004), "jaw"))
    pivot = Vector((0, -2.75, 4.9))
    for o in parts:
        o.location = pivot + (Vector(o.location) - pivot) * HEAD_SCALE
        o.scale = tuple(c * HEAD_SCALE for c in o.scale)
    return parts


def legs(m, bones):
    """Four legs: a thigh on the upper bone, a shin and a clawed foot on
    the lower, each weighted to one bone."""
    parts = []
    hide, horn = m["hide"], m["horn"]
    for tag, (x0, y0, z0, knee, ankle, toe, r) in LEGS.items():
        for side, sx in (("L", 1), ("R", -1)):
            x = sx * x0
            upper, lower = "leg_%s%s" % (tag, side), "shin_%s%s" % (tag, side)
            bones += [(upper, "body" if tag == "B" else "chest", (x, y0, z0)), (lower, upper, (x, knee[0], knee[1]))]
            hip = Vector((x, y0, z0))
            kn = Vector((x, knee[0], knee[1]))
            an = Vector((x, ankle[0], ankle[1]))
            # Thigh: a plump ball stretched from hip to knee.
            mid = (hip + kn) / 2
            d = kn - hip
            pitch = math.atan2(d.y, -d.z)
            parts.append(kit.bind(kit.ball("Thigh" + tag + side, r, mid, hide, segs=7, rings=5,
                                           scale=(0.85, 0.9, (d.length / (2 * r)) * 1.15),
                                           rot=(pitch, 0, 0)), upper))
            # Shin: knee to ankle.
            d2 = an - kn
            pitch2 = math.atan2(d2.y, -d2.z)
            parts.append(kit.bind(kit.cyl("Shin" + tag + side, r * 0.55, d2.length + r * 0.3, (kn + an) / 2,
                                          hide, verts=6, r2=r * 0.42, rot=(R(180) + pitch2, 0, 0)), lower))
            # Foot and three ivory claws.
            foot = Vector((x, (ankle[0] + toe) / 2, 0.16))
            parts.append(kit.bind(kit.box("Foot" + tag + side, (r * 1.1, abs(toe - ankle[0]) + 0.3, 0.3), foot,
                                          hide, bevel=0.06), lower))
            for k in (-1, 0, 1):
                parts.append(kit.bind(kit.cyl("Claw%s%s%d" % (tag, side, k), 0.07, 0.26,
                                              (x + k * r * 0.38, toe - 0.12, 0.1), horn, verts=4, r2=0.008,
                                              rot=(R(100), 0, 0)), lower))
    return parts


# The left wing in the bind pose, spread flat at shoulder height; the right
# mirrors it. Points are (x, y) in the membrane's plane.
SHOULDER = (0.62, -1.05)
ELBOW = (2.5, -1.45)
WRIST = (3.95, -0.75)
FINGERS = [(5.75, 0.35), (4.85, 1.75), (3.3, 2.45)]
ROOT = (0.75, 0.75)
WING_Z = 3.1


def wing(m, side, sx):
    """One membranous wing: arm and finger spars in the hide's red over a
    thin membrane, its inner part on `wing_*` and the rest on `fore_*`."""
    upper, fore = "wing_" + side, "fore_" + side
    z = WING_Z
    def P(p):
        # Grown about the shoulder by WING_SCALE.
        x = SHOULDER[0] + (p[0] - SHOULDER[0]) * WING_SCALE
        y = SHOULDER[1] + (p[1] - SHOULDER[1]) * WING_SCALE
        return Vector((sx * x, y, z))

    bm = bmesh.new()
    s, e, w, r = (bm.verts.new(P(p)) for p in (SHOULDER, ELBOW, WRIST, ROOT))
    tips = [bm.verts.new(P(f)) for f in FINGERS]
    # The trailing edge scallops inward between the fingers.
    def scallop(a, b, k=0.28):
        mid = ((a[0] + b[0]) / 2, (a[1] + b[1]) / 2)
        toward = (WRIST[0] - mid[0], WRIST[1] - mid[1])
        return (mid[0] + toward[0] * k, mid[1] + toward[1] * k)

    sc = [bm.verts.new(P(scallop(a, b))) for a, b in zip(FINGERS, FINGERS[1:])]
    sc_root = bm.verts.new(P(scallop(FINGERS[-1], ROOT, 0.2)))
    inner = [
        (s, e, w),
        (s, w, r),
        (w, tips[0], sc[0]),
        (w, sc[0], tips[1]),
        (w, tips[1], sc[1]),
        (w, sc[1], tips[2]),
        (w, tips[2], sc_root),
        (w, sc_root, r),
    ]
    for tri in inner:
        bm.faces.new(tri if sx > 0 else tuple(reversed(tri)))
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    me = bpy.data.meshes.new("Membrane" + side)
    bm.to_mesh(me)
    bm.free()
    o = bpy.data.objects.new("Membrane" + side, me)
    bpy.context.scene.collection.objects.link(o)
    o.data.materials.append(m["wing"])
    kit.solidify(o, 0.04)
    vu = o.vertex_groups.new(name=upper)
    vf = o.vertex_groups.new(name=fore)
    for v in o.data.vertices:
        x = abs(v.co.x)
        # Inside the elbow on the upper arm, past it on the forearm, with a
        # blend across the elbow.
        k = min(max((x - P(ELBOW).x * sx + 0.4) / 1.2, 0.0), 1.0)
        if 1.0 - k > 0:
            vu.add([v.index], 1.0 - k, "REPLACE")
        if k > 0:
            vf.add([v.index], k, "REPLACE")
    parts = [o]

    def spar(name, a, b, r0, r1, bone, material):
        pa, pb = P(a), P(b)
        d = pb - pa
        mid = (pa + pb) / 2
        q = d.to_track_quat("Z", "Y")
        obj = kit.cyl(name, r0, d.length, mid, material, verts=5, r2=r1)
        obj.rotation_mode = "QUATERNION"
        obj.rotation_quaternion = q
        return kit.bind(obj, bone)

    parts.append(spar("Arm" + side, SHOULDER, ELBOW, 0.16, 0.1, upper, m["hide"]))
    parts.append(spar("Fore" + side, ELBOW, WRIST, 0.1, 0.08, fore, m["hide"]))
    for k, f in enumerate(FINGERS):
        parts.append(spar("Finger%s%d" % (side, k), WRIST, f, 0.06, 0.02, fore, m["dark"]))
    # A claw at the wrist.
    parts.append(kit.bind(kit.cyl("Thumb" + side, 0.07, 0.3, P((WRIST[0] + 0.05, WRIST[1] - 0.25)), m["horn"],
                                  verts=4, r2=0.006, rot=(R(90), 0, 0)), fore))
    return parts, [(upper, "chest", P(SHOULDER)[:]), (fore, upper, P(ELBOW)[:])]


def build():
    kit.reset()
    m = materials()
    bones = list(BONES)
    parts = [loft(m)]
    parts += spikes(m)
    parts += head(m)
    parts += legs(m, bones)
    for side, sx in (("L", 1), ("R", -1)):
        p, b = wing(m, side, sx)
        parts += p
        bones += b
    body = kit.join("Dragon", parts)
    kit.flat([body])
    arm = kit.armature("Dragon_Rig", [(b, parent, head, (head[0], head[1], head[2] + 0.1))
                                      for b, parent, head in bones])
    kit.skin(arm, body)
    return arm, body


# --- Clips ----------------------------------------------------------------


def smooth(x):
    x = min(max(x, 0.0), 1.0)
    return x * x * (3 - 2 * x)


def window(t, a, b):
    """0 before `a`, 1 after `b`, eased between."""
    return smooth((t - a) / max(b - a, 1e-6))


def pulse(t, a, b, c, d):
    """Rises over a..b, holds, falls over c..d."""
    return window(t, a, b) * (1.0 - window(t, c, d))


def folded(k=1.0):
    """The wings folded along the flanks, `k` of the way from spread."""
    return {
        "wing_L": (R(-62) * k, R(68) * k, R(18) * k),
        "wing_R": (R(-62) * k, R(-68) * k, R(-18) * k),
        "fore_L": (R(8) * k, R(48) * k, R(-6) * k),
        "fore_R": (R(8) * k, R(-48) * k, R(6) * k),
    }


def tail_wave(t, amp, lag=0.18, pitch=0.0, freq=1.0):
    return {
        "tail%d" % (i + 1): (pitch, amp * math.sin(TAU * (freq * t - lag * i)) * (0.6 + 0.2 * i), 0)
        for i in range(5)
    }


def legs_still(k=0.0):
    out = {}
    for leg in ("FL", "FR", "BL", "BR"):
        out["leg_" + leg] = (k, 0, 0)
        out["shin_" + leg] = (-k * 0.5, 0, 0)
    return out


def tucked():
    """Legs drawn back in flight."""
    out = {}
    for leg in ("FL", "FR"):
        out["leg_" + leg] = (R(55), 0, 0)
        out["shin_" + leg] = (R(-70), 0, 0)
    for leg in ("BL", "BR"):
        out["leg_" + leg] = (R(70), 0, 0)
        out["shin_" + leg] = (R(-20), 0, 0)
    return out


def merge(*parts):
    out = {}
    for p in parts:
        out.update(p)
    return out


def idle(t):
    s = math.sin(TAU * t)
    return merge(
        {
            "body": {"rot": (0.015 * s, 0, 0), "loc": (0, 0.03 * (1 + s), 0)},
            "chest": (-0.025 * s, 0, 0),
            "neck1": (R(6) + 0.04 * s, 0.06 * math.sin(TAU * t + 0.5), 0),
            "neck2": (0.03 * math.sin(TAU * t + 0.6), 0.05 * math.sin(TAU * t + 0.9), 0),
            "neck3": (0, 0.04 * math.sin(TAU * t + 1.2), 0),
            "head": (R(-8) - 0.04 * s, 0.12 * math.sin(TAU * t + 1.5), 0),
            "jaw": (0.03 * max(0.0, s), 0, 0),
        },
        tail_wave(t, 0.09),
        legs_still(),
        folded(),
    )


def walk(t):
    gait = {"BL": 0.0, "FL": 0.25, "BR": 0.5, "FR": 0.75}
    k = {
        "body": {"rot": (0, 0.03 * math.sin(TAU * t), 0.025 * math.sin(TAU * t)),
                 "loc": (0, 0.06 * (1 + math.cos(TAU * 2 * t)) / 2, 0)},
        "chest": (0, -0.04 * math.sin(TAU * t), 0),
        "neck1": (R(10), -0.06 * math.sin(TAU * t), 0),
        "neck2": (0.04 * math.cos(TAU * 2 * t), -0.04 * math.sin(TAU * t + 0.4), 0),
        "neck3": (0, 0, 0),
        "head": (R(-10) - 0.04 * math.cos(TAU * 2 * t), 0.06 * math.sin(TAU * t), 0),
        "jaw": (0, 0, 0),
    }
    for leg, phase in gait.items():
        a = TAU * (t + phase)
        k["leg_" + leg] = (0.42 * math.sin(a), 0, 0)
        k["shin_" + leg] = (-0.55 * max(0.0, -math.cos(a)) if leg[0] == "B" else 0.6 * max(0.0, -math.cos(a)), 0, 0)
    return merge(k, tail_wave(t, 0.16, lag=0.22), folded())


def spread(beat, lag):
    """The wings spread, beating: `beat` from -1 (down) to 1 (up)."""
    return {
        "wing_L": (0, R(-6), R(38) * beat + R(4)),
        "wing_R": (0, R(6), -R(38) * beat - R(4)),
        "fore_L": (0, R(4) * beat, R(26) * lag),
        "fore_R": (0, -R(4) * beat, -R(26) * lag),
    }


def fly(t):
    # A quick downstroke and a slower upstroke.
    phase = (t + 0.1) % 1.0
    beat = math.cos(TAU * phase) if phase < 0.5 else math.cos(TAU * phase)
    lag = math.cos(TAU * (phase - 0.12))
    return merge(
        {
            "body": {"rot": (R(-4) + 0.03 * beat, 0, 0), "loc": (0, -0.15 * beat, 0)},
            "chest": (0.02 * beat, 0, 0),
            "neck1": (R(48), 0, 0),
            "neck2": (R(12) - 0.04 * beat, 0, 0),
            "neck3": (R(4), 0, 0),
            "head": (R(-34) + 0.05 * beat, 0, 0),
            "jaw": (0, 0, 0),
        },
        tail_wave(t, 0.06, lag=0.15, pitch=R(-2)),
        tucked(),
        spread(beat, lag),
    )


def glide(t):
    s = math.sin(TAU * t)
    return merge(
        {
            "body": {"rot": (R(-2), 0, 0.04 * s), "loc": (0, 0.05 * s, 0)},
            "chest": (0, 0, 0),
            "neck1": (R(52), 0.04 * s, 0),
            "neck2": (R(12), 0, 0),
            "neck3": (R(4), 0, 0),
            "head": (R(-36), -0.05 * s, 0),
            "jaw": (0, 0, 0),
        },
        tail_wave(t, 0.08, lag=0.2, pitch=R(-3)),
        tucked(),
        {
            "wing_L": (0.02 * s, R(-10), R(9) + 0.05 * s),
            "wing_R": (0.02 * s, R(10), -R(9) - 0.05 * s),
            "fore_L": (0, R(-6), R(-4)),
            "fore_R": (0, R(6), R(4)),
        },
    )


def bite(t):
    wind = pulse(t, 0.0, 0.3, 0.32, 0.45)
    strike = pulse(t, 0.3, 0.42, 0.62, 0.95)
    gape = pulse(t, 0.22, 0.36, 0.44, 0.52)
    return merge(
        {
            "body": {"rot": (R(-4) * wind + R(5) * strike, 0, 0), "loc": (0, 0, 0)},
            "chest": (R(-6) * wind + R(4) * strike, 0, 0),
            "neck1": (R(6) - R(18) * wind + R(42) * strike, 0, 0),
            "neck2": (R(-10) * wind + R(16) * strike, 0, 0),
            "neck3": (R(-6) * wind + R(6) * strike, 0, 0),
            "head": (R(-8) + R(18) * wind - R(26) * strike, 0, 0),
            "jaw": (R(40) * gape, 0, 0),
        },
        tail_wave(t, 0.12, lag=0.2),
        legs_still(),
        folded(),
    )


def breath(t):
    # Draw in (0 to 0.25), then thrust the head forward and breathe with
    # the jaws wide (0.3 to 0.85), then recover.
    draw = pulse(t, 0.0, 0.22, 0.25, 0.34)
    blow = pulse(t, 0.24, 0.34, 0.82, 0.97)
    rumble = 0.02 * math.sin(TAU * 9 * t) * blow
    return merge(
        {
            "body": {"rot": (R(-7) * draw + R(3) * blow, 0, 0), "loc": (0, 0.12 * draw, 0)},
            "chest": (R(-10) * draw + R(2) * blow, 0, 0),
            "neck1": (R(6) - R(22) * draw + R(36) * blow, 0, 0),
            "neck2": (R(-14) * draw + R(14) * blow + rumble, 0, 0),
            "neck3": (R(-8) * draw + R(4) * blow, 0, 0),
            "head": (R(-8) + R(28) * draw - R(18) * blow + rumble, 0, 0),
            "jaw": (R(10) * draw + R(46) * blow, 0, 0),
        },
        tail_wave(t, 0.1 + 0.1 * blow, lag=0.2, freq=2.0),
        legs_still(),
        folded(1.0 - 0.35 * blow),
    )


def roar(t):
    rear = pulse(t, 0.0, 0.3, 0.75, 1.0)
    gape = pulse(t, 0.22, 0.32, 0.72, 0.88)
    shake = 0.05 * math.sin(TAU * 7 * t) * gape
    fold = 1.0 - 0.75 * pulse(t, 0.1, 0.3, 0.7, 0.95)
    return merge(
        {
            "body": {"rot": (R(-16) * rear, 0, 0), "loc": (0, 0.35 * rear, 0)},
            "chest": (R(-8) * rear, 0, 0),
            "neck1": (R(6) - R(14) * rear, shake, 0),
            "neck2": (R(-6) * rear, shake, 0),
            "neck3": (R(-4) * rear, 0, 0),
            "head": (R(-8) - R(30) * gape, shake, 0),
            "jaw": (R(50) * gape, 0, 0),
            "leg_FL": (R(-30) * rear, 0, 0),
            "leg_FR": (R(-30) * rear, 0, 0),
            "shin_FL": (R(50) * rear, 0, 0),
            "shin_FR": (R(50) * rear, 0, 0),
            "leg_BL": (R(14) * rear, 0, 0),
            "leg_BR": (R(14) * rear, 0, 0),
            "shin_BL": (R(-10) * rear, 0, 0),
            "shin_BR": (R(-10) * rear, 0, 0),
        },
        tail_wave(t, 0.25 * rear + 0.05, lag=0.15, freq=2.0),
        folded(fold),
        {
            "wing_L": (R(-62) * fold, R(68) * fold, R(18) * fold + R(30) * (1 - fold)),
            "wing_R": (R(-62) * fold, R(-68) * fold, -R(18) * fold - R(30) * (1 - fold)),
        },
    )


def sweep(t):
    # The body twists and the tail swings a wide arc around to one side,
    # then back.
    swing = math.sin(math.pi * window(t, 0.05, 0.75)) * pulse(t, 0.0, 0.1, 0.8, 1.0)
    out = {
        "body": {"rot": (0, R(-14) * swing, R(-4) * swing), "loc": (0, 0, 0)},
        "chest": (0, R(-10) * swing, 0),
        "neck1": (R(6), R(20) * swing, 0),
        "neck2": (0, R(12) * swing, 0),
        "neck3": (0, 0, 0),
        "head": (R(-8), R(10) * swing, 0),
        "jaw": (R(12) * swing, 0, 0),
    }
    lag = [0.0, 0.05, 0.1, 0.15, 0.2]
    for i in range(5):
        k = math.sin(math.pi * window(t, 0.05 + lag[i], 0.7 + lag[i] * 0.5))
        out["tail%d" % (i + 1)] = (R(-4) * k, R(34) * k, 0)
    return merge(out, legs_still(), folded())


CLIPS = [
    # (name, frames, keys, cyclic, step)
    ("idle", 12, idle, True, 8),
    ("walk", 16, walk, True, 3),
    ("fly", 12, fly, True, 2),
    ("glide", 12, glide, True, 8),
    ("bite", 14, bite, False, 2),
    ("breath", 24, breath, False, 2),
    ("roar", 24, roar, False, 2),
    ("sweep", 18, sweep, False, 2),
]


VIEWS = {
    # name: (camera position, target)
    "three": ((-9.0, -8.0, 6.5), (0, 0.8, 2.6)),
    "side": ((-15.0, 0.6, 3.2), (0, 0.6, 2.6)),
    "top": ((0.0, 0.6, 18.0), (0, 0.6, 2.0)),
    "front": ((0.0, -15.0, 4.0), (0, 0.0, 2.8)),
}


def preview(arm, folder, views=("three",), phases=(0.0, 0.25, 0.5, 0.75)):
    """Render each clip at a few phases from each of `views`."""
    os.makedirs(folder, exist_ok=True)
    s = bpy.context.scene
    s.render.engine = "BLENDER_EEVEE"
    s.render.resolution_x = 480
    s.render.resolution_y = 360
    s.world = bpy.data.worlds.new("w")
    s.world.color = (0.55, 0.6, 0.65)
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    s.collection.objects.link(cam)
    cam.data.lens = 30
    s.camera = cam
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    sun.rotation_euler = (0.8, 0.2, -0.6)
    sun.data.energy = 3.0
    s.collection.objects.link(sun)
    for track in arm.animation_data.nla_tracks:
        track.mute = True
    for name, frames, _, cyclic, step in CLIPS:
        act = bpy.data.actions[name]
        arm.animation_data.action = act
        if hasattr(arm.animation_data, "action_slot") and act.slots:
            arm.animation_data.action_slot = act.slots[0]
        last = 1 + (frames if cyclic else frames - 1) * step
        for view in views:
            at, target = VIEWS[view]
            cam.location = Vector(at)
            up = "Y" if view != "top" else "Y"
            cam.rotation_euler = (Vector(target) - cam.location).to_track_quat("-Z", up).to_euler()
            for k, f in enumerate(phases):
                s.frame_set(int(1 + f * (last - 1)))
                s.render.filepath = os.path.join(folder, "%s_%s_%d.png" % (name, view, k))
                bpy.ops.render.render(write_still=True)
    arm.animation_data.action = None


def main():
    a = kit.args()
    prev = None
    views = ("three",)
    if "--views" in a:
        i = a.index("--views")
        views = tuple(a[i + 1].split(","))
        a = a[:i] + a[i + 2 :]
    if "--preview" in a:
        i = a.index("--preview")
        prev = a[i + 1]
        a = a[:i] + a[i + 2 :]
    folder = a[0] if a else os.path.join(kit.REPO, "assets", "verse", "generated")
    arm, body = build()
    for name, frames, keys, cyclic, step in CLIPS:
        kit.action(arm, name, frames, keys, cyclic=cyclic, step=step)
    tris = kit.triangles()
    assert tris < 4000, tris
    kit.export(os.path.join(folder, "dragon.glb"), animations=True)
    if prev:
        preview(arm, prev, views)


main()
