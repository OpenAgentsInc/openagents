"""Build private whole-house levels from the Rust acceptance exports.

Run Blender headless with CPU geometry operations. The source glTF carries
the already graded kit images and linear vertex tints from the real renderer.
Middle and far share a base-color atlas; no illumination is baked here.
The optional near model uses raw kit pieces, so their images receive the
compiler's grade once. No licensed input or output may enter a Git checkout.

    Blender -b --factory-startup -t 4 --python-exit-code 1 \
      --python medieval_house_levels.py -- EXPORT BUILD [--batch N]
"""

import argparse
import json
import math
import struct
import sys
import zlib
from pathlib import Path

import bpy
import numpy as np
from mathutils import Matrix


def private(path):
    path = path.expanduser().resolve()
    if any((p / ".git").exists() for p in [path, *path.parents]):
        raise RuntimeError("Licensed house levels must stay outside Git")
    return path


def triangles(obj):
    return sum(len(p.vertices) - 2 for p in obj.data.polygons)


def join():
    objects = [o for o in bpy.context.scene.objects if o.type == "MESH"]
    if not objects:
        raise RuntimeError("The private house has no geometry")
    bpy.ops.object.select_all(action="DESELECT")
    for obj in objects:
        obj.select_set(True)
    bpy.context.view_layer.objects.active = objects[0]
    bpy.ops.object.join()
    obj = bpy.context.object
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)
    return obj


def merge_materials(obj):
    slots = list(obj.data.materials)
    unique, indices, mapping = [], {}, []
    for material in slots:
        nodes = material.node_tree.nodes
        bsdf = next(n for n in nodes if n.type == "BSDF_PRINCIPLED")
        images = tuple(sorted(str(Path(n.image.filepath).resolve()) for n in nodes
                              if n.type == "TEX_IMAGE" and n.image))
        key = (images, tuple(bsdf.inputs["Base Color"].default_value),
               material.surface_render_method, material.use_backface_culling)
        if key not in indices:
            indices[key] = len(unique)
            unique.append(material)
        mapping.append(indices[key])
    face_indices = [mapping[p.material_index] for p in obj.data.polygons]
    obj.data.materials.clear()
    for material in unique:
        obj.data.materials.append(material)
    for face, index in zip(obj.data.polygons, face_indices):
        face.material_index = index


def reduced(source, budget):
    obj = source.copy()
    obj.data = source.data.copy()
    bpy.context.collection.objects.link(obj)
    bpy.context.view_layer.objects.active = obj
    low = np.array([min(v.co[a] for v in obj.data.vertices) for a in range(3)])
    high = np.array([max(v.co[a] for v in obj.data.vertices) for a in range(3)])
    fixed = obj.vertex_groups.new(name="House silhouette")
    fixed.add(list(range(len(obj.data.vertices))), 1.0, "REPLACE")
    fixed.remove([v.index for v in obj.data.vertices if any(
        abs(v.co[a] - low[a]) < 1e-6 or abs(v.co[a] - high[a]) < 1e-6
        for a in range(3))])
    ratio = min(1.0, budget / triangles(obj))
    original = obj.data.copy()
    for _ in range(12):
        obj.data = original.copy()
        modifier = obj.modifiers.new("House level", "DECIMATE")
        modifier.ratio = ratio
        modifier.use_collapse_triangulate = True
        modifier.vertex_group = fixed.name
        modifier.vertex_group_factor = 1.0
        bpy.ops.object.modifier_apply(modifier=modifier.name)
        if triangles(obj) <= budget:
            break
        ratio *= 0.92
    if not 0 < triangles(obj) <= budget:
        raise RuntimeError(f"House reduction has {triangles(obj)} triangles, budget {budget}")
    for v in obj.data.vertices:
        for axis in range(3):
            v.co[axis] = min(high[axis], max(low[axis], v.co[axis]))
    modifier = obj.modifiers.new("Triangles", "TRIANGULATE")
    bpy.ops.object.modifier_apply(modifier=modifier.name)
    return obj


def texture(material):
    nodes = material.node_tree.nodes
    bsdf = next(n for n in nodes if n.type == "BSDF_PRINCIPLED")
    factor = np.array(bsdf.inputs["Base Color"].default_value)
    image = next((n.image for n in nodes if n.type == "TEX_IMAGE" and n.image), None)
    pixels = None
    if image:
        pixels = np.empty(image.size[0] * image.size[1] * 4, dtype=np.float32)
        image.pixels.foreach_get(pixels)
        pixels = pixels.reshape(image.size[1], image.size[0], 4)
        # The glTF material factor is one when the texture/color nodes link
        # to the socket. Vertex colors are sampled separately below.
        factor = np.ones(4)
    return pixels, factor


