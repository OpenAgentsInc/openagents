"""Build the Grove's sacred-grove props as binary glTF.

Run headless:
    Blender -b --factory-startup --python scripts/blender/grove_props.py -- [OUT_DIR] [NAME ...]

OUT_DIR defaults to `assets/verse/generated/grove`. Each model also gets
`<name>.footprint.json` beside it when it blocks walking.

Reference models, built from primitives in the flat-shaded low-poly style of
`town_props.py`, with no kit pieces or images. They share that script's
material names and values (`Prop_Stone`, `Prop_Moss`, `Prop_Bark`, and the
rest), so the pack keeps one copy of each:

- `grove_standing_stone`: a weathered menhir, 2.6 m, leaning slightly, with
  a moss band on its crown.
- `grove_rune_stone`: a 1.4 m stone whose carved runes glow.
- `grove_altar`: a mossy stone slab on two uprights with a bowl, apples,
  gourds, and three lit candles.
- `grove_oak`: the great ancient oak at the grove's heart: a twisted trunk
  with flaring roots and a dark hollow, under a broad crown of leaf clumps.
- `grove_brazier`: an iron bowl on three legs with coals and flames.
- `grove_torch`: a 2.2 m post with an iron cage and a flame.
- `grove_campfire`: a ring of stones round crossed logs, embers, and flames.
- `grove_archery_butt`: a straw target with painted rings on an A-frame.
- `grove_training_ring`: a 6 m arc of a sparring ring's posts and rails.
- `grove_hanging_lantern`: a small iron lantern on a hook and chain.

Glowing materials are named `Emit...` (`EmitFlame`, `EmitEmber`,
`EmitRune`, `EmitLantern`), with their base color the glow's color: the
Everglade pack carries no emission, so Verse reads the glow from the name.
No other material name starts with `Emit`. They also carry glTF emission.

Frame: 1 unit = 1 m; fronts face -Y in Blender, +Z after export; the origin
is the center of the base on the ground. Randomness is seeded per model.
"""

import json
import math
import os
import random
import sys

import bpy
from mathutils import Vector

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

# Shared with town_props.py and street_props.py: the same names and values.
IRON = (0.035, 0.035, 0.04)
OAK = (0.32, 0.17, 0.07)
OAK_DARK = (0.18, 0.09, 0.035)
STONE = (0.42, 0.4, 0.36)
STONE_DARK = (0.26, 0.25, 0.23)

FLAME = (1.0, 0.55, 0.15)
EMBER = (1.0, 0.3, 0.05)
RUNE = (0.35, 1.0, 0.6)
LANTERN = (1.0, 0.75, 0.4)


def materials():
    return {
        "iron": kit.mat("Prop_Iron", IRON, 0.5),
        "oak": kit.mat("Prop_Oak", OAK, 0.85),
        "oak_dark": kit.mat("Prop_OakDark", OAK_DARK, 0.85),
        "stone": kit.mat("Prop_Stone", STONE, 0.95),
        "stone_dark": kit.mat("Prop_StoneDark", STONE_DARK, 0.95),
        "rock": kit.mat("Prop_Rock", (0.3, 0.3, 0.28), 0.95),
        "moss": kit.mat("Prop_Moss", (0.1, 0.24, 0.04), 1.0),
        "bark": kit.mat("Prop_Bark", (0.16, 0.08, 0.035), 0.9),
        "leaf": kit.mat("Prop_Leaf", (0.07, 0.2, 0.04), 0.9),
        "leaf_dark": kit.mat("Prop_LeafDark", (0.045, 0.13, 0.03), 0.9),
        "canopy": kit.mat("Prop_Canopy", (0.07, 0.2, 0.04), 0.9),
        "canopy_light": kit.mat("Prop_CanopyLight", (0.12, 0.27, 0.05), 0.9),
        "white": kit.mat("Prop_FlowerWhite", (0.85, 0.82, 0.75), 0.7),
        "fruit": kit.mat("Prop_Fruit", (0.62, 0.06, 0.03), 0.6),
        "hay": kit.mat("Prop_Hay", (0.62, 0.48, 0.16), 1.0),
        "hay_dark": kit.mat("Prop_HayDark", (0.45, 0.33, 0.1), 1.0),
        "red": kit.mat("Prop_FlowerRed", (0.7, 0.05, 0.05), 0.7),
        "paint_white": kit.mat("Prop_PaintWhite", (0.78, 0.76, 0.7), 0.8),
        "gold": kit.mat("Prop_FlowerGold", (0.75, 0.42, 0.02), 0.7),
        "sack": kit.mat("Prop_Sack", (0.55, 0.42, 0.25), 1.0),
        # This script's own.
        "hollow": kit.mat("Grove_Hollow", (0.015, 0.01, 0.006), 1.0),
        "lichen": kit.mat("Grove_Lichen", (0.36, 0.4, 0.22), 1.0),
        "wax": kit.mat("Grove_Wax", (0.86, 0.8, 0.62), 0.6),
        "gourd": kit.mat("Grove_Gourd", (0.7, 0.36, 0.05), 0.7),
        "coal": kit.mat("Grove_Coal", (0.03, 0.025, 0.02), 1.0),
        "flame": kit.mat("EmitFlame", FLAME, 0.5, emit=FLAME, strength=1500.0),
        "ember": kit.mat("EmitEmber", EMBER, 0.8, emit=EMBER, strength=800.0),
        "rune": kit.mat("EmitRune", RUNE, 0.5, emit=RUNE, strength=300.0),
        "lantern": kit.mat("EmitLantern", LANTERN, 0.3, emit=LANTERN, strength=900.0),
    }


