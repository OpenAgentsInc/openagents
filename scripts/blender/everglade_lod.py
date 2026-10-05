"""Build the far levels of detail for Everglade's heaviest models.

Run headless from the repository root:

    Blender -b --factory-startup --python scripts/blender/everglade_lod.py [-- NAME...]

Everglade draws a model's far level of detail instead of the model itself
beyond a switch distance (`zones::everglade::detail`). This script reads each
model in `RECIPES` from its admitted glTF under `assets/verse/everglade/` and
writes a lighter copy to `assets/verse/everglade/lod/<set>.<name>.gltf` and
`.bin`, which the pack compiler admits as the model `lod/<set>.<name>`:

- `building` and `piece`: welds the model's parts, drops loose parts too small
  to see from the switch distance (nails, hinges, latches, and thin trim),
  dissolves nearly flat faces within each texture island, collapses the
  rest to the recipe's share of the source triangles, and shades by angle.
- `tree`: collapses the bark, and thins the leaf cards: it keeps the
  recipe's share of the cards, chosen by a fixed seed, and grows each one
  about its center so the canopy keeps its cover.

The exported glTF carries no images. Each material is replaced by the
source model's material of the same name, with its image named as a sibling
set's admitted file (`../<set>/<file>.png`), so the pack keeps one copy of
every image and the far model matches the near one's color. The script
writes the set's `manifest.json` with each file's digest, the digest of the
source glTF it came from, and the recipe. Pass model names (`village/...`)
to rebuild only those; the manifest still lists every file present.
"""

import hashlib
import json
import math
import os
import random
import sys

import bmesh
import bpy
from mathutils import Vector

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

EVERGLADE = os.path.join(kit.REPO, "assets", "verse", "everglade")
OUT = os.path.join(EVERGLADE, "lod")

# Model: (kind, share of the source's triangles, smallest part kept in m).
BUILDING = 0.15
RECIPES = {
    # Generated buildings and landmarks.
    **{
        f"generated/{name}": ("building", BUILDING, 0.35)
        for name in [
            "townhouse_jettied",
            "townhouse_balcony",
            "row_townhouse",
            "library",
            "tavern",
            "market_hall",
            "corner_shop",
            "l_house",
            "cottage_tower",
            "music_hall",
            "meeting_hall",
            "guild_hall",
            "bakery",
            "boardwalk_cafe",
            "smithy",
            "clock_tower",
            "farmhouse",
            "cottage_thatch",
            "hip_house",
            "windmill",
            "boathouse",
            "observatory",
            "bandshell",
            "fountain",
            "market_stall_red",
            "market_stall_blue",
            "market_stall_green",
            "market_stall_gold",
        ]
    },
    "generated/roof_round_tiles_8x10": ("piece", 0.12, 0.2),
    # The village kit's pieces that kit-built houses repeat.
    **{
        f"village/{name}": ("piece", share, 0.2)
        for name, share in [
            ("Wall_Plaster_WoodGrid", 0.3),
            ("Window_Wide_Flat1", 0.25),
            ("Window_Wide_Round1", 0.2),
            ("Roof_Front_Brick8", 0.25),
            ("Roof_RoundTiles_8x10", 0.08),
            ("Wall_Plaster_Straight", 0.35),
            ("Wall_Plaster_Window_Wide_Flat", 0.4),
            ("Wall_Plaster_Window_Wide_Round", 0.35),
            ("Wall_Plaster_Door_Round", 0.35),
            ("Wall_Plaster_Straight_Base", 0.4),
            ("DoorFrame_Round_WoodDark", 0.2),
            ("Door_4_Round", 0.15),
            ("Prop_Chimney", 0.2),
            ("Prop_Wagon", 0.2),
            ("Prop_MetalFence_Simple", 0.15),
            ("Prop_MetalFence_Ornament", 0.15),
        ]
    },
    # The nature kit's trees and bushes: bark share, leaf cards kept.
    **{
        f"nature/{name}": ("tree", 0.45, 0.5)
        for name in [
            "Pine_1",
            "Pine_2",
            "CommonTree_1",
            "CommonTree_3",
            "CommonTree_4",
            "CommonTree_5",
            "Bush_Common",
            "Bush_Common_Flowers",
        ]
    },
}
SEED = 10_650
LICENSE = b"""Everglade far levels of detail

Lighter copies of models admitted into the Everglade pack, made by
scripts/blender/everglade_lod.py: pieces of the Medieval Village MegaKit and
trees of the Stylized Nature MegaKit by Quaternius (https://quaternius.com),
and models OpenAgents generated from them with scripts/blender.

License:
CC0 1.0 Universal (CC0 1.0)
Public Domain Dedication
https://creativecommons.org/publicdomain/zero/1.0/
"""


def sha(data):
    return hashlib.sha256(data).hexdigest()


def source_path(model):
    s, name = model.split("/")
    return os.path.join(EVERGLADE, s, name + ".gltf")


