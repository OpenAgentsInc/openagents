"""Build Everglade's street furniture and write each piece as binary glTF.

Run headless:
    Blender -b --factory-startup --python scripts/blender/street_props.py -- [OUT_DIR]

Writes small, cheap pieces the city repeats along its streets, plazas, and
ponds, each a few hundred triangles at most so hundreds of them fit the
zone's triangle budget:

- `lamp_post`: an iron street lamp, 3.4 m, with a warm lantern.
- `barrel`: an oak barrel with iron hoops.
- `flower_box`: a timber planter of red, yellow, and violet flowers.
- `well`: a stone well under a little tiled roof, with a bucket.
- `stone_wall`: a 2 m run of low dry-stone wall with a cap.
- `hedge`: a 2 m clipped hedge.
- `hand_cart`: a two-wheeled hand cart with sacks.
- `signpost`: a post with two pointing boards.
- `lily_pads`: a cluster of floating pads with two water lilies.
- `footbridge`: a 7 m arched timber footbridge with rails.
- `wildflowers`: a scatter of meadow flowers on short stems.
- `bunting`: an 8 m string of pennants between two poles.
- `pine_low` and `oak_low`: far-forest trees of under 200 triangles, for
  the dense woods beyond the kit's trees.

Fronts face -Y in Blender, which is +Z (glTF's front) after export; the
origin is the center of the base.
"""

import math
import os
import random
import sys

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

IRON = (0.035, 0.035, 0.04)
OAK = (0.32, 0.17, 0.07)
OAK_DARK = (0.18, 0.09, 0.035)
STONE = (0.42, 0.4, 0.36)
STONE_DARK = (0.26, 0.25, 0.23)
LEAF = (0.07, 0.2, 0.04)
LEAF_DARK = (0.045, 0.13, 0.03)
GLOW = (1.0, 0.62, 0.22)


def materials():
    return {
        "iron": kit.mat("Prop_Iron", IRON, 0.5),
        "oak": kit.mat("Prop_Oak", OAK, 0.85),
        "oak_dark": kit.mat("Prop_OakDark", OAK_DARK, 0.85),
        "stone": kit.mat("Prop_Stone", STONE, 0.95),
        "stone_dark": kit.mat("Prop_StoneDark", STONE_DARK, 0.95),
        "leaf": kit.mat("Prop_Leaf", LEAF, 0.9),
        "leaf_dark": kit.mat("Prop_LeafDark", LEAF_DARK, 0.9),
        "glow": kit.mat("Prop_LampGlow", GLOW, 0.3),
        "red": kit.mat("Prop_FlowerRed", (0.7, 0.05, 0.05), 0.7),
        "yellow": kit.mat("Prop_FlowerYellow", (0.85, 0.6, 0.05), 0.7),
        "violet": kit.mat("Prop_FlowerViolet", (0.32, 0.12, 0.6), 0.7),
        "white": kit.mat("Prop_FlowerWhite", (0.85, 0.82, 0.75), 0.7),
        "pink": kit.mat("Prop_FlowerPink", (0.85, 0.35, 0.5), 0.7),
        "tile": kit.mat("Prop_Tile", (0.4, 0.09, 0.05), 0.8),
        "sack": kit.mat("Prop_Sack", (0.55, 0.42, 0.25), 1.0),
        "pad": kit.mat("Prop_LilyPad", (0.06, 0.22, 0.05), 0.4),
        "paint": kit.mat("Prop_SignPaint", (0.6, 0.45, 0.2), 0.8),
    }


def finish(name, out):
    body = kit.join(name)
    kit.ground(body)
    kit.flat()
    kit.export(os.path.join(out, name + ".glb"))
    kit.flickers(os.path.join(out, name + ".glb"))


def lamp_post(m):
    kit.cyl("Base", 0.2, 0.3, (0, 0, 0.15), m["iron"], verts=8, r2=0.14)
    kit.cyl("Post", 0.06, 3.0, (0, 0, 1.75), m["iron"], verts=6)
    kit.cyl("Collar", 0.1, 0.1, (0, 0, 3.15), m["iron"], verts=8)
    # The lantern: a glowing glass box under a pyramid cap.
    kit.box("Glass", (0.3, 0.3, 0.4), (0, 0, 3.42), m["glow"])
    for sx in (-1, 1):
        for sy in (-1, 1):
            kit.box("Frame", (0.035, 0.035, 0.44), (sx * 0.16, sy * 0.16, 3.42), m["iron"])
    kit.box("Floor", (0.38, 0.38, 0.04), (0, 0, 3.2), m["iron"])
    kit.cyl("Cap", 0.3, 0.22, (0, 0, 3.74), m["iron"], verts=4, r2=0.02, rot=(0, 0, math.pi / 4))
    kit.cyl("Finial", 0.03, 0.12, (0, 0, 3.9), m["iron"], verts=4)