def corners(obj):
    mesh = obj.data
    uv = mesh.uv_layers.active
    color = mesh.color_attributes.active_color
    materials = [texture(m) for m in mesh.materials]
    result = []
    for face in mesh.polygons:
        ids = list(face.loop_indices)
        if len(ids) != 3:
            raise RuntimeError("A house level is not triangulated")
        positions = np.array([mesh.vertices[mesh.loops[i].vertex_index].co[:] for i in ids])
        normal = np.array(face.normal[:])
        uvs = np.array([uv.data[i].uv[:] for i in ids])
        colors = np.array([color.data[i if color.domain == "CORNER" else mesh.loops[i].vertex_index].color[:]
                           if color else [1.0] * 4 for i in ids])
        result.append((positions, normal, uvs, colors, materials[face.material_index]))
    return result


def png(path, linear):
    rgb = np.maximum(0.0, linear[:, :, :3])
    srgb = np.where(rgb <= 0.0031308, rgb * 12.92, 1.055 * rgb ** (1 / 2.4) - 0.055)
    rgba = np.concatenate([srgb, np.ones((*rgb.shape[:2], 1))], axis=2)
    rgba = np.rint(np.clip(rgba, 0, 1) * 255).astype(np.uint8)
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    rows = b"".join(b"\0" + row.tobytes() for row in rgba[::-1])
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 512, 512, 8, 6, 0, 0, 0))
                     + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))


