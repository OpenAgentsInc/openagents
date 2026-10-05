"""Render review views of generated buildings.

Run headless:
    Blender -b --factory-startup --python scripts/blender/building_views.py -- \
        views IN.glb OUT.png
    Blender -b --factory-startup --python scripts/blender/building_views.py -- \
        gallery IN_DIR OUT.png

`views` writes one sheet with four views of a building: front three-quarter,
street level (eye height 1.7 m across a 9 m street), rear three-quarter, and
straight front. `gallery` lays every glb in IN_DIR on a grid of short streets and
renders them from above. Buildings face +Z in glTF, which is -Y here.
"""

import math
import os
import sys

import bpy
import numpy as np
from mathutils import Vector


def scene_setup(w, h):
    s = bpy.context.scene
    s.render.engine = "BLENDER_EEVEE"
    s.render.resolution_x, s.render.resolution_y = w, h
    s.render.film_transparent = False
    s.view_settings.view_transform = "Standard"
    world = bpy.data.worlds.new("sky")
    world.use_nodes = True
    world.node_tree.nodes["Background"].inputs[0].default_value = (0.52, 0.66, 0.85, 1)
    world.node_tree.nodes["Background"].inputs[1].default_value = 0.9
    s.world = world
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    sun.data.energy = 3.2
    sun.data.angle = math.radians(3)
    sun.rotation_euler = (math.radians(50), 0, math.radians(-35))
    s.collection.objects.link(sun)
    ground = bpy.data.meshes.new("ground")
    ground.from_pydata([(-3000, -3000, 0), (3000, -3000, 0), (3000, 3000, 0), (-3000, 3000, 0)], [], [(0, 1, 2, 3)])
    mat = bpy.data.materials.new("ground")
    mat.use_nodes = True
    mat.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.20, 0.24, 0.16, 1)
    mat.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 1.0
    ground.materials.append(mat)
    g = bpy.data.objects.new("ground", ground)
    g.location.z = -0.005
    s.collection.objects.link(g)


def bounds(objs):
    pts = []
    for o in objs:
        if o.type == "MESH":
            pts += [o.matrix_world @ Vector(c) for c in o.bound_box]
    lo = Vector([min(p[i] for p in pts) for i in range(3)])
    hi = Vector([max(p[i] for p in pts) for i in range(3)])
    return lo, hi


def shoot(cam, eye, target, lens, path):
    cam.location = Vector(eye)
    cam.rotation_euler = (Vector(target) - cam.location).to_track_quat("-Z", "Y").to_euler()
    cam.data.lens = lens
    s = bpy.context.scene
    s.render.filepath = path
    bpy.ops.render.render(write_still=True)
    img = bpy.data.images.load(path)
    px = np.empty(img.size[0] * img.size[1] * 4, dtype=np.float32)
    img.pixels.foreach_get(px)
    shape = (img.size[1], img.size[0], 4)
    bpy.data.images.remove(img)
    return px.reshape(shape)


def save_sheet(tiles, cols, path):
    h, w = tiles[0].shape[:2]
    rows = math.ceil(len(tiles) / cols)
    sheet = np.ones((rows * h, cols * w, 4), dtype=np.float32)
    for i, t in enumerate(tiles):
        gx, gy = i % cols, i // cols
        y0 = (rows - 1 - gy) * h
        sheet[y0:y0 + h, gx * w:(gx + 1) * w] = t
    img = bpy.data.images.new("sheet", cols * w, rows * h)
    img.pixels.foreach_set(sheet.ravel())
    img.filepath_raw = path
    img.file_format = "PNG"
    img.save()


def import_glb(path):
    before = set(bpy.data.objects)
    bpy.ops.import_scene.gltf(filepath=path)
    return [o for o in bpy.data.objects if o not in before]