def rock_mesh(name, r, loc, material, seed, squash=0.7, subdiv=1, stretch=(1, 1, 1)):
    """A rough rock: an icosphere with its vertices pushed in and out."""
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=subdiv, radius=r)
    o = bpy.context.object
    rng = random.Random(seed)
    for v in o.data.vertices:
        k = rng.uniform(0.82, 1.1)
        v.co.x *= k * stretch[0]
        v.co.y *= k * rng.uniform(0.9, 1.1) * stretch[1]
        v.co.z *= k * squash * stretch[2]
    o.name = name
    o.location = loc
    o.data.materials.append(material)
    return o


def slab(name, w, d, h, loc, material, seed, taper=0.8, jitter=0.06, rot=(0, 0, 0)):
    """A rough-hewn stone block: a box tapering to its top, corners nudged."""
    bpy.ops.mesh.primitive_cube_add(size=1)
    o = bpy.context.object
    rng = random.Random(seed)
    for v in o.data.vertices:
        top = v.co.z > 0
        k = taper if top else 1.0
        v.co.x = v.co.x * w * k + rng.uniform(-jitter, jitter)
        v.co.y = v.co.y * d * k + rng.uniform(-jitter, jitter)
        v.co.z = v.co.z * h + (rng.uniform(-jitter, jitter) if top else 0.0)
    mod = o.modifiers.new("Bevel", "BEVEL")
    mod.width = min(w, d) * 0.12
    mod.segments = 1
    o.name = name
    o.location = loc
    o.rotation_euler = rot
    o.data.materials.append(material)
    return o


def strut(name, r0, r1, start, end, material, verts=5):
    """A tapering rod from `start` to `end`."""
    d = Vector(end) - Vector(start)
    o = kit.cyl(name, r0, d.length, tuple((Vector(start) + Vector(end)) / 2), material, verts=verts, r2=r1)
    o.rotation_mode = "QUATERNION"
    o.rotation_quaternion = Vector((0, 0, 1)).rotation_difference(d)
    return o


def flame(name, r, h, loc, m, rot=0.0):
    """A licking flame: an outer orange cone over a smaller ember core.
    The core's base stands a little above the cone's, so the two bases
    don't share a plane."""
    kit.cyl(name, r, h, (loc[0], loc[1], loc[2] + h / 2), m["flame"], verts=5, r2=0.0, rot=(0, 0, rot))
    kit.cyl(name + "Core", r * 0.55, h * 0.5, (loc[0], loc[1], loc[2] + h * 0.33), m["ember"], verts=4, r2=0.0)


# --- Stones ------------------------------------------------------------------


