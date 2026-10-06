"""Admit Alice's variants into the character sources the compilers read.

Run from the repository root with Python 3 (NumPy only):

    python3 scripts/blender/character_admit.py

`scripts/blender/alice.py` writes `alice.<variant>.glb` under
`assets/verse/characters/original/alice/`: one skinned mesh on the Universal
rig and its baked base-color atlas. The character compiler reads glTF with a
separate `.bin` buffer and PNG image, so this script writes, beside each glb,
`alice.<variant>.gltf`, `.bin`, and `.png`, and the folder's `manifest.json`
(`openagents.verse.character-sources.v1`) with every file's SHA-256.

The skeleton is not Blender's. Blender's armature import can change a bone's
orientation, which would change the joints' rest rotations, and
`retarget_clip` corrects each key by the rest delta, so it is exact only when
the rest transforms match the Universal rig's. The script therefore writes the
CC0 base file's own joint nodes (`Superhero_Female_FullBody.gltf`) verbatim,
computes the inverse bind matrices from them, maps each vertex's joints to
them by name, and checks that Blender's joints stand where the base file's do
(within 0.1 mm), so the mesh is bound to exactly the rig it was built on.

It checks each variant: its triangle budget, at most 16 primitives and 65,536
vertices a primitive, every vertex with one to four influences that sum to
one after rounding to 1/255, and the joint names: exactly the Universal 65.
"""

import hashlib
import json
import os
import struct
import sys

import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
BASE = os.path.join(ROOT, "assets", "verse", "characters", "quaternius", "base", "Superhero_Female_FullBody.gltf")
DIR = os.path.join(ROOT, "assets", "verse", "characters", "original", "alice")
BUDGETS = {"lod0": 100000, "lod1": 46000, "lod2": 10000, "lod3": 3000}
# Where alice.py writes its glb files: build output, not committed.
BUILD = os.path.join(DIR, "build")
LICENSE = b"""Alice, an original player character for Verse

Made by OpenAgents with scripts/blender/alice.py. Her body, hair, clothing,
and gear are generated from primitives and procedural shading (Reference
mode). Her head, eyes, and brows are those of Superhero_Female_FullBody.gltf
from Quaternius's Universal Base Characters (CC0 1.0, https://quaternius.com),
reshaped by the script, with that file's skin and eye images retinted in the
bake (Compose mode). Epic Games' Valley of the Ancient was studied in
Reference-only mode for general qualities (docs/verse/female-character.md);
none of its content was opened in Blender, traced, or used.

Her skeleton is the same file's Universal rig, whose joint names, parents,
and rest transforms are copied verbatim so the Universal Animation Library
plays on her. See PROVENANCE.md.

License:
CC0 1.0 Universal (CC0 1.0)
Public Domain Dedication
https://creativecommons.org/publicdomain/zero/1.0/
"""

COMPONENTS = {5120: np.int8, 5121: np.uint8, 5122: np.int16, 5123: np.uint16, 5125: np.uint32, 5126: np.float32}
WIDTH = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read_glb(path):
    data = open(path, "rb").read()
    magic, version, _ = struct.unpack_from("<4sII", data, 0)
    assert magic == b"glTF" and version == 2, path
    offset, doc, blob = 12, None, b""
    while offset < len(data):
        length, kind = struct.unpack_from("<I4s", data, offset)
        chunk = data[offset + 8: offset + 8 + length]
        if kind == b"JSON":
            doc = json.loads(chunk)
        elif kind == b"BIN\x00":
            blob = chunk
        offset += 8 + length
    return doc, blob


def accessor(doc, blob, index):
    a = doc["accessors"][index]
    view = doc["bufferViews"][a["bufferView"]]
    dtype = COMPONENTS[a["componentType"]]
    width = WIDTH[a["type"]]
    start = view.get("byteOffset", 0) + a.get("byteOffset", 0)
    stride = view.get("byteStride", 0)
    item = np.dtype(dtype).itemsize * width
    if stride and stride != item:
        rows = [np.frombuffer(blob, dtype, width, start + i * stride) for i in range(a["count"])]
        out = np.array(rows)
    else:
        out = np.frombuffer(blob, dtype, a["count"] * width, start).reshape(a["count"], width)
    if a.get("normalized") and dtype in (np.uint8, np.uint16):
        out = out.astype(np.float32) / np.iinfo(dtype).max
    return out


