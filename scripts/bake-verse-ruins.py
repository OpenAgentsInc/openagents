#!/usr/bin/env python3
"""Bake pinned Ruins of Atlantis geometry into Verse's bounded forest pack.

Requires Python 3, NumPy, and Pillow. Run with --source /path/to/ruinsofatlantis.
The source checkout is read-only. The output contains geometry, sampled albedo,
and original skeletal poses; it contains no code, credentials, or executable data.
"""

import argparse
import base64
import hashlib
import io
import json
import math
from pathlib import Path
import struct

import numpy as np
from PIL import Image


SOURCE_COMMIT = "daeb5d0895270159ec8c18341b4adb84bf7a4346"
INPUTS = {
    "assets/models/wizard.gltf": "071a2189e2f90900bd89972dab18fb60053e88b66facb182b8f06d48c1f34675",
    "assets/models/zombie.glb": "4b0892cfe93070e2a9289b48ac18eaf24c90f8c0ba137ce8305b7558c94a8f8c",
    "assets/models/trees/CommonTree_3/CommonTree_3.gltf": "17c6537aa8042bb7255fbb501bf7aeb2b6db87c16766f794f9c37be3ce70c976",
    "assets/models/trees/CommonTree_3/CommonTree_3.bin": "24541f46fd9553e2389aba15575f2697009226e82118ed76868284114fd12b49",
    "assets/models/trees/CommonTree_3/Bark_NormalTree.png": "f36e94fdd73255347325b998e02a0b2575fdf714b0315bfe5e8ff34bc8e13603",
    "assets/models/trees/CommonTree_3/Leaves_NormalTree_C.png": "998612bdf0b76a22f5f54d65cd7a457815d2842d89653fa4348a4ebb42933ad3",
}
MAX_PACK = 25 * 1024 * 1024
FRAMES = 12


def rotation(q):
    x, y, z, w = q / np.linalg.norm(q)
    return np.array([
        [1-2*(y*y+z*z), 2*(x*y-z*w), 2*(x*z+y*w), 0],
        [2*(x*y+z*w), 1-2*(x*x+z*z), 2*(y*z-x*w), 0],
        [2*(x*z-y*w), 2*(y*z+x*w), 1-2*(x*x+y*y), 0],
        [0, 0, 0, 1],
    ], dtype=np.float64)


def slerp(a, b, t):
    a, b = a / np.linalg.norm(a), b / np.linalg.norm(b)
    dot = np.dot(a, b)
    if dot < 0:
        b, dot = -b, -dot
    if dot > .9995:
        q = a + (b-a)*t
        return q / np.linalg.norm(q)
    theta = math.acos(float(np.clip(dot, -1, 1)))
    return (math.sin((1-t)*theta)*a + math.sin(t*theta)*b) / math.sin(theta)


