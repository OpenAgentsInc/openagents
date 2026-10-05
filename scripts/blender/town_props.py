"""Build Everglade's park, garden, water, and woodland detail as binary glTF.

Run headless:
    Blender -b --factory-startup --python scripts/blender/town_props.py -- [OUT_DIR] [NAME ...]

The second round of the city's small pieces, after `street_props.py`, in the
same flat-shaded low-poly style and the same shared materials, so the pack
merges them by material. Each is a few hundred triangles at most:

- Sculpture Walk and plazas: `statue` (a robed figure holding up a lamp on
  a stone plinth), `sculpture` (a bronze ring on stacked stones), and
  `sundial`.
- Gardens: `planter` (a round stone planter with a shrub and flowers),
  `garden_arch` (a timber arch under climbing roses), `picket_fence` (2 m of
  white pickets), `garden_gate` (stone posts with a timber gate, for the
  dry-stone walls), `flower_bed` (a timber-edged bed of flowers), and
  `veg_bed` (a raised bed of cabbages and beans).
- Farm edges: `rail_fence` (2 m of split rails), `haystack` (a stacked
  haystack round a pole), `hay_bales` (three round bales), and `beehives`
  (three box hives on a stand).
- Water: `rowboat`, `dock` (a 6 m timber jetty with a mooring post), and
  `reeds` (a clump of cattails).
- Woods: `fallen_log`, `mossy_rock`, `mushrooms` (red toadstools and brown
  caps), and `stump`.
- Cafés: `cafe_table` (a round table, two chairs, and a parasol).
- Trees: `birch_low`, `poplar_low`, `spruce_low`, `fruit_tree`, and
  `bush_round`, cheap stand-ins that vary the woods and the gardens.

Fronts face -Y in Blender, which is +Z (glTF's front) after export; the
origin is the center of the base.
"""

import math
import os
import random
import sys

import bmesh
import bpy

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

# Shared with street_props.py: the same names and values, so the pack keeps
# one copy of each material.
IRON = (0.035, 0.035, 0.04)
OAK = (0.32, 0.17, 0.07)
OAK_DARK = (0.18, 0.09, 0.035)
STONE = (0.42, 0.4, 0.36)
STONE_DARK = (0.26, 0.25, 0.23)
LEAF = (0.07, 0.2, 0.04)
LEAF_DARK = (0.045, 0.13, 0.03)


def materials():
    return {
        "iron": kit.mat("Prop_Iron", IRON, 0.5),
        "oak": kit.mat("Prop_Oak", OAK, 0.85),
        "oak_dark": kit.mat("Prop_OakDark", OAK_DARK, 0.85),
        "stone": kit.mat("Prop_Stone", STONE, 0.95),
        "stone_dark": kit.mat("Prop_StoneDark", STONE_DARK, 0.95),
        "leaf": kit.mat("Prop_Leaf", LEAF, 0.9),
        "leaf_dark": kit.mat("Prop_LeafDark", LEAF_DARK, 0.9),
        "red": kit.mat("Prop_FlowerRed", (0.7, 0.05, 0.05), 0.7),
        "yellow": kit.mat("Prop_FlowerYellow", (0.85, 0.6, 0.05), 0.7),
        "violet": kit.mat("Prop_FlowerViolet", (0.32, 0.12, 0.6), 0.7),
        "white": kit.mat("Prop_FlowerWhite", (0.85, 0.82, 0.75), 0.7),
        "pink": kit.mat("Prop_FlowerPink", (0.85, 0.35, 0.5), 0.7),
        "sack": kit.mat("Prop_Sack", (0.55, 0.42, 0.25), 1.0),
        "pad": kit.mat("Prop_LilyPad", (0.06, 0.22, 0.05), 0.4),
        # This script's own.
        "bronze": kit.mat("Prop_Bronze", (0.13, 0.26, 0.2), 0.45, metal=0.6),
        "marble": kit.mat("Prop_Marble", (0.72, 0.7, 0.64), 0.6),
        "paint_white": kit.mat("Prop_PaintWhite", (0.78, 0.76, 0.7), 0.8),
        "paint_blue": kit.mat("Prop_PaintBlue", (0.12, 0.25, 0.45), 0.7),
        "paint_green": kit.mat("Prop_PaintGreen", (0.14, 0.3, 0.16), 0.7),
        "hay": kit.mat("Prop_Hay", (0.62, 0.48, 0.16), 1.0),
        "hay_dark": kit.mat("Prop_HayDark", (0.45, 0.33, 0.1), 1.0),
        "soil": kit.mat("Prop_Soil", (0.12, 0.07, 0.035), 1.0),
        "moss": kit.mat("Prop_Moss", (0.1, 0.24, 0.04), 1.0),
        "rock": kit.mat("Prop_Rock", (0.3, 0.3, 0.28), 0.95),
        "bark": kit.mat("Prop_Bark", (0.16, 0.08, 0.035), 0.9),
        "birch": kit.mat("Prop_BirchBark", (0.75, 0.73, 0.66), 0.9),
        # The trees share the far forest's materials (`street_props.py`), so a
        # stand of mixed trees draws in few batches.
        "birch_leaf": kit.mat("Prop_CanopyLight", (0.12, 0.27, 0.05), 0.9),
        "poplar_leaf": kit.mat("Prop_Canopy", (0.07, 0.2, 0.04), 0.9),
        "spruce": kit.mat("Prop_Needles", (0.03, 0.11, 0.035), 0.9),
        "fruit": kit.mat("Prop_Fruit", (0.62, 0.06, 0.03), 0.6),
        "cream_cloth": kit.mat("Prop_CreamCloth", (0.8, 0.72, 0.56), 1.0),
        "red_cloth": kit.mat("Prop_RedCloth", (0.55, 0.08, 0.06), 1.0),
        "cattail": kit.mat("Prop_Cattail", (0.22, 0.12, 0.05), 1.0),
        "reed": kit.mat("Prop_Reed", (0.2, 0.3, 0.08), 0.9),
        "cap_red": kit.mat("Prop_CapRed", (0.62, 0.06, 0.03), 0.6),
        "cap_brown": kit.mat("Prop_CapBrown", (0.4, 0.22, 0.09), 0.8),
        "cabbage": kit.mat("Prop_Cabbage", (0.22, 0.4, 0.12), 0.8),
        "hive": kit.mat("Prop_HiveWhite", (0.86, 0.82, 0.68), 0.8),
        "honey": kit.mat("Prop_Honey", (0.8, 0.5, 0.08), 0.6),
        # The third round's: street_props.py's lamp glow and sign paint, and
        # flower and lantern colors of its own.
        "glow": kit.mat("Prop_LampGlow", (1.0, 0.62, 0.22), 0.3),
        "sign_paint": kit.mat("Prop_SignPaint", (0.6, 0.45, 0.2), 0.8),
        "orange": kit.mat("Prop_FlowerOrange", (0.9, 0.32, 0.04), 0.7),
        "blue": kit.mat("Prop_FlowerBlue", (0.12, 0.25, 0.75), 0.7),
        "rust": kit.mat("Prop_FlowerRust", (0.5, 0.12, 0.03), 0.7),
        "plum": kit.mat("Prop_FlowerPlum", (0.38, 0.06, 0.2), 0.7),
        "gold": kit.mat("Prop_FlowerGold", (0.75, 0.42, 0.02), 0.7),
        "lantern_red": kit.mat("Prop_PaperRed", (0.8, 0.12, 0.06), 0.6),
        "lantern_cream": kit.mat("Prop_PaperCream", (0.95, 0.78, 0.45), 0.6),
    }