def barrel(m):
    kit.lathe("Staves", [(0.0, 0.0), (0.3, 0.0), (0.36, 0.25), (0.38, 0.45), (0.36, 0.65), (0.3, 0.9), (0.0, 0.9)],
              material=m["oak"], segs=10)
    for z in (0.12, 0.78):
        kit.cyl("Hoop", 0.345, 0.05, (0, 0, z), m["iron"], verts=10, cap=False)
    kit.cyl("Lid", 0.29, 0.02, (0, 0, 0.905), m["oak_dark"], verts=10)


def flower_box(m):
    kit.box("Planter", (1.2, 0.4, 0.32), (0, 0, 0.16), m["oak"], bevel=0.02)
    # The soil heaps 1.5 cm over the planter's rim, so its top doesn't
    # share the rim's plane.
    kit.box("Soil", (1.12, 0.32, 0.02), (0, 0, 0.325), m["oak_dark"])
    rng = random.Random(7)
    colors = ["red", "yellow", "violet", "red", "white"]
    for i in range(9):
        x = -0.48 + i * 0.12
        y = rng.uniform(-0.08, 0.08)
        kit.ball("Leaves%d" % i, 0.11, (x, y, 0.38), m["leaf"], segs=6, rings=3, scale=(1, 1, 0.8))
        kit.ball("Bloom%d" % i, 0.06, (x + rng.uniform(-0.03, 0.03), y, 0.48 + rng.uniform(0, 0.05)),
                 m[colors[i % len(colors)]], segs=5, rings=3)


def well(m):
    kit.lathe("Ring", [(0.0, 0.0), (0.95, 0.0), (0.95, 0.75), (0.75, 0.75), (0.75, 0.1), (0.0, 0.1)],
              material=m["stone"], segs=12)
    kit.cyl("Water", 0.75, 0.02, (0, 0, 0.4), m["pad"], verts=12)
    kit.cyl("Coping", 0.98, 0.08, (0, 0, 0.79), m["stone_dark"], verts=12, cap=False)
    for sx in (-1, 1):
        kit.box("Post", (0.12, 0.12, 1.9), (sx * 0.85, 0, 1.6), m["oak_dark"])
    kit.cyl("Windlass", 0.05, 1.8, (0, 0, 1.75), m["oak"], verts=6, rot=(0, math.pi / 2, 0))
    kit.box("Rope", (0.02, 0.02, 0.6), (0, 0, 1.43), m["sack"])
    kit.cyl("Bucket", 0.14, 0.22, (0, 0, 1.03), m["oak"], verts=8, r2=0.17)
    # A little gabled roof of two tiled boards.
    for sy in (-1, 1):
        kit.box("Roof", (2.2, 0.85, 0.06), (0, sy * 0.36, 2.72), m["tile"], rot=(sy * math.radians(32), 0, 0))
    kit.box("Ridge", (2.25, 0.1, 0.1), (0, 0, 2.95), m["oak_dark"])


def stone_wall(m):
    rng = random.Random(3)
    # Courses of rough stones, then a flat cap.
    for row, z in enumerate((0.15, 0.42)):
        x = -1.0
        while x < 0.99:
            w = min(rng.uniform(0.35, 0.6), 1.0 - x)
            kit.box("Stone", (w - 0.03, 0.5, 0.27), (x + w / 2, rng.uniform(-0.02, 0.02), z),
                    m["stone" if (row + int(x * 10)) % 3 else "stone_dark"], bevel=0.03)
            x += w
    kit.box("Cap", (2.04, 0.56, 0.1), (0, 0, 0.6), m["stone_dark"], bevel=0.02)


def hedge(m):
    kit.box("Core", (1.95, 0.7, 0.85), (0, 0, 0.45), m["leaf_dark"], bevel=0.12)
    rng = random.Random(5)
    for i in range(5):
        x = -0.8 + i * 0.4
        kit.ball("Tuft%d" % i, 0.32, (x, rng.uniform(-0.05, 0.05), 0.8 + rng.uniform(-0.04, 0.04)), m["leaf"],
                 segs=7, rings=4, scale=(1.1, 1.0, 0.6))


