"""Original crypt-lab models, in the arrangement of a studied private laboratory.

Run headless:
    Blender -b --factory-startup --python scripts/blender/chamber_lab.py -- \
        [OUT_DIR] [NAME ...]

Reference mode only. The private 1.12.1 Scholomance view was studied for what
the room contains and where those kinds of objects sit: a stone hall, slab
tables along both walls, three cauldron colors, candelabra, floor candles,
two rugs, specimen jars, alchemy benches, bones, and cobwebs. Nothing here
is that geometry, those textures, or those silhouettes. Every solid is a
primitive built in this file.

Frame: 1 unit = 1 m. Fronts face -Y in Blender, which the glTF export turns
into +Z. A prop's origin is on the ground at the base center. The hall's
origin is the center of the floor; glTF +Y is up, glTF +Z is the entrance
end (the studied approach). The hall is an open ruin so a directional light
reaches the floor.

Budgets: the hall stays under 20,000 triangles, and every prop under 1,000.
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

# glTF space after export. The entrance is +Z; the long walls run along Z.
HALL_HALF_X = 20.0
HALL_HALF_Z = 30.0
WALL_T = 0.55
WALL_H = 5.8

NAMES = [
    "crypt_hall",
    "slab_table",
    "cauldron_green",
    "cauldron_red",
    "cauldron_amber",
    "candelabrum_tall",
    "candelabrum_short",
    "floor_candles",
    "ritual_rug",
    "specimen_jar",
    "specimen_jar_bones",
    "alchemy_bench",
    "bone_scatter",
    "cobweb",
]
BUDGET = {"crypt_hall": 20000}
BLOCKERS = ("wall", "column", "ledge")


def out_dir_and_names():
    argv = kit.args()
    default = os.path.join(kit.REPO, "assets", "verse", "generated", "chamber")
    if argv and argv[0] not in NAMES:
        return argv[0], (argv[1:] or NAMES)
    return default, (argv or NAMES)


def fade(material, alpha):
    """Mark a material as blended and double-sided. Verse reads that alpha."""
    material.node_tree.nodes["Principled BSDF"].inputs["Alpha"].default_value = alpha
    material.blend_method = "BLEND"
    if hasattr(material, "surface_render_method"):
        material.surface_render_method = "BLENDED"
    material.use_backface_culling = False
    color = material.diffuse_color
    material.diffuse_color = (color[0], color[1], color[2], alpha)


def finish(name, ground=True, blockers=BLOCKERS):
    kit.flat()
    sources = list(kit.meshes())
    # Record the hall's pieces before join() drops their names. A single box
    # around the hall would seal the interior.
    boxes = None
    if not ground:
        boxes = [aabb_box(obj, obj.name) for obj in sources if obj.name.startswith(blockers)]
    obj = kit.join(name) if len(sources) > 1 else sources[0]
    if ground:
        kit.ground(obj)
        boxes = [aabb_box(obj, name)]
    write_footprint(name, boxes)
    folder = out_dir_and_names()[0]
    info = kit.export(os.path.join(folder, name + ".glb"))
    limit = BUDGET.get(name, 1000)
    if info["triangles"] > limit:
        sys.exit("%s is %s triangles; the budget is %s" % (name, info["triangles"], limit))
    return info


def write_footprint(name, boxes):
    """Write glTF-space collision boxes beside the model."""
    path = os.path.join(out_dir_and_names()[0], name + ".footprint.json")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as handle:
        kit.json.dump(
            {
                "model": name + ".glb",
                "frame": "glTF: 1 unit = 1 m, +Y up, +Z toward the entrance, origin at the base center",
                "triangles": kit.triangles(),
                "boxes": boxes,
            },
            handle,
            indent=2,
        )
        handle.write("\n")


def aabb_box(obj, name):
    pts = [obj.matrix_world @ v.co for v in obj.data.vertices]
    # Blender (x, y, z) becomes glTF (x, z, -y).
    glo = [
        min(p.x for p in pts),
        min(p.z for p in pts),
        min(-p.y for p in pts),
    ]
    ghi = [
        max(p.x for p in pts),
        max(p.z for p in pts),
        max(-p.y for p in pts),
    ]
    return {
        "name": name,
        "center": [round((a + b) / 2, 3) for a, b in zip(glo, ghi)],
        "half_extents": [round((b - a) / 2, 3) for a, b in zip(glo, ghi)],
    }


def stone_materials():
    return (
        kit.mat("Crypt_Stone", (0.34, 0.32, 0.29), 0.92),
        kit.mat("Crypt_StoneDark", (0.16, 0.15, 0.14), 0.95),
        kit.mat("Crypt_StoneTrim", (0.42, 0.40, 0.36), 0.85),
    )


def build_crypt_hall():
    """An open ruined hall. The floor center is the origin. The entrance is at Blender -Y, which the export turns into glTF +Z."""
    kit.reset()
    stone, dark, trim = stone_materials()
    # Blender Y runs toward the entrance, which becomes glTF -Z. Build the
    # entrance at blender -Y so it lands on glTF +Z? 
    # Export: glTF z = -blender y. Entrance at glTF +Z means blender y negative.
    kit.box(
        "floor",
        (HALL_HALF_X * 2 + WALL_T, HALL_HALF_Z * 2 + WALL_T, 0.28),
        (0, 0, -0.14),
        dark,
    )
    kit.cyl("floor_inlay", 8.4, 0.04, (0, 0, 0.02), stone, verts=24)
    kit.ring("trim_curb", 8.15, 0.11, (0, 0, 0.1), trim, segs=24, minor_segs=6)
    # Long walls, along blender Y (glTF Z).
    for side, x in (("east", HALL_HALF_X), ("west", -HALL_HALF_X)):
        kit.box(
            "wall_%s" % side,
            (WALL_T, HALL_HALF_Z * 2 + WALL_T, WALL_H),
            (x + (WALL_T / 2 if side == "east" else -WALL_T / 2), 0, WALL_H / 2),
            stone,
        )
        kit.box(
            "ledge_%s" % side,
            (1.15, HALL_HALF_Z * 2 - 4, 0.18),
            (x - 0.85 if side == "east" else x + 0.85, 0, 3.15),
            dark,
        )
    # Far wall, opposite the entrance: blender +Y, glTF -Z.
    kit.box(
        "wall_far",
        (HALL_HALF_X * 2, WALL_T, WALL_H),
        (0, HALL_HALF_Z + WALL_T / 2, WALL_H / 2),
        stone,
    )
    # Entrance wall, split around a 4 m door. blender -Y is glTF +Z.
    door_y = -(HALL_HALF_Z + WALL_T / 2)
    kit.box("wall_door_west", (HALL_HALF_X - 2, WALL_T, WALL_H), (-(HALL_HALF_X + 2) / 2, door_y, WALL_H / 2), stone)
    kit.box("wall_door_east", (HALL_HALF_X - 2, WALL_T, WALL_H), ((HALL_HALF_X + 2) / 2, door_y, WALL_H / 2), stone)
    kit.box("wall_lintel", (4.0, WALL_T, WALL_H - 3.3), (0, door_y, 3.3 + (WALL_H - 3.3) / 2), stone)
    # Broken courses along the wall tops. Seeded from the model name.
    span = int((HALL_HALF_Z - 1.2) / 2.4)
    for i in range(-span, span + 1):
        y = i * 2.4
        for side, x in (("e", HALL_HALF_X), ("w", -HALL_HALF_X)):
            h = 0.35 + (mix(i, side) % 5) * 0.16
            kit.box(
                "trim_merlon_%s_%s" % (side, i),
                (WALL_T + 0.08, 1.15, h),
                (x, y, WALL_H + h / 2),
                trim if mix(i, side) % 2 == 0 else stone,
            )
    # Columns outside the 8 m circle, clear of the table rows.
    for x in (-12.5, 12.5):
        for y in (-16, 0, 16):
            kit.cyl("column_%s_%s" % (x, y), 0.42, WALL_H - 0.4, (x, y, (WALL_H - 0.4) / 2), dark, verts=8)
            kit.box("column_cap_%s_%s" % (x, y), (1.15, 1.15, 0.22), (x, y, WALL_H - 0.55), trim)
    # A few remaining ribs, high enough to walk under.
    for y in (-20, -8, 8, 20):
        kit.box("trim_rib_%s" % y, (HALL_HALF_X * 2 - 1.5, 0.28, 0.28), (0, y, 5.35), dark)
    return finish("crypt_hall", ground=False)


def mix(i, side):
    text = "crypt_hall%s%s" % (side, i)
    value = 2166136261
    for char in text:
        value ^= ord(char)
        value = (value * 16777619) & 0xFFFFFFFF
    return value


def build_slab_table():
    kit.reset()
    iron = kit.mat("Table_Iron", (0.15, 0.14, 0.13), 0.4, 0.65)
    dark = kit.mat("Table_Shelf", (0.1, 0.09, 0.08), 0.7, 0.4)
    kit.box("Top", (2.35, 0.86, 0.07), (0, 0, 0.86), iron, bevel=0.01)
    kit.box("Shelf", (2.05, 0.62, 0.04), (0, 0, 0.34), dark)
    for x in (-1.0, 1.0):
        for y in (-0.3, 0.3):
            kit.box("Leg_%s_%s" % (x, y), (0.08, 0.08, 0.82), (x, y, 0.41), iron)
    return finish("slab_table")


def build_cauldron(name, rgb):
    kit.reset()
    iron = kit.mat("Cauldron_Iron", (0.13, 0.12, 0.11), 0.38, 0.72)
    liquid = kit.mat("Cauldron_Liquid_" + name, rgb, 0.22, 0.0, emit=rgb, strength=0.55)
    kit.lathe(
        "Bowl",
        [(0, 0.16), (0.28, 0.16), (0.58, 0.28), (0.66, 0.55), (0.52, 0.74)],
        material=iron,
        segs=16,
    )
    kit.ring("Rim", 0.56, 0.045, (0, 0, 0.74), iron, segs=16, minor_segs=5)
    kit.cyl("Liquid", 0.48, 0.05, (0, 0, 0.62), liquid, verts=16)
    for i in range(3):
        angle = i * math.tau / 3
        kit.cyl(
            "Leg%s" % i,
            0.045,
            0.24,
            (math.cos(angle) * 0.38, math.sin(angle) * 0.38, 0.1),
            iron,
            verts=6,
        )
    return finish(name)


def candle(stem, flame, index, loc, height):
    kit.cyl("Candle%s" % index, 0.035, height, (loc[0], loc[1], loc[2] + height / 2), stem, verts=8)
    kit.ball("Flame%s" % index, 0.045, (loc[0], loc[1], loc[2] + height + 0.03), flame, segs=8, rings=4)


def build_candelabrum_tall():
    kit.reset()
    iron = kit.mat("Candelabrum_Iron", (0.12, 0.13, 0.12), 0.35, 0.7)
    glass = kit.mat("Candelabrum_Glass", (0.25, 0.9, 0.4), 0.12, 0.0, emit=(0.3, 1.0, 0.45), strength=0.8)
    wax = kit.mat("Candle_Wax", (0.86, 0.8, 0.62), 0.7)
    flame = kit.mat("Candle_Flame", (1.0, 0.72, 0.22), 0.4, 0.0, emit=(1.0, 0.65, 0.15), strength=1.4)
    kit.cyl("Stem", 0.06, 1.7, (0, 0, 0.85), iron, verts=8)
    kit.cyl("Foot", 0.28, 0.06, (0, 0, 0.03), iron, verts=8)
    kit.ball("Globe", 0.16, (0, 0, 0.7), glass, segs=10, rings=6)
    for i in range(5):
        angle = i * math.tau / 5
        arm = (math.cos(angle) * 0.42, math.sin(angle) * 0.42, 1.55)
        kit.cyl(
            "Arm%s" % i,
            0.025,
            0.46,
            (arm[0] / 2, arm[1] / 2, 1.5),
            iron,
            verts=6,
            rot=(0, math.radians(70), angle),
        )
        kit.cyl("Cup%s" % i, 0.05, 0.04, arm, iron, verts=8)
        candle(wax, flame, i, arm, 0.16)
    return finish("candelabrum_tall")


def build_candelabrum_short():
    kit.reset()
    iron = kit.mat("Candelabrum_IronShort", (0.16, 0.15, 0.13), 0.4, 0.55)
    wax = kit.mat("Candle_WaxShort", (0.9, 0.86, 0.7), 0.65)
    flame = kit.mat("Candle_FlameShort", (1.0, 0.78, 0.3), 0.4, 0.0, emit=(1.0, 0.7, 0.2), strength=1.2)
    kit.cyl("Base", 0.16, 0.04, (0, 0, 0.02), iron, verts=10)
    for i, (x, y) in enumerate(((0, 0), (0.1, 0.06), (-0.09, 0.05))):
        candle(wax, flame, i, (x, y, 0.04), 0.22 + i * 0.05)
    return finish("candelabrum_short")


def build_floor_candles():
    kit.reset()
    iron = kit.mat("CandlePlate", (0.18, 0.16, 0.14), 0.45, 0.4)
    wax = kit.mat("FloorCandle_Wax", (0.93, 0.88, 0.72), 0.6)
    flame = kit.mat("FloorCandle_Flame", (1.0, 0.7, 0.25), 0.35, 0.0, emit=(1.0, 0.62, 0.12), strength=1.3)
    kit.cyl("Plate", 0.34, 0.03, (0, 0, 0.015), iron, verts=12)
    spots = ((0, 0, 0.28), (0.14, 0.05, 0.18), (-0.12, 0.08, 0.22), (0.02, -0.14, 0.16), (-0.05, -0.1, 0.26), (0.16, -0.08, 0.14), (-0.15, -0.02, 0.2))
    for i, (x, y, h) in enumerate(spots):
        candle(wax, flame, i, (x, y, 0.03), h)
    return finish("floor_candles")


def build_ritual_rug():
    kit.reset()
    cloth = kit.mat("Rug_Cloth", (0.1, 0.24, 0.16), 0.9)
    border = kit.mat("Rug_Border", (0.55, 0.46, 0.22), 0.85)
    kit.box("Field", (3.2, 6.4, 0.02), (0, 0, 0.01), cloth)
    kit.box("Border", (3.55, 6.75, 0.015), (0, 0, 0.008), border)
    return finish("ritual_rug")


def build_jar(name, bones):
    kit.reset()
    iron = kit.mat("Jar_Iron", (0.2, 0.18, 0.15), 0.35, 0.6)
    glass = kit.mat("Jar_Glass", (0.7, 0.86, 0.78), 0.08)
    fade(glass, 0.38)
    kit.cyl("Base", 0.3, 0.06, (0, 0, 0.03), iron, verts=12)
    kit.cyl("Body", 0.26, 0.85, (0, 0, 0.5), glass, verts=12)
    kit.cyl("Neck", 0.12, 0.12, (0, 0, 0.98), glass, verts=10)
    kit.cyl("Lid", 0.14, 0.05, (0, 0, 1.06), iron, verts=10)
    for i in range(4):
        z = 0.22 + i * 0.18
        kit.ring("Band%s" % i, 0.27, 0.012, (0, 0, z), iron, segs=12, minor_segs=4)
    if bones:
        bone = kit.mat("Jar_Bone", (0.84, 0.8, 0.68), 0.75)
        kit.ball("Skull", 0.09, (0, 0, 0.28), bone, segs=8, rings=5)
        kit.cyl("Spine", 0.025, 0.28, (0, 0, 0.48), bone, verts=6)
        for i in range(4):
            kit.cyl(
                "Rib%s" % i,
                0.012,
                0.16,
                (0, 0, 0.4 + i * 0.06),
                bone,
                verts=6,
                rot=(0, math.radians(90), 0),
            )
    return finish(name)


def build_alchemy_bench():
    kit.reset()
    wood = kit.mat("Bench_Wood", (0.32, 0.18, 0.09), 0.8)
    iron = kit.mat("Bench_Iron", (0.18, 0.17, 0.16), 0.4, 0.5)
    glass = kit.mat("Bench_Glass", (0.45, 0.7, 0.85), 0.15)
    green = kit.mat("Bench_Green", (0.2, 0.72, 0.28), 0.3, 0.0, emit=(0.2, 0.7, 0.25), strength=0.3)
    kit.box("Top", (1.4, 0.7, 0.06), (0, 0, 0.78), wood, bevel=0.008)
    for x in (-0.58, 0.58):
        for y in (-0.24, 0.24):
            kit.box("Leg_%s_%s" % (x, y), (0.07, 0.07, 0.75), (x, y, 0.375), wood)
    kit.cyl("Retort", 0.1, 0.16, (-0.35, 0, 0.9), glass, verts=10)
    kit.cyl("Neck", 0.035, 0.18, (-0.18, 0, 1.02), glass, verts=8, rot=(0, math.radians(55), 0))
    kit.cyl("Bottle", 0.05, 0.16, (0.15, 0.1, 0.9), green, verts=8)
    kit.cyl("BottleB", 0.04, 0.12, (0.28, -0.08, 0.87), glass, verts=8)
    kit.box("Book", (0.22, 0.16, 0.05), (0.4, 0.12, 0.84), iron)
    return finish("alchemy_bench")


def build_bone_scatter():
    kit.reset()
    bone = kit.mat("Scatter_Bone", (0.8, 0.76, 0.64), 0.8)
    kit.ball("Skull", 0.09, (0, 0.05, 0.08), bone, segs=8, rings=5)
    kit.cyl("Femur", 0.018, 0.34, (0.12, -0.02, 0.03), bone, verts=6, rot=(0, math.radians(80), math.radians(30)))
    kit.cyl("Rib", 0.012, 0.16, (-0.1, 0.08, 0.025), bone, verts=5, rot=(math.radians(70), 0, 0))
    kit.cyl("Shard", 0.01, 0.12, (0.02, -0.14, 0.02), bone, verts=5, rot=(0, math.radians(60), math.radians(-20)))
    return finish("bone_scatter")


def build_cobweb():
    kit.reset()
    silk = kit.mat("Web_Silk", (0.78, 0.8, 0.76), 0.6)
    fade(silk, 0.45)
    # A corner fan in the XY plane, standing up the wall (+Z) and out (+X).
    kit.box("SheetA", (1.4, 0.02, 1.1), (0.7, 0, 0.55), silk)
    kit.box("SheetB", (0.02, 1.1, 1.2), (0, 0.55, 0.6), silk)
    for i in range(4):
        kit.cyl(
            "Strand%s" % i,
            0.008,
            1.3,
            (0.35, 0.15 * i, 0.7),
            silk,
            verts=4,
            rot=(0, math.radians(50 + i * 8), math.radians(20)),
        )
    return finish("cobweb")


BUILDERS = {
    "crypt_hall": build_crypt_hall,
    "slab_table": build_slab_table,
    "cauldron_green": lambda: build_cauldron("cauldron_green", (0.15, 0.75, 0.28)),
    "cauldron_red": lambda: build_cauldron("cauldron_red", (0.75, 0.1, 0.08)),
    "cauldron_amber": lambda: build_cauldron("cauldron_amber", (0.9, 0.5, 0.08)),
    "candelabrum_tall": build_candelabrum_tall,
    "candelabrum_short": build_candelabrum_short,
    "floor_candles": build_floor_candles,
    "ritual_rug": build_ritual_rug,
    "specimen_jar": lambda: build_jar("specimen_jar", False),
    "specimen_jar_bones": lambda: build_jar("specimen_jar_bones", True),
    "alchemy_bench": build_alchemy_bench,
    "bone_scatter": build_bone_scatter,
    "cobweb": build_cobweb,
}


def write_provenance(built):
    folder = out_dir_and_names()[0]
    lines = [
        "# Crypt lab models",
        "",
        "Mode: **Reference**. These models are original solids built from",
        "primitives. A private 1.12.1 Scholomance laboratory view was studied",
        "for the kinds of objects in the room and where they sit. No mesh,",
        "texture, font, or UI file from that view is in this folder, and the",
        "models are not copies of those silhouettes.",
        "",
        "Script: `scripts/blender/chamber_lab.py`",
        "",
        "Command:",
        "",
        "```sh",
        "Blender -b --factory-startup --python scripts/blender/chamber_lab.py -- \\",
        "    assets/verse/generated/chamber",
        "```",
        "",
        "Blender %s." % built[0]["blender"],
        "",
        "The hall is an open ruin. A closed ceiling would leave the floor in",
        "shadow under the zone renderer's directional light.",
        "",
        "| Model | Triangles |",
        "| --- | --- |",
    ]
    for info in built:
        lines.append("| `%s` | %s |" % (os.path.basename(info["out"]), info["triangles"]))
    lines.append("")
    path = os.path.join(folder, "PROVENANCE.md")
    with open(path, "w") as handle:
        handle.write("\n".join(lines))


def main():
    folder, names = out_dir_and_names()
    os.makedirs(folder, exist_ok=True)
    unknown = [name for name in names if name not in BUILDERS]
    if unknown:
        sys.exit("Unknown model %s" % ", ".join(unknown))
    built = [BUILDERS[name]() for name in names]
    if names == NAMES:
        write_provenance(built)


if __name__ == "__main__":
    main()
