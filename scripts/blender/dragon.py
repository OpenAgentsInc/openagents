"""Build Everglade's Shapechange dragon and write it as glTF.

Firm reason: 6 (Blender's bpy models, rigs, animates and exports it; it has no C API).

Run headless:
    Blender -b --factory-startup --python scripts/blender/dragon.py -- [OUT_DIR] [--stills DIR] [--preview DIR [--views V,...]]

Writes `dragon.gltf` and `dragon.bin` (default folder `assets/metaverse/beasts`): a stylized
low-poly young red dragon in the Quaternius look, a Reference model built
from primitives with no source asset, under 4,000 triangles. `--stills DIR`
renders the review stills (dragon-model-front.png, -side.png, -below.png);
`--preview DIR` renders each clip at a few phases.

Style rules, from the Quaternius kits the zone already carries: chunky,
readable silhouettes; few segments, flat-shaded facets for scales; flat
saturated colors in a few tones (crimson hide, a gold belly, a wine-dark
wing membrane with a warmer underside, ivory horns and claws, glowing amber
eyes); no textures.

The body, neck, and tail are one lofted tube along the spine, skinned
smoothly across the spine's bones: a deep keeled chest, a slim waist, a long
tapered neck and a whip of a tail. The head, legs, horns, plates, and wings
are solids weighted to one bone each, as `animals.py` builds its beasts: a
wedge head with curved horns, cheek frills and a jaw with a hinge angle and
chin; muscled shoulders and haunches; feet of three clawed toes and a
dewclaw; a ridge of back plates that leaves the saddle clear. Each wing has
an arm, forearm and hand bone; four jointed finger bones under a membrane
scalloped between them. The wings are rigged spread (the bind pose) and fold
along the flanks on the ground. The `saddle` bone, on the chest, marks where
a rider sits.

Clips, in the Metaverse's clip vocabulary: `idle` (breathing, the tail swaying),
`walk` (a four-beat walk), `fly` (wingbeats), `glide` (wings held, the
tail streaming), `bite`, `breath` (drawing in, then a breath held with the
jaws wide), `roar` (rearing, wings half open), `sweep` (the tail swung
around), and the mount's: `idle_saddled` (standing still for a rider, the
neck lowered), `takeoff`, `hover`, `land`, `bank_left` and `bank_right`.
Every bone points up +Z from its pivot, so `rx` pitches, `ry` yaws, and `rz`
rolls about the forward axis. The dragon faces -Y in Blender, which is +Z
(glTF's front) after export; its feet stand at the origin's height. 1 unit =
1 m: it stands about 5.8 m to the tips of its horns, 14 m from snout to tail,
and 22.5 m across the spread wings (over 1.5 times its length, to lift a
rider).
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
# half height). The dragon faces -Y. A long whip of a tail, a deep keeled
# chest for the wing muscles, a slim waist, and a long neck tapering to the
# head: proportions for flight.
SPINE = [
    (8.5, 0.9, 0.03, 0.03),
    (7.8, 0.96, 0.07, 0.06),
    (7.05, 1.06, 0.11, 0.1),
    (6.3, 1.2, 0.16, 0.14),
    (5.5, 1.4, 0.22, 0.2),
    (4.7, 1.64, 0.29, 0.26),
    (3.9, 1.9, 0.37, 0.34),
    (3.1, 2.15, 0.47, 0.43),
    (2.3, 2.36, 0.6, 0.56),
    (1.5, 2.48, 0.68, 0.66),
    (0.7, 2.55, 0.72, 0.76),
    (-0.1, 2.66, 0.84, 0.9),
    (-0.85, 2.8, 0.8, 0.88),
    (-1.5, 3.08, 0.58, 0.62),
    (-1.95, 3.5, 0.43, 0.45),
    (-2.3, 3.95, 0.36, 0.37),
    (-2.6, 4.38, 0.31, 0.32),
    (-2.82, 4.75, 0.28, 0.29),
    (-2.95, 5.0, 0.26, 0.27),
]
SIDES = 10

# book:begin dragon-bones
# Bones: (name, parent, head). Spine bones sit on the spine; their weights
# along the loft blend between neighbors. `saddle` carries no mesh: it marks
# where a rider sits, astride the base of the neck.
SADDLE = (0, -1.2, 3.62)
BONES = [
    ("body", None, (0, 0.7, 2.55)),
    ("chest", "body", (0, -0.85, 2.8)),
    ("saddle", "chest", SADDLE),
    ("neck1", "chest", (0, -1.55, 3.1)),
    ("neck2", "neck1", (0, -2.05, 3.6)),
    ("neck3", "neck2", (0, -2.5, 4.2)),
    ("head", "neck3", (0, -2.95, 5.0)),
    ("jaw", "head", (0, -3.1, 4.88)),
    ("tail1", "body", (0, 2.3, 2.36)),
    ("tail2", "tail1", (0, 3.9, 1.9)),
    ("tail3", "tail2", (0, 5.5, 1.4)),
    ("tail4", "tail3", (0, 6.7, 1.12)),
    ("tail5", "tail4", (0, 7.6, 0.98)),
]
# book:end dragon-bones
# Each spine bone's place along the loft, as a y position, tail to neck;
# the loft's weights blend linearly between neighboring bones' places.
CHAIN = [
    ("tail5", 7.9),
    ("tail4", 6.9),
    ("tail3", 5.6),
    ("tail2", 4.0),
    ("tail1", 2.6),
    ("body", 0.9),
    ("chest", -0.85),
    ("neck1", -1.6),
    ("neck2", -2.1),
    ("neck3", -2.55),
    ("head", -2.92),
]

# The head is modeled about the head bone (HEAD_AT) and drawn this much larger.
HEAD_AT = Vector((0, -2.95, 5.0))
HEAD_SCALE = 1.25
# The wings are drawn this much larger than modeled, about the shoulder.
WING_SCALE = 1.45

LEGS = {
    # tag: (side x, hip or shoulder y, hip z, knee (y, z), ankle (y, z), toe y, thigh radius)
    "B": (0.8, 1.55, 2.3, (1.0, 1.35), (1.75, 0.42), 1.15, 0.56),
    "F": (0.76, -0.9, 2.35, (-0.62, 1.3), (-0.95, 0.4), -1.45, 0.36),
}


def materials():
    return dict(
        hide=kit.mat("Dragon_Hide", (0.56, 0.08, 0.06), 0.75),
        belly=kit.mat("Dragon_Belly", (0.92, 0.54, 0.07), 0.8),
        wing=kit.mat("Dragon_Membrane", (0.33, 0.05, 0.08), 0.85),
        # The membrane's underside, lit through from the sky: a warmer red, so the wings read
        # from below.
        under=kit.mat("Dragon_MembraneUnder", (0.62, 0.16, 0.1), 0.85),
        horn=kit.mat("Dragon_Horn", (0.9, 0.84, 0.68), 0.6),
        dark=kit.mat("Dragon_Ridge", (0.2, 0.04, 0.05), 0.7),
        eye=kit.mat("Dragon_Eye", (1.0, 0.75, 0.15), 0.3, emit=(1.0, 0.6, 0.1), strength=6.0),
        mouth=kit.mat("Dragon_Mouth", (0.42, 0.06, 0.08), 0.9),
    )


def rod(name, a, b, r0, r1, material, verts=5):
    """A tapered rod from point `a` (radius `r0`) to `b` (radius `r1`)."""
    a, b = Vector(a), Vector(b)
    d = b - a
    o = kit.cyl(name, r0, d.length, (a + b) / 2, material, verts=verts, r2=r1)
    o.rotation_mode = "QUATERNION"
    o.rotation_quaternion = d.to_track_quat("Z", "Y")
    return o


def fan(name, hub, tips, material, thickness=0.03):
    """A thin membrane fanned from `hub` across `tips`, in order."""
    bm = bmesh.new()
    h = bm.verts.new(hub)
    t = [bm.verts.new(p) for p in tips]
    for p, q in zip(t, t[1:]):
        bm.faces.new((h, p, q))
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    o = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(o)
    o.data.materials.append(material)
    kit.solidify(o, thickness)
    return o


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


def plates(m):
    """A ridge of dark back plates from the neck to the tail, a large and a small one per
    segment, leaving the saddle clear, and a spade at the tail's tip."""
    parts = []
    for i, (y, z, w, h) in enumerate(SPINE[1:-1], start=1):
        if -1.62 < y < -0.7:
            continue
        nxt = SPINE[i + 1]
        for k, f in enumerate((0.0, 0.5)):
            yy = y + (nxt[0] - y) * f
            zz = z + (nxt[1] - z) * f
            hh = h + (nxt[3] - h) * f
            s = min(hh, 0.55) * (0.95 if k == 0 else 0.6)
            if s < 0.07:
                continue
            # A diamond plate, thin across the back, swept toward the tail.
            o = kit.ball("Plate%d_%d" % (i, k), 1.0, (0, yy + s * 0.15, zz + hh * 0.98 + s * 0.38), m["dark"],
                         segs=4, rings=2, scale=(0.05, s * 0.75, s), rot=(R(-22), 0, 0))
            parts.append(kit.bind(o, bone_at(yy)))
    o = kit.ball("Spade", 1.0, (0, 8.75, 0.9), m["dark"], segs=4, rings=2, scale=(0.4, 0.7, 0.06))
    parts.append(kit.bind(o, "tail5"))
    return parts