def grove_standing_stone(m):
    """A weathered menhir, leaning a little, with moss on its crown."""
    stone = slab("Menhir", 0.95, 0.6, 2.6, (0, 0, 1.3), m["stone"], 11, taper=0.62, jitter=0.07)
    # Weather it: one more cut keeps the silhouette irregular.
    sub = stone.modifiers.new("Sub", "SUBSURF")
    sub.levels = 1
    sub.subdivision_type = "SIMPLE"
    disp_rng = random.Random(12)
    kit.bake(stone)
    for v in stone.data.vertices:
        v.co.x += disp_rng.uniform(-0.04, 0.04)
        v.co.y += disp_rng.uniform(-0.04, 0.04)
    stone.rotation_euler = (0.05, -0.06, 0)
    kit.ball("Moss", 0.4, (0.04, 0.0, 2.52), m["moss"], segs=7, rings=4, scale=(1.1, 0.85, 0.32),
             rot=(0.05, -0.06, 0))
    kit.ball("Lichen", 0.22, (-0.3, -0.2, 0.9), m["lichen"], segs=6, rings=3, scale=(1.0, 0.25, 1.3))
    rock_mesh("Foot", 0.32, (0.45, -0.25, 0.08), m["rock"], 13, squash=0.5, subdiv=1)
    kit.ball("Turf", 0.55, (0, 0, 0.0), m["moss"], segs=8, rings=3, scale=(1.4, 1.1, 0.2))


def grove_rune_stone(m):
    """A squat stone with glowing runes carved on its face."""
    slab("Stone", 0.85, 0.42, 1.4, (0, 0, 0.7), m["stone_dark"], 21, taper=0.7, jitter=0.05)
    kit.ball("Moss", 0.32, (0.08, 0.02, 1.36), m["moss"], segs=7, rings=3, scale=(1.2, 0.7, 0.3))
    # Runes on the front face (-Y), which tapers inward with height.
    face = lambda z: -(0.21 * (1 - (z / 1.4) * 0.3)) - 0.02  # noqa: E731
    strokes = [
        # (x, z, length, angle) in the face's plane; angle 0 is vertical.
        (-0.18, 0.95, 0.34, 0.0), (-0.18, 1.02, 0.16, 0.8), (-0.18, 0.88, 0.16, -0.8),
        (0.05, 0.95, 0.34, 0.0), (0.12, 1.04, 0.18, 0.7),
        (0.24, 0.95, 0.3, 0.25),
        (-0.12, 0.5, 0.3, 0.0), (-0.04, 0.55, 0.18, -0.9), (-0.04, 0.45, 0.18, 0.9),
        (0.16, 0.5, 0.3, 0.0), (0.2, 0.5, 0.16, 1.57),
    ]
    for i, (x, z, length, angle) in enumerate(strokes):
        y = face(z)
        kit.box(f"Rune{i}", (0.035, 0.02, length), (x, y, z), m["rune"], rot=(0.1, angle, 0))
    kit.ball("Turf", 0.5, (0, 0, 0.0), m["moss"], segs=8, rings=3, scale=(1.3, 0.9, 0.18))


# --- The altar ----------------------------------------------------------------