def finish(name, out):
    body = kit.join(name)
    kit.ground(body)
    kit.flat()
    kit.export(os.path.join(out, name + ".glb"))


def rock_mesh(name, r, loc, material, seed, squash=0.7, subdiv=1):
    """A rough rock: an icosphere with its vertices pushed in and out."""
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=subdiv, radius=r)
    o = bpy.context.object
    rng = random.Random(seed)
    for v in o.data.vertices:
        k = rng.uniform(0.78, 1.12)
        v.co.x *= k
        v.co.y *= k * rng.uniform(0.9, 1.1)
        v.co.z *= k * squash
    o.name = name
    o.location = loc
    o.data.materials.append(material)
    return o


# --- Sculpture Walk and plazas ---------------------------------------------


def statue(m):
    """A robed figure on a stone plinth, holding a lamp up toward the sky."""
    kit.box("Plinth", (1.3, 1.3, 0.3), (0, 0, 0.15), m["stone_dark"], bevel=0.04)
    kit.box("Pedestal", (0.95, 0.95, 1.2), (0, 0, 0.9), m["marble"], bevel=0.05)
    kit.box("Cornice", (1.1, 1.1, 0.14), (0, 0, 1.55), m["marble"], bevel=0.03)
    kit.box("Plaque", (0.5, 0.03, 0.3), (0, -0.48, 1.0), m["bronze"])
    base = 1.62
    # The robe falls in folds from the shoulders to a wide hem.
    kit.lathe("Robe", [(0.0, base), (0.46, base), (0.44, base + 0.15), (0.34, base + 0.9), (0.27, base + 1.45),
                       (0.3, base + 1.62), (0.0, base + 1.66)], material=m["bronze"], segs=8)
    kit.box("Shoulders", (0.7, 0.3, 0.18), (0, 0, base + 1.6), m["bronze"], bevel=0.06)
    kit.cyl("Neck", 0.08, 0.14, (0, 0, base + 1.74), m["bronze"], verts=6)
    kit.ball("Head", 0.17, (0, -0.02, base + 1.92), m["bronze"], segs=8, rings=6, scale=(0.9, 1.0, 1.1))
    kit.ball("Hair", 0.175, (0, 0.03, base + 1.97), m["bronze"], segs=8, rings=5, scale=(0.95, 1.0, 0.9))
    # The raised right arm holds a lamp up; the left holds a book.
    kit.cyl("ArmUp", 0.07, 0.8, (0.48, -0.02, base + 1.97), m["bronze"], verts=6, rot=(0, math.radians(22), 0))
    kit.ball("Hand", 0.08, (0.63, -0.02, base + 2.36), m["bronze"], segs=6, rings=4)
    kit.box("Lamp", (0.16, 0.16, 0.2), (0.64, -0.02, base + 2.5), m["bronze"])
    kit.cyl("Flame", 0.06, 0.18, (0.64, -0.02, base + 2.69), m["yellow"], verts=5, r2=0.0)
    kit.cyl("ArmDown", 0.07, 0.6, (-0.35, -0.19, base + 1.35), m["bronze"], verts=6, rot=(math.radians(-40), 0, 0))
    kit.box("Book", (0.26, 0.08, 0.32), (-0.35, -0.4, base + 1.12), m["bronze"], rot=(math.radians(25), 0, 0))


def sculpture(m):
    """A bronze ring standing on a cairn of stacked stones."""
    kit.box("Plinth", (1.4, 1.4, 0.25), (0, 0, 0.125), m["stone_dark"], bevel=0.04)
    z = 0.25
    for i, (r, h) in enumerate(((0.6, 0.35), (0.48, 0.3), (0.36, 0.28))):
        rock_mesh("Stone%d" % i, r, (0, 0, z + h / 2), m["stone" if i % 2 == 0 else "rock"], seed=20 + i, squash=h / r)
        z += h
    kit.ring("Ring", 0.65, 0.07, (0, 0, z + 0.62), m["bronze"], segs=16, minor_segs=5, rot=(math.pi / 2, 0, 0))
    kit.ball("Orb", 0.16, (0, 0, z + 0.62), m["marble"], segs=8, rings=5)