def load(model):
    """Imports the admitted model with its transforms applied; returns its meshes."""
    kit.reset()
    bpy.ops.import_scene.gltf(filepath=source_path(model))
    objs = kit.meshes()
    bpy.ops.object.select_all(action="DESELECT")
    for o in objs:
        o.select_set(True)
    bpy.context.view_layer.objects.active = objs[0]
    bpy.ops.object.parent_clear(type="CLEAR_KEEP_TRANSFORM")
    for o in list(bpy.data.objects):
        if o.type != "MESH":
            bpy.data.objects.remove(o)
    for o in objs:
        # Instanced kit pieces share a mesh; each copy is baked on its own.
        if o.data.users > 1:
            o.data = o.data.copy()
        kit.bake(o)
    return objs


def triangles(objs):
    total = 0
    for o in objs:
        o.data.calc_loop_triangles()
        total += len(o.data.loop_triangles)
    return total


def islands(bm, faces):
    """The connected groups of `faces`, by shared vertices."""
    left, groups = set(faces), []
    while left:
        seed = left.pop()
        group, stack = [seed], [seed]
        while stack:
            f = stack.pop()
            for v in f.verts:
                for g in v.link_faces:
                    if g in left:
                        left.remove(g)
                        group.append(g)
                        stack.append(g)
        groups.append(group)
    return groups


def extent(faces):
    pts = [v.co for f in faces for v in f.verts]
    lo = Vector([min(p[i] for p in pts) for i in range(3)])
    hi = Vector([max(p[i] for p in pts) for i in range(3)])
    return (hi - lo).length, (lo + hi) * 0.5


def clear_normals(o):
    bpy.ops.object.select_all(action="DESELECT")
    o.select_set(True)
    bpy.context.view_layer.objects.active = o
    if o.data.has_custom_normals:
        bpy.ops.mesh.customdata_custom_splitnormals_clear()


def shade(o, angle):
    bpy.ops.object.select_all(action="DESELECT")
    o.select_set(True)
    bpy.context.view_layer.objects.active = o
    bpy.ops.object.shade_smooth_by_angle(angle=math.radians(angle))


def collapse(o, target):
    """Collapses `o` toward `target` triangles."""
    now = triangles([o])
    if now <= target or now == 0:
        return
    mod = o.modifiers.new("Decimate", "DECIMATE")
    mod.decimate_type = "COLLAPSE"
    mod.ratio = max(0.01, target / now)
    mod.use_collapse_triangulate = True
    kit.apply_modifiers(o)


def simplify(objs, share, smallest):
    """The `building` and `piece` recipe."""
    source = triangles(objs)
    o = kit.join("lod", objs) if len(objs) > 1 else objs[0]
    clear_normals(o)
    bm = bmesh.new()
    bm.from_mesh(o.data)
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-4)
    small = [f for group in islands(bm, list(bm.faces)) if extent(group)[0] < smallest for f in group]
    bmesh.ops.delete(bm, geom=small, context="FACES")
    bmesh.ops.dissolve_limit(
        bm,
        angle_limit=math.radians(4.0),
        use_dissolve_boundaries=False,
        verts=bm.verts,
        edges=bm.edges,
        delimit={"UV", "MATERIAL"},
    )
    bmesh.ops.triangulate(bm, faces=bm.faces)
    bm.to_mesh(o.data)
    bm.free()
    collapse(o, int(source * share))
    shade(o, 40.0)
    return source


def thin(objs, share, keep):
    """The `tree` recipe: collapsed bark, fewer and larger leaf cards."""
    source = triangles(objs)
    o = kit.join("lod", objs) if len(objs) > 1 else objs[0]
    bpy.ops.object.select_all(action="DESELECT")
    o.select_set(True)
    bpy.context.view_layer.objects.active = o
    bpy.ops.mesh.separate(type="MATERIAL")
    rng = random.Random(SEED)
    for part in kit.meshes():
        names = [m.name for m in part.data.materials if m]
        if any(n.startswith("Bark") for n in names):
            clear_normals(part)
            bm = bmesh.new()
            bm.from_mesh(part.data)
            twigs = [f for g in islands(bm, list(bm.faces)) if extent(g)[0] < 0.6 for f in g]
            bmesh.ops.delete(bm, geom=twigs, context="FACES")
            bm.to_mesh(part.data)
            bm.free()
            collapse(part, int(triangles([part]) * share))
            shade(part, 60.0)
            continue
        # Leaf and flower cards keep their own normals; only whole cards go.
        bm = bmesh.new()
        bm.from_mesh(part.data)
        cards = islands(bm, list(bm.faces))
        gone, grow = [], 1.0 / math.sqrt(keep)
        for card in cards:
            if rng.random() >= keep:
                gone.extend(card)
                continue
            _, center = extent(card)
            for v in {v for f in card for v in f.verts}:
                v.co = center + (v.co - center) * grow
        bmesh.ops.delete(bm, geom=gone, context="FACES")
        bm.to_mesh(part.data)
        bm.free()
    return source


