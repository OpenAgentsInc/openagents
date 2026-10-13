"""The dungeon classroom behind Draconia, for her parity portraits.

Run headless:
    Blender -b --factory-startup --python scripts/blender/draconia_set.py -- [OUT.glb]

An original set of a common kind: a dim stone wall of rough blocks, dark
wooden shelves of specimen jars whose liquid glows a cool teal (the room's
practical light), a tall dark cabinet, and a table with a candle and
papers in front of her. The jars carry dark original specimen shapes (roots,
curled forms), never anything recognisable. The set sits in her frame
(+Z up, she stands at the origin facing -Y), so the wall is behind her at
+Y. Materials are standard PBR, the jar liquid emissive.

Writes ~/work/film-out/draconia/build/draconia_set.glb by default.
"""

import math
import os
import random
import sys

import bmesh
import bpy
from mathutils import Vector

OUT = os.path.expanduser("~/work/film-out/draconia/build/draconia_set.glb")


def mat(name, color, rough=0.8, metal=0.0, emit=None, strength=1.0):
    m = bpy.data.materials.get(name)
    if m:
        return m
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    b = m.node_tree.nodes["Principled BSDF"]
    b.inputs["Base Color"].default_value = color
    b.inputs["Roughness"].default_value = rough
    b.inputs["Metallic"].default_value = metal
    if emit:
        b.inputs["Emission Color"].default_value = emit
        b.inputs["Emission Strength"].default_value = strength
    return m


def box(name, size, loc, material, bevel=0.0, rot=0.0):
    bpy.ops.mesh.primitive_cube_add(size=1, location=loc, rotation=(0, 0, rot))
    o = bpy.context.object
    o.name = name
    o.scale = size
    bpy.ops.object.transform_apply(scale=True)
    if bevel:
        m = o.modifiers.new("b", "BEVEL")
        m.width = bevel
        m.segments = 2
        bpy.ops.object.modifier_apply(modifier=m.name)
    o.data.materials.append(material)
    return o


def cylinder(name, r, h, loc, material, verts=32):
    bpy.ops.mesh.primitive_cylinder_add(radius=r, depth=h, location=loc, vertices=verts)
    o = bpy.context.object
    o.name = name
    o.data.materials.append(material)
    for p in o.data.polygons:
        p.use_smooth = True
    return o


