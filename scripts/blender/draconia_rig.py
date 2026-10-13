"""Draconia on the Universal rig: posing, weights and the glTF skin.

Every Universal clip retargets by joint name, correcting each key by the
rest delta (`retarget_clip`), which is exact only when the rest joint
*orientations* match the Universal rig's; rest *translations* may differ.
So Draconia keeps her own realistic proportions (the CC0 MakeHuman joints)
and borrows only the Universal joints' orientations:

1. `to_rest` turns her limbs, segment by segment from the shoulder and hip
   down, to point the way the Universal rest pose's bones point (arms out,
   legs straight), with the CC0 skeleton's own weights (linear blend, as
   MakeHuman poses).
2. `weights` maps the CC0 skeleton's 163 bones onto the Universal 65 (all
   face bones to `Head`, the spine's five to the Universal three, and so
   on) and keeps the four largest per vertex.
3. `skin_glb` writes the Universal joint nodes with their own rotations and
   scales, translated so each joint stands at her joint, plus Draconia's
   extra joints (hair chains, `hair_<chain>_<link>`, children of `Head`
   then of each other, with identity rest rotations), the inverse binds,
   and JOINTS_0 and WEIGHTS_0 from the per-vertex `_VJ` / `_VW` attributes
   the build exported.

Frames: Blender metres, +Z up, she faces -Y; glTF is (x, z, -y) of that.
"""

import json
import os
import struct

import numpy as np

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
UNIVERSAL = os.path.join(ROOT, "assets", "verse", "characters", "quaternius", "base", "Superhero_Female_FullBody.gltf")

# Universal joint -> the CC0 joint (a `bone____head` or `bone____tail`
# point) it stands at.
JOINT_AT = {
    "pelvis": "root____head",
    "spine_01": "spine04____head",
    "spine_02": "spine03____head",
    "spine_03": "spine02____head",
    "neck_01": "neck01____head",
    "Head": "head____head",
}
for s, S in (("l", "L"), ("r", "R")):
    JOINT_AT.update(
        {
            f"clavicle_{s}": f"clavicle.{S}____head",
            f"upperarm_{s}": f"upperarm01.{S}____head",
            f"lowerarm_{s}": f"lowerarm01.{S}____head",
            f"hand_{s}": f"wrist.{S}____head",
            f"thigh_{s}": f"upperleg01.{S}____head",
            f"calf_{s}": f"lowerleg01.{S}____head",
            f"foot_{s}": f"foot.{S}____head",
            f"ball_{s}": f"toe3-1.{S}____head",
            f"ball_leaf_{s}": f"toe3-1.{S}____tail",
        }
    )
    for u, m in (("thumb", 1), ("index", 2), ("middle", 3), ("ring", 4), ("pinky", 5)):
        JOINT_AT[f"{u}_01_{s}"] = f"finger{m}-1.{S}____head"
        JOINT_AT[f"{u}_02_{s}"] = f"finger{m}-2.{S}____head"
        JOINT_AT[f"{u}_03_{s}"] = f"finger{m}-3.{S}____head"
        JOINT_AT[f"{u}_04_leaf_{s}"] = f"finger{m}-3.{S}____tail"

# The limb segments `to_rest` aligns, parents first: (CC0 bone whose
# subtree turns, from joint, to joint, Universal joint, Universal child).
SEGMENTS = []
for s, S in (("l", "L"), ("r", "R")):
    SEGMENTS += [
        (f"upperarm01.{S}", f"upperarm01.{S}____head", f"lowerarm01.{S}____head", f"upperarm_{s}", f"lowerarm_{s}"),
        (f"lowerarm01.{S}", f"lowerarm01.{S}____head", f"wrist.{S}____head", f"lowerarm_{s}", f"hand_{s}"),
        (f"wrist.{S}", f"wrist.{S}____head", f"finger3-1.{S}____head", f"hand_{s}", f"middle_01_{s}"),
        (f"upperleg01.{S}", f"upperleg01.{S}____head", f"lowerleg01.{S}____head", f"thigh_{s}", f"calf_{s}"),
        (f"lowerleg01.{S}", f"lowerleg01.{S}____head", f"foot.{S}____head", f"calf_{s}", f"foot_{s}"),
    ]


