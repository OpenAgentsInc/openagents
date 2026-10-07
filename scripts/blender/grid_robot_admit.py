"""Admit the Grid robot's levels of detail into the character sources.

Run from the repository root with Python 3 (NumPy only):

    python3 scripts/blender/grid_robot_admit.py

`scripts/blender/grid_robot.py` writes `build/grid-robot.<variant>.glb` under
`assets/verse/characters/original/grid-robot/`. The Grid's pack compiler
(`crates/verse/src/grid_pack.rs`) reads glTF with a separate `.bin` buffer,
so this script writes `grid-robot.<variant>.gltf` and `.bin` beside the build
folder, and the folder's `manifest.json`
(`openagents.verse.character-sources.v1`) with every file's SHA-256.

As for Alice (`character_admit.py`), the skeleton is not Blender's: the
script writes the CC0 base file's own joint nodes
(`Superhero_Male_FullBody.gltf`) verbatim, computes the inverse bind
matrices from them, maps each vertex's joint to them by name, and checks
that Blender's joints stand where the base file's do (within 0.1 mm), so
the Universal Animation Library retargets exactly.

It checks each variant: its triangle budget, rigid binding (every vertex on
exactly one joint at full weight), the joint names (exactly the Universal
65), soles on the ground, and the four named materials. The materials carry
only flat factors; the robot has no image.
"""

import hashlib
import json
import os
import struct
import sys