def head(m):
    """The head on the `head` bone, the lower jaw on `jaw`: a wedge skull, a long snout,
    heavy brows, curved swept-back horns, cheek frills of spines and membrane, glowing eyes,
    teeth, and a shaped jaw with an angle at the hinge and a squared chin."""
    parts = []
    hide, horn, dark = m["hide"], m["horn"], m["dark"]
    H = lambda x, y, z: HEAD_AT + Vector((x, y, z))
    def on(o, bone="head"):
        parts.append(kit.bind(o, bone))
    on(kit.ball("Skull", 0.5, H(0, -0.2, 0.12), hide, segs=8, rings=5, scale=(0.78, 1.1, 0.66)))
    on(kit.cyl("Snout", 0.3, 1.0, H(0, -0.9, 0.06), hide, verts=6, r2=0.17, rot=(R(95), 0, 0)))
    on(kit.box("Muzzle", (0.32, 0.34, 0.15), H(0, -1.3, 0.09), hide, rot=(R(6), 0, 0)))
    on(kit.box("Palate", (0.28, 0.9, 0.04), H(0, -0.86, -0.07), m["mouth"]))
    on(kit.cyl("NoseHorn", 0.06, 0.22, H(0, -1.3, 0.24), horn, verts=4, r2=0.006, rot=(R(-30), 0, 0)))
    for sx in (-1, 1):
        on(kit.box("Brow%d" % sx, (0.22, 0.5, 0.12), H(sx * 0.26, -0.5, 0.33), dark,
                   rot=(R(-14), 0, sx * R(-16))))
        on(kit.ball("Eye%d" % sx, 0.085, H(sx * 0.33, -0.56, 0.21), m["eye"], segs=6, rings=4,
                    scale=(0.7, 1.2, 0.8)))
        on(kit.ball("Nostril%d" % sx, 0.05, H(sx * 0.12, -1.43, 0.15), dark, segs=5, rings=3))
        # Horns: a long pair curving back in two pieces, a short pair below them.
        b0 = H(sx * 0.27, 0.05, 0.42)
        b1 = H(sx * 0.33, 0.55, 0.72)
        b2 = H(sx * 0.4, 1.12, 0.66)
        on(rod("Horn%d" % sx, b0, b1, 0.12, 0.07, horn, verts=6))
        on(rod("HornTip%d" % sx, b1, b2, 0.07, 0.01, horn, verts=5))
        on(rod("HornLow%d" % sx, H(sx * 0.4, 0.0, 0.14), H(sx * 0.62, 0.42, 0.02), 0.07, 0.01, horn))
        # A cheek frill: three spines fanned back from the jaw's hinge, webbed.
        hub = H(sx * 0.4, -0.08, -0.04)
        tips = [H(sx * 0.72, 0.3, 0.3), H(sx * 0.86, 0.42, 0.0), H(sx * 0.7, 0.36, -0.32)]
        on(fan("Frill%d" % sx, hub, tips, m["wing"]))
        for k, t in enumerate(tips):
            on(rod("FrillSpine%d_%d" % (sx, k), hub, t + (t - hub) * 0.18, 0.05, 0.006, dark, verts=4))
        for k in range(3):
            y = -0.8 - k * 0.22
            on(kit.cyl("Tooth%d_%d" % (sx, k), 0.035, 0.12, H(sx * 0.16, y, -0.12), horn, verts=4, r2=0.004,
                       rot=(R(180), 0, 0)))
    # The lower jaw, hinged at the back of the skull: the mandible, its angle at the hinge on
    # each side, and a squared chin with two short barbs under it.
    on(kit.cyl("Jaw", 0.23, 1.05, H(0, -0.8, -0.2), hide, verts=6, r2=0.12, rot=(R(94), 0, 0)), "jaw")
    on(kit.box("Chin", (0.24, 0.16, 0.13), H(0, -1.3, -0.21), hide), "jaw")
    on(kit.box("Tongue", (0.18, 0.7, 0.04), H(0, -0.8, -0.08), m["mouth"]), "jaw")
    for sx in (-1, 1):
        on(kit.box("JawAngle%d" % sx, (0.12, 0.42, 0.3), H(sx * 0.27, -0.2, -0.2), hide,
                   rot=(R(22), 0, sx * R(8))), "jaw")
        on(rod("Barb%d" % sx, H(sx * 0.1, -1.2, -0.25), H(sx * 0.14, -1.05, -0.5), 0.04, 0.005, dark, verts=4),
           "jaw")
        for k in range(2):
            y = -0.85 - k * 0.25
            on(kit.cyl("LowTooth%d_%d" % (sx, k), 0.03, 0.1, H(sx * 0.12, y, -0.04), horn, verts=4, r2=0.004),
               "jaw")
    for o in parts:
        o.location = HEAD_AT + (Vector(o.location) - HEAD_AT) * HEAD_SCALE
        o.scale = tuple(c * HEAD_SCALE for c in o.scale)
    return parts