def hand_cart(m):
    kit.box("Bed", (1.0, 1.5, 0.08), (0, 0, 0.62), m["oak"], bevel=0.01)
    for sx in (-1, 1):
        kit.box("Side", (0.05, 1.5, 0.3), (sx * 0.5, 0, 0.8), m["oak"])
        kit.box("Shaft", (0.06, 1.6, 0.06), (sx * 0.35, -1.4, 0.5), m["oak_dark"], rot=(math.radians(-10), 0, 0))
        kit.cyl("Wheel", 0.42, 0.06, (sx * 0.6, 0.15, 0.42), m["oak_dark"], verts=12, rot=(0, math.pi / 2, 0))
        kit.cyl("Hub", 0.08, 0.1, (sx * 0.6, 0.15, 0.42), m["iron"], verts=6, rot=(0, math.pi / 2, 0))
    kit.box("End", (1.0, 0.05, 0.3), (0, 0.75, 0.8), m["oak"])
    kit.cyl("Axle", 0.04, 1.3, (0, 0.15, 0.42), m["iron"], verts=6, rot=(0, math.pi / 2, 0))
    for i, (x, y) in enumerate(((-0.2, 0.3), (0.2, 0.1), (0.0, -0.35))):
        kit.ball("Sack%d" % i, 0.25, (x, y, 0.85), m["sack"], segs=7, rings=4, scale=(1, 1.2, 0.8))


def signpost(m):
    kit.box("Post", (0.12, 0.12, 2.4), (0, 0, 1.2), m["oak_dark"])
    kit.cyl("Cap", 0.1, 0.12, (0, 0, 2.46), m["oak_dark"], verts=4, r2=0.01, rot=(0, 0, math.pi / 4))
    for z, ang in ((2.05, 0.3), (1.72, -1.9)):
        board = kit.box("Board", (0.9, 0.04, 0.22), (0.4, 0, 0), m["paint"])
        tip = kit.cyl("Tip", 0.16, 0.04, (0.9, 0, 0), m["paint"], verts=3, rot=(math.pi / 2, 0, 0))
        for o in (board, tip):
            o.location = (o.location.x * math.cos(ang), o.location.x * math.sin(ang), z)
            o.rotation_euler = (o.rotation_euler.x, 0, ang)


def lily_pads(m):
    rng = random.Random(11)
    for i in range(7):
        a = rng.uniform(0, 2 * math.pi)
        r = rng.uniform(0.0, 1.1)
        kit.cyl("Pad%d" % i, rng.uniform(0.18, 0.3), 0.015, (r * math.cos(a), r * math.sin(a), 0.01), m["pad"],
                verts=8)
    for i, (x, y) in enumerate(((0.3, 0.2), (-0.5, -0.4))):
        kit.cyl("Lily%d" % i, 0.1, 0.08, (x, y, 0.05), m["pink" if i == 0 else "white"], verts=6, r2=0.04)
        kit.ball("Heart%d" % i, 0.03, (x, y, 0.1), m["yellow"], segs=4, rings=3)


def footbridge(m):
    span, width, rise = 7.0, 2.2, 0.55
    planks = 14
    for i in range(planks):
        t = (i + 0.5) / planks
        y = -span / 2 + span * t
        z = 0.2 + rise * math.sin(math.pi * t)
        slope = math.atan(rise * math.pi / span * math.cos(math.pi * t))
        kit.box("Plank%d" % i, (width, span / planks - 0.03, 0.08), (0, y, z), m["oak"], rot=(slope, 0, 0))
    for sx in (-1, 1):
        for i in range(5):
            t = i / 4
            y = -span / 2 + span * t
            z = 0.2 + rise * math.sin(math.pi * t)
            # The posts and rails stand 1 cm proud of the planks' ends,
            # so the posts' outer faces and the planks' ends don't share
            # a plane.
            kit.box("Post", (0.1, 0.1, 1.0), (sx * (width / 2 - 0.04), y, z + 0.5), m["oak_dark"])
        for i in range(4):
            t0, t1 = i / 4, (i + 1) / 4
            y0, y1 = -span / 2 + span * t0, -span / 2 + span * t1
            z0 = 0.2 + rise * math.sin(math.pi * t0) + 0.95
            z1 = 0.2 + rise * math.sin(math.pi * t1) + 0.95
            length = math.hypot(y1 - y0, z1 - z0)
            kit.box("Rail", (0.08, length, 0.08), (sx * (width / 2 - 0.04), (y0 + y1) / 2, (z0 + z1) / 2),
                    m["oak"], rot=(math.atan2(z1 - z0, y1 - y0), 0, 0))
        kit.box("Beam", (0.15, span, 0.2), (sx * (width / 2 - 0.1), 0, 0.1), m["oak_dark"])