def sundial(m):
    kit.cyl("Step", 0.55, 0.12, (0, 0, 0.06), m["stone_dark"], verts=10)
    kit.lathe("Column", [(0.0, 0.12), (0.22, 0.12), (0.15, 0.3), (0.12, 0.7), (0.18, 0.88), (0.0, 0.88)],
              material=m["stone"], segs=8)
    kit.cyl("Dial", 0.36, 0.05, (0, 0, 0.9), m["bronze"], verts=12)
    kit.cyl("Gnomon", 0.22, 0.02, (0, 0.04, 1.0), m["bronze"], verts=3, rot=(0, math.pi / 2, 0))
    for i in range(12):
        a = 2 * math.pi * i / 12
        kit.box("Mark", (0.02, 0.07, 0.01), (0.3 * math.cos(a), 0.3 * math.sin(a), 0.93), m["marble"],
                rot=(0, 0, a + math.pi / 2))


# --- Gardens ----------------------------------------------------------------


def blooms(m, n, center, spread, z, seed, colors=("red", "yellow", "violet", "white", "pink"), size=0.06):
    rng = random.Random(seed)
    for i in range(n):
        a = rng.uniform(0, 2 * math.pi)
        r = spread * math.sqrt(rng.uniform(0, 1))
        kit.ball("Bloom", size * rng.uniform(0.8, 1.2),
                 (center[0] + r * math.cos(a), center[1] + r * math.sin(a), z + rng.uniform(-0.04, 0.06)),
                 m[colors[i % len(colors)]], segs=5, rings=3)


def planter(m):
    kit.lathe("Pot", [(0.0, 0.0), (0.42, 0.0), (0.48, 0.08), (0.52, 0.5), (0.58, 0.55), (0.58, 0.62),
                      (0.48, 0.62), (0.46, 0.55), (0.0, 0.55)], material=m["stone"], segs=10)
    kit.cyl("Soil", 0.46, 0.02, (0, 0, 0.57), m["soil"], verts=10)
    kit.ball("Shrub", 0.36, (0, 0, 0.88), m["leaf"], segs=8, rings=5, scale=(1, 1, 1.05))
    kit.ball("Shrub2", 0.24, (0.12, -0.1, 1.15), m["leaf_dark"], segs=7, rings=4)
    blooms(m, 8, (0, 0), 0.32, 0.86, 3, colors=("pink", "white", "red"))


def garden_arch(m):
    """A timber arch, 2.6 m tall and 1.6 m wide, under climbing roses."""
    for sx in (-1, 1):
        for sy in (-1, 1):
            kit.box("Post", (0.1, 0.1, 2.3), (sx * 0.8, sy * 0.3, 1.15), m["paint_white"])
        for z in (0.6, 1.3, 2.0):
            kit.box("Lattice", (0.04, 0.6, 0.04), (sx * 0.8, 0, z), m["paint_white"])
    # The curved top: a half ring of slats.
    for y in (0.3, -0.3):
        kit.ring("Top", 0.8, 0.05, (0, y, 2.3), m["paint_white"], segs=12, minor_segs=3, rot=(math.pi / 2, 0, 0))
    for i in range(5):
        a = math.pi * i / 4
        kit.box("Slat", (0.05, 0.7, 0.04), (0.8 * math.cos(a), 0, 2.3 + 0.8 * math.sin(a)), m["paint_white"])
    # Climbing roses: leafy clumps up both sides and over the top.
    rng = random.Random(9)
    pts = [(sx * 0.82, rng.uniform(-0.25, 0.25), z) for sx in (-1, 1) for z in (0.35, 0.95, 1.55, 2.1)]
    pts += [(0.8 * math.cos(a), rng.uniform(-0.2, 0.2), 2.3 + 0.82 * math.sin(a)) for a in (0.5, 1.2, 1.9, 2.6)]
    for i, (x, y, z) in enumerate(pts):
        kit.ball("Vine", rng.uniform(0.2, 0.28), (x, y, z), m["leaf" if i % 2 else "leaf_dark"], segs=5, rings=3,
                 scale=(1.0, 1.3, 1.0))
        for k in range(2):
            kit.ball("Rose", 0.07, (x + rng.uniform(-0.15, 0.15), y + rng.uniform(-0.3, 0.3), z + rng.uniform(-0.1, 0.15)),
                     m["pink" if (i + k) % 3 else "red"], segs=5, rings=3)


def picket_fence(m):
    """Two meters of white pickets on two rails."""
    for sx in (-1, 1):
        kit.box("Post", (0.1, 0.1, 1.05), (sx * 0.95, 0, 0.525), m["paint_white"])
        kit.cyl("PostCap", 0.08, 0.1, (sx * 0.95, 0, 1.1), m["paint_white"], verts=4, r2=0.0, rot=(0, 0, math.pi / 4))
    for z in (0.3, 0.72):
        kit.box("Rail", (1.9, 0.04, 0.07), (0, 0.04, z), m["paint_white"])
    for i in range(9):
        x = -0.8 + i * 0.2
        kit.box("Picket", (0.08, 0.025, 0.78), (x, -0.01, 0.43), m["paint_white"])
        kit.cyl("Point", 0.057, 0.1, (x, -0.01, 0.87), m["paint_white"], verts=4, r2=0.0, rot=(0, 0, math.pi / 4))


