"""Render a labeled contact sheet of admitted Everglade models.

Run headless from the repository root:

    Blender -b --factory-startup --python scripts/blender/foliage_views.py -- OUT_DIR SET/NAME...

Each model is read from its admitted glTF under `assets/verse/everglade/`,
with the nature set's real images, and rendered beside a 1.8 m figure for
scale to `OUT_DIR/<name>.png`. Agents look at every render before placing a
model.
"""

import math
import os
import sys

import bpy
from mathutils import Vector

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

EVERGLADE = os.path.join(kit.REPO, "assets", "verse", "everglade")


def render(model, out):
    kit.reset()
    s, name = model.split("/")
    bpy.ops.import_scene.gltf(filepath=os.path.join(EVERGLADE, s, name + ".gltf"))
    objs = kit.meshes()
    dg = bpy.context.evaluated_depsgraph_get()
    pts = []
    for o in objs:
        e = o.evaluated_get(dg)
        me = e.to_mesh()
        pts += [e.matrix_world @ v.co for v in me.vertices]
        e.to_mesh_clear()
    lo = Vector([min(p[i] for p in pts) for i in range(3)])
    hi = Vector([max(p[i] for p in pts) for i in range(3)])
    # A 1.8 m figure beside the model, for scale.
    figure = kit.mat("Figure", (0.6, 0.2, 0.15))
    kit.cyl("Figure", 0.22, 1.8, (lo.x - 0.6, hi.y, 0.9), figure, verts=8)
    ground = kit.mat("Ground", (0.12, 0.2, 0.06))
    kit.box("Ground", (40, 40, 0.02), (0, 0, -0.01), ground)
    lo.x -= 0.9
    hi.z = max(hi.z, 1.9)
    c = (lo + hi) / 2
    r = max((hi - lo).length, 2.0)
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    bpy.context.scene.collection.objects.link(cam)
    cam.location = c + Vector((r * 0.55, -r * 0.95, r * 0.42))
    cam.rotation_euler = (c - cam.location).to_track_quat("-Z", "Y").to_euler()
    bpy.context.scene.camera = cam
    sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", "SUN"))
    sun.data.energy = 3.5
    sun.rotation_euler = (math.radians(50), 0.2, math.radians(30))
    bpy.context.scene.collection.objects.link(sun)
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_EEVEE"
    scene.render.resolution_x = 480
    scene.render.resolution_y = 400
    scene.world = bpy.data.worlds.new("w")
    scene.world.color = (0.5, 0.6, 0.7)
    scene.render.filepath = out
    bpy.ops.render.render(write_still=True)


def main():
    a = kit.args()
    out_dir, models = a[0], a[1:]
    if not models:
        folder = os.path.join(EVERGLADE, "foliage")
        models = sorted("foliage/" + f[:-5] for f in os.listdir(folder) if f.endswith(".gltf"))
    os.makedirs(out_dir, exist_ok=True)
    for model in models:
        render(model, os.path.join(out_dir, model.split("/")[1] + ".png"))


main()