def atlas_triangles(all_triangles, atlas, half):
    count = sum(len(t) for t in all_triangles)
    columns = math.ceil(math.sqrt(count * 2))
    rows = math.ceil(count / columns)
    cell = np.array([512 / columns, 256 / rows])
    output = []
    sequence = 0
    for level in all_triangles:
        vertices = []
        for positions, normal, uv, color, (image, factor) in level:
            offset = np.array([(sequence % columns) * cell[0], half * 256 + (sequence // columns) * cell[1]])
            sequence += 1
            points = offset + np.array([[0.7, 0.7], [cell[0] - 0.7, 0.7], [0.7, cell[1] - 0.7]])
            if min(cell) <= 2:
                raise RuntimeError("The house atlas has too few texels per triangle")
            # Fill the entire rectangular gutter by clamping barycentric
            # weights to the triangle. Linear filtering cannot read black
            # or a neighboring material at a triangle edge.
            x0, y0 = np.floor(offset).astype(int)
            x1, y1 = np.ceil(offset + cell).astype(int)
            ys, xs = np.mgrid[y0:min(y1, 512), x0:min(x1, 512)]
            b = np.maximum(0, (xs + 0.5 - points[0, 0]) / (points[1, 0] - points[0, 0]))
            c = np.maximum(0, (ys + 0.5 - points[0, 1]) / (points[2, 1] - points[0, 1]))
            a = np.maximum(0, 1 - b - c)
            weights = np.stack([a, b, c], axis=-1)
            weights /= weights.sum(axis=-1, keepdims=True)
            source_uv = weights @ uv
            tint = (weights @ color) * factor
            if image is not None:
                # Blender's imported glTF UVs and image pixels use bottom
                # origin. Repeat wrapping matches the runtime sampler.
                sample_x = np.floor((source_uv[..., 0] % 1) * image.shape[1]).astype(int)
                sample_y = np.floor((source_uv[..., 1] % 1) * image.shape[0]).astype(int)
                tint *= image[sample_y, sample_x]
            atlas[y0:min(y1, 512), x0:min(x1, 512)] = tint
            for p, target in zip(positions, points):
                # Blender Z-up -> the renderer's glTF Y-up.
                vertices.append(([p[0], p[2], -p[1]], [normal[0], normal[2], -normal[1]],
                                 [target[0] / 512, 1 - target[1] / 512]))
        output.append(vertices)
    return output


def write_model(build, name, image, vertices):
    blob = bytearray()
    views, accessors = [], []
    def add(data, count, width, component, target):
        while len(blob) % 4:
            blob.append(0)
        views.append({"buffer": 0, "byteOffset": len(blob), "byteLength": len(data), "target": target})
        blob.extend(data)
        accessors.append({"bufferView": len(views)-1, "componentType": component, "count": count, "type": width})
        return len(accessors)-1
    p = np.array([v[0] for v in vertices], dtype="<f4")
    pos = add(p.tobytes(), len(p), "VEC3", 5126, 34962)
    accessors[pos].update(min=p.min(axis=0).tolist(), max=p.max(axis=0).tolist())
    norm = add(np.array([v[1] for v in vertices], dtype="<f4").tobytes(), len(p), "VEC3", 5126, 34962)
    uv = add(np.array([v[2] for v in vertices], dtype="<f4").tobytes(), len(p), "VEC2", 5126, 34962)
    index = add(np.arange(len(p), dtype="<u4").tobytes(), len(p), "SCALAR", 5125, 34963)
    doc = {"asset": {"version": "2.0"}, "scene": 0, "scenes": [{"nodes": [0]}], "nodes": [{"mesh": 0}],
           "meshes": [{"primitives": [{"attributes": {"POSITION": pos, "NORMAL": norm, "TEXCOORD_0": uv},
                                        "indices": index, "material": 0}]}],
           "materials": [{"name": "HouseAtlas", "doubleSided": True,
                          "pbrMetallicRoughness": {"baseColorTexture": {"index": 0}, "baseColorFactor": [1]*4,
                                                   "metallicFactor": 0, "roughnessFactor": 1}}],
           "images": [{"uri": image}], "textures": [{"source": 0}], "accessors": accessors,
           "bufferViews": views, "buffers": [{"uri": name + ".bin", "byteLength": len(blob)}]}
    (build / (name + ".bin")).write_bytes(blob)
    (build / (name + ".gltf")).write_text(json.dumps(doc, sort_keys=True))


def near_model(build, house):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    # Coordinate change conjugates the full Rust matrix, including the
    # terrain baseline. Mesh data stays in Blender's Z-up frame.
    change = Matrix(((1, 0, 0, 0), (0, 0, -1, 0), (0, 1, 0, 0), (0, 0, 0, 1)))
    for p in house["pieces"]:
        before = set(bpy.context.scene.objects)
        bpy.ops.import_scene.gltf(filepath=str(build / (p["model"] + ".gltf")))
        values = p["matrix"]
        matrix = Matrix([values[i::4] for i in range(4)])
        for obj in set(bpy.context.scene.objects) - before:
            obj.matrix_world = change @ matrix @ change.inverted() @ obj.matrix_world
    obj = join()
    merge_materials(obj)
    obj = reduced(obj, house["triangles"][0])
    if len(obj.data.materials)>house["draws"][0]:
        raise RuntimeError("The near house exceeds its material-submission budget")
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.export_scene.gltf(filepath=str(build / (house["key"] + "-near.gltf")),
        export_format="GLTF_SEPARATE", use_selection=True, export_yup=True,
        export_normals=True, export_tangents=False, export_vertex_color="ACTIVE",
        export_animations=False, export_cameras=False, export_lights=False)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("export", type=Path)
    parser.add_argument("build", type=Path)
    parser.add_argument("--batch", type=int)
    args = parser.parse_args(sys.argv[sys.argv.index("--")+1:])
    export, build = private(args.export), private(args.build)
    document = json.loads((export / "houses.json").read_text())
    if document["schema"] != "openagents.verse.private-house-build.v1":
        raise RuntimeError("The Rust house manifest has an unknown schema")
    houses = sorted(document["houses"], key=lambda h: h["key"])
    for batch in range(math.ceil(len(houses)/2)):
        if args.batch is not None and batch != args.batch:
            continue
        atlas = np.ones((512, 512, 4), dtype=np.float32)
        image = f"house-atlas-{batch:02}.png"
        for half, house in enumerate(houses[batch*2:batch*2+2]):
            bpy.ops.wm.read_factory_settings(use_empty=True)
            bpy.ops.import_scene.gltf(filepath=str(export / house["source"]))
            original = join()
            levels = [reduced(original, budget) for budget in house["triangles"][1:]]
            selected = atlas_triangles([corners(obj) for obj in levels], atlas, half)
            for label, vertices in zip(["middle", "far"], selected):
                write_model(build, house["key"] + "-" + label, image, vertices)
            if house["near"]:
                near_model(build, house)
            print(json.dumps({"house": house["key"], "middle": len(selected[0])//3,
                              "far": len(selected[1])//3, "atlas": image}), flush=True)
        png(build / image, atlas)


main()