def garden_gate(m):
    """Stone gate posts with a ball on each and a timber gate between, 2 m."""
    for sx in (-1, 1):
        kit.box("Pier", (0.45, 0.55, 1.1), (sx * 0.9, 0, 0.55), m["stone"], bevel=0.04)
        kit.box("Cap", (0.55, 0.65, 0.1), (sx * 0.9, 0, 1.15), m["stone_dark"], bevel=0.02)
        kit.ball("Ball", 0.14, (sx * 0.9, 0, 1.32), m["stone_dark"], segs=7, rings=4)
    # The gate: two leaves of boards, standing a little ajar.
    for sx, ang in ((-1, 0.2), (1, -0.35)):
        leaf_x = sx * 0.33
        for z in (0.25, 0.8):
            o = kit.box("Ledge", (0.62, 0.05, 0.08), (leaf_x, 0, z), m["oak"])
            o.rotation_euler = (0, 0, ang * sx)
        for i in range(4):
            x = leaf_x + (-0.24 + i * 0.16)
            kit.box("Board", (0.12, 0.03, 0.8 - 0.05 * abs(i - 1.5)), (x, -0.03, 0.52), m["oak"])


def flower_bed(m):
    """A 2 x 1 m bed edged with timber, rows of flowers."""
    for sy in (-1, 1):
        kit.box("Edge", (2.0, 0.08, 0.18), (0, sy * 0.5, 0.09), m["oak_dark"])
    for sx in (-1, 1):
        kit.box("Edge", (0.08, 1.0, 0.18), (sx * 0.96, 0, 0.09), m["oak_dark"])
    kit.box("Soil", (1.86, 0.9, 0.12), (0, 0, 0.08), m["soil"])
    rows = (("red", "yellow"), ("violet", "white"), ("pink", "yellow"))
    rng = random.Random(4)
    for r, colors in enumerate(rows):
        y = -0.3 + r * 0.3
        for i in range(6):
            x = -0.75 + i * 0.3
            kit.ball("Leaves", 0.12, (x, y, 0.2), m["leaf"], segs=5, rings=3, scale=(1, 1, 0.8))
            kit.ball("Bloom", 0.065, (x + rng.uniform(-0.04, 0.04), y, 0.3 + rng.uniform(0, 0.05)),
                     m[colors[i % 2]], segs=5, rings=3)


def veg_bed(m):
    """A raised bed of cabbages and a row of beans on canes."""
    kit.box("Bed", (2.4, 1.1, 0.3), (0, 0, 0.15), m["oak"], bevel=0.02)
    kit.box("Soil", (2.3, 1.0, 0.04), (0, 0, 0.31), m["soil"])
    for i in range(4):
        for j in range(2):
            x, y = -0.85 + i * 0.57, -0.25 + j * 0.3
            kit.ball("Cabbage", 0.16, (x, y, 0.42), m["cabbage"], segs=7, rings=4, scale=(1, 1, 0.75))
            kit.ball("Heart", 0.09, (x, y, 0.5), m["leaf"], segs=5, rings=3)
    for i in range(4):
        x = -0.9 + i * 0.6
        for sx in (-0.08, 0.08):
            kit.cyl("Cane", 0.012, 1.4, (x + sx, 0.38, 1.0), m["sack"], verts=3, rot=(0, sx * 1.2, 0))
        kit.ball("Beans", 0.13, (x, 0.38, 0.8), m["leaf_dark"], segs=5, rings=4, scale=(0.8, 0.8, 2.0))


# --- Farm edges --------------------------------------------------------------


def rail_fence(m):
    """Two meters of split-rail fence."""
    for sx in (-1, 1):
        kit.box("Post", (0.13, 0.13, 1.15), (sx * 0.95, 0, 0.575), m["oak_dark"])
    for z, tilt in ((0.4, 0.02), (0.85, -0.02)):
        kit.box("Rail", (2.05, 0.08, 0.1), (0, 0.02, z), m["oak"], rot=(0, tilt, 0))


def haystack(m):
    kit.lathe("Stack", [(0.0, 0.0), (1.2, 0.0), (1.35, 0.6), (1.25, 1.4), (0.9, 2.1), (0.45, 2.6), (0.0, 2.8)],
              material=m["hay"], segs=10)
    kit.lathe("Skirt", [(0.0, 0.02), (1.42, 0.02), (1.45, 0.3), (0.0, 0.3)], material=m["hay_dark"], segs=10)
    kit.cyl("Pole", 0.05, 0.8, (0, 0, 3.0), m["oak_dark"], verts=5)


def hay_bales(m):
    for i, (x, y, z, rot) in enumerate(((-0.7, 0, 0.6, 0.0), (0.7, 0.1, 0.6, 0.2), (0.0, 0.05, 1.62, 0.1))):
        kit.cyl("Bale%d" % i, 0.6, 1.1, (x, y, z), m["hay"], verts=10, rot=(math.pi / 2, 0, rot))
        for sy in (-1, 1):
            kit.cyl("Face%d" % i, 0.52, 0.02, (x, y + sy * 0.56, z), m["hay_dark"], verts=10,
                    rot=(math.pi / 2, 0, rot))