def grove_altar(m):
    """A druid's stone altar: a slab on two uprights, with offerings."""
    for sx in (-1, 1):
        # The uprights stand 2 cm into the step, so their bottoms don't
        # share the ground plane with the step's.
        slab(f"Upright{sx}", 0.38, 0.62, 0.78, (sx * 0.62, 0, 0.41), m["stone"], 30 + sx, taper=0.9, jitter=0.03)
    slab("Top", 1.9, 0.95, 0.2, (0, 0, 0.9), m["stone"], 33, taper=0.95, jitter=0.03)
    kit.box("Step", (2.3, 1.4, 0.12), (0, 0, 0.06), m["stone_dark"], bevel=0.04)
    # Moss over the top's back edge and down one upright.
    kit.ball("MossTop", 0.5, (0.35, 0.3, 1.0), m["moss"], segs=8, rings=4, scale=(1.6, 0.55, 0.22))
    kit.ball("MossSide", 0.3, (-0.62, 0.3, 0.55), m["moss"], segs=6, rings=3, scale=(0.9, 0.5, 1.5))
    kit.ball("MossFoot", 0.4, (0.7, -0.5, 0.12), m["moss"], segs=7, rings=3, scale=(1.4, 0.8, 0.3))
    top = 1.0
    # A carved stone bowl with fruit.
    kit.lathe("Bowl", [(0.0, 0.0), (0.12, 0.0), (0.2, 0.08), (0.18, 0.1), (0.1, 0.04), (0.0, 0.04)],
              loc=(0, -0.05, top), material=m["stone_dark"], segs=10)
    for i, (x, y, r) in enumerate(((-0.06, -0.08, 0.07), (0.07, -0.02, 0.065), (0.0, 0.05, 0.06))):
        kit.ball(f"Apple{i}", r, (x, y - 0.05, top + 0.1), m["fruit"], segs=6, rings=3)
    # Gourds, a sheaf, and three candles.
    kit.ball("Gourd", 0.11, (0.48, -0.15, top + 0.09), m["gourd"], segs=8, rings=5, scale=(1, 1, 0.8))
    kit.cyl("GourdStem", 0.015, 0.06, (0.48, -0.15, top + 0.2), m["oak_dark"], verts=4)
    kit.ball("Gourd2", 0.08, (0.62, 0.05, top + 0.07), m["gourd"], segs=7, rings=4, scale=(1, 1, 1.2))
    for i in range(3):
        a = (i - 1) * 0.16
        kit.cyl(f"Wheat{i}", 0.02, 0.5, (-0.5 + math.sin(a) * 0.2, 0.15, top + 0.22), m["hay"], verts=4,
                rot=(0, a, 0))
    kit.cyl("Band", 0.07, 0.04, (-0.5, 0.15, top + 0.18), m["red"], verts=6)
    for i, (x, y, h) in enumerate(((-0.78, -0.25, 0.22), (-0.66, -0.3, 0.15), (0.8, 0.25, 0.26))):
        kit.cyl(f"Candle{i}", 0.04, h, (x, y, top + h / 2), m["wax"], verts=6)
        kit.cyl(f"Drip{i}", 0.065, 0.015, (x, y, top + 0.008), m["wax"], verts=6)
        flame(f"Flame{i}", 0.025, 0.09, (x, y, top + h + 0.005), m)


# --- The great oak --------------------------------------------------------------


