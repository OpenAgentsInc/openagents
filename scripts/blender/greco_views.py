"""Render review views of the Greco-futurism house and kit.

Run headless:
    Blender -b --factory-startup --python scripts/blender/greco_views.py -- \
        house IN.glb OUT_DIR
    Blender -b --factory-startup --python scripts/blender/greco_views.py -- \
        kit KIT_DIR OUT.png

`house` writes `house_<view>.png` for the street front, a three-quarter
view, the stair, the portico's door, an aerial view, and two views of the
great room, under a low warm sun with stand-in trees framing the lot.
`kit` lays every glb in KIT_DIR in a row of labeled cells and renders them
from the front three-quarter. Models face +Z in glTF, which is -Y here.
"""

import math
import os
import sys

import bpy
from mathutils import Vector


def scene(w, h, sun_energy=4.0):
    s = bpy.context.scene
    s.render.engine = "BLENDER_EEVEE"
    s.render.resolution_x, s.render.resolution_y = w, h
    s.view_settings.view_transform = "AgX"
    world = bpy.data.worlds.new("sky")
    world.use_nodes = True
    bg = world.node_tree.nodes["Background"]
    bg.inputs[0].default_value = (0.62, 0.72, 0.86, 1)
    bg.inputs[1].default_value = 0.8
    s.world = world
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    sun.data.energy = sun_energy
    sun.data.color = (1.0, 0.86, 0.68)
    sun.data.angle = math.radians(2)
    # A low afternoon sun from the front left.
    sun.rotation_euler = (math.radians(62), 0, math.radians(-38))
    s.collection.objects.link(sun)
    ground = bpy.data.meshes.new("ground")
    ground.from_pydata([(-400, -400, 0), (400, -400, 0), (400, 400, 0), (-400, 400, 0)], [], [(0, 1, 2, 3)])
    mat = bpy.data.materials.new("ground")
    mat.use_nodes = True
    mat.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.13, 0.2, 0.08, 1)
    mat.node_tree.nodes["Principled BSDF"].inputs["Roughness"].default_value = 1.0
    ground.materials.append(mat)
    g = bpy.data.objects.new("ground", ground)
    g.location.z = -0.005
    s.collection.objects.link(g)


def tree(x, y, h, r):
    """A stand-in broadleaf: a trunk and a clumped crown."""
    bpy.ops.mesh.primitive_cylinder_add(vertices=8, radius=0.3, depth=h * 0.6, location=(x, y, h * 0.3))
    trunk = bpy.context.object
    m = bpy.data.materials.new("bark")
    m.use_nodes = True
    m.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.08, 0.06, 0.04, 1)
    trunk.data.materials.append(m)
    leaf = bpy.data.materials.new("leaf")
    leaf.use_nodes = True
    leaf.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = (0.06, 0.16, 0.04, 1)
    for k, (dx, dy, dz, rr) in enumerate(((0, 0, 0.75, 1.0), (0.5, 0.3, 0.6, 0.7), (-0.5, -0.2, 0.62, 0.75),
                                          (0.1, -0.5, 0.9, 0.6))):
        bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=2, radius=r * rr,
                                              location=(x + dx * r, y + dy * r, h * dz))
        bpy.context.object.data.materials.append(leaf)


def camera(loc, target, lens=35):
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    cam.data.lens = lens
    cam.data.clip_end = 2000
    bpy.context.scene.collection.objects.link(cam)
    cam.location = Vector(loc)
    cam.rotation_euler = (Vector(target) - cam.location).to_track_quat("-Z", "Y").to_euler()
    bpy.context.scene.camera = cam
    return cam


def render(path):
    bpy.context.scene.render.filepath = path
    bpy.ops.render.render(write_still=True)
    print("WROTE", path)


def house(src, out):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene(1600, 1000)
    bpy.ops.import_scene.gltf(filepath=src)
    for x, y, h, r in ((-17, 4, 15, 5.5), (17, 6, 16, 6.0), (-16, 22, 14, 5.0), (17, 24, 15, 5.0),
                       (-24, -6, 12, 4.5), (24, -4, 13, 5.0)):
        tree(x, y, h, r)
    os.makedirs(out, exist_ok=True)
    views = {
        "front": ((0, -34, 4.0), (0, 10, 7.0), 40),
        "three_quarter": ((-26, -24, 9.0), (0, 12, 6.0), 32),
        "stair": ((0, -6, 1.7), (0, 12, 5.0), 24),
        "door": ((-3.5, 3.2, 2.4), (0.6, 12, 4.0), 28),
        "aerial": ((30, -30, 34), (0, 13, 2.0), 30),
        "room_dark_wall": ((0, 12.9, 3.0), (0, 25, 3.2), 20),
        "room_to_door": ((5.5, 24.2, 3.4), (-1.0, 12, 2.8), 18),
        "side": ((34, 16, 4.0), (0, 14, 6.0), 32),
    }
    for name, (loc, target, lens) in views.items():
        camera(loc, target, lens)
        render(os.path.join(out, f"house_{name}.png"))


def kit(src_dir, out):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    names = sorted(f for f in os.listdir(src_dir) if f.endswith(".glb"))
    cols = 6
    rows = (len(names) + cols - 1) // cols
    scene(1800, 300 * rows + 100, sun_energy=3.5)
    cell = 11.0
    for i, f in enumerate(names):
        before = set(bpy.data.objects)
        bpy.ops.import_scene.gltf(filepath=os.path.join(src_dir, f))
        new = [o for o in bpy.data.objects if o not in before]
        pts = [o.matrix_world @ Vector(c) for o in new if o.type == "MESH" for c in o.bound_box]
        lo = Vector([min(p[k] for p in pts) for k in range(3)])
        hi = Vector([max(p[k] for p in pts) for k in range(3)])
        size = max(hi - lo)
        # Each piece fills its cell, so a lamp reads beside a column.
        k = 7.0 / size if size > 0 else 1.0
        if f.startswith("coffer"):
            # A ceiling: turned over to show its coffers from below.
            for o in new:
                if o.parent is None:
                    o.rotation_mode = "XYZ"
                    o.rotation_euler.x += math.pi
            lo, hi = Vector((lo.x, -hi.y, -hi.z)), Vector((hi.x, -lo.y, -lo.z))
        cx, cy = (i % cols) * cell, -(i // cols) * cell
        for o in new:
            if o.parent is None:
                o.scale = o.scale * k
                o.location = Vector((cx, cy, 0)) - Vector(((lo.x + hi.x) / 2, (lo.y + hi.y) / 2, lo.z)) * k
        curve = bpy.data.curves.new(f"label{i}", "FONT")
        curve.body = f[:-4]
        curve.size = 0.7
        curve.align_x = "CENTER"
        label = bpy.data.objects.new(f"label{i}", curve)
        label.location = (cx, cy - 4.6, 0.02)
        bpy.context.scene.collection.objects.link(label)
    w = (cols - 1) * cell
    h = (rows - 1) * cell
    camera((w / 2, -h / 2 - 60, 30), (w / 2, -h / 2 - 1.0, 2.5), 30)
    bpy.context.scene.camera.data.type = "ORTHO"
    bpy.context.scene.camera.data.ortho_scale = cols * cell * 1.02
    render(out)


def main():
    args = sys.argv[sys.argv.index("--") + 1:]
    mode = args[0]
    if mode == "house":
        house(args[1], args[2])
    elif mode == "kit":
        kit(args[1], args[2])
    else:
        raise SystemExit(f"unknown mode {mode}")


main()