def export(model, source_doc):
    name = model.replace("/", ".")
    path = os.path.join(OUT, name + ".gltf")
    bpy.ops.object.select_all(action="SELECT")
    has_colors = any(
        "COLOR_0" in p["attributes"] for m in source_doc["meshes"] for p in m["primitives"]
    )
    bpy.ops.export_scene.gltf(
        filepath=path,
        export_format="GLTF_SEPARATE",
        export_image_format="NONE",
        export_apply=True,
        export_yup=True,
        export_animations=False,
        export_texcoords=True,
        export_normals=True,
        export_vertex_color="ACTIVE" if has_colors else "NONE",
        export_all_vertex_colors=False,
    )
    doc = json.load(open(path))
    # The source's materials by name, with images named from this set.
    s = model.split("/")[0]
    by_name = {m.get("name"): m for m in source_doc.get("materials", [])}
    images, textures = [], []

    def image_index(uri):
        uri = uri if uri.startswith("../") else f"../{s}/{uri}"
        if uri not in images:
            images.append(uri)
        return images.index(uri)

    materials = []
    for m in doc.get("materials", []):
        base = m.get("name", "")
        src = by_name.get(base) or by_name.get(base.rsplit(".", 1)[0])
        if src is None:
            raise SystemExit(f"{model}: no source material named {base}")
        src = json.loads(json.dumps(src))
        src.pop("extensions", None)
        for key in ("normalTexture", "occlusionTexture", "emissiveTexture"):
            src.pop(key, None)
        pbr = src.setdefault("pbrMetallicRoughness", {})
        pbr.pop("metallicRoughnessTexture", None)
        info = pbr.get("baseColorTexture")
        if info is not None:
            texture = source_doc["textures"][info["index"]]
            uri = source_doc["images"][texture["source"]]["uri"]
            index = image_index(uri)
            pbr["baseColorTexture"] = {"index": index}
        materials.append(src)
    doc["materials"] = materials
    doc["images"] = [{"name": os.path.basename(u)[:-4], "uri": u} for u in images]
    doc["textures"] = [{"source": i} for i in range(len(images))]
    for key in ("samplers", "extensionsUsed", "extensionsRequired"):
        doc.pop(key, None)
    if not doc["images"]:
        doc.pop("images")
        doc.pop("textures")
    doc.get("asset", {}).pop("generator", None)
    text = (json.dumps(doc, sort_keys=True, separators=(",", ":")) + "\n").encode()
    open(path, "wb").write(text)
    return name


def build(model):
    kind, share, extra = RECIPES[model]
    source_doc = json.load(open(source_path(model)))
    objs = load(model)
    if kind == "tree":
        source = thin(objs, share, extra)
    else:
        source = simplify(objs, share, extra)
    after = triangles(kit.meshes())
    name = export(model, source_doc)
    print("LOD", json.dumps({"model": model, "source": source, "triangles": after}))
    return name


def write_manifest():
    files, originals, transforms = {}, {}, {}
    license_text = LICENSE
    open(os.path.join(OUT, "license.txt"), "wb").write(license_text)
    files["license.txt"] = originals["license.txt"] = sha(license_text)
    for model, (kind, share, extra) in sorted(RECIPES.items()):
        name = model.replace("/", ".")
        if not os.path.isfile(source_path(model)):
            continue
        source = open(source_path(model), "rb").read()
        for ext in (".gltf", ".bin"):
            path = os.path.join(OUT, name + ext)
            if not os.path.isfile(path):
                continue
            data = open(path, "rb").read()
            files[name + ext] = sha(data)
            originals[name + ext] = sha(source)
            how = (
                f"far level of detail of {model} by scripts/blender/everglade_lod.py: "
                + (
                    f"bark collapsed to {share:.0%}, {extra:.0%} of the leaf cards kept and grown"
                    if kind == "tree"
                    else f"parts under {extra} m dropped, flat faces dissolved, collapsed to {share:.0%}"
                )
            )
            transforms[name + ext] = how
    manifest = {
        "schema": "openagents.verse.source-manifest.v1",
        "creator": "OpenAgents",
        "license": "CC0-1.0",
        "package": "Everglade far levels of detail (scripts/blender/everglade_lod.py), from models of or built with the Medieval Village and Stylized Nature MegaKit Standard",
        "files": dict(sorted(files.items())),
        "originals": dict(sorted(originals.items())),
        "transforms": dict(sorted(transforms.items())),
    }
    with open(os.path.join(OUT, "manifest.json"), "w") as f:
        f.write(json.dumps(manifest, indent=2) + "\n")


def main():
    os.makedirs(OUT, exist_ok=True)
    wanted = kit.args() or sorted(RECIPES)
    for model in wanted:
        if not os.path.isfile(source_path(model)):
            print("SKIP", model)
            continue
        build(model)
    write_manifest()


main()