def grove_oak(m):
    """The grove's ancient oak: twisted trunk, roots, hollow, broad crown."""
    rng = random.Random(41)

    def along(name, r0, r1, start, end, material, verts=7, gnarl=0.0):
        """A tapering limb from `start` to `end`, gnarled when asked."""
        d = Vector(end) - Vector(start)
        mid = (Vector(start) + Vector(end)) / 2
        o = kit.cyl(name, r0, d.length * 1.06, tuple(mid), material, verts=verts, r2=r1)
        if gnarl:
            for v in o.data.vertices:
                k = rng.uniform(1 - gnarl, 1 + gnarl)
                v.co.x *= k
                v.co.y *= k
        o.rotation_mode = "QUATERNION"
        o.rotation_quaternion = Vector((0, 0, 1)).rotation_difference(d)
        return o

    # The trunk: a squat, wandering stack of wide segments, broad at the foot.
    rings = [(0.0, 1.45, 0.0, 0.0), (0.8, 1.05, 0.1, 0.04), (2.0, 0.92, 0.22, -0.06),
             (3.3, 0.84, 0.08, -0.16), (4.6, 0.74, -0.12, -0.06)]
    for i in range(len(rings) - 1):
        z0, r0, x0, y0 = rings[i]
        z1, r1, x1, y1 = rings[i + 1]
        along(f"Trunk{i}", r0, r1, (x0, y0, z0), (x1, y1, z1), m["bark"], verts=8, gnarl=0.12)
    # Burls on the trunk.
    for i, (a, z, r) in enumerate(((0.6, 1.3, 0.34), (2.6, 2.2, 0.3), (4.4, 3.1, 0.26))):
        kit.ball(f"Burl{i}", r, (0.95 * math.cos(a), 0.95 * math.sin(a), z), m["bark"], segs=5, rings=3)
    # The hollow, facing front (-Y): a dark recess with a lip.
    kit.ball("Hollow", 0.42, (0.08, -0.92, 1.35), m["hollow"], segs=8, rings=5, scale=(0.7, 0.3, 1.3))
    kit.ring("HollowLip", 0.4, 0.09, (0.08, -0.96, 1.35), m["bark"], segs=9, minor_segs=4,
             rot=(math.pi / 2, 0, 0))
    kit.ball("HollowMoss", 0.25, (0.08, -1.0, 0.85), m["moss"], segs=6, rings=3, scale=(1.4, 0.6, 0.4))
    # Flaring roots, arching out of the ground and back into it.
    for i in range(7):
        a = i / 7 * 2 * math.pi + rng.uniform(-0.2, 0.2)
        reach = rng.uniform(2.1, 2.8)
        r = rng.uniform(0.3, 0.42)
        c, s_ = math.cos(a), math.sin(a)
        knee = (c * reach * 0.5, s_ * reach * 0.5, rng.uniform(0.3, 0.45))
        along(f"RootUp{i}", r * 1.4, r, (c * 0.7, s_ * 0.7, 0.9), knee, m["bark"], verts=5)
        along(f"RootDown{i}", r, 0.14, knee, (c * reach, s_ * reach, -0.1), m["bark"], verts=5)
    # Main limbs: broad and low, as an old oak's crown spreads.
    top = (-0.12, -0.06, 4.5)
    limbs = []
    for i in range(6):
        a = i / 6 * 2 * math.pi + 0.3 + rng.uniform(-0.2, 0.2)
        rise = rng.uniform(0.3, 0.6)
        length = rng.uniform(2.8, 3.4)
        d = Vector((math.cos(a) * math.cos(rise), math.sin(a) * math.cos(rise), math.sin(rise)))
        elbow = Vector(top) + d * length * 0.55
        tip = elbow + Vector((d.x, d.y, d.z + 0.6)).normalized() * length * 0.5
        along(f"Limb{i}", 0.42, 0.24, top, tuple(elbow), m["bark"], verts=5)
        along(f"Bough{i}", 0.24, 0.1, tuple(elbow), tuple(tip), m["bark"], verts=5)
        limbs.append(tuple(tip))
    along("Leader", 0.5, 0.2, top, (0.2, 0.1, 8.0), m["bark"], verts=6)
    # The crown: broad, flattened leaf clumps over the boughs and the top.
    clumps = limbs + [(0.2, 0.1, 8.7), (1.6, 1.0, 8.2), (-1.7, -0.9, 8.3)]
    for i, (x, y, z) in enumerate(clumps):
        r = rng.uniform(1.6, 2.0)
        tone = ("canopy", "canopy_light", "leaf_dark")[i % 3]
        rock_mesh(f"Clump{i}", r, (x, y, z + 0.35), m[tone], 50 + i, squash=0.6, subdiv=2,
                  stretch=(1.2, 1.2, 1.0))
    kit.ball("Turf", 1.8, (0, 0, 0.0), m["moss"], segs=10, rings=3, scale=(1.5, 1.4, 0.12))
    for i in range(3):
        a = 1.3 + i * 2.0
        kit.cyl(f"Cap{i}", 0.12, 0.05, (2.0 * math.cos(a), 2.0 * math.sin(a), 0.2), m["white"], verts=6, r2=0.03)


# --- Fire ---------------------------------------------------------------------


def grove_brazier(m):
    """An iron bowl on three legs, heaped with coals and burning."""
    for i in range(3):
        a = i / 3 * 2 * math.pi + math.pi / 2
        c, s_ = math.cos(a), math.sin(a)
        strut(f"Leg{i}", 0.035, 0.035, (0.16 * c, 0.16 * s_, 0.86), (0.38 * c, 0.38 * s_, 0.02), m["iron"])
        kit.ball(f"Foot{i}", 0.055, (0.38 * c, 0.38 * s_, 0.03), m["iron"], segs=5, rings=3)
    kit.ring("Brace", 0.29, 0.02, (0, 0, 0.4), m["iron"], segs=10, minor_segs=3)
    kit.lathe("Bowl", [(0.0, 0.0), (0.12, 0.0), (0.38, 0.18), (0.45, 0.3), (0.42, 0.3), (0.34, 0.2), (0.0, 0.16)],
              loc=(0, 0, 0.8), material=m["iron"], segs=12)
    kit.ring("Rim", 0.44, 0.03, (0, 0, 1.1), m["iron"], segs=12, minor_segs=3)
    kit.ball("Coals", 0.36, (0, 0, 1.02), m["coal"], segs=10, rings=4, scale=(1, 1, 0.3))
    rng = random.Random(61)
    for i in range(5):
        a = rng.uniform(0, 2 * math.pi)
        r = rng.uniform(0.0, 0.25)
        kit.ball(f"Ember{i}", 0.07, (r * math.cos(a), r * math.sin(a), 1.09), m["ember"], segs=4, rings=2)
    for i, (x, y, r, h) in enumerate(((0, 0, 0.17, 0.6), (0.14, 0.08, 0.1, 0.38), (-0.12, 0.1, 0.1, 0.42),
                                      (0.02, -0.15, 0.09, 0.34))):
        flame(f"Flame{i}", r, h, (x, y, 1.06), m, rot=i * 0.7)


