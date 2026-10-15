"""Any glTF as a destructible assembly for Verse's meteor physics.

`verse::zones::meteor_rover::Rover::assembly` breaks an assembly apart the
way it breaks the CAD Lab rover: one rigid body per solid, every touching
pair of solids bolted, the bolts failing under a meteor's blast. It reads a
plain binary glTF (one node per solid, one triangle primitive, 32-bit
indices, metres, Y up, the node's `extras.instance` naming the solid) and a
spec like `assets/verse/cad/rover.json`. This script writes both from any
glTF:

- every mesh object becomes a solid, its transform applied, triangulated;
- its material (by name: wood, iron, brass, glass, velvet, leather, bone,
  slate) sets the material and density; its color is its base color,
  averaged over its base color texture when it has one;
- a closed solid (after welding its seams) is weighed from its triangles in
  Verse; an open one (a pane, a trim sheet) gets `open` and a mass from its
  area, as a wall `OPEN_THICKNESS` thick;
- the largest solid is the core, where a meteor aimed at it lands.

Run headless:

    Blender -b --factory-startup --python scripts/blender/destructible_glb.py -- \\
        IN.glb OUT.glb OUT.json [--scale S] [--core NAME]

Check a model's licence before committing what this writes.
"""

import json
import math
import struct
import sys

import bmesh
import bpy
from mathutils import Vector

argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
if len(argv) < 3:
    sys.exit("usage: destructible_glb.py IN.glb OUT.glb OUT.json [--scale S] [--core NAME]")
SRC, OUT_GLB, OUT_SPEC = argv[:3]


def option(name, default):
    return argv[argv.index(name) + 1] if name in argv else default


SCALE = float(option("--scale", "1"))
CORE = option("--core", None)

# (keyword in the material's name, Verse material, density g/cm3)
MATERIALS = [
    ("glass", "Glass", 2.5),
    ("brass", "Brass", 8.5),
    ("gold", "Brass", 8.5),
    ("iron", "Cast Iron", 7.2),
    ("steel", "Steel", 7.85),
    ("velvet", "Fabric", 0.4),
    ("fabric", "Fabric", 0.4),
    ("leather", "Leather", 0.9),
    ("bone", "Bone", 1.9),
    ("skull", "Bone", 1.9),
    ("wood", "Wood", 0.7),
    ("slate", "Wood", 0.7),
]
# An open part's wall thickness, m: its mass is its area times this.
OPEN_THICKNESS = 0.01

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=SRC)


def material_of(obj):
    name = obj.active_material.name.lower() if obj.active_material else ""
    name += " " + obj.name.lower()
    for key, material, density in MATERIALS:
        if key in name:
            return material, density
    return "Wood", 0.7