def quat_matrix(q):
    x, y, z, w = q
    return np.array([
        [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
        [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
        [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
    ])


def local(node):
    m = np.eye(4)
    if "matrix" in node:
        return np.array(node["matrix"], dtype=np.float64).reshape(4, 4).T
    r = quat_matrix(node.get("rotation", [0, 0, 0, 1]))
    s = np.diag(node.get("scale", [1, 1, 1]))
    m[:3, :3] = r @ s
    m[:3, 3] = node.get("translation", [0, 0, 0])
    return m


def globals_by_name(doc):
    parent = {}
    for i, n in enumerate(doc["nodes"]):
        for c in n.get("children", []):
            parent[c] = i
    out = {}

    def world(i):
        m = local(doc["nodes"][i])
        p = parent.get(i)
        return world(p) @ m if p is not None else m

    for i, n in enumerate(doc["nodes"]):
        out[n.get("name", str(i))] = world(i)
    return out


def admit(variant, base):
    glb = os.path.join(BUILD, f"alice.{variant}.glb")
    doc, blob = read_glb(glb)
    assert len(doc["skins"]) == 1, "one skin"
    skin = doc["skins"][0]
    names = [doc["nodes"][j]["name"] for j in skin["joints"]]
    base_skin = base["skins"][0]["joints"]
    base_names = [base["nodes"][j]["name"] for j in base_skin]
    assert sorted(names) == sorted(base_names), "joint names differ from the Universal 65"
    assert len(base_names) == 65
    to_base = np.array([base_names.index(n) for n in names], dtype=np.int64)
    # Blender's joints must stand where the base file's do.
    ours, theirs = globals_by_name(doc), globals_by_name(base)
    drift = max(np.abs(ours[n][:3, 3] - theirs[n][:3, 3]).max() for n in names)
    assert drift < 1e-4, f"{variant}: joints moved {drift} m from the base rig"
    rot_drift = max(np.abs(local(doc["nodes"][skin["joints"][k]]) - local(base["nodes"][base_skin[to_base[k]]]))[:3, :3].max()
                    for k in range(len(names)))
    mesh_nodes = [n for n in doc["nodes"] if "mesh" in n]
    assert len(mesh_nodes) == 1
    mesh_node = mesh_nodes[0]
    assert local(mesh_node).round(9).tolist() == np.eye(4).tolist(), "mesh node is not at the rig's origin"
    prims = doc["meshes"][mesh_node["mesh"]]["primitives"]
    assert 1 <= len(prims) <= 16
    images = doc.get("images", [])
    assert len(images) == 1, "one baked atlas"
    image = images[0]
    view = doc["bufferViews"][image["bufferView"]]
    png = blob[view.get("byteOffset", 0): view.get("byteOffset", 0) + view["byteLength"]]
    assert image["mimeType"] == "image/png" and png[:8] == b"\x89PNG\r\n\x1a\n"
    width, height = struct.unpack(">II", png[16:24])

    out_bin = bytearray()
    views, accessors, primitives = [], [], []

    def put(array, component, kind, target=None, minmax=False):
        nonlocal out_bin
        while len(out_bin) % 4:
            out_bin += b"\x00"
        data = np.ascontiguousarray(array).tobytes()
        v = {"buffer": 0, "byteOffset": len(out_bin), "byteLength": len(data)}
        if target:
            v["target"] = target
        views.append(v)
        out_bin += data
        acc = {"bufferView": len(views) - 1, "componentType": component, "count": int(array.shape[0]), "type": kind}
        if minmax:
            acc["min"] = array.min(axis=0).tolist()
            acc["max"] = array.max(axis=0).tolist()
        accessors.append(acc)
        return len(accessors) - 1

    triangles = 0
    for prim in prims:
        at = prim["attributes"]
        pos = accessor(doc, blob, at["POSITION"]).astype(np.float32)
        nrm = accessor(doc, blob, at["NORMAL"]).astype(np.float32)
        uv = accessor(doc, blob, at["TEXCOORD_0"]).astype(np.float32)
        joints = accessor(doc, blob, at["JOINTS_0"]).astype(np.int64)
        weights = accessor(doc, blob, at["WEIGHTS_0"]).astype(np.float64)
        assert "JOINTS_1" not in at, "more than four influences"
        idx = accessor(doc, blob, prim["indices"]).reshape(-1).astype(np.uint32)
        assert len(pos) <= 65536
        triangles += len(idx) // 3
        # Round to 255ths that sum to exactly one, as the pack stores them.
        weights[weights < 1e-6] = 0
        weights /= weights.sum(axis=1, keepdims=True)
        q = np.floor(weights * 255 + 0.5)
        short = 255 - q.sum(axis=1)
        q[np.arange(len(q)), weights.argmax(axis=1)] += short
        assert (q >= 0).all() and (q.sum(axis=1) == 255).all()
        counts = (q > 0).sum(axis=1)
        assert ((counts >= 1) & (counts <= 4)).all()
        mapped = np.where(q > 0, to_base[joints], 0).astype(np.uint8)
        w = (q / 255).astype(np.float32)
        a = {
            "POSITION": put(pos, 5126, "VEC3", 34962, minmax=True),
            "NORMAL": put(nrm, 5126, "VEC3", 34962),
            "TEXCOORD_0": put(uv, 5126, "VEC2", 34962),
            "JOINTS_0": put(mapped, 5121, "VEC4", 34962),
            "WEIGHTS_0": put(w, 5126, "VEC4", 34962),
        }
        i = put(idx.astype(np.uint16) if len(pos) <= 65535 else idx, 5123 if len(pos) <= 65535 else 5125,
                "SCALAR", 34963)
        primitives.append({"attributes": a, "indices": i, "material": 0, "mode": 4})
    assert triangles <= BUDGETS[variant], f"{variant}: {triangles} triangles"
    # Inverse binds from the base rig's own joints.
    theirs_by_index = [theirs[n] for n in base_names]
    ibm = np.array([np.linalg.inv(m).T.reshape(-1) for m in theirs_by_index], dtype=np.float32)
    ibm_acc = put(ibm, 5126, "MAT4")

    nodes = [dict(n) for n in base["nodes"] if "mesh" not in n and n.get("name") != "Armature"]
    joint_count = len(nodes)
    assert joint_count == 65 and [n["name"] for n in nodes] == [base["nodes"][i]["name"] for i in range(65)]
    root = next(i for i, n in enumerate(base["nodes"]) if n.get("name") == "root")
    nodes.append({"name": "Alice", "mesh": 0, "skin": 0})
    nodes.append({"name": "Armature", "children": [joint_count, root]})
    stem = f"alice.{variant}"
    gltf = {
        "asset": {"version": "2.0", "generator": "OpenAgents scripts/blender/character_admit.py"},
        "scene": 0,
        "scenes": [{"name": "Scene", "nodes": [len(nodes) - 1]}],
        "nodes": nodes,
        "skins": [{"name": "Universal", "joints": base_skin, "inverseBindMatrices": ibm_acc}],
        "meshes": [{"name": "Alice", "primitives": primitives}],
        "materials": [{
            "name": "alice",
            "doubleSided": True,
            "pbrMetallicRoughness": {"baseColorTexture": {"index": 0}, "metallicFactor": 0.0,
                                     "roughnessFactor": 0.85},
        }],
        "textures": [{"source": 0, "sampler": 0}],
        "samplers": [{"magFilter": 9729, "minFilter": 9987, "wrapS": 33071, "wrapT": 33071}],
        "images": [{"name": "Alice_BaseColor", "mimeType": "image/png", "uri": f"{stem}.png"}],
        "accessors": accessors,
        "bufferViews": views,
        "buffers": [{"byteLength": len(out_bin), "uri": f"{stem}.bin"}],
    }
    with open(os.path.join(DIR, f"{stem}.gltf"), "w") as f:
        json.dump(gltf, f, indent=1)
        f.write("\n")
    with open(os.path.join(DIR, f"{stem}.bin"), "wb") as f:
        f.write(bytes(out_bin))
    with open(os.path.join(DIR, f"{stem}.png"), "wb") as f:
        f.write(png)
    return {
        "variant": variant,
        "triangles": triangles,
        "budget": BUDGETS[variant],
        "primitives": len(primitives),
        "image": [width, height],
        "joint_drift_m": float(drift),
        "rest_rotation_delta": float(rot_drift),
        "source": f"build/{stem}.glb",
        "source_sha256": sha(open(glb, "rb").read()),
    }


def main():
    base = json.load(open(BASE))
    variants = [v for v in BUDGETS if os.path.exists(os.path.join(BUILD, f"alice.{v}.glb"))]
    if not variants:
        sys.exit("no alice.<variant>.glb; run scripts/blender/alice.py first")
    report = [admit(v, base) for v in variants]
    with open(os.path.join(DIR, "license.txt"), "wb") as f:
        f.write(LICENSE)
    files = {}
    for name in sorted(os.listdir(DIR)):
        if name.endswith((".gltf", ".bin", ".png")) or name in ("license.txt", "PROVENANCE.md"):
            files[name] = sha(open(os.path.join(DIR, name), "rb").read())
    manifest = {
        "schema": "openagents.verse.character-sources.v1",
        "creator": "OpenAgents",
        "license": "CC0-1.0",
        "package": "Alice (original), on the Universal rig",
        "files": files,
        "variants": report,
    }
    with open(os.path.join(DIR, "manifest.json"), "w") as f:
        json.dump(manifest, f, indent=2)
        f.write("\n")
    for r in report:
        print("ADMITTED", json.dumps(r))


main()