def grove_torch(m):
    """A tall post with an iron cage at its head and a flame in it."""
    # The post stands 2 cm into the base, so their bottoms don't share
    # the ground plane.
    kit.cyl("Post", 0.06, 1.93, (0, 0, 0.985), m["oak_dark"], verts=6, r2=0.05)
    kit.cyl("Base", 0.12, 0.12, (0, 0, 0.06), m["stone_dark"], verts=6, r2=0.1)
    for i in range(4):
        a = i / 4 * 2 * math.pi + math.pi / 4
        kit.cyl(f"Bar{i}", 0.012, 0.32, (0.09 * math.cos(a), 0.09 * math.sin(a), 2.05), m["iron"], verts=4,
                rot=(-0.3 * math.sin(a), 0.3 * math.cos(a), 0))
    kit.ring("Hoop", 0.13, 0.015, (0, 0, 2.2), m["iron"], segs=8, minor_segs=3)
    kit.cyl("Cup", 0.07, 0.05, (0, 0, 1.96), m["iron"], verts=6, r2=0.09)
    kit.ball("Pitch", 0.075, (0, 0, 2.0), m["coal"], segs=6, rings=3)
    flame("Flame", 0.1, 0.36, (0, 0, 2.02), m)


def grove_campfire(m):
    """Stones round crossed logs, with embers and flames."""
    rng = random.Random(71)
    for i in range(10):
        a = i / 10 * 2 * math.pi + rng.uniform(-0.1, 0.1)
        rock_mesh(f"Stone{i}", rng.uniform(0.14, 0.19), (0.62 * math.cos(a), 0.62 * math.sin(a), 0.08),
                  m["stone" if i % 2 else "stone_dark"], 72 + i, squash=0.65, subdiv=1)
    kit.cyl("Ash", 0.5, 0.03, (0, 0, 0.015), m["coal"], verts=10)
    for i in range(4):
        a = i / 4 * math.pi + 0.3
        kit.cyl(f"Log{i}", 0.07, 0.95, (0, 0, 0.14 + 0.06 * i), m["bark"], verts=6,
                rot=(0, math.pi / 2 - 0.25, a))
        for s in (-1, 1):
            kit.cyl(f"LogEnd{i}{s}", 0.06, 0.02, (s * 0.47 * math.cos(a), s * 0.47 * math.sin(a), 0.13 + 0.06 * i),
                    m["sack"], verts=6, rot=(0, math.pi / 2 - 0.25 * s, a))
    for i in range(6):
        a = rng.uniform(0, 2 * math.pi)
        r = rng.uniform(0.05, 0.32)
        kit.ball(f"Ember{i}", rng.uniform(0.05, 0.08), (r * math.cos(a), r * math.sin(a), 0.06), m["ember"],
                 segs=4, rings=2)
    for i, (x, y, r, h) in enumerate(((0, 0, 0.2, 0.75), (0.15, 0.1, 0.12, 0.5), (-0.14, 0.08, 0.13, 0.55),
                                      (0.04, -0.16, 0.11, 0.45), (-0.08, -0.08, 0.09, 0.62))):
        flame(f"Flame{i}", r, h, (x, y, 0.12), m, rot=i * 0.9)


# --- Training ---------------------------------------------------------------