def build():
    rng = random.Random(3)
    stones = [mat(f"stone_{k}", (0.055 + 0.02 * k, 0.065 + 0.018 * k, 0.062 + 0.016 * k, 1), rough=0.92) for k in range(4)]
    mortar = mat("mortar", (0.025, 0.03, 0.03, 1), rough=1.0)
    wood = mat("wood_dark", (0.035, 0.022, 0.014, 1), rough=0.6)
    wood2 = mat("wood_shelf", (0.06, 0.04, 0.026, 1), rough=0.65)
    glass = mat("jar_glass", (0.5, 0.6, 0.58, 1), rough=0.08)
    liquids = [mat(f"jar_liquid_{k}", (0.1, 0.4, 0.35, 1), rough=0.3, emit=(0.25 + 0.05 * k, 0.95, 0.8 - 0.05 * k, 1), strength=0.05 + 0.03 * k) for k in range(3)]
    murk = mat("jar_specimen", (0.05, 0.04, 0.025, 1), rough=0.7)
    lid = mat("jar_lid", (0.08, 0.07, 0.06, 1), rough=0.4, metal=0.6)
    paper = mat("paper", (0.55, 0.5, 0.4, 1), rough=0.9)
    wax = mat("candle_wax", (0.75, 0.68, 0.5, 1), rough=0.6)
    flame = mat("candle_flame", (1.0, 0.7, 0.3, 1), emit=(1.0, 0.62, 0.25, 1), strength=40)
    objs = []
    # Wall: rough blocks in courses, 2.6 m behind her.
    wall_y = 2.6
    objs.append(box("mortar", (8, 0.1, 5), (0, wall_y + 0.1, 2.0), mortar))
    z = -0.4
    course = 0
    while z < 4.4:
        h = rng.uniform(0.24, 0.34)
        x = -4.0 + (0.25 if course % 2 else 0)
        while x < 4.0:
            w = rng.uniform(0.38, 0.62)
            d = rng.uniform(0.05, 0.12)
            o = box("stone", (w - 0.018, d, h - 0.018), (x + w / 2, wall_y - d / 2 + 0.05, z + h / 2), rng.choice(stones), bevel=0.02)
            objs.append(o)
            x += w
        z += h
        course += 1
    # Shelves with jars, between her and the wall.
    shelf_y = 2.25
    for k, sz in enumerate((0.75, 1.2, 1.65, 2.1)):
        objs.append(box("shelf", (3.4, 0.34, 0.035), (0.6, shelf_y, sz), wood2, bevel=0.004))
        x = -1.0
        while x < 2.2:
            r = rng.uniform(0.06, 0.11)
            h = rng.uniform(0.18, 0.34)
            cx = x + r
            base = sz + 0.0175
            # Most jars glow teal; some are dark and murky.
            kind = rng.choice(liquids + liquids + [murk])
            liq = cylinder("liquid", r * 0.9, h * 0.85, (cx, shelf_y + rng.uniform(-0.05, 0.05), base + h * 0.85 / 2), kind)
            objs.append(liq)
            # A dark specimen inside: a lumpy blob or a curled root.
            bpy.ops.mesh.primitive_ico_sphere_add(radius=r * 0.55, subdivisions=2, location=(cx, liq.location.y, base + h * 0.42))
            blob = bpy.context.object
            blob.scale = (rng.uniform(0.6, 1.0), rng.uniform(0.6, 1.0), rng.uniform(0.8, 1.6))
            for v in blob.data.vertices:
                v.co *= rng.uniform(0.8, 1.2)
            bpy.ops.object.transform_apply(scale=True)
            blob.data.materials.append(murk)
            objs.append(blob)
            objs.append(cylinder("lid", r * 1.02, 0.03, (cx, liq.location.y, base + h + 0.015), lid))
            x += 2 * r + rng.uniform(0.08, 0.35)
    # Uprights.
    for x in (-1.1, 2.3):
        objs.append(box("upright", (0.05, 0.36, 1.6), (x, shelf_y, 1.45), wood2))
    # Cabinet, left.
    objs.append(box("cabinet", (0.9, 0.5, 2.3), (-1.9, 2.0, 1.15), wood, bevel=0.01))
    objs.append(box("cabinet_door_l", (0.42, 0.02, 1.9), (-2.12, 1.74, 1.2), wood, bevel=0.01))
    objs.append(box("cabinet_door_r", (0.42, 0.02, 1.9), (-1.68, 1.74, 1.2), wood, bevel=0.01))
    # Table in front of her, to camera left, with a candle and papers.
    objs.append(box("table", (1.4, 0.8, 0.05), (-0.9, -1.1, 0.78), wood, bevel=0.006))
    objs.append(box("paper", (0.3, 0.22, 0.002), (-0.75, -1.05, 0.806), paper, rot=0.2))
    objs.append(cylinder("candle", 0.022, 0.16, (-0.55, -0.95, 0.885), wax))
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.008, location=(-0.55, -0.95, 0.985))
    f = bpy.context.object
    f.scale = (1, 1, 2.2)
    bpy.ops.object.transform_apply(scale=True)
    f.data.materials.append(flame)
    objs.append(f)
    # Floor.
    objs.append(box("floor", (8, 8, 0.1), (0, 0, -0.05), stones[0]))
    return objs


def glow():
    """The peacock guardian's core as a backdrop: a large disc of white-blue
    light behind her, brightest at the centre, with a darker blue ring
    round it (concentric discs, so no texture is needed)."""
    objs = []
    for k, (r, col) in enumerate([(3.0, (0.35, 0.45, 0.75, 1)), (2.0, (0.7, 0.8, 1.0, 1)), (1.2, (0.95, 0.97, 1.0, 1)), (0.6, (1.0, 1.0, 1.0, 1))]):
        m = mat(f"glow_{k}", col, emit=col, strength=1.0)
        bpy.ops.mesh.primitive_circle_add(radius=r, vertices=96, fill_type="NGON", location=(0, 2.5 - 0.01 * k, 1.5), rotation=(math.pi / 2, 0, 0))
        o = bpy.context.object
        o.data.materials.append(m)
        objs.append(o)
    return objs


def main():
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    glow_only = "--glow" in argv
    argv = [a for a in argv if a != "--glow"]
    out = argv[0] if argv else (OUT if not glow_only else OUT.replace("draconia_set", "draconia_glow"))
    bpy.ops.wm.read_factory_settings(use_empty=True)
    objs = glow() if glow_only else build()
    # One mesh (one glTF node), its materials as primitives: Verse's
    # importer takes at most 256 nodes.
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    bpy.ops.object.join()
    os.makedirs(os.path.dirname(out), exist_ok=True)
    bpy.ops.export_scene.gltf(filepath=out, export_format="GLB", use_selection=True, export_apply=True, export_yup=True, export_animations=False, export_skins=False)
    print("SET", out, len(objs))


if __name__ == "__main__":
    main()
