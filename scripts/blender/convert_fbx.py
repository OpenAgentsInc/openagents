"""Convert an FBX model, with its animations, to binary glTF.

Run headless:
    Blender -b --factory-startup --python scripts/blender/convert_fbx.py -- IN.fbx OUT.glb

Some FBX exports (Quaternius's Easy Animated Enemy Pack among them) carry
materials with alpha 0 and hashed blending, which import as invisible; every
material is made opaque. Prints one JSON line starting with CONVERTED.
"""

import json
import sys

import bpy

src, out = sys.argv[sys.argv.index("--") + 1 :][:2]
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.fbx(filepath=src)
for material in bpy.data.materials:
    if material.use_nodes:
        bsdf = material.node_tree.nodes.get("Principled BSDF")
        if bsdf:
            bsdf.inputs["Alpha"].default_value = 1.0
    material.blend_method = "OPAQUE"
bpy.ops.export_scene.gltf(
    filepath=out,
    export_format="GLB",
    export_animations=True,
    export_animation_mode="ACTIONS",
)
print(
    "CONVERTED",
    json.dumps(
        {
            "actions": sorted(a.name.split("|")[-1] for a in bpy.data.actions),
            "meshes": {
                o.name: len(o.data.polygons)
                for o in bpy.data.objects
                if o.type == "MESH"
            },
            "armatures": [o.name for o in bpy.data.objects if o.type == "ARMATURE"],
        }
    ),
)
