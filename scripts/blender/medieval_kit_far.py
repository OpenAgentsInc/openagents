"""Write private far levels for the medieval kit's heavy street props.

Run headless with CPU geometry operations:
    Blender -b --factory-startup -t 4 --python medieval_kit_far.py -- BUILD RECIPE

The recipe holds IDs and triangle budgets only. Inputs and outputs stay
outside the repository. Materials, texture coordinates, and vertex tints
survive collapse decimation; this script performs no rendering or baking.
"""

import json
import sys
from pathlib import Path

import bpy

REPO = Path(__file__).resolve().parents[2]


def triangles():
    return sum(len(p.vertices) - 2 for o in bpy.context.scene.objects
               if o.type == "MESH" for p in o.data.polygons)


def main():
    args = sys.argv[sys.argv.index("--") + 1:]
    build, recipe = (Path(a).expanduser().resolve() for a in args)
    if build == REPO or REPO in build.parents:
        raise RuntimeError("Licensed far levels must stay outside the repository")
    report = {}
    for piece, options in sorted(json.loads(recipe.read_text())["pieces"].items()):
        budget = options.get("far_triangles")
        if budget is None or not (build / f"{piece}.gltf").is_file():
            continue
        bpy.ops.wm.read_factory_settings(use_empty=True)
        bpy.ops.import_scene.gltf(filepath=str(build / f"{piece}.gltf"))
        before = triangles()
        for obj in list(bpy.context.scene.objects):
            if obj.type != "MESH":
                continue
            bpy.context.view_layer.objects.active = obj
            modifier = obj.modifiers.new("Far level", "DECIMATE")
            modifier.ratio = min(1.0, budget / max(before, 1))
            modifier.use_collapse_triangulate = True
            bpy.ops.object.modifier_apply(modifier=modifier.name)
        after = triangles()
        if after > budget or after == 0:
            raise RuntimeError(f"{piece}: far level has {after} triangles, budget {budget}")
        bpy.ops.export_scene.gltf(
            filepath=str(build / f"{piece}.far.gltf"),
            export_format="GLTF_SEPARATE", export_yup=True,
            export_normals=True, export_tangents=False,
            export_vertex_color="ACTIVE", export_materials="EXPORT",
            export_animations=False, export_cameras=False, export_lights=False,
        )
        report[piece] = {"near_triangles": before, "far_triangles": after,
                         "budget": budget}
    print(json.dumps({"far_levels": report}, sort_keys=True))


main()