def wildflowers(m):
    rng = random.Random(13)
    colors = ["yellow", "white", "violet", "red", "pink", "yellow", "white"]
    # Fourteen round blooms: about half the first version's triangles,
    # since the meadows repeat it dozens of times.
    for i in range(14):
        a = rng.uniform(0, 2 * math.pi)
        r = 1.3 * math.sqrt(rng.uniform(0, 1))
        x, y = r * math.cos(a), r * math.sin(a)
        h = rng.uniform(0.25, 0.5)
        kit.cyl("Stem%d" % i, 0.012, h, (x, y, h / 2), m["leaf"], verts=3, cap=False)
        kit.ball("Bloom%d" % i, rng.uniform(0.07, 0.1), (x, y, h), m[colors[i % len(colors)]], segs=5, rings=2,
                 scale=(1, 1, 0.6))


def bunting(m):
    length, height = 8.0, 4.2
    colors = ["red", "yellow", "violet", "white"]
    for sx in (-1, 1):
        kit.cyl("Pole", 0.06, height + 0.3, (sx * length / 2, 0, (height + 0.3) / 2), m["oak_dark"], verts=6)
    flags = 13
    sag = 0.6
    for i in range(flags):
        t = (i + 0.5) / flags
        x = -length / 2 + length * t
        z = height - sag * math.sin(math.pi * t)
        flag = kit.cyl("Flag%d" % i, 0.22, 0.01, (x, 0, z - 0.2), m[colors[i % len(colors)]], verts=3,
                       rot=(math.pi / 2, 0, math.pi / 2))
        flag.rotation_euler = (math.pi / 2, math.pi / 2, 0)
    for i in range(8):
        t0, t1 = i / 8, (i + 1) / 8
        x0, x1 = -length / 2 + length * t0, -length / 2 + length * t1
        z0, z1 = height - sag * math.sin(math.pi * t0), height - sag * math.sin(math.pi * t1)
        kit.box("Cord", (math.hypot(x1 - x0, z1 - z0), 0.015, 0.015), ((x0 + x1) / 2, 0, (z0 + z1) / 2), m["sack"],
                rot=(0, -math.atan2(z1 - z0, x1 - x0), 0))


def pine_low(m):
    bark = kit.mat("Prop_Bark", (0.16, 0.08, 0.035), 0.9)
    needles = kit.mat("Prop_Needles", (0.03, 0.11, 0.035), 0.9)
    kit.cyl("Trunk", 0.22, 2.4, (0, 0, 1.2), bark, verts=5, r2=0.14)
    for i, (r, z, h) in enumerate(((2.0, 1.6, 3.0), (1.55, 3.4, 2.7), (1.05, 5.1, 2.4))):
        kit.cyl("Tier%d" % i, r, h, (0, 0, z + h / 2), needles, verts=7, r2=0.05)


def oak_low(m):
    bark = kit.mat("Prop_Bark", (0.16, 0.08, 0.035), 0.9)
    leaves = kit.mat("Prop_Canopy", (0.07, 0.2, 0.04), 0.9)
    leaves_light = kit.mat("Prop_CanopyLight", (0.12, 0.27, 0.05), 0.9)
    kit.cyl("Trunk", 0.3, 3.0, (0, 0, 1.5), bark, verts=5, r2=0.2)
    for i, (x, y, z, r) in enumerate(((0.0, 0.0, 4.4, 2.1), (1.1, 0.5, 3.7, 1.5), (-0.9, -0.6, 3.8, 1.6),
                                      (0.2, -0.3, 5.5, 1.4))):
        kit.ball("Crown%d" % i, r, (x, y, z), leaves if i % 2 == 0 else leaves_light, segs=7, rings=4,
                 scale=(1, 1, 0.85))


PROPS = {
    "lamp_post": lamp_post,
    "barrel": barrel,
    "flower_box": flower_box,
    "well": well,
    "stone_wall": stone_wall,
    "hedge": hedge,
    "hand_cart": hand_cart,
    "signpost": signpost,
    "lily_pads": lily_pads,
    "footbridge": footbridge,
    "wildflowers": wildflowers,
    "bunting": bunting,
    "pine_low": pine_low,
    "oak_low": oak_low,
}


def main():
    a = kit.args()
    out = a[0] if a else os.path.join(kit.REPO, "assets", "verse", "generated", "street")
    names = a[1:] or list(PROPS)
    for name in names:
        kit.reset()
        PROPS[name](materials())
        finish(name, out)
    kit.fail_on_flickers()


main()
