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


def generated_levels(source, out):
    """Make two levels from original generated inputs without reading a kit."""
    import hashlib
    import json
    import bmesh
    from pathlib import Path

    source, out = Path(source), Path(out)
    for path in sorted(source.glob('*/*.glb')):
        if path.parent.name in ('wildlife', 'lod'):
            continue
        record = json.loads(path.with_suffix('.source.json').read_text())
        if record.get('mode') != 'Reference' or record.get('external_inputs') != []:
            raise ValueError(f'{path}: original generated input required')
        for level, ratio in [(1, 0.55), (2, 0.25)]:
            kit.reset()
            bpy.ops.import_scene.gltf(filepath=str(path))
            for obj in kit.meshes():
                # glTF splits vertices at face normals and UV seams. Weld
                # those copies before collapse so closed surfaces stay closed.
                bm = bmesh.new()
                bm.from_mesh(obj.data)
                bmesh.ops.remove_doubles(bm, verts=list(bm.verts), dist=0.00001)
                bmesh.ops.recalc_face_normals(bm, faces=list(bm.faces))
                was_closed = all(edge.is_manifold for edge in bm.edges)
                bm.to_mesh(obj.data)
                bm.free()
                if len(obj.data.polygons) <= 12:
                    continue
                mod = obj.modifiers.new('Coast level', 'DECIMATE')
                mod.ratio = ratio
                mod.use_collapse_triangulate = True
                kit.bake(obj)
                if was_closed:
                    bm = bmesh.new()
                    bm.from_mesh(obj.data)
                    closed = all(edge.is_manifold for edge in bm.edges)
                    bm.free()
                    if not closed:
                        raise ValueError(f'{path}: reduction opened a closed surface')
            name = f'{path.parent.name}.{path.stem}.lod{level}'
            target = out / (name + '.glb')
            info = kit.export(str(target), extra={
                'mode': 'Reference', 'external_inputs': [],
                'triangle_budget': record['triangle_budget'],
                'generated_from': record['sha256'], 'level': level,
            })
            if info['triangles'] > record['triangles']:
                raise ValueError(f'{name}: a lighter level gained triangles')
            info['out'] = 'lod/' + name + '.glb'
            info['sha256'] = hashlib.sha256(target.read_bytes()).hexdigest()
            target.with_suffix('.source.json').write_text(json.dumps(info, indent=2)+'\n')


def main():
    a = kit.args()
    if a and a[0] == '--generated':
        if len(a) != 3:
            raise ValueError('Usage: --generated SOURCE_DIRECTORY OUTPUT_DIRECTORY')
        generated_levels(a[1], a[2])
        return
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


if __name__ == '__main__':
    main()