def beehives(m):
    """Three white box hives on a timber stand, with a little honey pot."""
    kit.box("Stand", (2.4, 0.7, 0.08), (0, 0, 0.42), m["oak_dark"])
    for sx in (-1.05, 0, 1.05):
        for sy in (-0.25, 0.25):
            kit.box("Leg", (0.07, 0.07, 0.42), (sx, sy, 0.21), m["oak_dark"])
    for i, x in enumerate((-0.78, 0.0, 0.78)):
        levels = 3 if i == 1 else 2
        z = 0.46
        for k in range(levels):
            kit.box("Super", (0.56, 0.5, 0.24), (x, 0, z + 0.12), m["hive"], bevel=0.01)
            z += 0.25
        kit.cyl("Roof", 0.45, 0.16, (x, 0, z + 0.08), m["stone_dark"], verts=4, r2=0.08, rot=(0, 0, math.pi / 4))
        kit.box("Entrance", (0.22, 0.02, 0.04), (x, -0.25, 0.5), m["oak_dark"])
    kit.cyl("Pot", 0.1, 0.14, (1.15, -0.1, 0.53), m["honey"], verts=7, r2=0.08)


# --- Water ---------------------------------------------------------------------


def hull_mesh(name, length, beam, depth, material, sections=7):
    """An open boat hull: rings of a U section, pinched to the bow and stern."""
    bm = bmesh.new()
    rows = []
    for i in range(sections + 1):
        t = i / sections
        y = -length / 2 + length * t
        w = beam / 2 * (math.sin(math.pi * t) ** 0.6)
        sheer = depth + 0.12 * (2 * t - 1) ** 2
        pts = [(-w, y, sheer), (-w * 0.85, y, depth * 0.35), (0.0, y, 0.0), (w * 0.85, y, depth * 0.35),
               (w, y, sheer)]
        rows.append([bm.verts.new(p) for p in pts])
    for a, b in zip(rows, rows[1:]):
        for k in range(4):
            bm.faces.new((a[k], a[k + 1], b[k + 1], b[k]))
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-4)
    bmesh.ops.recalc_face_normals(bm, faces=bm.faces)
    me = bpy.data.meshes.new(name)
    bm.to_mesh(me)
    bm.free()
    o = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(o)
    o.data.materials.append(material)
    kit.solidify(o, 0.05)
    return o


def rowboat(m):
    hull_mesh("Hull", 3.2, 1.25, 0.55, m["paint_blue"])
    kit.box("Gunwale", (0.08, 2.6, 0.06), (-0.58, 0, 0.6), m["paint_white"])
    kit.box("Gunwale", (0.08, 2.6, 0.06), (0.58, 0, 0.6), m["paint_white"])
    for y in (-0.6, 0.45):
        kit.box("Thwart", (1.1, 0.24, 0.05), (0, y, 0.42), m["oak"])
    kit.box("Floor", (0.6, 2.2, 0.04), (0, 0, 0.1), m["oak"])
    for sx in (-1, 1):
        kit.box("Oar", (0.05, 2.0, 0.04), (sx * 0.38, 0.1, 0.5), m["oak"], rot=(0, 0, sx * 0.08))
        kit.box("Blade", (0.14, 0.4, 0.02), (sx * 0.42, 1.05, 0.5), m["oak"], rot=(0, 0, sx * 0.08))


def dock(m):
    """A 6 m jetty, 1.8 m wide, from the bank (+Y in Blender, the back)
    out over the water (-Y, the front), with a mooring post and a lantern."""
    length, width = 6.0, 1.8
    for i in range(int(length / 0.4)):
        y = -length / 2 + 0.2 + i * 0.4
        kit.box("Plank", (width, 0.36, 0.06), (0, y, 0.55), m["oak"])
    for sx in (-1, 1):
        kit.box("Stringer", (0.12, length, 0.16), (sx * (width / 2 - 0.15), 0, 0.44), m["oak_dark"])
        for y in (-length / 2 + 0.2, -0.6, length / 2 - 0.4):
            kit.box("Pile", (0.16, 0.16, 1.4), (sx * (width / 2 - 0.08), y, 0.1), m["oak_dark"])
    kit.cyl("Bollard", 0.1, 0.6, (width / 2 - 0.15, -length / 2 + 0.35, 0.88), m["oak_dark"], verts=6)
    kit.ring("Rope", 0.12, 0.025, (width / 2 - 0.15, -length / 2 + 0.35, 0.95), m["sack"], segs=8, minor_segs=3)
    kit.box("LampPost", (0.08, 0.08, 1.5), (-width / 2 + 0.15, -length / 2 + 0.35, 1.33), m["oak_dark"])
    kit.box("Lamp", (0.2, 0.2, 0.26), (-width / 2 + 0.15, -length / 2 + 0.35, 2.2), m["iron"])
    kit.box("LampGlow", (0.14, 0.14, 0.18), (-width / 2 + 0.15, -length / 2 + 0.35 - 0.04, 2.2), m["yellow"])


def reeds(m):
    """A clump of cattails and reed blades, 1.2 m across."""
    rng = random.Random(17)
    for i in range(16):
        a = rng.uniform(0, 2 * math.pi)
        r = 0.6 * math.sqrt(rng.uniform(0, 1))
        x, y = r * math.cos(a), r * math.sin(a)
        h = rng.uniform(1.0, 1.7)
        lean = (rng.uniform(-0.12, 0.12), rng.uniform(-0.12, 0.12), 0)
        kit.cyl("Stem", 0.018, h, (x, y, h / 2), m["reed"], verts=3, cap=False, rot=lean)
        if i % 3 == 0:
            kit.cyl("Head", 0.045, 0.24, (x + lean[1] * h * 0.5, y - lean[0] * h * 0.5, h - 0.05), m["cattail"],
                    verts=5, rot=lean)
    for i in range(10):
        a = rng.uniform(0, 2 * math.pi)
        x, y = 0.45 * math.cos(a), 0.45 * math.sin(a)
        h = rng.uniform(0.6, 1.0)
        kit.cyl("Blade", 0.05, h, (x, y, h / 2), m["leaf"], verts=3, r2=0.0, rot=(rng.uniform(-0.3, 0.3), rng.uniform(-0.3, 0.3), a))