def weight_target(mh_bone):
    """The Universal joint a CC0 bone's weights go to."""
    b = mh_bone
    side = "l" if b.endswith(".L") else "r" if b.endswith(".R") else ""
    stem = b[:-2] if side else b
    if stem in ("root", "spine05", "pelvis"):
        return "pelvis"
    if stem == "spine04":
        return "spine_01"
    if stem == "spine03":
        return "spine_02"
    if stem in ("spine02", "spine01", "breast"):
        return "spine_03"
    if stem.startswith("neck"):
        return "neck_01"
    if stem in ("clavicle", "shoulder01"):
        return f"clavicle_{side}"
    if stem.startswith("upperarm"):
        return f"upperarm_{side}"
    if stem.startswith("lowerarm"):
        return f"lowerarm_{side}"
    if stem in ("wrist",) or stem.startswith("metacarpal"):
        return f"hand_{side}"
    if stem.startswith("finger"):
        m, k = stem[6:].split("-")
        u = {"1": "thumb", "2": "index", "3": "middle", "4": "ring", "5": "pinky"}[m]
        return f"{u}_0{k}_{side}"
    if stem.startswith("upperleg"):
        return f"thigh_{side}"
    if stem.startswith("lowerleg"):
        return f"calf_{side}"
    if stem == "foot":
        return f"foot_{side}"
    if stem.startswith("toe"):
        return f"ball_{side}"
    return "Head"  # the head, jaw, eyes, tongue and every face bone


# ------------------------------------------------------- the Universal rig ---


