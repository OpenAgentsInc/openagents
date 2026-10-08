"""Export one original demo actor for private Verse merge and grade captures.

Run with Blender's CPU Python runtime, followed by REPO EXPORT OUTPUT ACTOR.
The output preserves the supplied component transforms and material overrides.
It uses the kit compiler's base-color, UV, tint, and alpha conversion. No
licensed inputs, geometry, images, or output belong in a Git checkout.
"""

import hashlib
import json
import shutil
import struct
import sys
from pathlib import Path

from mathutils import Matrix, Quaternion, Vector


def private(path):
    path = Path(path).expanduser().resolve()
    if any((p / ".git").exists() for p in (path, *path.parents)):
        raise RuntimeError("Private comparison inputs and outputs must stay outside Git")
    return path


def main():
    repo, root, output, actor = sys.argv[sys.argv.index("--") + 1:]
    repo, root, output = Path(repo), private(root), private(output)
    sys.path.insert(0, str(repo / "scripts/unreal"))
    from medieval_kit_build import Export, Glb, Writer, f32, load_primitives

    source = Export(root)
    map_path = root / "maps/Maps__medieval_town.json"
    document = json.loads(map_path.read_text())
    original = next(a for a in document["actors"] if a["actor"] == actor)
    parts = [p for p in document["placed"]
             if p["actor"] == actor and p.get("visible", True)]
    basis = Matrix(((1, 0, 0, 0), (0, 0, 1, 0),
                    (0, 1, 0, 0), (0, 0, 0, 1)))

    def transform(part):
        x, y, z, w = part["rotation_quat"]
        return basis @ Matrix.LocRotScale(
            Vector([v / 100 for v in part["location_cm"]]),
            Quaternion((w, x, y, z)), Vector(part["scale"])) @ basis.inverted()

    origin = transform(original).inverted()
    output.mkdir(parents=True, exist_ok=True)
    writer, meshes, nodes, materials, images, textures = Writer(), [], [], [], [], []
    keys, image_keys, identities = {}, {}, []
    for part in parts:
        name = part["mesh"].split("/Game/Medieval_Town/")[-1].split(".")[0]
        glb_path = root / "meshes" / (name + ".glb")
        primitives = []
        for slot, (identity, positions, normals, uvs, indices) in enumerate(
                load_primitives(Glb(glb_path), part["component"])):
            overrides = part.get("override_materials", [])
            if slot < len(overrides) and overrides[slot]:
                identity = overrides[slot].split("/")[-1].split(".")[0]
            image, tint, (tu, tv), blend, two_sided = source.base_color(identity)
            alpha = ("BLEND" if "TRANSLUCENT" in blend or "ADDITIVE" in blend
                     or "glass" in identity.lower() else
                     "MASK" if "MASKED" in blend else "OPAQUE")
            factor = [1.0] * 4 if image else [min(max(c, 0), 1) for c in tint[:3]] + [1.0]
            key = (image, alpha, two_sided, tuple(factor))
            if key not in keys:
                material = {"name": f"demo-material-{len(materials)}",
                            "alphaMode": alpha, "doubleSided": two_sided,
                            "pbrMetallicRoughness": {"baseColorFactor": factor,
                                                     "metallicFactor": 0.0,
                                                     "roughnessFactor": 1.0}}
                if alpha == "MASK":
                    material["alphaCutoff"] = 0.5
                if image:
                    if image not in image_keys:
                        original_png = root / source.textures[image]["png"]
                        filename = f"demo-image-{len(images)}.png"
                        shutil.copyfile(original_png, output / filename)
                        image_keys[image] = len(images)
                        images.append({"uri": filename})
                        textures.append({"source": len(images) - 1})
                    material["pbrMetallicRoughness"]["baseColorTexture"] = {
                        "index": image_keys[image]}
                keys[key] = len(materials)
                materials.append(material)
            count = len(positions)
            attributes = {}
            for attribute, values, kind, length in (
                    ("POSITION", positions, "VEC3", 3),
                    ("NORMAL", normals, "VEC3", 3),
                    ("TEXCOORD_0", [(u * tu, v * tv) for u, v in uvs], "VEC2", 2)):
                attributes[attribute] = writer.add(
                    struct.pack(f"<{count * length}f", *[f32(c) for v in values for c in v]),
                    {"componentType": 5126, "count": count, "type": kind}, 34962)
            positions = [tuple(f32(c) for c in p) for p in positions]
            writer.accessors[attributes["POSITION"]].update(
                min=[min(p[a] for p in positions) for a in range(3)],
                max=[max(p[a] for p in positions) for a in range(3)])
            color = bytes([round(min(max(c, 0), 1) * 255) for c in tint[:3]] + [255])
            if not image:
                color = bytes([255] * 4)
            attributes["COLOR_0"] = writer.add(
                color * count, {"componentType": 5121, "count": count,
                                "type": "VEC4", "normalized": True}, 34962)
            index = writer.add(struct.pack(f"<{len(indices)}I", *indices),
                               {"componentType": 5125, "count": len(indices),
                                "type": "SCALAR"}, 34963)
            primitives.append({"attributes": attributes, "indices": index,
                               "material": keys[key]})
            identities.append({"component": part["component"], "material": identity,
                               "source_glb_sha256": hashlib.sha256(glb_path.read_bytes()).hexdigest()})
        matrix = origin @ transform(part)
        nodes.append({"name": part["component"], "mesh": len(meshes),
                      "matrix": [matrix[r][c] for c in range(4) for r in range(4)]})
        meshes.append({"primitives": primitives})
    assert len(nodes) == len(parts) > 0
    doc = {"asset": {"version": "2.0", "generator": "OpenAgents exported-demo comparison"},
           "scene": 0, "scenes": [{"nodes": list(range(len(nodes)))}],
           "nodes": nodes, "meshes": meshes, "materials": materials,
           "images": images, "textures": textures, "bufferViews": writer.views,
           "accessors": writer.accessors,
           "buffers": [{"uri": "demo.bin", "byteLength": len(writer.bin)}]}
    (output / "demo.bin").write_bytes(writer.bin)
    (output / "demo.gltf").write_text(json.dumps(doc))
    report = {"label": "exported Unreal demo input", "actor": original,
              "map_sha256": hashlib.sha256(map_path.read_bytes()).hexdigest(),
              "components": len(parts), "materials": len(materials),
              "images": len(images), "identities": identities,
              "scope": "Original demo actor; Verse base-color conversion, not an Unreal-engine screenshot"}
    (output / "input.json").write_text(json.dumps(report, indent=2))
    print(json.dumps({k: v for k, v in report.items() if k not in ("actor", "identities")}))


if __name__ == "__main__":
    main()