class Model:
    def __init__(self, path):
        self.path = path
        data = path.read_bytes()
        self.buffers = []
        if data[:4] == b"glTF":
            size = struct.unpack_from("<I", data, 12)[0]
            self.doc = json.loads(data[20:20+size])
            at = 20 + size
            length, kind = struct.unpack_from("<II", data, at)
            assert kind == 0x004E4942
            self.buffers.append(data[at+8:at+8+length])
        else:
            self.doc = json.loads(data)
            for buf in self.doc["buffers"]:
                uri = buf["uri"]
                self.buffers.append(base64.b64decode(uri.split(",", 1)[1])
                                    if uri.startswith("data:") else (path.parent / uri).read_bytes())
        assert not self.doc.get("extensionsRequired"), "Predecode required glTF extensions."
        self.images = {}
        self.parent = {}
        for i, node in enumerate(self.doc["nodes"]):
            for child in node.get("children", []):
                self.parent[child] = i

    def accessor(self, index):
        a = self.doc["accessors"][index]
        assert "sparse" not in a
        view = self.doc["bufferViews"][a["bufferView"]]
        dtype = {5120:"i1", 5121:"u1", 5122:"<i2", 5123:"<u2", 5125:"<u4", 5126:"<f4"}[a["componentType"]]
        width = {"SCALAR":1,"VEC2":2,"VEC3":3,"VEC4":4,"MAT4":16}[a["type"]]
        item = np.dtype(dtype).itemsize
        offset = view.get("byteOffset", 0) + a.get("byteOffset", 0)
        result = np.ndarray((a["count"], width), dtype=dtype,
                            buffer=self.buffers[view["buffer"]], offset=offset,
                            strides=(view.get("byteStride", width*item), item)).copy()
        if a.get("normalized"):
            result = result.astype(np.float64) / np.iinfo(dtype).max
        return result[:, 0] if width == 1 else result

    def image(self, index):
        if index not in self.images:
            im = self.doc["images"][index]
            if "uri" in im:
                raw = (self.path.parent / im["uri"]).read_bytes()
            else:
                view = self.doc["bufferViews"][im["bufferView"]]
                start = view.get("byteOffset", 0)
                raw = self.buffers[view["buffer"]][start:start+view["byteLength"]]
            self.images[index] = np.asarray(Image.open(io.BytesIO(raw)).convert("RGBA")) / 255.
        return self.images[index]

    def matrices(self, clip_name=None, time=0):
        transforms = {}
        for i, node in enumerate(self.doc["nodes"]):
            transforms[i] = {
                "translation": np.array(node.get("translation", [0, 0, 0]), dtype=float),
                "rotation": np.array(node.get("rotation", [0, 0, 0, 1]), dtype=float),
                "scale": np.array(node.get("scale", [1, 1, 1]), dtype=float),
            }
        if clip_name:
            clip = next(a for a in self.doc["animations"] if a["name"] == clip_name)
            for channel in clip["channels"]:
                target = channel["target"]
                sampler = clip["samplers"][channel["sampler"]]
                times, values = self.accessor(sampler["input"]), self.accessor(sampler["output"])
                assert sampler.get("interpolation", "LINEAR") in ["LINEAR", "STEP"]
                lower = max(0, min(len(times)-1, int(np.searchsorted(times, time, side="right"))-1))
                upper = min(lower+1, len(times)-1)
                t = 0 if times[upper] == times[lower] else np.clip((time-times[lower])/(times[upper]-times[lower]), 0, 1)
                a, b = values[lower], values[upper]
                if sampler.get("interpolation") == "STEP":
                    value = a
                elif target["path"] == "rotation":
                    value = slerp(a, b, t)
                else:
                    value = a + (b-a)*t
                transforms[target["node"]][target["path"]] = value
        matrices = {}
        def global_matrix(i):
            if i not in matrices:
                node, t = self.doc["nodes"][i], transforms[i]
                if "matrix" in node:
                    local = np.array(node["matrix"]).reshape(4, 4).T
                else:
                    local = rotation(t["rotation"]) @ np.diag([*t["scale"], 1])
                    local[:3, 3] = t["translation"]
                matrices[i] = global_matrix(self.parent[i]) @ local if i in self.parent else local
            return matrices[i]
        for i in transforms:
            global_matrix(i)
        return matrices

    def primitives(self, clip=None, time=0):
        matrices = self.matrices(clip, time)
        for i, node in enumerate(self.doc["nodes"]):
            if "mesh" not in node:
                continue
            for prim in self.doc["meshes"][node["mesh"]]["primitives"]:
                attrs = prim["attributes"]
                pos = self.accessor(attrs["POSITION"]).astype(float)
                nrm = self.accessor(attrs["NORMAL"]).astype(float)
                uv = self.accessor(attrs["TEXCOORD_0"]).astype(float)
                indices = self.accessor(prim["indices"]).reshape(-1, 3)
                color = self.accessor(attrs["COLOR_0"]) if "COLOR_0" in attrs else np.ones((len(pos), 4))
                if color.shape[1] == 3:
                    color = np.column_stack((color, np.ones(len(color))))
                if "skin" in node:
                    skin = self.doc["skins"][node["skin"]]
                    inverse = self.accessor(skin["inverseBindMatrices"]).reshape(-1, 4, 4).transpose(0, 2, 1)
                    palette = np.array([matrices[j] @ inv for j, inv in zip(skin["joints"], inverse)])
                    joints, weights = self.accessor(attrs["JOINTS_0"]), self.accessor(attrs["WEIGHTS_0"])
                    weights /= np.maximum(weights.sum(axis=1)[:, None], 1e-8)
                    blend = (palette[joints] * weights[:, :, None, None]).sum(axis=1)
                    pos = np.einsum("nij,nj->ni", blend, np.column_stack((pos, np.ones(len(pos)))))[:, :3]
                    nrm = np.einsum("nij,nj->ni", blend[:, :3, :3], nrm)
                else:
                    pos = np.einsum("nj,ij->ni", np.column_stack((pos, np.ones(len(pos)))), matrices[i])
                    pos = pos[:, :3]
                    nrm = np.einsum("nj,ji->ni", nrm, np.linalg.inv(matrices[i][:3, :3]))
                yield pos, nrm, uv, indices, color, self.doc["materials"][prim["material"]]

    def duration(self, name):
        clip = next(a for a in self.doc["animations"] if a["name"] == name)
        return max(float(self.accessor(s["input"])[-1]) for s in clip["samplers"])


