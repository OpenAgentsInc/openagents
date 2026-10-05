"""Write lighter copies of the Medieval Village MegaKit's densest pieces.

Run headless:
    Blender -b --factory-startup --python scripts/blender/kit_lod.py -- [OUT_DIR] [--kit KIT_DIR]

Everglade's city repeats the kit's `Roof_RoundTiles_8x10` on every kit-built
house; at 4,480 triangles it is the largest share of the zone's triangle
budget. This script imports the kit's glTF, thins the piece with Blender's
collapse decimation (the ratio `buildings.py` uses for its roofs), drops
every image but base color, and writes `roof_round_tiles_8x10.glb` to
OUT_DIR (default: `assets/verse/generated/kit`). KIT_DIR defaults to
`~/Downloads/Medieval Village MegaKit[Standard]` (CC0 1.0, Quaternius).
"""

import os
import sys

import bpy

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

PIECES = {"Roof_RoundTiles_8x10": ("roof_round_tiles_8x10", 0.55)}


def main():
    a = kit.args()
    kit_dir = os.path.expanduser("~/Downloads/Medieval Village MegaKit[Standard]")
    if "--kit" in a:
        kit_dir = a[a.index("--kit") + 1]
        a = a[: a.index("--kit")]
    out = a[0] if a else os.path.join(kit.REPO, "assets", "verse", "generated", "kit")
    for piece, (name, ratio) in PIECES.items():
        kit.reset()
        bpy.ops.import_scene.gltf(filepath=os.path.join(kit_dir, "glTF", piece + ".gltf"))
        for o in kit.meshes():
            mod = o.modifiers.new("Decimate", "DECIMATE")
            mod.decimate_type = "COLLAPSE"
            mod.ratio = ratio
            kit.bake(o)
        # Base color only: unlink the normal, roughness, and ORM images.
        for m in bpy.data.materials:
            if not m.use_nodes:
                continue
            tree = m.node_tree
            for node in list(tree.nodes):
                if node.type == "TEX_IMAGE" and node.image and not node.image.name.endswith("BaseColor.png") \
                        and "BaseColor" not in node.image.name:
                    tree.nodes.remove(node)
                elif node.type == "NORMAL_MAP":
                    tree.nodes.remove(node)
        # The admitted copy samples the village set's own images
        # (`everglade_admit.py`), so a thumbnail of each keeps the glb small
        # while still naming its image.
        for image in bpy.data.images:
            if image.size[0] > 64:
                image.scale(64, 64)
                image.pack()
        kit.export(os.path.join(out, name + ".glb"))


main()