def grove_archery_butt(m):
    """A straw butt on an A-frame stand, its face painted with rings."""
    tilt = 0.22
    cz = 1.05
    # The straw boss: a thick disc facing front (-Y), leaned back.
    kit.cyl("Boss", 0.55, 0.3, (0, 0.05, cz), m["hay"], verts=14, rot=(math.pi / 2 - tilt, 0, 0))
    kit.ring("Binding", 0.55, 0.04, (0, 0.05, cz), m["hay_dark"], segs=14, minor_segs=3,
             rot=(math.pi / 2 - tilt, 0, 0))
    # Painted rings on the front face.
    fy = 0.05 - 0.155 * math.cos(tilt)
    fz = cz - 0.155 * math.sin(tilt) * -1
    for i, (r, mat) in enumerate(((0.44, "paint_white"), (0.34, "red"), (0.24, "paint_white"), (0.14, "gold"),
                                  (0.06, "red"))):
        off = 0.004 * (i + 1)
        kit.cyl(f"Ring{i}", r, 0.01, (0, fy - off * math.cos(tilt), fz + off * math.sin(tilt)), m[mat], verts=14,
                rot=(math.pi / 2 - tilt, 0, 0))
    # The A-frame: two front legs and a back prop.
    for sx in (-1, 1):
        kit.cyl(f"Leg{sx}", 0.04, 1.75, (sx * 0.45, -0.05, 0.85), m["oak"], verts=5, rot=(-tilt, sx * 0.12, 0))
    kit.cyl("Prop", 0.04, 1.5, (0, 0.5, 0.72), m["oak"], verts=5, rot=(0.55, 0, 0))
    kit.box("Rail", (1.05, 0.06, 0.06), (0, 0.05, 0.42), m["oak"])
    # A spent arrow in the boss.
    kit.cyl("Arrow", 0.008, 0.6, (0.12, fy - 0.2, fz + 0.12), m["oak"], verts=4, rot=(math.pi / 2 - 0.1, 0, 0.1))
    kit.cyl("Fletch", 0.03, 0.08, (0.14, fy - 0.47, fz + 0.15), m["paint_white"], verts=3,
            rot=(math.pi / 2 - 0.1, 0, 0.1))


def grove_training_ring(m):
    """A 6 m arc of a sparring ring: posts and two rails, curving round."""
    radius = 6.0
    span = 1.0  # radians: an arc of 6 m
    posts = 5
    pts = []
    for i in range(posts):
        a = -span / 2 + span * i / (posts - 1)
        x, y = radius * math.sin(a), radius - radius * math.cos(a)
        pts.append((x, y))
        kit.cyl(f"Post{i}", 0.08, 1.0, (x, y, 0.5), m["oak_dark"], verts=6)
        kit.cyl(f"Cap{i}", 0.09, 0.06, (x, y, 1.02), m["oak_dark"], verts=6, r2=0.05)
    for i in range(posts - 1):
        (x0, y0), (x1, y1) = pts[i], pts[i + 1]
        length = math.hypot(x1 - x0, y1 - y0)
        a = math.atan2(y1 - y0, x1 - x0)
        for z in (0.45, 0.85):
            kit.box(f"Rail{i}_{z}", (length + 0.1, 0.06, 0.09), ((x0 + x1) / 2, (y0 + y1) / 2, z), m["oak"],
                    rot=(0, 0, a))
    kit.cyl("Rope", 0.02, 0.5, (pts[2][0] + 0.02, pts[2][1] - 0.09, 0.7), m["sack"], verts=4)


def grove_hanging_lantern(m):
    """A small iron lantern under a hook and chain, to hang from a branch."""
    kit.ring("Hook", 0.05, 0.01, (0, 0, 0.72), m["iron"], segs=8, minor_segs=3, rot=(math.pi / 2, 0, 0))
    for i in range(3):
        kit.ring(f"Link{i}", 0.03, 0.008, (0, 0, 0.62 - i * 0.06), m["iron"], segs=6, minor_segs=3,
                 rot=(math.pi / 2, 0, i * math.pi / 2))
    kit.cyl("Cap", 0.11, 0.08, (0, 0, 0.44), m["iron"], verts=6, r2=0.03)
    kit.cyl("Glass", 0.075, 0.2, (0, 0, 0.3), m["lantern"], verts=6)
    for i in range(3):
        a = i / 3 * 2 * math.pi
        kit.box(f"Bar{i}", (0.015, 0.015, 0.22), (0.08 * math.cos(a), 0.08 * math.sin(a), 0.3), m["iron"])
    kit.cyl("Base", 0.1, 0.05, (0, 0, 0.18), m["iron"], verts=6, r2=0.07)


