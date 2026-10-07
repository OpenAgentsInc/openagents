"""Find coplanar faces that overlap in a glTF model: the z-fighting check.

Two faces that face the same way in one plane, or within `EPS` of it,
draw at the same depth, so the renderer picks between them pixel by pixel
and the surface flickers as the camera moves. A box set on another box
whose top reaches the same height is the usual cause: the lower box's
top and the upper box's top coincide where they overlap.

Run from the repository root with Python 3 and NumPy:

    python3 scripts/blender/coplanar.py MODEL.glb|MODEL.gltf ...

It prints each overlapping plane and exits 1 when it finds one. It
counts two faces of different materials: two faces of one material shade
alike, so they don't show a flicker, and count only with `--same`. Faces
in one plane that face opposite ways count only with `--opposite`: an
opaque material culls back faces, so they don't fight. `greco_futurism.py` runs
the check on every model and kit piece it saves, and `greco_admit.py`
refuses a model that fails it.
"""

import json
import os
import struct
import sys

import numpy as np

# How far apart two parallel faces may be and still fight, m.
EPS = 0.0015
# The smallest shared area that counts as an overlap, m^2.
MIN_AREA = 1e-4

COMPONENTS = {5120: np.int8, 5121: np.uint8, 5122: np.int16, 5123: np.uint16, 5125: np.uint32, 5126: np.float32}
WIDTH = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT4": 16}


def load(path):
    """The glTF document and its binary buffer, from a .glb or a .gltf."""
    data = open(path, "rb").read()
    if data[:4] == b"glTF":
        length = struct.unpack_from("<I", data, 12)[0]
        doc = json.loads(data[20:20 + length])
        rest = 20 + length
        blob = data[rest + 8:rest + 8 + struct.unpack_from("<I", data, rest)[0]] if rest < len(data) else b""
        return doc, blob
    doc = json.loads(data)
    uri = doc["buffers"][0]["uri"]
    return doc, open(os.path.join(os.path.dirname(path), uri), "rb").read()


def accessor(doc, blob, i):
    a = doc["accessors"][i]
    view = doc["bufferViews"][a["bufferView"]]
    dtype = np.dtype(COMPONENTS[a["componentType"]])
    width = WIDTH[a["type"]]
    start = view.get("byteOffset", 0) + a.get("byteOffset", 0)
    stride = view.get("byteStride", dtype.itemsize * width)
    rows = np.lib.stride_tricks.as_strided(
        np.frombuffer(blob, dtype=np.uint8, offset=start),
        shape=(a["count"], dtype.itemsize * width), strides=(stride, 1))
    return np.ascontiguousarray(rows).view(dtype).reshape(a["count"], width)