def limb(name, rings, sides=6):
    """One leg as a single welded tube through `rings` of (point, radius, {bone: weight}), each
    ring square to the limb's direction there, so hip, knee and ankle bend as one limb."""
    bm = bmesh.new()
    loops = []
    pts = [Vector(p) for p, _, _ in rings]
    for i, (p, r, _) in enumerate(rings):
        a, b = pts[max(i - 1, 0)], pts[min(i + 1, len(pts) - 1)]
        d = (b - a).normalized()
        u = Vector((1, 0, 0))
        v = d.cross(u).normalized()
        loops.append([bm.verts.new(pts[i] + (u * math.cos(TAU * k / sides) + v * math.sin(TAU * k / sides)) * r)
                      for k in range(sides)])
    for r0, r1 in zip(loops, loops[1:]):
        for k in range(sides):
            j = (k + 1) % sides
            bm.faces.new((r0[k], r0[j], r1[j], r1[k]))
    bm.faces.new(list(reversed(loops[0])))
    bm.faces.new(loops[-1])
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    o = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(o)
    groups = {}
    for i, (_, _, wts) in enumerate(rings):
        for bone, w in wts.items():
            if bone not in groups:
                groups[bone] = o.vertex_groups.new(name=bone)
            groups[bone].add([i * sides + k for k in range(sides)], w, "REPLACE")
    return o