# --- Woods ---------------------------------------------------------------------


def fallen_log(m):
    kit.cyl("Log", 0.32, 3.6, (0, 0, 0.3), m["bark"], verts=8, rot=(0, math.pi / 2, 0.15))
    for sx in (-1, 1):
        kit.cyl("End", 0.27, 0.04, (sx * 1.79, sx * 0.27, 0.3), m["sack"], verts=8, rot=(0, math.pi / 2, 0.15))
    kit.ball("Moss", 0.34, (-0.4, -0.06, 0.5), m["moss"], segs=7, rings=4, scale=(2.6, 0.95, 0.4))
    kit.cyl("Branch", 0.08, 0.9, (0.7, -0.4, 0.55), m["bark"], verts=5, rot=(0.9, 0.3, 0.4))
    for i, (x, y) in enumerate(((0.9, 0.3), (1.1, 0.25), (-1.2, 0.3))):
        kit.cyl("Stalk", 0.03, 0.14, (x, y, 0.07), m["white"], verts=5)
        kit.cyl("Cap", 0.09, 0.06, (x, y, 0.17), m["cap_brown"], verts=7, r2=0.02)


def mossy_rock(m):
    rock_mesh("Rock", 0.9, (0, 0, 0.45), m["rock"], seed=31, squash=0.62)
    rock_mesh("Side", 0.45, (0.85, 0.3, 0.2), m["rock"], seed=32, squash=0.6)
    kit.ball("Moss", 0.72, (-0.05, 0.05, 0.84), m["moss"], segs=8, rings=4, scale=(1.05, 0.95, 0.28))
    kit.ball("Fern", 0.3, (-0.7, -0.5, 0.15), m["leaf"], segs=6, rings=3, scale=(1, 1, 0.6))


def mushrooms(m):
    """Red toadstools with white spots and a few brown caps."""
    rng = random.Random(23)
    for i, (x, y, h, r) in enumerate(((0, 0, 0.32, 0.17), (0.25, 0.12, 0.22, 0.12), (-0.18, 0.2, 0.18, 0.1),
                                      (0.1, -0.25, 0.26, 0.14))):
        kit.cyl("Stalk", 0.035 + r * 0.15, h, (x, y, h / 2), m["white"], verts=6, r2=0.03)
        kit.lathe("Cap", [(0.0, h - 0.03), (r, h - 0.02), (r * 0.85, h + r * 0.35), (r * 0.4, h + r * 0.6),
                          (0.0, h + r * 0.65)], material=m["cap_red" if i < 3 else "cap_brown"], segs=8,
                  loc=(x, y, 0))
        if i < 3:
            for k in range(3):
                a = rng.uniform(0, 2 * math.pi)
                kit.ball("Spot", r * 0.14, (x + r * 0.55 * math.cos(a), y + r * 0.55 * math.sin(a), h + r * 0.38),
                         m["white"], segs=4, rings=2)
    for i in range(3):
        a = 2.1 * i
        x, y = 0.45 * math.cos(a), 0.4 * math.sin(a)
        kit.cyl("Small", 0.02, 0.1, (x, y, 0.05), m["white"], verts=5)
        kit.cyl("SmallCap", 0.07, 0.05, (x, y, 0.12), m["cap_brown"], verts=6, r2=0.015)


def stump(m):
    kit.lathe("Stump", [(0.0, 0.0), (0.55, 0.0), (0.42, 0.15), (0.38, 0.55), (0.0, 0.55)], material=m["bark"], segs=8)
    kit.cyl("Rings", 0.36, 0.02, (0, 0, 0.56), m["sack"], verts=8)
    for i in range(4):
        a = i * math.pi / 2 + 0.4
        kit.cyl("Root", 0.1, 0.6, (0.45 * math.cos(a), 0.45 * math.sin(a), 0.06), m["bark"], verts=4, r2=0.03,
                rot=(0, math.pi / 2 - 0.2, a))
    kit.ball("Moss", 0.3, (0.2, -0.3, 0.1), m["moss"], segs=6, rings=3, scale=(1, 1, 0.4))


# --- Cafés ---------------------------------------------------------------------


def cafe_table(m):
    """A round café table with two chairs under a striped parasol."""
    kit.cyl("Top", 0.42, 0.04, (0, 0, 0.74), m["paint_white"], verts=10)
    kit.cyl("Leg", 0.035, 0.72, (0, 0, 0.36), m["iron"], verts=5)
    kit.cyl("Foot", 0.22, 0.03, (0, 0, 0.015), m["iron"], verts=6)
    for sx in (-1, 1):
        x = sx * 0.62
        kit.box("Seat", (0.4, 0.4, 0.05), (x, 0, 0.45), m["paint_green"])
        kit.box("Back", (0.05, 0.4, 0.42), (x + sx * 0.2, 0, 0.68), m["paint_green"])
        for lx in (-0.17, 0.17):
            for ly in (-0.17, 0.17):
                kit.box("ChairLeg", (0.03, 0.03, 0.45), (x + lx, ly, 0.225), m["iron"])
    kit.cyl("Pole", 0.025, 2.2, (0, 0, 1.1), m["oak_dark"], verts=5)
    # The parasol: eight panels in two colors.
    for i in range(8):
        a0, a1 = 2 * math.pi * i / 8, 2 * math.pi * (i + 1) / 8
        bm = bmesh.new()
        tip = bm.verts.new((0, 0, 2.3))
        e0 = bm.verts.new((1.25 * math.cos(a0), 1.25 * math.sin(a0), 1.85))
        e1 = bm.verts.new((1.25 * math.cos(a1), 1.25 * math.sin(a1), 1.85))
        bm.faces.new((tip, e0, e1))
        me = bpy.data.meshes.new("Panel")
        bm.to_mesh(me)
        bm.free()
        o = bpy.data.objects.new("Panel", me)
        bpy.context.scene.collection.objects.link(o)
        o.data.materials.append(m["red_cloth" if i % 2 else "cream_cloth"])
        kit.solidify(o, 0.02)


