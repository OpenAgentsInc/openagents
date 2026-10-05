"""Render a preview PNG of a glTF model, framed on its posed meshes.

Run headless:
    Blender -b --factory-startup --python scripts/blender/preview.py -- IN.glb OUT.png

Agents look at the preview before admitting a generated or converted model.
"""

import sys

import bpy
from mathutils import Vector
src, out = sys.argv[sys.argv.index("--") + 1 :][:2]
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=src)
for o in bpy.data.objects:
    if o.type == 'MESH' and o.name.startswith('Icosphere'):
        o.hide_render = True
objs = [o for o in bpy.data.objects if o.type == 'MESH' and not o.hide_render]
dg = bpy.context.evaluated_depsgraph_get()
pts = []
for o in objs:
    e = o.evaluated_get(dg); m = e.to_mesh()
    pts += [e.matrix_world @ v.co for v in m.vertices]
    e.to_mesh_clear()
lo = Vector([min(p[i] for p in pts) for i in range(3)]); hi = Vector([max(p[i] for p in pts) for i in range(3)])
c = (lo + hi) / 2; r = (hi - lo).length
cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam")); bpy.context.scene.collection.objects.link(cam)
cam.location = c + Vector((r*0.75, -r*0.9, r*0.55))
cam.rotation_euler = (c - cam.location).to_track_quat('-Z', 'Y').to_euler()
bpy.context.scene.camera = cam
sun = bpy.data.objects.new("sun", bpy.data.lights.new("sun", 'SUN')); sun.rotation_euler = (0.8, 0.2, 0.6)
bpy.context.scene.collection.objects.link(sun)
s = bpy.context.scene; s.render.engine = 'BLENDER_EEVEE'; s.render.resolution_x = 640; s.render.resolution_y = 480
s.world = bpy.data.worlds.new("w"); s.world.color = (0.55, 0.6, 0.65)
s.render.filepath = out; bpy.ops.render.render(write_still=True)