def legs(m, bones):
    """Four legs, each one welded tube from inside the body through the knee to the ankle and
    down into the foot: a muscled haunch or shoulder over the hip, the hind thighs heavier than
    the forelegs since they bear the weight; a foot flat on the ground under the ankle, three
    toes each with an ivory claw, and a dewclaw."""
    parts = []
    hide, horn = m["hide"], m["horn"]
    for tag, (x0, y0, z0, knee, ankle, toe, r) in LEGS.items():
        for side, sx in (("L", 1), ("R", -1)):
            x = sx * x0
            upper, lower = "leg_%s%s" % (tag, side), "shin_%s%s" % (tag, side)
            bones += [(upper, "body" if tag == "B" else "chest", (x, y0, z0)), (lower, upper, (x, knee[0], knee[1]))]
            hip = Vector((x, y0, z0 + 0.15))
            kn = Vector((x, knee[0], knee[1]))
            an = Vector((x, ankle[0], ankle[1]))
            U, L, J = {upper: 1.0}, {lower: 1.0}, {upper: 0.5, lower: 0.5}
            o = limb("Leg" + tag + side, [
                (hip, r * 0.85, U),
                (hip + (kn - hip) * 0.45, r * 1.05, U),
                (kn, r * 0.62, J),
                (kn + (an - kn) * 0.5, r * 0.45, L),
                (an, r * 0.38, L),
                (Vector((x, ankle[0], 0.16)), r * 0.42, L),
            ])
            o.data.materials.append(hide)
            parts.append(o)
            if tag == "B":
                parts.append(kit.bind(kit.ball("Haunch" + side, 0.74, (x + sx * 0.06, y0 + 0.02, z0 - 0.05), hide,
                                               segs=8, rings=5, scale=(0.6, 1.12, 1.05)), upper))
            else:
                parts.append(kit.bind(kit.ball("Shoulder" + side, 0.47, (x + sx * 0.05, y0 - 0.05, z0 + 0.02), hide,
                                               segs=8, rings=5, scale=(0.55, 0.95, 1.0)), upper))
            fwd = -1.0 if toe < ankle[0] else 1.0
            ball_y = ankle[0] + (toe - ankle[0]) * 0.55
            heel_y = ankle[0] - fwd * 0.12
            # The foot: a pad on the ground from the heel to the ball, under the ankle.
            parts.append(kit.bind(kit.box("Foot" + tag + side, (r * 1.15, abs(ball_y - heel_y), 0.26),
                                          (x, (heel_y + ball_y) / 2, 0.13), hide, bevel=0.05), lower))
            for k in (-1, 0, 1):
                base = Vector((x + k * r * 0.3, ball_y - fwd * 0.08, 0.12))
                tip = Vector((x + k * r * 0.55, toe + fwd * (0.04 if k == 0 else -0.06), 0.08))
                parts.append(kit.bind(rod("Toe%s%s%d" % (tag, side, k), base, tip, 0.1, 0.07, hide, verts=4), lower))
                parts.append(kit.bind(rod("Claw%s%s%d" % (tag, side, k), tip, tip + Vector((k * 0.03, fwd * 0.2, -0.07)),
                                          0.06, 0.005, horn, verts=4), lower))
            heel = Vector((x, heel_y, 0.26))
            parts.append(kit.bind(rod("Dewclaw%s%s" % (tag, side), heel, heel + Vector((0, -fwd * 0.16, -0.14)),
                                      0.045, 0.004, horn, verts=4), lower))
    return parts