# --- Trees -------------------------------------------------------------------


def birch_low(m):
    # The woods repeat it by the hundred, so it stays near 150 triangles.
    kit.cyl("Trunk", 0.16, 5.2, (0, 0, 2.6), m["birch"], verts=5, r2=0.08, cap=False)
    for i, (x, y, z, r) in enumerate(((0.0, 0.0, 4.9, 1.3), (0.6, 0.3, 3.9, 0.95), (-0.5, -0.3, 4.2, 1.0))):
        kit.ball("Crown%d" % i, r, (x, y, z), m["birch_leaf"], segs=6, rings=4, scale=(1, 1, 1.25))


def poplar_low(m):
    kit.cyl("Trunk", 0.2, 2.0, (0, 0, 1.0), m["bark"], verts=5, r2=0.14)
    kit.ball("Crown", 1.15, (0, 0, 4.8), m["poplar_leaf"], segs=8, rings=6, scale=(1, 1, 3.0))


def spruce_low(m):
    kit.cyl("Trunk", 0.2, 1.8, (0, 0, 0.9), m["bark"], verts=5, r2=0.14)
    for i, (r, z, h) in enumerate(((1.7, 1.0, 2.6), (1.4, 2.6, 2.4), (1.1, 4.1, 2.2), (0.75, 5.5, 2.0))):
        kit.cyl("Tier%d" % i, r, h, (0, 0, z + h / 2), m["spruce"], verts=7, r2=0.04)


def fruit_tree(m):
    kit.cyl("Trunk", 0.17, 1.8, (0, 0, 0.9), m["bark"], verts=5, r2=0.12)
    kit.ball("Crown", 1.35, (0, 0, 2.6), m["leaf"], segs=8, rings=5, scale=(1, 1, 0.8))
    kit.ball("Crown2", 0.85, (0.55, 0.25, 3.25), m["birch_leaf"], segs=7, rings=4)
    rng = random.Random(41)
    for i in range(10):
        a = rng.uniform(0, 2 * math.pi)
        e = rng.uniform(-0.4, 0.6)
        kit.ball("Fruit", 0.09, (1.32 * math.cos(a) * math.cos(e), 1.32 * math.sin(a) * math.cos(e),
                                 2.6 + 1.0 * math.sin(e)), m["fruit"], segs=4, rings=3)


def bush_round(m):
    for i, (x, y, z, r) in enumerate(((0, 0, 0.5, 0.75), (0.55, 0.2, 0.38, 0.5), (-0.5, -0.15, 0.36, 0.52))):
        kit.ball("Bush%d" % i, r, (x, y, z), m["leaf" if i == 0 else "leaf_dark"], segs=7, rings=4,
                 scale=(1, 1, 0.85))


# --- Third round: seasons, lanterns, signs, and benches ----------------------


# Each patch's bloom colors: spring pastels, high-summer reds, and autumn
# golds and plums.
SEASONS = {
    "flower_patch_spring": ("pink", "white", "yellow", "blue"),
    "flower_patch_summer": ("red", "orange", "yellow", "red"),
    "flower_patch_autumn": ("rust", "gold", "plum", "orange"),
}


def flower_patch(m, colors, seed):
    """A 3 m drift of one season's flowers over a low leafy mound.

    The lawns repeat it by the dozen, so its blooms are double pyramids and it
    stays near 440 triangles."""
    rng = random.Random(seed)
    mounds = []
    for i in range(4):
        a = 2 * math.pi * i / 4 + rng.uniform(-0.3, 0.3)
        r = 0.0 if i == 0 else rng.uniform(0.7, 0.9)
        size = rng.uniform(0.65, 0.8)
        center = (r * math.cos(a), r * math.sin(a))
        mounds.append((center, size))
        kit.ball("Mound%d" % i, size, (center[0], center[1], 0.0), m["leaf" if i % 2 else "leaf_dark"],
                 segs=5, rings=3, scale=(1, 1, 0.5))
    # Blooms sit on the mounds' crowns, thickest at their tops.
    for i in range(30):
        (cx, cy), size = mounds[i % len(mounds)]
        a = rng.uniform(0, 2 * math.pi)
        t = math.sqrt(rng.uniform(0, 1)) * 0.8
        z = 0.5 * size * math.sqrt(max(0.0, 1 - t * t)) + 0.04
        kit.ball("Bloom", rng.uniform(0.08, 0.11), (cx + t * size * math.cos(a), cy + t * size * math.sin(a), z),
                 m[colors[i % len(colors)]], segs=3, rings=2)