def node_matrix(node):
    if "matrix" in node:
        return np.array(node["matrix"], dtype=np.float64).reshape(4, 4).T
    t = np.array(node.get("translation", [0, 0, 0]), dtype=np.float64)
    x, y, z, w = node.get("rotation", [0, 0, 0, 1])
    r = np.array([
        [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
        [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
        [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
    ])
    m = np.eye(4)
    m[:3, :3] = r * np.array(node.get("scale", [1, 1, 1]))
    m[:3, 3] = t
    return m


def triangles(path):
    """Every triangle of the model's scene in model space: corners, and
    each one's material name."""
    doc, blob = load(path)
    tris, mats = [], []

    def visit(i, parent):
        node = doc["nodes"][i]
        m = parent @ node_matrix(node)
        if "mesh" in node:
            for prim in doc["meshes"][node["mesh"]]["primitives"]:
                if prim.get("mode", 4) != 4:
                    continue
                p = accessor(doc, blob, prim["attributes"]["POSITION"]).astype(np.float64)
                p = np.einsum("ij,kj->ki", m[:3, :3], p) + m[:3, 3]
                idx = (accessor(doc, blob, prim["indices"]).reshape(-1) if "indices" in prim
                       else np.arange(len(p)))
                tris.append(p[idx.reshape(-1, 3)])
                name = doc["materials"][prim["material"]].get("name", "?") if "material" in prim else "-"
                mats.extend([name] * (len(idx) // 3))
        for c in node.get("children", []):
            visit(c, m)

    scene = doc["scenes"][doc.get("scene", 0)]
    for i in scene["nodes"]:
        visit(i, np.eye(4))
    return (np.concatenate(tris) if tris else np.zeros((0, 3, 3))), mats


def clip(poly, a, b):
    """The part of convex polygon `poly` left of the directed edge a-b."""
    out = []
    n = len(poly)
    for i in range(n):
        p, q = poly[i], poly[(i + 1) % n]
        sp = (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
        sq = (b[0] - a[0]) * (q[1] - a[1]) - (b[1] - a[1]) * (q[0] - a[0])
        if sp >= 0:
            out.append(p)
        if (sp >= 0) != (sq >= 0):
            t = sp / (sp - sq)
            out.append((p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1])))
    return out


def area(poly):
    return 0.5 * sum(poly[i][0] * poly[(i + 1) % len(poly)][1] - poly[(i + 1) % len(poly)][0] * poly[i][1]
                     for i in range(len(poly)))


def shared(t, u):
    """The area two counterclockwise 2D triangles share."""
    poly = list(t)
    for i in range(3):
        poly = clip(poly, u[i], u[(i + 1) % 3])
        if len(poly) < 3:
            return 0.0
    return area(poly)


def overlaps(path, eps=EPS, opposite=False):
    """Pairs of faces in one plane that share area: each pair's plane
    normal, its height along it, the shared area, and both materials."""
    tris, mats = triangles(path)
    if not len(tris):
        return []
    n = np.cross(tris[:, 1] - tris[:, 0], tris[:, 2] - tris[:, 0])
    twice = np.linalg.norm(n, axis=1)
    keep = twice > 2 * MIN_AREA
    tris, n, twice = tris[keep], n[keep], twice[keep]
    mats = [m for m, k in zip(mats, keep) if k]
    n = n / twice[:, None]
    d = np.einsum("ij,ij->i", n, tris[:, 0])
    # Planes keyed by the normal (up to sign with `opposite`) and the
    # height, so parallel faces within `eps` share a key or a neighbor's.
    sign = np.ones(len(n))
    if opposite:
        first = np.argmax(np.abs(n) > 1e-6, axis=1)
        sign = np.sign(n[np.arange(len(n)), first])
    key_n = np.round(n * sign[:, None], 3)
    key_d = np.floor(d * sign / eps).astype(np.int64)
    groups = {}
    for i in range(len(n)):
        groups.setdefault((tuple(key_n[i]), key_d[i]), []).append(i)
    found = []
    for (kn, kd), members in groups.items():
        others = members + groups.get((kn, kd + 1), [])
        if len(others) < 2:
            continue
        axis = np.array(kn)
        # A basis in the plane, for 2D overlap.
        helper = np.array([1.0, 0.0, 0.0]) if abs(axis[0]) < 0.9 else np.array([0.0, 1.0, 0.0])
        e1 = np.cross(axis, helper)
        e1 /= np.linalg.norm(e1)
        e2 = np.cross(axis, e1)
        flat = {}
        for i in others:
            p = np.einsum("ij,kj->ik", tris[i], np.stack([e1, e2]))
            if area(p) < 0:
                p = p[::-1]
            flat[i] = p
        lo = {i: flat[i].min(axis=0) for i in others}
        hi = {i: flat[i].max(axis=0) for i in others}
        for a_i, i in enumerate(members):
            for j in others[a_i + 1:]:
                if j == i or abs(d[i] * sign[i] - d[j] * sign[j]) > eps:
                    continue
                if (lo[i] >= hi[j] - 1e-6).any() or (lo[j] >= hi[i] - 1e-6).any():
                    continue
                s = shared(flat[i], flat[j])
                if s > MIN_AREA:
                    found.append((tuple(n[i]), float(d[i]), float(d[j]), s, mats[i], mats[j], i, j))
    return found


def report(path, eps=EPS, opposite=False, same=False, limit=200):
    """Print the overlaps of the model at `path`, one line a plane and
    pair of materials; True when it has none. Two faces of one material
    shade alike, so they count only with `same`."""
    found = [f for f in overlaps(path, eps, opposite) if same or f[4] != f[5]]
    if not found:
        return True
    planes = {}
    for normal, d0, _, s, m0, m1, _, _ in found:
        key = (tuple(float(round(c, 3)) + 0.0 for c in normal), float(round(d0, 2)) + 0.0, tuple(sorted((m0, m1))))
        planes[key] = planes.get(key, 0.0) + s
    total = sum(planes.values())
    print(f"COPLANAR {path}: {len(planes)} overlapping planes, {total:.2f} m^2")
    for (normal, d, mats), s in sorted(planes.items(), key=lambda kv: -kv[1])[:limit]:
        print(f"  normal {normal} at {d:.2f}: {s:.3f} m^2, {mats[0]} and {mats[1]}")
    return False


def main():
    args = sys.argv[1:]
    opposite, same = "--opposite" in args, "--same" in args
    paths = [a for a in args if not a.startswith("--")]
    if not paths:
        print(__doc__)
        return 2
    ok = all([report(p, opposite=opposite, same=same) for p in paths])
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