import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
BASE = os.path.join(ROOT, "assets", "verse", "characters", "quaternius", "base", "Superhero_Male_FullBody.gltf")
DIR = os.path.join(ROOT, "assets", "verse", "characters", "original", "grid-robot")
BUILD = os.path.join(DIR, "build")
BUDGETS = {"lod0": 7000, "lod1": 1500}
# Base color and emission per material, linear. The Grid draws these flat;
# the factors keep the robot readable in any other renderer too.
MATERIALS = {
    "Armor": ([0.022, 0.023, 0.026, 1.0], [0.0, 0.0, 0.0]),
    "Trim": ([0.06, 0.062, 0.068, 1.0], [0.0, 0.0, 0.0]),
    "EmitWhite": ([1.0, 1.0, 1.0, 1.0], [1.0, 1.0, 1.0]),
    "EmitPale": ([0.8, 0.84, 0.9, 1.0], [0.8, 0.86, 0.95]),
}
LICENSE = b"""The Grid robot, an original character for Verse

Made by OpenAgents with scripts/blender/grid_robot.py, from primitives
(Reference mode). GarderX's "Low Poly Sci-fi Robot" (Fab, standard license)
was studied for proportions, part breakdown, and design language only:
inspired by, nothing copied. None of its geometry, UVs, or materials were
copied, edited, or exported.

Its skeleton is the Universal rig of Superhero_Male_FullBody.gltf from
Quaternius's Universal Base Characters (CC0 1.0, https://quaternius.com),
whose joint names, parents, and rest transforms are copied verbatim so the
Universal Animation Library plays on it. See PROVENANCE.md.

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
        out = np.array([np.frombuffer(blob, dtype, width, start + i * stride) for i in range(a["count"])])
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
    if "matrix" in node:
        return np.array(node["matrix"], dtype=np.float64).reshape(4, 4).T
    m = np.eye(4)
    m[:3, :3] = quat_matrix(node.get("rotation", [0, 0, 0, 1])) @ np.diag(node.get("scale", [1, 1, 1]))
    m[:3, 3] = node.get("translation", [0, 0, 0])
    return m


def globals_by_name(doc):
    parent = {}
    for i, n in enumerate(doc["nodes"]):
        for c in n.get("children", []):
            parent[c] = i

    def world(i):
        m = local(doc["nodes"][i])
        p = parent.get(i)
        return world(p) @ m if p is not None else m

    return {n.get("name", str(i)): world(i) for i, n in enumerate(doc["nodes"])}


def admit(variant, base):
    glb = os.path.join(BUILD, f"grid-robot.{variant}.glb")
    doc, blob = read_glb(glb)
    assert len(doc["skins"]) == 1, "one skin"
    skin = doc["skins"][0]
    names = [doc["nodes"][j]["name"] for j in skin["joints"]]
    base_skin = base["skins"][0]["joints"]
    base_names = [base["nodes"][j]["name"] for j in base_skin]
    assert len(base_names) == 65 and sorted(names) == sorted(base_names), "joint names differ from the Universal 65"
    to_base = np.array([base_names.index(n) for n in names], dtype=np.int64)
    ours, theirs = globals_by_name(doc), globals_by_name(base)
    drift = max(np.abs(ours[n][:3, 3] - theirs[n][:3, 3]).max() for n in names)
    assert drift < 1e-4, f"{variant}: joints moved {drift} m from the base rig"
    mesh_nodes = [n for n in doc["nodes"] if "mesh" in n]
    assert len(mesh_nodes) == 1
    assert local(mesh_nodes[0]).round(9).tolist() == np.eye(4).tolist(), "mesh node is not at the rig's origin"
    prims = doc["meshes"][mesh_nodes[0]["mesh"]]["primitives"]

    out_bin = bytearray()
    views, accessors, primitives, materials = [], [], [], []

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

    triangles, low = 0, 1e9
    for prim in sorted(prims, key=lambda p: list(MATERIALS).index(doc["materials"][p["material"]]["name"])):
        name = doc["materials"][prim["material"]]["name"]
        assert name in MATERIALS, f"unknown material {name}"
        at = prim["attributes"]
        pos = accessor(doc, blob, at["POSITION"]).astype(np.float32)
        nrm = accessor(doc, blob, at["NORMAL"]).astype(np.float32)
        uv = accessor(doc, blob, at["TEXCOORD_0"]).astype(np.float32)
        joints = accessor(doc, blob, at["JOINTS_0"]).astype(np.int64)
        weights = accessor(doc, blob, at["WEIGHTS_0"]).astype(np.float64)
        assert "JOINTS_1" not in at
        idx = accessor(doc, blob, prim["indices"]).reshape(-1).astype(np.uint32)
        assert len(pos) <= 65535
        triangles += len(idx) // 3
        low = min(low, float(pos[:, 1].min()))
        # Rigid: one joint at full weight.
        main = weights.argmax(axis=1)
        assert np.allclose(weights.max(axis=1), 1.0, atol=1e-4), "a vertex blends two joints"
        bone = to_base[joints[np.arange(len(joints)), main]]
        mapped = np.zeros((len(pos), 4), dtype=np.uint8)
        mapped[:, 0] = bone
        w = np.zeros((len(pos), 4), dtype=np.float32)
        w[:, 0] = 1.0
        materials.append(name)
        a = {
            "POSITION": put(pos, 5126, "VEC3", 34962, minmax=True),
            "NORMAL": put(nrm, 5126, "VEC3", 34962),
            "TEXCOORD_0": put(uv, 5126, "VEC2", 34962),
            "JOINTS_0": put(mapped, 5121, "VEC4", 34962),
            "WEIGHTS_0": put(w, 5126, "VEC4", 34962),
        }
        i = put(idx.astype(np.uint16), 5123, "SCALAR", 34963)
        primitives.append({"attributes": a, "indices": i, "material": len(materials) - 1, "mode": 4})
    assert triangles <= BUDGETS[variant], f"{variant}: {triangles} triangles"
    assert abs(low) < 1e-4, f"{variant}: soles at {low} m"
    ibm = np.array([np.linalg.inv(theirs[n]).T.reshape(-1) for n in base_names], dtype=np.float32)
    ibm_acc = put(ibm, 5126, "MAT4")

    nodes = [dict(n) for n in base["nodes"] if "mesh" not in n and n.get("name") != "Armature"]
    joint_count = len(nodes)
    assert joint_count == 65 and [n["name"] for n in nodes] == [base["nodes"][i]["name"] for i in range(65)]
    root = next(i for i, n in enumerate(base["nodes"]) if n.get("name") == "root")
    nodes.append({"name": "GridRobot", "mesh": 0, "skin": 0})
    nodes.append({"name": "Armature", "children": [joint_count, root]})
    stem = f"grid-robot.{variant}"
    gltf = {
        "asset": {"version": "2.0", "generator": "OpenAgents scripts/blender/grid_robot_admit.py"},
        "scene": 0,
        "scenes": [{"name": "Scene", "nodes": [len(nodes) - 1]}],
        "nodes": nodes,
        "skins": [{"name": "Universal", "joints": base_skin, "inverseBindMatrices": ibm_acc}],
        "meshes": [{"name": "GridRobot", "primitives": primitives}],
        "materials": [{
            "name": m,
            "pbrMetallicRoughness": {"baseColorFactor": MATERIALS[m][0], "metallicFactor": 0.0, "roughnessFactor": 0.6},
            "emissiveFactor": MATERIALS[m][1],
        } for m in materials],
        "accessors": accessors,
        "bufferViews": views,
        "buffers": [{"byteLength": len(out_bin), "uri": f"{stem}.bin"}],
    }
    with open(os.path.join(DIR, f"{stem}.gltf"), "w") as f:
        json.dump(gltf, f, indent=1)
        f.write("\n")
    with open(os.path.join(DIR, f"{stem}.bin"), "wb") as f:
        f.write(bytes(out_bin))
    return {
        "variant": variant,
        "triangles": triangles,
        "budget": BUDGETS[variant],
        "materials": materials,
        "joint_drift_m": float(drift),
        "source": f"build/{stem}.glb",
        "source_sha256": sha(open(glb, "rb").read()),
    }


def main():
    base = json.load(open(BASE))
    missing = [v for v in BUDGETS if not os.path.exists(os.path.join(BUILD, f"grid-robot.{v}.glb"))]
    if missing:
        sys.exit(f"no grid-robot.{missing[0]}.glb; run scripts/blender/grid_robot.py first")
    report = [admit(v, base) for v in BUDGETS]
    with open(os.path.join(DIR, "license.txt"), "wb") as f:
        f.write(LICENSE)
    files = {}
    for name in sorted(os.listdir(DIR)):
        if name.endswith((".gltf", ".bin")) or name in ("license.txt", "PROVENANCE.md"):
            files[name] = sha(open(os.path.join(DIR, name), "rb").read())
    manifest = {
        "schema": "openagents.verse.character-sources.v1",
        "creator": "OpenAgents",
        "license": "CC0-1.0",
        "package": "The Grid robot (original), on the Universal rig",
        "files": files,
        "variants": report,
    }
    with open(os.path.join(DIR, "manifest.json"), "w") as f:
        json.dump(manifest, f, indent=2)
        f.write("\n")
    for r in report:
        print("ADMITTED", json.dumps(r))


main()