def lantern_string(m):
    """Paper lanterns on a cord between two posts, 8 m apart and 3.6 m
    tall, to hang across the Lantern Quarter's streets."""
    length, height, sag = 8.0, 3.6, 0.5
    for sx in (-1, 1):
        kit.cyl("Post", 0.07, height + 0.2, (sx * length / 2, 0, (height + 0.2) / 2), m["oak_dark"], verts=6)
        kit.box("Arm", (0.35, 0.06, 0.06), (sx * (length / 2 - 0.15), 0, height), m["oak_dark"])
    segments = 8
    for i in range(segments):
        t0, t1 = i / segments, (i + 1) / segments
        x0, x1 = -length / 2 + length * t0, -length / 2 + length * t1
        z0, z1 = height - sag * math.sin(math.pi * t0), height - sag * math.sin(math.pi * t1)
        kit.box("Cord", (math.hypot(x1 - x0, z1 - z0), 0.015, 0.015), ((x0 + x1) / 2, 0, (z0 + z1) / 2),
                m["sack"], rot=(0, -math.atan2(z1 - z0, x1 - x0), 0))
    lanterns = 7
    for i in range(lanterns):
        t = (i + 1) / (lanterns + 1)
        x = -length / 2 + length * t
        z = height - sag * math.sin(math.pi * t) - 0.3
        paper = ("lantern_red", "lantern_cream", "glow")[i % 3]
        kit.ball("Lantern", 0.17, (x, 0, z), m[paper], segs=6, rings=4, scale=(1, 1, 1.25))
        kit.cyl("LanternCap", 0.08, 0.06, (x, 0, z + 0.22), m["iron"], verts=6)


def lamp_double(m):
    """A tall iron lamp post with a crossarm and a lantern hanging from
    each end, for the Lantern Quarter's corners."""
    kit.cyl("Base", 0.24, 0.35, (0, 0, 0.175), m["iron"], verts=8, r2=0.16)
    kit.cyl("Post", 0.07, 3.6, (0, 0, 2.05), m["iron"], verts=6)
    kit.box("Arm", (1.6, 0.06, 0.06), (0, 0, 3.8), m["iron"])
    kit.cyl("Finial", 0.06, 0.4, (0, 0, 4.05), m["iron"], verts=6, r2=0.01)
    for sx in (-1, 1):
        x = sx * 0.72
        kit.box("Hook", (0.03, 0.03, 0.22), (x, 0, 3.66), m["iron"])
        kit.box("Glass", (0.26, 0.26, 0.34), (x, 0, 3.32), m["glow"])
        kit.box("Floor", (0.32, 0.32, 0.04), (x, 0, 3.13), m["iron"])
        kit.cyl("Cap", 0.25, 0.18, (x, 0, 3.58), m["iron"], verts=4, r2=0.02, rot=(0, 0, math.pi / 4))


def shop_sign(m):
    """A post by a shop's walk with an arm and a painted board swinging
    from it, its emblem a gilded disc."""
    kit.cyl("Post", 0.07, 3.0, (0, 0, 1.5), m["oak_dark"], verts=6)
    kit.box("Arm", (1.2, 0.07, 0.08), (0.55, 0, 2.85), m["oak_dark"])
    kit.box("Brace", (0.6, 0.05, 0.05), (0.25, 0, 2.6), m["oak_dark"], rot=(0, math.radians(-38), 0))
    for x in (0.38, 0.92):
        kit.box("Chain", (0.02, 0.02, 0.2), (x, 0, 2.72), m["iron"])
    kit.box("Board", (0.75, 0.06, 0.5), (0.65, 0, 2.38), m["sign_paint"], bevel=0.02)
    kit.cyl("Emblem", 0.15, 0.02, (0.65, -0.04, 2.38), m["gold"], verts=8, rot=(math.pi / 2, 0, 0))
    kit.cyl("Emblem", 0.15, 0.02, (0.65, 0.04, 2.38), m["gold"], verts=8, rot=(math.pi / 2, 0, 0))


def park_bench(m):
    """A park bench of oak slats on cast-iron ends, 1.8 m long."""
    for sx in (-1, 1):
        x = sx * 0.8
        kit.box("Leg", (0.06, 0.5, 0.45), (x, 0, 0.225), m["iron"])
        kit.box("Back", (0.06, 0.06, 0.5), (x, 0.24, 0.7), m["iron"])
        kit.box("Arm", (0.06, 0.5, 0.05), (x, 0.0, 0.66), m["iron"])
    for i, y in enumerate((-0.17, -0.05, 0.07, 0.19)):
        kit.box("Slat", (1.8, 0.1, 0.04), (0, y, 0.46), m["oak"])
    for z in (0.62, 0.78, 0.92):
        kit.box("BackSlat", (1.8, 0.04, 0.1), (0, 0.27, z), m["oak"])


PROPS = {
    "flower_patch_spring": lambda m: flower_patch(m, SEASONS["flower_patch_spring"], 51),
    "flower_patch_summer": lambda m: flower_patch(m, SEASONS["flower_patch_summer"], 52),
    "flower_patch_autumn": lambda m: flower_patch(m, SEASONS["flower_patch_autumn"], 53),
    "lantern_string": lantern_string,
    "lamp_double": lamp_double,
    "shop_sign": shop_sign,
    "park_bench": park_bench,
    "statue": statue,
    "sculpture": sculpture,
    "sundial": sundial,
    "planter": planter,
    "garden_arch": garden_arch,
    "picket_fence": picket_fence,
    "garden_gate": garden_gate,
    "flower_bed": flower_bed,
    "veg_bed": veg_bed,
    "rail_fence": rail_fence,
    "haystack": haystack,
    "hay_bales": hay_bales,
    "beehives": beehives,
    "rowboat": rowboat,
    "dock": dock,
    "reeds": reeds,
    "fallen_log": fallen_log,
    "mossy_rock": mossy_rock,
    "mushrooms": mushrooms,
    "stump": stump,
    "cafe_table": cafe_table,
    "birch_low": birch_low,
    "poplar_low": poplar_low,
    "spruce_low": spruce_low,
    "fruit_tree": fruit_tree,
    "bush_round": bush_round,
}


def main():
    a = kit.args()
    out = a[0] if a else os.path.join(kit.REPO, "assets", "verse", "generated", "town")
    names = a[1:] or list(PROPS)
    for name in names:
        kit.reset()
        PROPS[name](materials())
        finish(name, out)


main()
