"""Build a sledgehammer from code and write it as binary glTF.

Run headless:
    Blender -b --factory-startup --python scripts/blender/sledgehammer.py -- OUT.glb

The origin is the handle's butt; the handle runs up +Z, 0.9 m, and the iron
head sits across its top. The proof of the generated-model pipeline in
docs/verse/blender-pipeline.md.
"""

import sys

import bpy

out = sys.argv[sys.argv.index("--") + 1]
bpy.ops.wm.read_factory_settings(use_empty=True)


def material(name, rgb, roughness, metallic=0.0):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    bsdf = m.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (*rgb, 1.0)
    bsdf.inputs["Roughness"].default_value = roughness
    bsdf.inputs["Metallic"].default_value = metallic
    return m


wood = material("Hammer_Wood", (0.36, 0.2, 0.09), 0.8)
grip = material("Hammer_Grip", (0.16, 0.09, 0.05), 0.9)
iron = material("Hammer_Iron", (0.2, 0.21, 0.23), 0.45, 0.8)

bpy.ops.mesh.primitive_cylinder_add(vertices=12, radius=0.022, depth=0.9, location=(0, 0, 0.45))
bpy.context.object.name = "Handle"
bpy.context.object.data.materials.append(wood)
bpy.ops.mesh.primitive_cylinder_add(vertices=12, radius=0.026, depth=0.22, location=(0, 0, 0.13))
bpy.context.object.name = "Grip"
bpy.context.object.data.materials.append(grip)
bpy.ops.mesh.primitive_cube_add(size=1, location=(0, 0, 0.92))
head = bpy.context.object
head.name = "Head"
head.scale = (0.24, 0.09, 0.09)
head.data.materials.append(iron)
bevel = head.modifiers.new("Bevel", "BEVEL")
bevel.width = 0.012
bevel.segments = 3
bpy.ops.object.select_all(action="SELECT")
bpy.ops.export_scene.gltf(filepath=out, export_format="GLB", export_apply=True)