# The left wing in the bind pose, spread flat at shoulder height; the right
# mirrors it. Points are (x, y) in the membrane's plane: the arm to the
# wrist, four fingers fanned back from it, and the membrane's root at the hip.
SHOULDER = (0.58, -0.95)
ELBOW = (2.7, -1.5)
WRIST = (4.4, -0.85)
FINGERS = [(7.8, 0.3), (7.0, 1.6), (5.9, 2.6), (4.6, 3.2)]
ROOT = (0.62, 0.9)
WING_Z = 3.15


def wing(m, side, sx):
    """One membranous wing: the arm, a clawed thumb and four jointed finger bones over a
    membrane scalloped between them, dark on top and a warmer red beneath. The arm rides
    `wing_*`, the forearm `fore_*`, the fingers `hand_*`, and the root stays on the body."""
    upper, fore, hand = "wing_" + side, "fore_" + side, "hand_" + side
    z = WING_Z

    def P(p, dz=0.0):
        # Grown about the shoulder by WING_SCALE.
        x = SHOULDER[0] + (p[0] - SHOULDER[0]) * WING_SCALE
        y = SHOULDER[1] + (p[1] - SHOULDER[1]) * WING_SCALE
        return Vector((sx * x, y, z + dz))

    def lerp(a, b, f):
        return (a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f)

    def scallop(a, b, depth):
        """Points along the trailing edge from `a` to `b`, drawn in toward the wrist."""
        out = []
        for f, k in ((0.25, 0.65), (0.5, 1.0), (0.75, 0.65)):
            q = lerp(a, b, f)
            out.append(lerp(q, WRIST, depth * k))
        return out

    edge = []
    for a, b in zip(FINGERS, FINGERS[1:]):
        edge += [a] + scallop(a, b, 0.24)
    edge += [FINGERS[-1]] + scallop(FINGERS[-1], ROOT, 0.16) + [ROOT]
    bm = bmesh.new()
    s, e, w = (bm.verts.new(P(p)) for p in (SHOULDER, ELBOW, WRIST))
    rim = [bm.verts.new(P(p)) for p in edge]
    faces = [(s, e, w), (s, w, rim[-1])] + [(w, p, q) for p, q in zip(rim, rim[1:])]
    for tri in faces:
        bm.faces.new(tri if sx > 0 else tuple(reversed(tri)))
    me = bpy.data.meshes.new("Membrane" + side)
    bm.to_mesh(me)
    bm.free()
    o = bpy.data.objects.new("Membrane" + side, me)
    bpy.context.scene.collection.objects.link(o)
    o.data.materials.append(m["wing"])
    o.data.materials.append(m["under"])
    kit.solidify(o, 0.04)
    # The shoulder and elbow ride the upper arm, the wrist the forearm, the rim between the
    # fingers the hand, and the rim from the last finger back to the root blends from the
    # hand to the body.
    groups = {g: o.vertex_groups.new(name=g) for g in (upper, fore, hand, "body")}
    tail = len(edge) - 5  # the last finger's index in `edge`; three scallops and the root follow
    for i, v in enumerate(o.data.vertices):
        if i < 3:
            groups[(upper, upper, fore)[i]].add([i], 1.0, "REPLACE")
            continue
        j = i - 3
        f = 0.0 if j <= tail else (j - tail) / 4.0
        if f < 1.0:
            groups[hand].add([i], 1.0 - f, "REPLACE")
        if f > 0.0:
            groups["body"].add([i], f, "REPLACE")
    parts = [o]
    # The bones ride just under the membrane, so they show from below.
    def bone(name, a, b, r0, r1, group, material, verts=5):
        parts.append(kit.bind(rod(name, a, b, r0, r1, material, verts=verts), group))

    bone("Arm" + side, P(SHOULDER, -0.04), P(ELBOW, -0.04), 0.17, 0.1, upper, m["hide"], 6)
    bone("Fore" + side, P(ELBOW, -0.04), P(WRIST, -0.04), 0.1, 0.08, fore, m["hide"])
    for name, at, r, group in (("Elbow", ELBOW, 0.13, upper), ("Wrist", WRIST, 0.12, fore)):
        parts.append(kit.bind(kit.ball(name + side, r, P(at, -0.04), m["hide"], segs=6, rings=4), group))
    for k, f in enumerate(FINGERS):
        # Each finger bends once, toward the trailing edge, at a knuckle.
        mid = lerp(WRIST, f, 0.5)
        d = (f[0] - WRIST[0], f[1] - WRIST[1])
        mid = (mid[0] - d[1] * 0.06, mid[1] + d[0] * 0.06)
        r = 0.07 - 0.01 * k
        bone("Finger%s%d" % (side, k), P(WRIST, -0.03), P(mid, -0.03), r, r * 0.75, hand, m["dark"], 4)
        bone("FingerTip%s%d" % (side, k), P(mid, -0.03), P(f, -0.02), r * 0.75, 0.015, hand, m["dark"], 4)
        parts.append(kit.bind(kit.ball("Knuckle%s%d" % (side, k), r * 1.05, P(mid, -0.03), m["dark"], segs=4,
                                       rings=3), hand))
    # A hooked thumb claw at the wrist's leading edge.
    t0 = P((WRIST[0] + 0.05, WRIST[1] - 0.12), -0.02)
    bone("Thumb" + side, t0, t0 + Vector((sx * 0.08, -0.38, -0.06)), 0.07, 0.006, fore, m["horn"], 4)
    return parts, [(upper, "chest", P(SHOULDER)[:]), (fore, upper, P(ELBOW)[:]), (hand, fore, P(WRIST)[:])]


