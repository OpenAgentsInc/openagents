"""Loads Modular Medieval Town pieces from a private export into Blender.

The export is what `scripts/unreal/medieval_town_export.py` writes outside
the repository: one `.glb` per static mesh, whose materials carry names
only, plus `materials.json` and the textures as PNG. This module rebuilds
each material as Verse draws it, base color only: the material's base
color texture, tiled as its parameters say, times its tint. The pack's
layered masters (a base layer blended with a second by a height mask, a
trim with a painted layer) reduce to their base layer; normal, roughness,
and mask textures are dropped, as every Verse pack drops them.

Run headless to render a thumbnail of each piece:

    Blender -b --factory-startup --python scripts/blender/medieval_kit.py -- \
        thumbs EXPORT_DIR OUT_DIR [--match TEXT] [--size PX]

It writes `OUT_DIR/<mesh name>.png` (an orthographic three-quarter view,
textured, on a neutral background) and `OUT_DIR/thumbs.json`. Everything
it writes is licensed content and stays outside the repository.
"""

import json
import math
import os
import sys

import bpy
from mathutils import Vector

sys.path.insert(0, os.path.dirname(__file__))
import kit  # noqa: E402

# Texture parameters that hold a material's base color, most specific first.
BASE_TEXTURES = ["Base_BC", "BC_2", "BC", "Diffuse", "Layer 1_BC", "BC_1", "Base color", "Texture"]
# Vector parameters that tint it.
TINTS = ["Base_tint", "Base BC tint", "Tint", "tint", "BC_tint", "Color main", "color", "texture_color"]
# Scalar parameters that tile the texture.
TILING = ["main tiling", "UV tiling", "Tiling zoom"]


class Export:
    """A private export directory: its materials and textures."""

    def __init__(self, root):
        self.root = root
        with open(os.path.join(root, "materials.json")) as f:
            self.materials = {m["path"].split("/")[-1]: m for m in json.load(f)}
        with open(os.path.join(root, "textures.json")) as f:
            self.textures = {t["asset"]: t for t in json.load(f)}
        with open(os.path.join(root, "meshes.json")) as f:
            self.meshes = json.load(f)
        self._images = {}

    def image(self, asset):
        texture = self.textures.get(asset)
        if not texture or not texture.get("png"):
            return None
        if asset not in self._images:
            self._images[asset] = bpy.data.images.load(os.path.join(self.root, texture["png"]))
        return self._images[asset]

    def base_color(self, name):
        """(image or None, tint RGBA, tiling (u, v), alpha-masked) for the
        material named `name`."""
        record = self.materials.get(name.split(".")[0])
        if record is None:
            return None, (0.6, 0.6, 0.6, 1.0), (1.0, 1.0), False
        master = record["parents"][-1].split(".")[-1] if record["parents"] else ""
        if master.startswith("MM_water"):
            # Water is a scattering shader with no base color; Verse draws
            # still water flat, so take the mean of its two tints.
            a = record["vectors"].get("Absorption_tint", (0.2, 0.3, 0.5, 1.0))
            s = record["vectors"].get("Scattering_tint", (0.3, 0.35, 0.3, 1.0))
            return None, tuple((a[i] + s[i]) / 2 for i in range(3)) + (1.0,), (1.0, 1.0), False
        image = None
        for key in BASE_TEXTURES:
            path = record["textures"].get(key)
            if path and "/Utility/" not in path:
                image = self.image(path)
                if image is not None:
                    break
        tint = (1.0, 1.0, 1.0, 1.0)
        for key in TINTS:
            if key in record["vectors"]:
                tint = tuple(record["vectors"][key])
                break
        if master == "MM_blend_2":
            # The tint colors the painted layer, which covers part of the
            # trim; take half of it.
            tint = tuple(0.5 + 0.5 * c for c in tint[:3]) + (1.0,)
        scale = 1.0
        for key in TILING:
            if key in record["scalars"]:
                scale = record["scalars"][key] or 1.0
                break
        u = scale * record["scalars"].get("UV - X", 1.0)
        v = scale * record["scalars"].get("UV - Y", 1.0)
        masked = "BLEND_MASKED" in record.get("blend_mode", "") or "TRANSLUCENT" in record.get("blend_mode", "")
        return image, tint, (u, v), masked

    def material(self, name):
        """A Blender material drawing `name`'s base color."""
        key = "medieval:" + name
        if key in bpy.data.materials:
            return bpy.data.materials[key]
        image, tint, (u, v), masked = self.base_color(name)
        mat = bpy.data.materials.new(key)
        mat.use_nodes = True
        nodes, links = mat.node_tree.nodes, mat.node_tree.links
        bsdf = nodes["Principled BSDF"]
        bsdf.inputs["Roughness"].default_value = 0.85
        if image is None:
            bsdf.inputs["Base Color"].default_value = tint
            return mat
        tex = nodes.new("ShaderNodeTexImage")
        tex.image = image
        if (u, v) != (1.0, 1.0):
            coord = nodes.new("ShaderNodeUVMap")
            mapping = nodes.new("ShaderNodeMapping")
            mapping.inputs["Scale"].default_value = (u, v, 1.0)
            links.new(coord.outputs["UV"], mapping.inputs["Vector"])
            links.new(mapping.outputs["Vector"], tex.inputs["Vector"])
        mix = nodes.new("ShaderNodeMix")
        mix.data_type = "RGBA"
        mix.blend_type = "MULTIPLY"
        mix.inputs["Factor"].default_value = 1.0
        links.new(tex.outputs["Color"], mix.inputs["A"])
        mix.inputs["B"].default_value = tint
        links.new(mix.outputs["Result"], bsdf.inputs["Base Color"])
        if masked:
            links.new(tex.outputs["Alpha"], bsdf.inputs["Alpha"])
        nodes.active = tex
        return mat

    def glb(self, rel):
        return os.path.join(self.root, "meshes", rel + ".glb")

    def load(self, rel):
        """Imports the piece at `rel` (such as `Meshes/Props/SM_anvil_01`)
        into an empty scene with its materials rebuilt; returns its mesh
        objects, in meters, Z up."""
        # Clear the scene but keep the images and materials already built,
        # which every piece shares.
        for o in list(bpy.data.objects):
            bpy.data.objects.remove(o, do_unlink=True)
        for collection in (bpy.data.meshes, bpy.data.cameras, bpy.data.lights, bpy.data.armatures):
            for block in list(collection):
                if block.users == 0:
                    collection.remove(block)
        bpy.ops.import_scene.gltf(filepath=self.glb(rel))
        objs = [o for o in bpy.data.objects if o.type == "MESH"]
        for o in list(bpy.data.objects):
            if o.type != "MESH":
                bpy.data.objects.remove(o, do_unlink=True)
        for o in objs:
            for slot in o.material_slots:
                if slot.material is not None:
                    slot.material = self.material(slot.material.name)
        return objs


