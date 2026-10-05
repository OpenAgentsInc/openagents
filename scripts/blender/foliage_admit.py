"""Admit the foliage models into the Everglade pack's `foliage` set.

Run from the repository root with Python 3:

    python3 scripts/blender/foliage_admit.py

`scripts/blender/foliage.py` writes each model as binary glTF under
`assets/verse/generated/foliage/`, with 16 px stand-ins for the nature kit's
images. This script converts each one to
`assets/verse/everglade/foliage/<name>.gltf` and `<name>.bin` without
touching its geometry or its base-color factors, and points every image at
the nature set's admitted file of the same name (`../nature/<file>.png`), so
the pack carries no new image. Materials that sample a leaf, flower, or fern
image are alpha-masked and double-sided, as the nature kit's are; the rest
are opaque. It then writes the set's `manifest.json`, which records each
file's digest, the digest of the glb it came from, and the conversion.
"""

import hashlib
import json
import os
import struct
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
SOURCES = os.path.join(ROOT, "assets", "verse", "generated", "foliage")
EVERGLADE = os.path.join(ROOT, "assets", "verse", "everglade")
OUT = os.path.join(EVERGLADE, "foliage")
NATURE = os.path.join(EVERGLADE, "nature")

# Admitted nature images that hold cut-out cards.
MASKED = {"Leaves_NormalTree_C", "Leaves", "Flowers", "Leaf_Pine_C"}
CUTOFF = 0.3

LICENSE = b"""Everglade foliage

Models OpenAgents built from primitives with scripts/blender/foliage.py, in
the style of the Stylized Nature MegaKit by Quaternius
(https://quaternius.com), sampling only that kit's admitted images.

License:
CC0 1.0 Universal (CC0 1.0)
Public Domain Dedication
https://creativecommons.org/publicdomain/zero/1.0/
"""


def sha(data):
    return hashlib.sha256(data).hexdigest()


def models():
    sys.path.insert(0, os.path.dirname(__file__))
    # The generator's table, without importing Blender.
    text = open(os.path.join(os.path.dirname(__file__), "foliage.py")).read()
    table = text[text.index("MODELS = {") : text.index("}\n", text.index("MODELS = {"))]
    return [line.split('"')[1] for line in table.splitlines()[1:] if '"' in line]


def read_glb(path):
    data = open(path, "rb").read()
    magic, version, _ = struct.unpack_from("<4sII", data, 0)
    assert magic == b"glTF" and version == 2, path
    offset, doc, blob = 12, None, b""
    while offset < len(data):
        length, kind = struct.unpack_from("<I4s", data, offset)
        chunk = data[offset + 8 : offset + 8 + length]
        if kind == b"JSON":
            doc = json.loads(chunk)
        elif kind == b"BIN\x00":
            blob = chunk
        offset += 8 + length
    return data, doc, blob


def convert(name):
    raw, doc, blob = read_glb(os.path.join(SOURCES, name + ".glb"))
    images = doc.get("images", [])
    image_views = {img["bufferView"] for img in images if "bufferView" in img}
    new_views, index_of, chunks, at = [], {}, [], 0
    for i, view in enumerate(doc["bufferViews"]):
        if i in image_views:
            continue
        start = view.get("byteOffset", 0)
        chunk = blob[start : start + view["byteLength"]]
        pad = (-at) % 4
        chunks.append(b"\0" * pad)
        at += pad
        view = dict(view, byteOffset=at, buffer=0)
        chunks.append(chunk)
        at += len(chunk)
        index_of[i] = len(new_views)
        new_views.append(view)
    bin_bytes = b"".join(chunks)
    bin_bytes += b"\0" * ((-len(bin_bytes)) % 4)
    for accessor in doc.get("accessors", []):
        if "bufferView" in accessor:
            accessor["bufferView"] = index_of[accessor["bufferView"]]
    doc["bufferViews"] = new_views
    doc["buffers"] = [{"uri": f"{name}.bin", "byteLength": len(bin_bytes)}]
    # Each source image by name, as the nature set's admitted file.
    names = [img["name"] for img in images]
    for image in names:
        if not os.path.isfile(os.path.join(NATURE, image + ".png")):
            raise SystemExit(f"{name}: no admitted nature image {image}.png")
    used = sorted(set(names))
    old_textures = doc.get("textures", [])
    doc["images"] = [{"name": n, "uri": f"../nature/{n}.png"} for n in used]
    doc["textures"] = [{"source": i} for i in range(len(used))]
    doc.pop("samplers", None)
    for material in doc.get("materials", []):
        for key in ("extensions", "emissiveTexture", "normalTexture", "occlusionTexture", "extras"):
            material.pop(key, None)
        pbr = material.setdefault("pbrMetallicRoughness", {})
        pbr.pop("metallicRoughnessTexture", None)
        info = pbr.get("baseColorTexture")
        material.pop("alphaMode", None)
        material.pop("alphaCutoff", None)
        if info is None:
            material["doubleSided"] = False
            continue
        image = names[old_textures[info["index"]]["source"]]
        pbr["baseColorTexture"] = {"index": used.index(image)}
        factor = pbr.get("baseColorFactor", [1.0, 1.0, 1.0, 1.0])
        pbr["baseColorFactor"] = [round(min(1.0, c), 4) for c in factor[:3]] + [1.0]
        if image in MASKED:
            material["alphaMode"] = "MASK"
            material["alphaCutoff"] = CUTOFF
            material["doubleSided"] = True
        else:
            material["doubleSided"] = False
    for key in ("extensionsUsed", "extensionsRequired"):
        doc.pop(key, None)
    doc.get("asset", {}).pop("generator", None)
    text = (json.dumps(doc, sort_keys=True, separators=(",", ":")) + "\n").encode()
    open(os.path.join(OUT, f"{name}.gltf"), "wb").write(text)
    open(os.path.join(OUT, f"{name}.bin"), "wb").write(bin_bytes)
    return sha(raw), text, bin_bytes


def main():
    os.makedirs(OUT, exist_ok=True)
    files, originals, transforms = {}, {}, {}
    for name in models():
        glb_digest, text, bin_bytes = convert(name)
        how = (
            f"converted from assets/verse/generated/foliage/{name}.glb by "
            "scripts/blender/foliage_admit.py: separate buffer, stand-in images "
            "replaced by the nature set's admitted images"
        )
        for file, data in [(f"{name}.gltf", text), (f"{name}.bin", bin_bytes)]:
            files[file] = sha(data)
            originals[file] = glb_digest
            transforms[file] = how
        print(f"{name}: {len(text)} + {len(bin_bytes)} bytes")
    open(os.path.join(OUT, "license.txt"), "wb").write(LICENSE)
    files["license.txt"] = originals["license.txt"] = sha(LICENSE)
    manifest = {
        "schema": "openagents.verse.source-manifest.v1",
        "creator": "OpenAgents",
        "license": "CC0-1.0",
        "package": "Everglade foliage (scripts/blender/foliage.py), built from primitives with the images of the Stylized Nature MegaKit Standard",
        "files": dict(sorted(files.items())),
        "originals": dict(sorted(originals.items())),
        "transforms": dict(sorted(transforms.items())),
    }
    with open(os.path.join(OUT, "manifest.json"), "w") as f:
        f.write(json.dumps(manifest, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