def albedo(model, material, uv, vertex_colors):
    pbr = material.get("pbrMetallicRoughness", {})
    colors = vertex_colors * np.array(pbr.get("baseColorFactor", [1, 1, 1, 1]))
    if "baseColorTexture" in pbr:
        texture = model.doc["textures"][pbr["baseColorTexture"]["index"]]
        im = model.image(texture["source"])
        # These pinned models use the standard repeating texture sampler.
        x = np.floor((uv[:, 0] % 1) * im.shape[1]).astype(int)
        y = np.floor((uv[:, 1] % 1) * im.shape[0]).astype(int)
        tex = im[y, x].copy()
        # Source PNGs are sRGB; vertex colors and material factors are linear.
        rgb = tex[:, :3]
        tex[:, :3] = np.where(rgb <= .04045, rgb / 12.92, ((rgb + .055) / 1.055) ** 2.4)
        colors *= tex
    return colors


def subdivide_masked(pos, nrm, uv, color):
    # Four cuts per edge preserve leaf-card silhouettes without a texture shader.
    n = 4
    bary = []
    for i in range(n):
        for j in range(n-i):
            a, b, c = (i/n, j/n), ((i+1)/n, j/n), (i/n, (j+1)/n)
            triangles = [(a, b, c)]
            if i+j < n-1:
                triangles.append((b, ((i+1)/n, (j+1)/n), c))
            for tri in triangles:
                bary.extend([[1-x-y, x, y] for x, y in tri])
    bary = np.array(bary)
    return tuple(np.einsum("vi,tij->tvj", bary, v).reshape(-1, v.shape[-1])
                 for v in [pos, nrm, uv, color])


def mesh(model, clip=None, time=0, normalization=None):
    positions, colors = [], []
    for pos, nrm, uv, indices, vertex_colors, material in model.primitives(clip, time):
        pbr = material.get("pbrMetallicRoughness", {})
        texture_info = pbr.get("baseColorTexture")
        transparent = False
        if texture_info:
            texture = model.doc["textures"][texture_info["index"]]
            transparent = bool(np.any(model.image(texture["source"])[:, :, 3] < material.get("alphaCutoff", .5)))
        if material.get("alphaMode") == "MASK" and transparent:
            pos, nrm, uv, vertex_colors = subdivide_masked(pos[indices], nrm[indices], uv[indices], vertex_colors[indices])
            centroid_uv = uv.reshape(-1, 3, 2).mean(axis=1)
            centroid_colors = vertex_colors.reshape(-1, 3, 4).mean(axis=1)
            keep = albedo(model, material, centroid_uv, centroid_colors)[:, 3] >= material.get("alphaCutoff", .5)
            pos, nrm, uv, vertex_colors = [v.reshape(-1, 3, v.shape[-1])[keep].reshape(-1, v.shape[-1])
                                         for v in [pos, nrm, uv, vertex_colors]]
        else:
            pos, nrm, uv, vertex_colors = [v[indices].reshape(-1, v.shape[-1]) for v in [pos, nrm, uv, vertex_colors]]
        rgb = albedo(model, material, uv, vertex_colors)[:, :3]
        nrm /= np.maximum(np.linalg.norm(nrm, axis=1)[:, None], 1e-8)
        light = np.array([-.3, .85, .4]); light /= np.linalg.norm(light)
        lighting = .50 + .50 * np.abs(np.einsum("nj,j->n", nrm, light))
        rgb = np.clip(rgb * lighting[:, None], 0, 1)
        positions.append(pos)
        colors.append(np.rint(rgb * 255).astype(np.uint8))
    pos, color = np.concatenate(positions), np.concatenate(colors)
    if normalization is not None:
        center, scale = normalization
        pos = (pos-center) * scale
    assert np.isfinite(pos).all() and len(pos) % 3 == 0
    return pos, color