def bounds(objs):
    points = [o.matrix_world @ Vector(c) for o in objs for c in o.bound_box]
    lo = Vector((min(p.x for p in points), min(p.y for p in points), min(p.z for p in points)))
    hi = Vector((max(p.x for p in points), max(p.y for p in points), max(p.z for p in points)))
    return lo, hi


def thumbnail(objs, path, size):
    """An orthographic three-quarter view of `objs`, textured."""
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.display.shading.light = "STUDIO"
    scene.display.shading.color_type = "TEXTURE"
    scene.display.shading.show_cavity = True
    scene.render.resolution_x = scene.render.resolution_y = size
    scene.render.film_transparent = False
    scene.world = scene.world or bpy.data.worlds.new("w")
    lo, hi = bounds(objs)
    center = (lo + hi) / 2
    extent = max((hi - lo).length, 0.05)
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    scene.collection.objects.link(cam)
    cam.data.type = "ORTHO"
    cam.data.ortho_scale = extent * 1.05
    cam.data.clip_end = extent * 10
    direction = Vector((-0.9, -1.2, 0.8)).normalized()
    cam.location = center + direction * extent * 3
    cam.rotation_euler = (center - cam.location).to_track_quat("-Z", "Y").to_euler()
    scene.camera = cam
    scene.render.filepath = path
    bpy.ops.render.render(write_still=True)


def thumbs(export_root, out, match="", size=256):
    export = Export(export_root)
    os.makedirs(out, exist_ok=True)
    made = []
    for m in export.meshes:
        rel = m["path"]
        if match and match not in rel:
            continue
        if not m.get("glb"):
            continue
        name = rel.split("/")[-1]
        try:
            objs = export.load(rel)
            if not objs:
                raise RuntimeError("no mesh")
            thumbnail(objs, os.path.join(out, name + ".png"), size)
            made.append({"path": rel, "thumb": name + ".png"})
        except Exception as error:  # noqa: BLE001 - report and continue
            made.append({"path": rel, "error": str(error)})
            print(f"MEDIEVAL_THUMB failed {rel}: {error}")
    with open(os.path.join(out, "thumbs.json"), "w") as f:
        json.dump(made, f, indent=1)
    print(f"MEDIEVAL_THUMB {sum(1 for m in made if 'thumb' in m)} of {len(made)}")


def main():
    a = kit.args()
    if len(a) >= 3 and a[0] == "thumbs":
        match, size = "", 256
        rest = a[3:]
        while rest:
            if rest[0] == "--match":
                match = rest[1]
            elif rest[0] == "--size":
                size = int(rest[1])
            rest = rest[2:]
        thumbs(a[1], a[2], match, size)
    else:
        sys.exit("usage: medieval_kit.py thumbs EXPORT_DIR OUT_DIR [--match TEXT] [--size PX]")


if __name__ == "__main__":
    main()