PROPS = {
    "grove_standing_stone": grove_standing_stone,
    "grove_rune_stone": grove_rune_stone,
    "grove_altar": grove_altar,
    "grove_oak": grove_oak,
    "grove_brazier": grove_brazier,
    "grove_torch": grove_torch,
    "grove_campfire": grove_campfire,
    "grove_archery_butt": grove_archery_butt,
    "grove_training_ring": grove_training_ring,
    "grove_hanging_lantern": grove_hanging_lantern,
}

FRAME = "glTF: 1 unit = 1 m, +Y up, +Z front, origin at the base center"

# Collision boxes in glTF axes (x, y up, z front): name, center, half extents.
FOOTPRINTS = {
    "grove_standing_stone": [("menhir", (0.0, 1.3, 0.0), (0.5, 1.3, 0.32))],
    "grove_rune_stone": [("stone", (0.0, 0.7, 0.0), (0.44, 0.7, 0.23))],
    "grove_altar": [("altar", (0.0, 0.5, 0.0), (1.0, 0.5, 0.5))],
    "grove_oak": [("trunk", (0.0, 2.3, 0.0), (1.1, 2.3, 1.1))],
    "grove_brazier": [("brazier", (0.0, 0.57, 0.0), (0.45, 0.57, 0.45))],
    "grove_torch": [("post", (0.0, 1.1, 0.0), (0.12, 1.1, 0.12))],
    "grove_campfire": [("fire", (0.0, 0.2, 0.0), (0.75, 0.2, 0.75))],
    "grove_archery_butt": [("butt", (0.0, 0.8, 0.0), (0.6, 0.8, 0.45))],
}


def ring_footprint():
    """One small box per post of the training ring's arc."""
    radius, span, posts = 6.0, 1.0, 5
    boxes = []
    for i in range(posts):
        a = -span / 2 + span * i / (posts - 1)
        x, y = radius * math.sin(a), radius - radius * math.cos(a)
        boxes.append((f"post{i}", (x, y)))
    return boxes


def finish(name, out):
    body = kit.join(name)
    # Texture-free: no coordinates to carry.
    while body.data.uv_layers:
        body.data.uv_layers.remove(body.data.uv_layers[0])
    # Keep the origin at the trunk or post centre rather than the bounds.
    kit.ground(body, recenter=False)
    kit.flat()
    info = kit.export(os.path.join(out, name + ".glb"))
    kit.flickers(os.path.join(out, name + ".glb"))
    boxes = []
    if name == "grove_training_ring":
        # Blender (x, y) maps to glTF (x, -z).
        for label, (x, y) in ring_footprint():
            boxes.append({"name": label, "center": [round(x, 3), 0.5, round(-y, 3)],
                          "half_extents": [0.12, 0.5, 0.12]})
        boxes.append({"name": "rails", "center": [0.0, 0.5, round(-(6.0 - 6.0 * math.cos(0.25)) / 2, 3)],
                      "half_extents": [1.5, 0.5, 0.12]})
    else:
        for label, center, half in FOOTPRINTS.get(name, []):
            boxes.append({"name": label, "center": list(center), "half_extents": list(half)})
    if boxes:
        doc = {"model": name + ".glb", "frame": FRAME, "triangles": info["triangles"], "boxes": boxes}
        with open(os.path.join(out, name + ".footprint.json"), "w") as f:
            f.write(json.dumps(doc, indent=2) + "\n")


def main():
    a = kit.args()
    out = a[0] if a else os.path.join(kit.REPO, "assets", "verse", "generated", "grove")
    names = a[1:] or list(PROPS)
    for name in names:
        kit.reset()
        PROPS[name](materials())
        finish(name, out)
    kit.fail_on_flickers()


main()