def normalize(model, height, clip=None):
    pos, _ = mesh(model, clip)
    low, high = pos.min(axis=0), pos.max(axis=0)
    return np.array([(low[0]+high[0])/2, low[1], (low[2]+high[2])/2]), height / (high[1]-low[1])


def encode_mesh(out, geometry):
    pos, color = geometry
    out.write(struct.pack("<I", len(pos)))
    record = np.empty(len(pos), dtype=[("pos", "<f4", 3), ("color", "u1", 3)])
    record["pos"], record["color"] = pos, color
    out.write(record.tobytes())


def bake(source, output):
    for path, expected in INPUTS.items():
        actual = hashlib.sha256((source / path).read_bytes()).hexdigest()
        if actual != expected:
            raise ValueError(f"Source digest mismatch: {path}")
    wizard = Model(source / "assets/models/wizard.gltf")
    zombie = Model(source / "assets/models/zombie.glb")
    tree = Model(source / "assets/models/trees/CommonTree_3/CommonTree_3.gltf")
    norms = {"tree": normalize(tree, 8), "wizard": normalize(wizard, 1.8, "Still"), "zombie": normalize(zombie, 1.8, "Idle")}
    out = io.BytesIO(); out.write(b"VZP1\r\n\x1a\n")
    stats = {}
    for name, model, clip in [("tree", tree, None), ("wizard_still", wizard, "Still")]:
        geometry = mesh(model, clip, normalization=norms["wizard" if name == "wizard_still" else name])
        encode_mesh(out, geometry)
        stats[name] = {"vertices": len(geometry[0]), "frames": 1}
    for name, model, clip, norm in [("wizard", wizard, "Waiting", norms["wizard"]), ("zombie", zombie, "Idle", norms["zombie"]), ("zombie_walk", zombie, "Walk", norms["zombie"])]:
        duration = model.duration(clip)
        out.write(struct.pack("<Hf", FRAMES, duration/FRAMES))
        for frame in range(FRAMES):
            geometry = mesh(model, clip, time=duration*frame/FRAMES, normalization=norm)
            encode_mesh(out, geometry)
        stats[name] = {"vertices_per_frame": len(geometry[0]), "frames": FRAMES, "source_clip": clip, "duration_seconds": duration}
    data = out.getvalue()
    if len(data) > MAX_PACK:
        raise ValueError("Pack exceeds 25 MiB")
    sha = hashlib.sha256(data).hexdigest()
    output.mkdir(parents=True, exist_ok=True)
    (output / f"{sha}.vzp").write_bytes(data)
    (output / f"{sha}.vzp.sha256").write_text(f"{sha}  {sha}.vzp\n")
    manifest = {"format": "verse-zone-pack-v1", "source_commit": SOURCE_COMMIT, "source_sha256": INPUTS,
                "pack_sha256": sha, "pack_file": f"{sha}.vzp", "pack_bytes": len(data), "meshes": stats,
                "authoring": "Original geometry and skeletal poses; sampled linear albedo, baked light, four-cut alpha leaf silhouettes.",
                "license_evidence": "Source repository Apache-2.0 and NOTICE retained. Asset-specific wizard/zombie upstream attribution is incomplete; no separate asset license is asserted."}
    (output / "provenance.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    for name in ["NOTICE", "LICENSE"]:
        (output / ("SOURCE_" + name)).write_bytes((source / name).read_bytes())
    print(json.dumps({"pack_bytes": len(data), "pack_sha256": sha, "meshes": stats}, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--output", type=Path, default=Path(__file__).resolve().parents[1] / "assets/verse/ruins")
    args = parser.parse_args()
    bake(args.source.resolve(), args.output.resolve())