def build():
    kit.reset()
    m = materials()
    bones = list(BONES)
    parts = [loft(m)]
    parts += plates(m)
    parts += head(m)
    parts += legs(m, bones)
    for side, sx in (("L", 1), ("R", -1)):
        p, b = wing(m, side, sx)
        parts += p
        bones += b
    body = kit.join("Dragon", parts)
    kit.flat([body])
    # The membrane's downward faces take the underside's color.
    names = [mt.name for mt in body.data.materials]
    top, under = names.index("Dragon_Membrane"), names.index("Dragon_MembraneUnder")
    for poly in body.data.polygons:
        if poly.material_index in (top, under):
            poly.material_index = under if poly.normal.z < -0.5 else top
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
    """The wings folded along the flanks, `k` of the way from spread: the elbow raised behind
    the shoulder, the forearm run forward to a wrist high beside the shoulder, and the hand
    turned back so the fingers trail along the body, the first rising over the back and the
    last running back above the hind leg, which shows below the wing. The angles were fitted to those directions for these bones.
    A part fold turns `k` of the full fold about its own axis (kit.turn), so the wing moves on
    one arc; scaling the Euler angles instead swung it through poses on the way."""
    return {bone: kit.turn(rot, k) for bone, rot in FOLD.items()}


FOLD = {
    "wing_L": (R(-60.8), R(41.3), R(53.7)),
    "wing_R": (R(-60.8), R(-41.3), R(-53.7)),
    "fore_L": (R(-6.5), R(-25.8), R(-146.5)),
    "fore_R": (R(-6.5), R(25.8), R(146.5)),
    "hand_L": (R(47.2), R(-46.3), R(146.2)),
    "hand_R": (R(47.2), R(46.3), R(-146.2)),
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
    # The hands are keyed in every clip (spread unless a part folds them), so no clip
    # inherits another's fold.
    out = {"hand_L": (0, 0, 0), "hand_R": (0, 0, 0)}
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
        "fore_L": (0, R(4) * beat, R(18) * lag),
        "fore_R": (0, -R(4) * beat, -R(18) * lag),
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


def idle_saddled(t):
    """Standing for a rider: the back held level and still, the neck lowered and forward so the
    rider sees past the head, the wings folded tight, slow breaths and a lazy tail."""
    s = math.sin(TAU * t)
    return merge(
        {
            "body": {"rot": (0.006 * s, 0, 0), "loc": (0, 0.015 * (1 + s), 0)},
            "chest": (-0.01 * s, 0, 0),
            "neck1": (R(18) + 0.02 * s, 0.03 * math.sin(TAU * t + 0.5), 0),
            "neck2": (R(-4) + 0.02 * math.sin(TAU * t + 0.6), 0.03 * math.sin(TAU * t + 0.9), 0),
            "neck3": (R(-4), 0.03 * math.sin(TAU * t + 1.2), 0),
            "head": (R(-12) - 0.03 * s, 0.08 * math.sin(TAU * t + 1.5), 0),
            "jaw": (0.02 * max(0.0, s), 0, 0),
        },
        tail_wave(t, 0.06, lag=0.15),
        legs_still(),
        folded(1.05),
    )


def flight_body(pitch=R(-4), roll=0.0, lift=0.0):
    return {
        "body": {"rot": (pitch, 0, roll), "loc": (0, lift, 0)},
        "chest": (0, 0, 0),
        "neck1": (R(48), 0, 0),
        "neck2": (R(12), 0, 0),
        "neck3": (R(4), 0, 0),
        "head": (R(-34), 0, 0),
        "jaw": (0, 0, 0),
    }


def blend(a, b, k):
    """Each bone's keys `k` of the way from pose `a` to pose `b`."""
    out = {}
    for bone in set(a) | set(b):
        va, vb = a.get(bone, (0, 0, 0)), b.get(bone, (0, 0, 0))
        ra, la = (va["rot"], va.get("loc", (0, 0, 0))) if isinstance(va, dict) else (va, (0, 0, 0))
        rb, lb = (vb["rot"], vb.get("loc", (0, 0, 0))) if isinstance(vb, dict) else (vb, (0, 0, 0))
        rot = kit.slerp(ra, rb, k)   # on the sphere: Euler lerps swing through odd poses
        loc = tuple(x + (y - x) * k for x, y in zip(la, lb))
        out[bone] = {"rot": rot, "loc": loc} if bone == "body" else rot
    return out


def takeoff(t):
    """A crouch with the wings raised, a spring with the first downstroke, then two beats
    climbing out with the legs drawn up, ending in the flying pose."""
    crouch = pulse(t, 0.0, 0.25, 0.3, 0.42)
    spring = window(t, 0.3, 0.45)
    # Wings: raised over the crouch, then two full beats.
    beat = 1.0 if t < 0.3 else math.cos(TAU * (t - 0.3) / 0.35)
    lag = 1.0 if t < 0.3 else math.cos(TAU * ((t - 0.3) / 0.35 - 0.12))
    wings = blend(folded(), spread(beat, lag), window(t, 0.0, 0.22))
    ground = merge(idle_saddled(0.0), legs_still(R(14) * crouch))
    for leg in ("BL", "BR", "FL", "FR"):
        ground["shin_" + leg] = (R(-30) * crouch, 0, 0)
    ground["body"] = {"rot": (R(-6) * crouch, 0, 0), "loc": (0, -0.35 * crouch, 0)}
    air = merge(flight_body(pitch=R(-14) * (1 - window(t, 0.6, 1.0)) + R(-4) * window(t, 0.6, 1.0),
                            lift=0.5 * pulse(t, 0.3, 0.5, 0.7, 1.0)), tucked())
    return merge(blend(ground, air, spring), wings, tail_wave(t, 0.08, lag=0.15, pitch=R(-3) * spring))


def hover(t):
    """Holding in the air: the body raised nose up, the neck curved forward, deep beats swept
    forward and back, the legs hanging, the tail down."""
    phase = (t + 0.1) % 1.0
    beat = math.cos(TAU * phase)
    lag = math.cos(TAU * (phase - 0.12))
    out = merge(
        {
            "body": {"rot": (R(-30) + 0.03 * beat, 0, 0), "loc": (0, -0.18 * beat, 0)},
            "chest": (R(-4), 0, 0),
            "neck1": (R(46), 0, 0),
            "neck2": (R(10), 0, 0),
            "neck3": (R(6), 0, 0),
            "head": (R(-26) + 0.04 * beat, 0, 0),
            "jaw": (0, 0, 0),
        },
        tail_wave(t, 0.05, lag=0.15, pitch=R(-6)),
        legs_still(R(-8)),
        {
            "wing_L": (R(14) * beat, R(-10), R(44) * beat + R(6)),
            "wing_R": (R(14) * beat, R(10), -R(44) * beat - R(6)),
            "fore_L": (0, R(6) * beat, R(30) * lag),
            "fore_R": (0, -R(6) * beat, -R(30) * lag),
        },
    )
    out["shin_BL"] = out["shin_BR"] = (R(18), 0, 0)
    return out


def land(t):
    """From flight: the body pitched up to flare, the wings spread wide and braking, the legs
    reaching down; the touchdown taken in a crouch; the wings folding as it settles to stand
    for its rider."""
    flare = window(t, 0.0, 0.35)
    touch = pulse(t, 0.45, 0.55, 0.62, 0.85)
    settle = window(t, 0.55, 1.0)
    beat = math.cos(TAU * min(t, 0.5) / 0.5) * (1 - window(t, 0.35, 0.55))
    air = merge(flight_body(pitch=R(-4) - R(28) * flare), tucked())
    reach = legs_still(R(-22))
    for leg in ("BL", "BR", "FL", "FR"):
        reach["shin_" + leg] = (R(20), 0, 0)
    air = blend(air, merge(air, reach), window(t, 0.15, 0.45))
    ground = merge(idle_saddled(0.0), legs_still(R(12) * touch))
    for leg in ("BL", "BR", "FL", "FR"):
        ground["shin_" + leg] = (R(-26) * touch, 0, 0)
    ground["body"] = {"rot": (R(-8) * touch, 0, 0), "loc": (0, -0.3 * touch, 0)}
    wings = blend(spread(beat * 0.8 + 0.2, beat), folded(1.05), settle)
    return merge(blend(air, ground, window(t, 0.4, 0.55)), wings,
                 tail_wave(t, 0.08, lag=0.15, pitch=R(-4) * (1 - settle)))


def bank(turn):
    """Gliding through a turn to the dragon's left (`turn` 1) or right (-1): rolled into it,
    the low wing drawn in a little and the high wing reaching, the head turned into the
    turn and held level, the tail trailing out of it."""
    def keys(t):
        s = math.sin(TAU * t)
        out = merge(
            flight_body(pitch=R(-3), roll=-turn * (R(30) + 0.03 * s), lift=0.04 * s),
            tail_wave(t, 0.05, lag=0.2, pitch=R(-3)),
            tucked(),
        )
        out["neck1"] = (R(50), turn * R(14), 0)
        out["neck2"] = (R(12), turn * R(8), 0)
        out["head"] = (R(-36), turn * R(6), turn * R(22))
        for i in range(5):
            out["tail%d" % (i + 1)] = (R(-3), -turn * R(5) + 0.04 * math.sin(TAU * t - 0.2 * i), 0)
        low, high = ("L", "R") if turn > 0 else ("R", "L")
        sgn = {"L": 1, "R": -1}
        out["wing_" + low] = (0, sgn[low] * R(-14), sgn[low] * R(4))
        out["fore_" + low] = (0, sgn[low] * R(-12), sgn[low] * R(-8))
        out["wing_" + high] = (0, sgn[high] * R(-6), sgn[high] * (R(14) + 0.04 * s))
        out["fore_" + high] = (0, 0, sgn[high] * R(-2))
        return out
    return keys


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
    ("idle_saddled", 12, idle_saddled, True, 8),
    ("takeoff", 20, takeoff, False, 2),
    ("hover", 12, hover, True, 2),
    ("land", 20, land, False, 2),
    ("bank_left", 12, bank(1), True, 8),
    ("bank_right", 12, bank(-1), True, 8),
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


STILLS = {
    # file: (clip, phase, camera position, target, lens)
    "dragon-model-front.png": ("idle_saddled", 0.0, (-9.5, -12.5, 5.8), (0, 0.4, 3.6), 36),
    "dragon-model-side.png": ("idle_saddled", 0.0, (-21.0, 1.6, 3.6), (0, 1.6, 3.4), 36),
    "dragon-model-below.png": ("glide", 0.25, (-4.0, 2.5, -15.0), (0, 1.5, 3.0), 20),
}


def stills(arm, folder):
    """The review stills: a front three-quarter and a side view standing for a rider, and the
    view from below in flight."""
    os.makedirs(folder, exist_ok=True)
    s = bpy.context.scene
    s.render.engine = "BLENDER_EEVEE"
    s.render.resolution_x = 1280
    s.render.resolution_y = 960
    s.world = bpy.data.worlds.new("w")
    s.world.use_nodes = True
    sky = s.world.node_tree.nodes["Background"]
    sky.inputs[0].default_value = (0.55, 0.65, 0.78, 1.0)
    sky.inputs[1].default_value = 0.55
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    s.collection.objects.link(cam)
    s.camera = cam
    for name, rot, energy in (("sun", (0.8, 0.2, -0.6), 3.0), ("fill", (2.6, -0.3, 2.4), 0.8)):
        light = bpy.data.objects.new(name, bpy.data.lights.new(name, "SUN"))
        light.rotation_euler = rot
        light.data.energy = energy
        s.collection.objects.link(light)
    for track in arm.animation_data.nla_tracks:
        track.mute = True
    frames = {name: (n, cyclic, step) for name, n, _, cyclic, step in CLIPS}
    for out, (clip, phase, at, target, lens) in STILLS.items():
        n, cyclic, step = frames[clip]
        act = bpy.data.actions[clip]
        arm.animation_data.action = act
        if hasattr(arm.animation_data, "action_slot") and act.slots:
            arm.animation_data.action_slot = act.slots[0]
        last = 1 + (n if cyclic else n - 1) * step
        s.frame_set(int(1 + phase * (last - 1)))
        cam.data.lens = lens
        cam.location = Vector(at)
        cam.rotation_euler = (Vector(target) - cam.location).to_track_quat("-Z", "Y").to_euler()
        s.render.filepath = os.path.join(folder, out)
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
    still = None
    if "--stills" in a:
        i = a.index("--stills")
        still = a[i + 1]
        a = a[:i] + a[i + 2 :]
    if "--preview" in a:
        i = a.index("--preview")
        prev = a[i + 1]
        a = a[:i] + a[i + 2 :]
    folder = a[0] if a else os.path.join(kit.REPO, "assets", "verse", "beasts")
    arm, body = build()
    for name, frames, keys, cyclic, step in CLIPS:
        kit.action(arm, name, frames, keys, cyclic=cyclic, step=step)
    tris = kit.triangles()
    assert tris < 4000, tris
    kit.export(os.path.join(folder, "dragon.gltf"), animations=True)
    if prev:
        preview(arm, prev, views)
    if still:
        stills(arm, still)


main()