def _trs(node):
    t = np.array(node.get("translation", [0, 0, 0]), float)
    q = node.get("rotation", [0, 0, 0, 1])
    s = np.array(node.get("scale", [1, 1, 1]), float)
    x, y, z, w = q
    R = np.array(
        [
            [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
            [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
            [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
        ]
    )
    M = np.eye(4)
    M[:3, :3] = R * s
    M[:3, 3] = t
    return M


class Universal:
    """The CC0 Universal rig: joint nodes in skin order, parents, and global
    rest matrices (glTF space, under the file's `Armature` node)."""

    def __init__(self):
        with open(UNIVERSAL) as f:
            self.doc = json.load(f)
        nodes = self.doc["nodes"]
        self.joints = self.doc["skins"][0]["joints"]
        self.names = [nodes[j]["name"] for j in self.joints]
        parent = {}
        for i, n in enumerate(nodes):
            for c in n.get("children", []):
                parent[c] = i
        self.parent_node = parent
        self.globals = {}

        def glob(i):
            if i in self.globals:
                return self.globals[i]
            M = _trs(nodes[i])
            if i in parent:
                M = glob(parent[i]) @ M
            self.globals[i] = M
            return M

        for j in self.joints:
            glob(j)
        self.by_name = {nodes[j]["name"]: j for j in self.joints}

    def global_of(self, name):
        return self.globals[self.by_name[name]]

    def position_blender(self, name):
        p = self.global_of(name)[:3, 3]
        return (p[0], -p[2], p[1])

    def parent_name(self, name):
        p = self.parent_node.get(self.by_name[name])
        nodes = self.doc["nodes"]
        return nodes[p]["name"] if p is not None and p in self.joints else None


# --------------------------------------------------------------- posing ---


def to_rest(base, rig, to_blender, from_blender):
    """Turns her limbs to the Universal rest directions (see the module
    doc). `rig` is draconia.Rig over `base`; positions move in place."""
    from mathutils import Matrix, Vector

    u = Universal()
    for mh_bone, a, b, ua, ub in SEGMENTS:
        pa = to_blender(rig.joint(a))
        pb = to_blender(rig.joint(b))
        cur = (pb - pa).normalized()
        want = (Vector(u.position_blender(ub)) - Vector(u.position_blender(ua))).normalized()
        R = cur.rotation_difference(want).to_matrix()
        w = rig.weight_of(rig.descendants(mh_bone))
        for i, wi in w.items():
            if wi <= 0:
                continue
            v = to_blender(base.co[i])
            r = R @ (v - pa) + pa
            base.co[i] = from_blender(v.lerp(r, min(wi, 1.0)))


def weights(base, rig):
    """Per base vertex, up to four (Universal joint, weight) pairs summing to
    one."""
    acc = [dict() for _ in base.co]
    for bone, pairs in rig.weights.items():
        target = weight_target(bone)
        for i, w in pairs:
            acc[i][target] = acc[i].get(target, 0.0) + w
    out = []
    for d in acc:
        top = sorted(d.items(), key=lambda kv: -kv[1])[:4]
        s = sum(w for _, w in top)
        out.append([(k, w / s) for k, w in top] if s > 0 else [("Head", 1.0)])
    return out


def joint_positions(rig, to_blender):
    """Each Universal joint's position on her (Blender frame)."""
    u = Universal()
    pos = {}
    for name in u.names:
        if name in JOINT_AT:
            pos[name] = to_blender(rig.joint(JOINT_AT[name]))
    pos["root"] = None
    return pos


# ------------------------------------------------------------ the glb skin ---


def _read_glb(path):
    with open(path, "rb") as f:
        data = f.read()
    n = struct.unpack("<I", data[12:16])[0]
    doc = json.loads(data[20 : 20 + n])
    blob = bytearray()
    off = 20 + n
    if off < len(data):
        bn = struct.unpack("<I", data[off : off + 4])[0]
        blob = bytearray(data[off + 8 : off + 8 + bn])
    return doc, blob


def _write_glb(path, doc, blob):
    while len(blob) % 4:
        blob.append(0)
    doc["buffers"] = [{"byteLength": len(blob)}]
    body = json.dumps(doc, separators=(",", ":")).encode()
    body += b" " * ((4 - len(body) % 4) % 4)
    out = struct.pack("<III", 0x46546C67, 2, 12 + 8 + len(body) + 8 + len(blob))
    out += struct.pack("<I", len(body)) + b"JSON" + body
    out += struct.pack("<I", len(blob)) + b"BIN\x00" + bytes(blob)
    with open(path, "wb") as f:
        f.write(out)


def _accessor_array(doc, blob, index):
    a = doc["accessors"][index]
    view = doc["bufferViews"][a["bufferView"]]
    comps = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}[a["type"]]
    dtype = {5126: np.float32, 5123: np.uint16, 5125: np.uint32, 5121: np.uint8}[a["componentType"]]
    start = view.get("byteOffset", 0) + a.get("byteOffset", 0)
    stride = view.get("byteStride", 0)
    count = a["count"]
    item = np.dtype(dtype).itemsize * comps
    if stride and stride != item:
        raw = np.frombuffer(bytes(blob[start : start + stride * count]), np.uint8).reshape(count, stride)[:, :item]
        return np.frombuffer(raw.tobytes(), dtype).reshape(count, comps)
    return np.frombuffer(bytes(blob[start : start + item * count]), dtype).reshape(count, comps)


def _append(doc, blob, array, ctype, atype, target=None, minmax=False):
    while len(blob) % 4:
        blob.append(0)
    data = np.ascontiguousarray(array).tobytes()
    view = {"buffer": 0, "byteOffset": len(blob), "byteLength": len(data)}
    if target:
        view["target"] = target
    blob.extend(data)
    doc["bufferViews"].append(view)
    acc = {"bufferView": len(doc["bufferViews"]) - 1, "componentType": ctype, "count": int(array.shape[0]), "type": atype}
    if minmax:
        acc["min"] = array.min(0).tolist()
        acc["max"] = array.max(0).tolist()
    doc["accessors"].append(acc)
    return len(doc["accessors"]) - 1


def skin_glb(path, joint_pos, extras):
    """Rewrites `path` (a glb whose primitives carry `_VJ` / `_VW`, joint
    indices into `joint_names` and weights) as a skinned glTF on the
    Universal rig plus `extras`: a list of (name, parent, position)."""
    u = Universal()
    doc, blob = _read_glb(path)
    src = u.doc["nodes"]
    names = list(u.names) + [e[0] for e in extras]
    # Global rest matrices on her: Universal rotations and scales, her
    # positions (glTF frame).
    def gl(p):
        return np.array([p[0], p[2], -p[1]], float)

    G = {}
    for name in u.names:
        M = u.global_of(name).copy()
        if joint_pos.get(name) is not None:
            M[:3, 3] = gl(joint_pos[name])
        G[name] = M
    # Leaf joints without a CC0 match keep their Universal offset from
    # their parent, in the parent's frame.
    for name in u.names:
        if joint_pos.get(name) is None and name != "root":
            p = u.parent_name(name)
            if p:
                local = np.linalg.inv(u.global_of(p)) @ u.global_of(name)
                G[name] = G[p] @ local
    for name, parent, pos in extras:
        M = np.eye(4)
        M[:3, :3] = G[parent][:3, :3]
        M[:3, 3] = gl(pos)
        G[name] = M
    parent_of = {n: u.parent_name(n) for n in u.names}
    for name, parent, _ in extras:
        parent_of[name] = parent
    # Nodes: the scene's existing mesh nodes stay; the skeleton is added
    # under an `Armature` node with the Universal root's own transform.
    base_nodes = len(doc["nodes"])
    index = {n: base_nodes + k for k, n in enumerate(names)}
    armature_root = u.doc["nodes"][68] if u.doc["nodes"][68].get("name") == "Armature" else {}
    A = _trs(armature_root) if armature_root else np.eye(4)
    for n in names:
        p = parent_of.get(n)
        Mp = G[p] if p else A
        L = np.linalg.inv(Mp) @ G[n]
        node = {"name": n}
        if n in u.by_name:
            # The Universal joint's own rotation and scale, our translation.
            s = src[u.by_name[n]]
            for k in ("rotation", "scale"):
                if k in s:
                    node[k] = s[k]
            node["translation"] = L[:3, 3].tolist()
        else:
            node["translation"] = L[:3, 3].tolist()
        doc["nodes"].append(node)
    for n in names:
        kids = [index[c] for c in names if parent_of.get(c) == n]
        if kids:
            doc["nodes"][index[n]]["children"] = kids
    mesh_nodes = [i for i, nd in enumerate(doc["nodes"][:base_nodes]) if "mesh" in nd]
    # Bake every mesh node's own transform into its vertices (the skin
    # ignores node transforms).
    arm = {"name": "Armature", "children": [index["root"]] + mesh_nodes}
    if armature_root:
        for k in ("rotation", "translation", "scale"):
            if k in armature_root:
                arm[k] = armature_root[k]
    doc["nodes"].append(arm)
    doc["scenes"] = [{"name": "Scene", "nodes": [len(doc["nodes"]) - 1]}]
    doc["scene"] = 0
    ibm = np.stack([np.linalg.inv(A @ G[n] if False else G[n]).T.reshape(16) for n in names]).astype(np.float32)
    ibm_acc = _append(doc, blob, ibm, 5126, "MAT4")
    doc["skins"] = [{"name": "draconia", "joints": [index[n] for n in names], "inverseBindMatrices": ibm_acc}]
    for i in mesh_nodes:
        doc["nodes"][i]["skin"] = 0
        for k in ("translation", "rotation", "scale"):
            doc["nodes"][i].pop(k, None)
        mesh = doc["meshes"][doc["nodes"][i]["mesh"]]
        for prim in mesh["primitives"]:
            attrs = prim["attributes"]
            vj = _accessor_array(doc, blob, attrs.pop("_VJ"))
            vw = _accessor_array(doc, blob, attrs.pop("_VW"))
            joints = np.clip(np.rint(vj), 0, len(names) - 1).astype(np.uint16)
            w = np.clip(vw, 0, 1).astype(np.float32)
            s = w.sum(1, keepdims=True)
            w = np.where(s > 0, w / np.maximum(s, 1e-9), np.array([[1, 0, 0, 0]], np.float32))
            attrs["JOINTS_0"] = _append(doc, blob, joints, 5123, "VEC4", 34962)
            attrs["WEIGHTS_0"] = _append(doc, blob, w.astype(np.float32), 5126, "VEC4", 34962)
    _write_glb(path, doc, blob)
    return names