def views(src, out):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    objs = import_glb(src)
    scene_setup(800, 600)
    lo, hi = bounds(objs)
    c = (lo + hi) / 2
    r = (hi - lo).length
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    bpy.context.scene.collection.objects.link(cam)
    bpy.context.scene.camera = cam
    tmp = out + ".tmp.png"
    tiles = [
        shoot(cam, c + Vector((r * 0.75, -r * 1.0, r * 0.45)), c, 40, tmp),
        shoot(cam, (lo.x - 3.0, lo.y - 9.0, 1.7), (c.x + 1.0, c.y, hi.z * 0.45), 22, tmp),
        shoot(cam, c + Vector((-r * 0.8, r * 0.95, r * 0.5)), c, 40, tmp),
        shoot(cam, (c.x, lo.y - r * 1.25, c.z), c, 40, tmp),
    ]
    os.remove(tmp)
    save_sheet(tiles, 2, out)


def gallery(src_dir, out):
    """Every glb in `src_dir` on a grid of short streets, seen from above."""
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene_setup(2400, 1500)
    names = sorted(f for f in os.listdir(src_dir) if f.endswith(".glb"))
    per_row = math.ceil(math.sqrt(len(names)))
    every = []
    x, y, row_depth = 0.0, 0.0, 0.0
    for i, f in enumerate(names):
        if i and i % per_row == 0:
            x, y, row_depth = 0.0, y + row_depth + 6.0, 0.0
        objs = import_glb(os.path.join(src_dir, f))
        roots = [o for o in objs if o.parent is None]
        lo, hi = bounds(objs)
        for o in roots:
            o.location.x += x - lo.x
            o.location.y += y - lo.y
        x += (hi.x - lo.x) + 3.0
        row_depth = max(row_depth, hi.y - lo.y)
        every += objs
    lo, hi = bounds(every)
    c = (lo + hi) / 2
    span = max(hi.x - lo.x, hi.y - lo.y)
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    bpy.context.scene.collection.objects.link(cam)
    bpy.context.scene.camera = cam
    tmp = out + ".tmp.png"
    tile = shoot(cam, c + Vector((span * 0.3, -span * 1.45, span * 0.95)), c + Vector((0, -span * 0.08, 0)), 35, tmp)
    os.remove(tmp)
    save_sheet([tile], 1, out)


def look(src, out, ex, ey, ez, tx, ty, tz, lens="35"):
    """One view from an eye point at a target, in Blender coordinates."""
    bpy.ops.wm.read_factory_settings(use_empty=True)
    import_glb(src)
    scene_setup(1000, 750)
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    bpy.context.scene.collection.objects.link(cam)
    bpy.context.scene.camera = cam
    tmp = out + ".tmp.png"
    tile = shoot(cam, tuple(map(float, (ex, ey, ez))), tuple(map(float, (tx, ty, tz))), float(lens), tmp)
    os.remove(tmp)
    save_sheet([tile], 1, out)


def boxes(src, footprint, out):
    """The building with its footprint boxes drawn over it in translucent red."""
    import json

    bpy.ops.wm.read_factory_settings(use_empty=True)
    objs = import_glb(src)
    scene_setup(800, 600)
    mat = bpy.data.materials.new("box")
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (1.0, 0.1, 0.1, 1)
    bsdf.inputs["Alpha"].default_value = 0.35
    mat.surface_render_method = "BLENDED"
    for box in json.load(open(footprint))["boxes"]:
        # glTF (x, y, z) is Blender (x, -z, y).
        (cx, cy, cz), (hx, hy, hz) = box["center"], box["half_extents"]
        bpy.ops.mesh.primitive_cube_add(location=(cx, -cz, cy), scale=(hx, hz, hy))
        bpy.context.active_object.data.materials.append(mat)
    lo, hi = bounds(objs)
    c, r = (lo + hi) / 2, (hi - lo).length
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    bpy.context.scene.collection.objects.link(cam)
    bpy.context.scene.camera = cam
    tmp = out + ".tmp.png"
    tiles = [
        shoot(cam, c + Vector((r * 0.75, -r * 1.0, r * 0.45)), c, 40, tmp),
        shoot(cam, c + Vector((-r * 0.8, r * 0.95, r * 0.5)), c, 40, tmp),
    ]
    os.remove(tmp)
    save_sheet(tiles, 2, out)


if __name__ == "__main__":
    args = sys.argv[sys.argv.index("--") + 1:]
    {"views": views, "gallery": gallery, "look": look, "boxes": boxes}[args[0]](*args[1:])