def linear(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def color_of(obj):
    mat = obj.active_material
    if not mat or not mat.use_nodes:
        return [0.5, 0.5, 0.5]
    bsdf = next((n for n in mat.node_tree.nodes if n.type == "BSDF_PRINCIPLED"), None)
    if bsdf is None:
        return [0.5, 0.5, 0.5]
    base = bsdf.inputs["Base Color"]
    factor = list(base.default_value)[:3]
    if base.is_linked:
        node = base.links[0].from_node
        while node.type != "TEX_IMAGE" and node.inputs and any(i.is_linked for i in node.inputs):
            node = next(i for i in node.inputs if i.is_linked).links[0].from_node
        if node.type == "TEX_IMAGE" and node.image:
            px = node.image.pixels[:]
            n = len(px) // 4
            step = max(1, n // 20000)
            acc = [0.0, 0.0, 0.0]
            count = 0
            for i in range(0, n, step):
                a = px[i * 4 + 3]
                for k in range(3):
                    acc[k] += px[i * 4 + k] * a
                count += a
            if count > 0:
                # An 8-bit image's pixels are its stored (sRGB) values;
                # glTF's base color factor is linear.
                factor = [acc[k] / count for k in range(3)]
                if node.image.colorspace_settings.name == "sRGB":
                    factor = [linear(c) for c in factor]
    return [max(0.0, min(1.0, c)) for c in factor]


solids = []
for obj in bpy.context.scene.objects:
    if obj.type != "MESH" or not obj.data.polygons:
        continue
    bm = bmesh.new()
    bm.from_mesh(obj.data)
    bm.transform(obj.matrix_world)
    # glTF splits vertices along UV and normal seams; weld them to tell a
    # closed solid from an open sheet.
    bmesh.ops.remove_doubles(bm, verts=bm.verts[:], dist=1e-4)
    bmesh.ops.triangulate(bm, faces=bm.faces[:])
    bm.normal_update()
    closed = all(e.is_manifold for e in bm.edges)
    # Blender (Z up) to glTF (Y up), scaled.
    def to_y_up(v):
        return (v.x * SCALE, v.z * SCALE, -v.y * SCALE)

    positions, normals, indices = [], [], []
    for face in bm.faces:
        for loop in face.loops:
            positions.append(to_y_up(loop.vert.co))
            n = face.normal if not face.smooth else loop.vert.normal
            normals.append((n.x, n.z, -n.y))
            indices.append(len(positions) - 1)
    material, density = material_of(obj)
    info = {
        "id": obj.name,
        "part": obj.name,
        "material": material,
        "density_g_cm3": density,
        "parent": None,
    }
    volume = abs(bm.calc_volume()) * SCALE**3 if closed else 0.0
    if not closed or volume < 1e-9:
        area = sum(f.calc_area() for f in bm.faces) * SCALE**2
        info["open"] = True
        info["mass_kg"] = max(0.05, density * 1000.0 * area * OPEN_THICKNESS)
    else:
        info["volume_m3"] = volume
    bm.free()
    solids.append((info, positions, normals, indices, color_of(obj)))
    print(f"solid {obj.name}: {material}, {'closed' if closed else 'open'}, {len(indices) // 3} triangles")

if not solids:
    sys.exit("no mesh objects")


def weight(s):
    info = s[0]
    if "mass_kg" in info:
        return info["mass_kg"]
    return info["volume_m3"] * info["density_g_cm3"] * 1000.0


core = CORE or max(solids, key=weight)[0]["id"]

# The binary glTF.
blob = bytearray()
views, accessors, meshes, nodes, materials = [], [], [], [], []


def add_view(data, target):
    while len(blob) % 4:
        blob.append(0)
    views.append({"buffer": 0, "byteOffset": len(blob), "byteLength": len(data), "target": target})
    blob.extend(data)
    return len(views) - 1


for info, positions, normals, indices, color in solids:
    pos = struct.pack(f"<{len(positions) * 3}f", *[c for p in positions for c in p])
    nor = struct.pack(f"<{len(normals) * 3}f", *[c for n in normals for c in n])
    idx = struct.pack(f"<{len(indices)}I", *indices)
    lo = [min(p[k] for p in positions) for k in range(3)]
    hi = [max(p[k] for p in positions) for k in range(3)]
    accessors.append({"bufferView": add_view(pos, 34962), "componentType": 5126, "count": len(positions), "type": "VEC3", "min": lo, "max": hi})
    accessors.append({"bufferView": add_view(nor, 34962), "componentType": 5126, "count": len(normals), "type": "VEC3"})
    accessors.append({"bufferView": add_view(idx, 34963), "componentType": 5125, "count": len(indices), "type": "SCALAR"})
    materials.append({"name": info["id"], "pbrMetallicRoughness": {"baseColorFactor": color + [1.0], "metallicFactor": 0.0, "roughnessFactor": 0.6}})
    a = len(accessors)
    meshes.append({"primitives": [{"attributes": {"POSITION": a - 3, "NORMAL": a - 2}, "indices": a - 1, "material": len(materials) - 1}]})
    nodes.append({"name": info["id"], "mesh": len(meshes) - 1, "extras": {"instance": info["id"]}})
    info.pop("volume_m3", None)

gltf = {
    "asset": {"version": "2.0", "generator": "scripts/blender/destructible_glb.py"},
    "scene": 0,
    "scenes": [{"nodes": list(range(len(nodes)))}],
    "nodes": nodes,
    "meshes": meshes,
    "materials": materials,
    "accessors": accessors,
    "bufferViews": views,
    "buffers": [{"byteLength": len(blob)}],
}
text = json.dumps(gltf, separators=(",", ":")).encode()
while len(text) % 4:
    text += b" "
while len(blob) % 4:
    blob.append(0)
total = 12 + 8 + len(text) + 8 + len(blob)
with open(OUT_GLB, "wb") as f:
    f.write(struct.pack("<III", 0x46546C67, 2, total))
    f.write(struct.pack("<II", len(text), 0x4E4F534A))
    f.write(text)
    f.write(struct.pack("<II", len(blob), 0x004E4942))
    f.write(blob)
spec = {
    "source": {"tool": "scripts/blender/destructible_glb.py", "from": SRC.split("/")[-1]},
    "core": core,
    "instances": [s[0] for s in solids],
    "joints": [],
    "contacts": [],
}
with open(OUT_SPEC, "w") as f:
    json.dump(spec, f, indent=1)
print(f"wrote {OUT_GLB} and {OUT_SPEC}: {len(solids)} solids, core {core}")
