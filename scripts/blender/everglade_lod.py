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
  rebuilds each slope of bumpy round tiles as one closed slab with the
  tiles' material and image coordinates, dissolves nearly flat faces within
  each texture island, and shades by angle. A `building` then collapses
  toward the recipe's share of the source triangles while every vertex on
  an open edge stays put, so no wall or roof tears open; a `piece` keeps
  its shape.
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
PIECE = 0.5
RECIPES = {
    # Generated buildings and landmarks.
    **{
        f"generated/{name}": ("building", BUILDING, 0.2)
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
            "market_stall_gold",
        ]
    },
    "generated/roof_round_tiles_8x10": ("piece", PIECE, 0.6),
    # The village kit's pieces that kit-built houses repeat: tiles as
    # slabs, the rest collapsed toward half, holding every open edge. The
    # plain plaster walls have none: at under 140 triangles, a far level
    # would save a frame little and cost the merged geometry as much.
    **{
        f"village/{name}": ("piece", PIECE, 0.2)
        for name in [
            "Wall_Plaster_WoodGrid",
            "Window_Wide_Flat1",
            "Window_Wide_Round1",
            "Roof_Front_Brick8",
            "Roof_RoundTiles_8x10",
            "DoorFrame_Round_WoodDark",
            "Door_4_Round",
            "Prop_Chimney",
            "Prop_Wagon",
            "Prop_MetalFence_Simple",
            "Prop_MetalFence_Ornament",
        ]
    },
    # The nature kit's trees and bushes: bark share, leaf cards kept.
    **{
        f"nature/{name}": ("tree", 0.45, 0.5)
        for name in [
            "Pine_1",
            "Pine_2",
            "CommonTree_3",
            "CommonTree_4",
            "CommonTree_5",
            "Bush_Common",
            "Bush_Common_Flowers",
        ]
    },
    # The foliage set's trees (`foliage.py`), whose dark crown cores count
    # as bark: collapsed rather than thinned.
    **{
        f"foliage/{name}": ("tree", 0.45, 0.4)
        for name in [
            "oak_forked",
            "beech_tall",
            "linden_broad",
            "willow_weeping",
            "oak_old",
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


def is_tiles(name):
    """Whether a material is a roof's round tiles."""
    return "Tiles" in name


def smoothed_normals(faces, reach=1.4):
    """Each face's area-weighted normal over the faces within `reach` m of
    it, so a slope of bumpy round tiles reads as one plane."""
    grid = {}
    for f in faces:
        c = f.calc_center_median()
        grid.setdefault(tuple(int(math.floor(x / reach)) for x in c), []).append(f)
    out = {}
    for f in faces:
        c = f.calc_center_median()
        k = tuple(int(math.floor(x / reach)) for x in c)
        n = Vector()
        for dx in (-1, 0, 1):
            for dy in (-1, 0, 1):
                for dz in (-1, 0, 1):
                    for g in grid.get((k[0] + dx, k[1] + dy, k[2] + dz), ()):
                        if (g.calc_center_median() - c).length <= reach:
                            n += g.normal * g.calc_area()
        out[f] = n.normalized() if n.length > 1e-9 else f.normal.copy()
    return out


def slopes(faces, normals, reach=0.8, angle=30.0):
    """Regions of tile faces whose centers lie within `reach` m of each
    other and whose smoothed normals agree within `angle` degrees."""
    grid = {}
    centers = {f: f.calc_center_median() for f in faces}
    for f in faces:
        grid.setdefault(tuple(int(math.floor(x / reach)) for x in centers[f]), []).append(f)
    limit = math.cos(math.radians(angle))
    left, regions = set(faces), []
    while left:
        seed = left.pop()
        region, stack = [seed], [seed]
        while stack:
            f = stack.pop()
            c = centers[f]
            k = tuple(int(math.floor(x / reach)) for x in c)
            for dx in (-1, 0, 1):
                for dy in (-1, 0, 1):
                    for dz in (-1, 0, 1):
                        for g in grid.get((k[0] + dx, k[1] + dy, k[2] + dz), ()):
                            if (
                                g in left
                                and (centers[g] - c).length <= reach
                                and normals[g].dot(normals[seed]) >= limit
                            ):
                                left.remove(g)
                                region.append(g)
                                stack.append(g)
        regions.append(region)
    return regions


def uv_fit(bm, faces, uv_layer, basis, origin):
    """The affine map from a slope's plane coordinates to its tiles' UVs,
    fitted over every corner, or None when the tiles don't share one map."""
    import numpy as np

    rows, us, vs = [], [], []
    u_axis, v_axis = basis
    for f in faces:
        for loop in f.loops:
            d = loop.vert.co - origin
            rows.append((d.dot(u_axis), d.dot(v_axis), 1.0))
            uv = loop[uv_layer].uv
            us.append(uv.x)
            vs.append(uv.y)
    if len(rows) < 6:
        return None
    a = np.array(rows)
    cu, ru, *_ = np.linalg.lstsq(a, np.array(us), rcond=None)
    cv, rv, *_ = np.linalg.lstsq(a, np.array(vs), rcond=None)
    err = math.sqrt(((a @ cu - us) ** 2).mean() + ((a @ cv - vs) ** 2).mean())
    # A fit that misses by more than a tile's width means each tile has
    # its own copy of the image.
    return (cu, cv) if err < 0.08 else None


def slabs(bm, uv_layer, materials):
    """Replaces every roof's round tiles with closed slabs: one convex hull
    per slope of tiles, its facets merged into planes, with the tiles'
    material and their UV map, so a far roof has no holes and keeps its
    color and pattern."""
    faces = [f for f in bm.faces if is_tiles(materials[f.material_index])]
    if not faces:
        return 0
    normals = smoothed_normals(faces)
    made, replaced = 0, []
    for region in slopes(faces, normals):
        verts = {v for f in region for v in f.verts}
        # A slope of a few large faces, such as a generated cone roof, is
        # already simple; only the kit's bumpy tile rows become slabs.
        if len(region) < 12:
            continue
        n = Vector()
        for f in region:
            n += normals[f] * f.calc_area()
        n = n.normalized() if n.length > 1e-9 else Vector((0, 0, 1))
        u_axis = n.cross(Vector((0, 0, 1)))
        if u_axis.length < 1e-3:
            u_axis = Vector((1, 0, 0))
        u_axis.normalize()
        v_axis = n.cross(u_axis).normalized()
        origin = sum((v.co for v in verts), Vector()) / len(verts)
        fit = uv_fit(bm, region, uv_layer, (u_axis, v_axis), origin)
        if fit is None:
            # Each tile carries its own copy: keep the image's scale, about
            # one repeat per 2 m.
            fit = ((1 / 2.0, 0.0, 0.5), (0.0, 1 / 2.0, 0.5))
        material = region[0].material_index
        temp = bmesh.new()
        for v in verts:
            temp.verts.new(v.co)
        bmesh.ops.convex_hull(temp, input=temp.verts)
        # The hull's many facets over the tiles' bumps merge into a few
        # planes.
        bmesh.ops.dissolve_limit(
            temp, angle_limit=math.radians(10.0), verts=temp.verts, edges=temp.edges
        )
        new_faces = []
        for f in temp.faces:
            corners = [bm.verts.new(v.co) for v in f.verts]
            try:
                nf = bm.faces.new(corners)
            except ValueError:
                continue
            nf.material_index = material
            nf.smooth = False
            for loop in nf.loops:
                d = loop.vert.co - origin
                pu, pv = d.dot(u_axis), d.dot(v_axis)
                cu, cv = fit
                loop[uv_layer].uv = (cu[0] * pu + cu[1] * pv + cu[2], cv[0] * pu + cv[1] * pv + cv[2])
            new_faces.append(nf)
        temp.free()
        if new_faces:
            made += len(new_faces)
            replaced.extend(region)
    bmesh.ops.delete(bm, geom=replaced, context="FACES")
    return made


def seams(bm, uv_layer):
    """The vertices on an open edge or where the material or the image
    coordinates change: the ones a collapse would pull apart."""
    keep = set()
    for e in bm.edges:
        faces = e.link_faces
        if len(faces) != 2:
            keep.update(e.verts)
    return keep


def collapse_inside(o, target, keep):
    """Collapses `o` toward `target` triangles, holding the vertices in
    `keep` (by index) so pieces don't part at their seams."""
    now = triangles([o])
    if now <= target or now == 0:
        return
    group = o.vertex_groups.new(name="keep")
    group.add(sorted(keep), 1.0, "REPLACE")
    mod = o.modifiers.new("Decimate", "DECIMATE")
    mod.decimate_type = "COLLAPSE"
    mod.ratio = max(0.01, target / now)
    mod.use_collapse_triangulate = True
    mod.vertex_group = "keep"
    mod.invert_vertex_group = True
    mod.vertex_group_factor = 1000.0
    kit.apply_modifiers(o)
    if o.vertex_groups.get("keep"):
        o.vertex_groups.remove(o.vertex_groups["keep"])


def simplify(objs, share, smallest):
    """The `building` and `piece` recipe: faithful and watertight. Round
    tiles become closed slabs; small loose parts go; nearly flat faces
    dissolve within each texture island; the rest collapses toward `share`
    of the source's triangles, holding every seam, so no wall or roof
    tears open."""
    source = triangles(objs)
    o = kit.join("lod", objs) if len(objs) > 1 else objs[0]
    clear_normals(o)
    materials = [m.name if m else "" for m in o.data.materials]
    bm = bmesh.new()
    bm.from_mesh(o.data)
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-4)
    uv_layer = bm.loops.layers.uv.active
    tiles = {f for f in bm.faces if is_tiles(materials[f.material_index])}
    small = [
        f
        for group in islands(bm, [f for f in bm.faces if f not in tiles])
        if extent(group)[0] < smallest
        for f in group
    ]
    bmesh.ops.delete(bm, geom=small, context="FACES")
    if uv_layer is not None:
        slabs(bm, uv_layer, materials)
    bmesh.ops.dissolve_limit(
        bm,
        angle_limit=math.radians(4.0),
        use_dissolve_boundaries=False,
        verts=bm.verts,
        edges=bm.edges,
        delimit={"UV", "MATERIAL"},
    )
    bmesh.ops.triangulate(bm, faces=bm.faces)
    bm.verts.index_update()
    keep = {v.index for v in seams(bm, bm.loops.layers.uv.active)}
    # Thatch is a few stacked slabs: collapsing them folds the roof.
    keep |= {
        v.index for f in bm.faces if "Thatch" in materials[f.material_index] for v in f.verts
    }
    bm.to_mesh(o.data)
    bm.free()
    if share < 1.0:
        collapse_inside(o, int(source * share), keep)
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
        if any(n.startswith("Core") for n in names):
            # A crown's dark cores are already a few faces each.
            continue
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
                    else f"parts under {extra} m dropped, round tiles rebuilt as closed slabs, flat faces dissolved"
                    + (f", collapsed toward {share:.0%} holding open edges" if share < 1.0 else "")
                )
            )
            transforms[name + ext] = how
    # Keep the far levels other scripts admit into the set
    # (`everglade_admit.py` for `town_houses.py`'s), while their files are
    # there.
    path = os.path.join(OUT, "manifest.json")
    if os.path.isfile(path):
        before = json.load(open(path))
        for file, digest in before["files"].items():
            if file not in files and os.path.isfile(os.path.join(OUT, file)):
                files[file] = digest
                originals[file] = before["originals"][file]
                transforms[file] = before["transforms"][file]
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
