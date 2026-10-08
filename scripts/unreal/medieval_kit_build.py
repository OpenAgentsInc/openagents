#!/usr/bin/env python3
"""Build the Modular Medieval Town kit pieces as glTF for `compile::kit`.

    medieval_kit_build.py [--export DIR] [--out DIR] [--recipe FILE] [--only ID,ID]

The script reads the private export that `medieval_town_export.py` writes
(`meshes.json`, `materials.json`, `textures.json`, the `.glb` meshes, and
the PNG textures) and the committed recipe `medieval_kit_recipe.json`,
which names each kit piece by our own ID and the vendor mesh it comes from.
For each piece it writes `<id>.gltf` and `<id>.bin` into the kit build
directory, with the shape the Everglade pack's glTF importer admits:

- One primitive per base-color texture. Primitives whose materials resolve
  to the same texture, alpha mode, and sidedness merge into one.
- POSITION, NORMAL, TEXCOORD_0, and COLOR_0 only. TEXCOORD_1, tangents,
  and the source's COLOR_0 (a layer mask in the vendor's shaders) are
  dropped.
- Each material's tint is baked into COLOR_0 and its tiling into
  TEXCOORD_0, as `scripts/blender/medieval_kit.py` resolves them: the base
  layer of a layered master, half the painted tint of `MM_blend_2`, and the
  mean of the two tints for water, which has no texture.

A recipe piece may set `mirror_x` (negate x and flip the winding), `scale`
([sx, sy, sz], non-uniform), and `lod0` (use Unreal's reduced LOD0,
`.lod0.glb`, when the export has one). Positions are in meters, Y up, in
the frame the export's glTF uses.

Textures are copied as-is beside the pieces, one PNG per source texture,
named after its file stem; the Rust compiler downsizes them. A piece whose
base color is EXR only is skipped and reported. `build.json` lists each
piece's source mesh, triangles, bounds, and materials, and every texture
with its pixel size. The same export and recipe give the same bytes.

Everything this script writes is licensed content: it refuses an output
directory or an export inside the repository. Only this script and the
recipe, which holds names and no geometry, are committed.
"""

import argparse
import hashlib
import json
import os
import re
import shutil
import struct
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent
PRIVATE = Path("~/.openagents/verse/private/medieval-town").expanduser()
DEFAULT_EXPORT = PRIVATE / "export"
DEFAULT_OUT = PRIVATE / "kit-build"
DEFAULT_RECIPE = HERE / "medieval_kit_recipe.json"
RECIPE_SCHEMA = "openagents.verse.medieval-kit-recipe.v1"
BUILD_SCHEMA = "openagents.verse.medieval-kit-build.v1"

# The material tables of scripts/blender/medieval_kit.py, kept in step.
# Texture parameters that hold a material's base color, most specific first.
BASE_TEXTURES = ["Base_BC", "BC_2", "BC", "Diffuse", "Layer 1_BC", "BC_1", "Base color", "Texture"]
# Vector parameters that tint it.
TINTS = ["Base_tint", "Base BC tint", "Tint", "tint", "BC_tint", "Color main", "color", "texture_color"]
# Scalar parameters that tile the texture.
TILING = ["main tiling", "UV tiling", "Tiling zoom"]

# What the Everglade pack's importer admits (compile.rs).
MAX_NODES = 256
MAX_FILE_NAME_BYTES = 96
MAX_VERTICES = 65536
MAX_UV = 256.0
# Kit materials that glow, by a word of their name: candle and lamp flames,
# and the street lamps' glass.
EMISSIVE = ("flame", "glass_04")
PIECE_ID = re.compile(r"^[a-z0-9-]+$")

COMPONENTS = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}
FORMATS = {5120: "b", 5121: "B", 5122: "h", 5123: "H", 5125: "I", 5126: "f"}
NORMALIZE = {5120: 127.0, 5121: 255.0, 5122: 32767.0, 5123: 65535.0}


class SkipPiece(Exception):
    """A piece that can't be built; the build reports it and continues."""


def refuse_inside_repo(path):
    resolved = path.expanduser().resolve()
    if resolved == REPO or REPO in resolved.parents:
        sys.exit(f"{resolved} is inside the repository; the kit never enters it")
    # Any other checkout or worktree counts too: no git work tree holds kit
    # content.
    for parent in [resolved, *resolved.parents]:
        if (parent / ".git").exists():
            sys.exit(f"{resolved} is inside the git work tree {parent}; the kit never enters it")


def sanitize(name):
    return re.sub(r"[^A-Za-z0-9_.-]", "_", name)


def png_size(path):
    with open(path, "rb") as f:
        head = f.read(24)
    if head[:8] != b"\x89PNG\r\n\x1a\n" or head[12:16] != b"IHDR":
        raise SkipPiece(f"{path.name} is not a PNG")
    return struct.unpack(">II", head[16:24])


class Export:
    """The private export: its meshes, materials, and textures."""

    def __init__(self, root):
        self.root = root
        meshes = json.loads((root / "meshes.json").read_text())
        self.meshes = {m["path"]: m for m in meshes}
        materials = json.loads((root / "materials.json").read_text())
        self.materials = {m["path"].split("/")[-1]: m for m in materials}
        textures = json.loads((root / "textures.json").read_text())
        self.textures = {t["asset"]: t for t in textures}
        # Each copied texture is named after its file stem; a stem two
        # textures share gets a digest of its asset path, so the name never
        # depends on which pieces a run builds.
        stems = {}
        for t in textures:
            if t.get("png"):
                stem = sanitize(Path(t["png"]).stem)
                stems.setdefault(stem, []).append(t["asset"])
        self.texture_names = {}
        for stem, assets in stems.items():
            for asset in assets:
                name = stem
                if len(assets) > 1:
                    name += "-" + hashlib.sha256(asset.encode()).hexdigest()[:8]
                self.texture_names[asset] = name + ".png"

    def base_color(self, name):
        """(texture asset or None, tint RGBA, tiling (u, v), blend mode, two
        sided) for the material named `name`; ports `Export.base_color` in
        scripts/blender/medieval_kit.py and raises SkipPiece when the base
        color exists only as EXR."""
        record = self.materials.get(name.split(".")[0])
        if record is None:
            return None, (0.6, 0.6, 0.6, 1.0), (1.0, 1.0), "", False
        blend = record.get("blend_mode", "")
        two_sided = str(record.get("two_sided", "False")) == "True"
        master = record["parents"][-1].split(".")[-1] if record["parents"] else ""
        if master.startswith("MM_water"):
            # Water is a scattering shader with no base color; Verse draws
            # still water flat, so take the mean of its two tints.
            a = record["vectors"].get("Absorption_tint", (0.2, 0.3, 0.5, 1.0))
            s = record["vectors"].get("Scattering_tint", (0.3, 0.35, 0.3, 1.0))
            tint = tuple((a[i] + s[i]) / 2 for i in range(3)) + (1.0,)
            return None, tint, (1.0, 1.0), blend, two_sided
        image = None
        exr_only = None
        for key in BASE_TEXTURES:
            path = record["textures"].get(key)
            if path and "/Utility/" not in path:
                texture = self.textures.get(path)
                if texture and texture.get("png"):
                    image = path
                    break
                if texture and texture.get("exr") and exr_only is None:
                    exr_only = path
        if image is None and exr_only is not None:
            raise SkipPiece(f"material {name} has an EXR-only base color ({exr_only})")
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
        return image, tint, (u, v), blend, two_sided


class Glb:
    """A glTF 2.0 binary: its JSON and its one binary chunk."""

    def __init__(self, path):
        data = path.read_bytes()
        magic, version, _ = struct.unpack_from("<4sII", data, 0)
        if magic != b"glTF" or version != 2:
            raise SkipPiece(f"{path.name} is not glTF 2.0 binary")
        offset, self.json, self.bin = 12, None, b""
        while offset < len(data):
            length, kind = struct.unpack_from("<I4s", data, offset)
            chunk = data[offset + 8 : offset + 8 + length]
            if kind == b"JSON":
                self.json = json.loads(chunk)
            elif kind == b"BIN\x00":
                self.bin = chunk
            offset += 8 + length
        if self.json is None:
            raise SkipPiece(f"{path.name} has no JSON chunk")

    def read(self, index):
        """The accessor at `index` as a list of tuples (or ints for
        scalars), normalized integers converted to floats."""
        accessor = self.json["accessors"][index]
        if "sparse" in accessor or "bufferView" not in accessor:
            raise SkipPiece("sparse or view-less accessors are not supported")
        view = self.json["bufferViews"][accessor["bufferView"]]
        if view.get("buffer", 0) != 0:
            raise SkipPiece("a buffer view names a second buffer")
        kind = accessor["componentType"]
        n = COMPONENTS[accessor["type"]]
        fmt = "<" + FORMATS[kind] * n
        size = struct.calcsize(fmt)
        stride = view.get("byteStride") or size
        start = view.get("byteOffset", 0) + accessor.get("byteOffset", 0)
        scale = NORMALIZE.get(kind) if accessor.get("normalized") else None
        out = []
        for i in range(accessor["count"]):
            value = struct.unpack_from(fmt, self.bin, start + i * stride)
            if scale is not None:
                value = tuple(max(c / scale, -1.0) for c in value)
            out.append(value[0] if n == 1 else value)
        return out


def f32(x):
    return struct.unpack("<f", struct.pack("<f", x))[0]


def load_primitives(glb, label):
    """Each source primitive as (material name, positions, normals, uvs,
    indices)."""
    doc = glb.json
    if doc.get("extensionsRequired") or doc.get("skins") or doc.get("animations"):
        raise SkipPiece(f"{label} has extensions, skins, or animations")
    scene = doc.get("scenes", [{}])[doc.get("scene", 0)]
    nodes = doc.get("nodes", [])
    out = []
    stack = list(scene.get("nodes", []))
    while stack:
        node = nodes[stack.pop()]
        stack.extend(node.get("children", []))
        transformed = any(
            node.get(k) not in (None, [0, 0, 0], [0, 0, 0, 1], [1, 1, 1])
            for k in ("translation", "rotation", "scale")
        ) or node.get("matrix") not in (None, [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1])
        if "mesh" not in node:
            continue
        if transformed:
            raise SkipPiece(f"{label} places its mesh with a node transform")
        if "skin" in node:
            raise SkipPiece(f"{label} is skinned")
        for primitive in doc["meshes"][node["mesh"]]["primitives"]:
            if primitive.get("mode", 4) != 4 or primitive.get("targets"):
                raise SkipPiece(f"{label} has a non-triangle or morphed primitive")
            attributes = primitive["attributes"]
            for needed in ("POSITION", "NORMAL", "TEXCOORD_0"):
                if needed not in attributes:
                    raise SkipPiece(f"{label} has a primitive without {needed}")
            positions = glb.read(attributes["POSITION"])
            normals = glb.read(attributes["NORMAL"])
            uvs = glb.read(attributes["TEXCOORD_0"])
            if "indices" in primitive:
                indices = glb.read(primitive["indices"])
            else:
                indices = list(range(len(positions)))
            if "material" not in primitive:
                raise SkipPiece(f"{label} has a primitive without a material")
            name = doc["materials"][primitive["material"]].get("name", "")
            out.append((name, positions, normals, uvs, indices))
    return out


def transform(piece, positions, normals, indices):
    """Applies `mirror_x` and `scale`; returns new positions, normals, and
    indices (winding flipped when the transform mirrors)."""
    s = list(piece.get("scale", [1.0, 1.0, 1.0]))
    if piece.get("mirror_x"):
        s[0] = -s[0]
    if s == [1.0, 1.0, 1.0]:
        return positions, normals, indices
    if any(c == 0 for c in s):
        raise SkipPiece("scale has a zero component")
    positions = [(p[0] * s[0], p[1] * s[1], p[2] * s[2]) for p in positions]
    fixed = []
    for n in normals:
        # Normals transform by the inverse transpose: divide by the scale.
        m = (n[0] / s[0], n[1] / s[1], n[2] / s[2])
        length = (m[0] ** 2 + m[1] ** 2 + m[2] ** 2) ** 0.5 or 1.0
        fixed.append((m[0] / length, m[1] / length, m[2] / length))
    if s[0] * s[1] * s[2] < 0:
        flipped = []
        for i in range(0, len(indices), 3):
            flipped += [indices[i], indices[i + 2], indices[i + 1]]
        indices = flipped
    return positions, fixed, indices


class Writer:
    """Packs accessors into one 4-byte-aligned binary buffer."""

    def __init__(self):
        self.bin = bytearray()
        self.views = []
        self.accessors = []

    def add(self, data, accessor, target):
        while len(self.bin) % 4:
            self.bin.append(0)
        self.views.append({"buffer": 0, "byteOffset": len(self.bin), "byteLength": len(data), "target": target})
        self.bin += data
        accessor["bufferView"] = len(self.views) - 1
        self.accessors.append(accessor)
        return len(self.accessors) - 1


def build_piece(export, out, piece_id, piece, textures_used):
    mesh = piece["mesh"]
    record = export.meshes.get(mesh)
    if record is None:
        raise SkipPiece(f"mesh {mesh} is not in meshes.json")
    glb_path = export.root / "meshes" / (mesh + ".glb")
    lod0 = export.root / "meshes" / (mesh + ".lod0.glb")
    used_lod0 = bool(piece.get("lod0")) and lod0.is_file()
    if used_lod0:
        glb_path = lod0
    if not glb_path.is_file():
        raise SkipPiece(f"{glb_path.name} is missing from the export")
    sources = load_primitives(Glb(glb_path), piece_id)

    # Group the source primitives by the glTF material they resolve to.
    groups = {}
    for name, positions, normals, uvs, indices in sources:
        image, tint, (tu, tv), blend, two_sided = export.base_color(name)
        if "TRANSLUCENT" in blend or "ADDITIVE" in blend or "glass" in name.lower():
            alpha, suffix = "BLEND", "-blend"
        elif "MASKED" in blend:
            alpha, suffix = "MASK", "-mask"
        else:
            alpha, suffix = "OPAQUE", ""
        if image is not None:
            texture = export.texture_names[image]
            gltf_name = Path(texture).stem + suffix
            color = tuple(min(max(c, 0.0), 1.0) for c in tint[:3]) + (1.0,)
            factor = None
        else:
            texture = None
            gltf_name = sanitize(name or "untextured") + suffix
            color = (1.0, 1.0, 1.0, 1.0)
            factor = [round(min(max(c, 0.0), 1.0), 6) for c in tint[:3]] + [1.0]
        # Flames and a lamp's glass glow: Everglade lights a material whose
        # name starts with `Emit` (`zones::everglade::scene::emits`).
        if any(word in name.lower() for word in EMISSIVE):
            gltf_name = "Emit_" + gltf_name
        key = (gltf_name, texture, alpha, two_sided, tuple(factor or ()))
        positions, normals, indices = transform(piece, positions, normals, indices)
        uvs = [(uv[0] * tu, uv[1] * tv) for uv in uvs]
        if any(abs(c) > MAX_UV for uv in uvs for c in uv):
            raise SkipPiece(f"material {name} tiles its coordinates past {MAX_UV:g}")
        if any(i >= len(positions) for i in indices) or len(indices) % 3:
            raise SkipPiece(f"material {name} has invalid indices")
        rgba = tuple(round(c * 255) for c in color)
        group = groups.setdefault(key, [])
        # Start a new primitive when the merged one would pass 65,536 vertices.
        if not group or len(group[-1]["positions"]) + len(positions) > MAX_VERTICES:
            group.append({"positions": [], "normals": [], "uvs": [], "colors": [], "indices": []})
        target = group[-1]
        base = len(target["positions"])
        target["positions"] += positions
        target["normals"] += normals
        target["uvs"] += uvs
        target["colors"] += [rgba] * len(positions)
        target["indices"] += [i + base for i in indices]

    writer = Writer()
    materials, images, gltf_textures, primitives = [], [], [], []
    triangles = 0
    lo, hi = [float("inf")] * 3, [float("-inf")] * 3
    for key in sorted(groups, key=lambda k: (k[0], k[2], k[3], k[4], k[1] or "")):
        gltf_name, texture, alpha, two_sided, factor = key
        material = {"name": gltf_name, "doubleSided": two_sided, "alphaMode": alpha}
        if alpha == "MASK":
            material["alphaCutoff"] = 0.5
        pbr = {"metallicFactor": 0.0, "roughnessFactor": 1.0}
        if texture is not None:
            if texture not in images:
                images.append(texture)
                gltf_textures.append({"source": len(images) - 1})
            pbr["baseColorFactor"] = [1.0, 1.0, 1.0, 1.0]
            pbr["baseColorTexture"] = {"index": images.index(texture)}
            textures_used.add(texture)
        else:
            pbr["baseColorFactor"] = list(factor)
        material["pbrMetallicRoughness"] = pbr
        materials.append(material)
        for group in groups[key]:
            count = len(group["positions"])
            pos = [tuple(f32(c) for c in p) for p in group["positions"]]
            for p in pos:
                for a in range(3):
                    lo[a] = min(lo[a], p[a])
                    hi[a] = max(hi[a], p[a])
            index_format = "H" if count <= 65536 else "I"
            attributes = {
                "POSITION": writer.add(
                    struct.pack(f"<{3 * count}f", *[c for p in pos for c in p]),
                    {
                        "componentType": 5126,
                        "count": count,
                        "type": "VEC3",
                        "min": [min(p[a] for p in pos) for a in range(3)],
                        "max": [max(p[a] for p in pos) for a in range(3)],
                    },
                    34962,
                ),
                "NORMAL": writer.add(
                    struct.pack(f"<{3 * count}f", *[c for n in group["normals"] for c in n]),
                    {"componentType": 5126, "count": count, "type": "VEC3"},
                    34962,
                ),
                "TEXCOORD_0": writer.add(
                    struct.pack(f"<{2 * count}f", *[c for uv in group["uvs"] for c in uv]),
                    {"componentType": 5126, "count": count, "type": "VEC2"},
                    34962,
                ),
                "COLOR_0": writer.add(
                    bytes(c for rgba in group["colors"] for c in rgba),
                    {"componentType": 5121, "count": count, "type": "VEC4", "normalized": True},
                    34962,
                ),
            }
            indices = writer.add(
                struct.pack(f"<{len(group['indices'])}{index_format}", *group["indices"]),
                {
                    "componentType": 5123 if index_format == "H" else 5125,
                    "count": len(group["indices"]),
                    "type": "SCALAR",
                },
                34963,
            )
            triangles += len(group["indices"]) // 3
            primitives.append(
                {"attributes": attributes, "indices": indices, "material": len(materials) - 1, "mode": 4}
            )
    if not primitives:
        raise SkipPiece("no triangles")

    bin_name = f"{piece_id}.bin"
    document = {
        "asset": {"version": "2.0", "generator": "openagents medieval_kit_build.py"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0, "name": piece_id}],
        "meshes": [{"name": piece_id, "primitives": primitives}],
        "materials": materials,
        "accessors": writer.accessors,
        "bufferViews": writer.views,
        "buffers": [{"uri": bin_name, "byteLength": len(writer.bin)}],
    }
    if images:
        document["images"] = [{"uri": name} for name in images]
        document["textures"] = gltf_textures
    gltf_bytes = (json.dumps(document, indent=1, sort_keys=True) + "\n").encode()
    (out / f"{piece_id}.gltf").write_bytes(gltf_bytes)
    (out / bin_name).write_bytes(bytes(writer.bin))
    return {
        "mesh": mesh,
        "source": glb_path.relative_to(export.root).as_posix(),
        "lod0": used_lod0,
        "triangles": triangles,
        "bounds_m": {"min": [round(c, 4) for c in lo], "max": [round(c, 4) for c in hi]},
        "materials": [m["name"] for m in materials],
        "primitives": len(primitives),
        "bytes": len(gltf_bytes) + len(writer.bin),
    }


def load_recipe(path):
    recipe = json.loads(path.read_text())
    if recipe.get("schema") != RECIPE_SCHEMA:
        sys.exit(f"{path} is not a {RECIPE_SCHEMA} recipe")
    pieces = recipe.get("pieces", {})
    for piece_id, piece in pieces.items():
        if not PIECE_ID.match(piece_id) or len(piece_id) + 5 > MAX_FILE_NAME_BYTES:
            sys.exit(f"recipe piece ID {piece_id!r} is not lowercase [a-z0-9-] within the name limit")
        unknown = set(piece) - {"mesh", "mirror_x", "scale", "lod0", "far_triangles"}
        if unknown or "mesh" not in piece:
            sys.exit(f"recipe piece {piece_id} needs `mesh` and allows only mirror_x, scale, lod0, far_triangles")
        far = piece.get("far_triangles")
        if far is not None and (not isinstance(far, int) or isinstance(far, bool) or far < 1):
            sys.exit(f"recipe piece {piece_id} needs a positive far_triangles budget")
        scale = piece.get("scale")
        if scale is not None and (len(scale) != 3 or not all(isinstance(c, (int, float)) for c in scale)):
            sys.exit(f"recipe piece {piece_id} has a scale that is not [sx, sy, sz]")
    return pieces


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--export", type=Path, default=DEFAULT_EXPORT)
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT)
    parser.add_argument("--recipe", type=Path, default=DEFAULT_RECIPE)
    parser.add_argument("--only", default="", help="comma-separated piece IDs")
    parser.add_argument("--blender", default=os.environ.get("BLENDER"),
                        help="headless Blender executable for far levels")
    args = parser.parse_args()

    export_root = args.export.expanduser().resolve()
    out = args.out.expanduser().resolve()
    refuse_inside_repo(export_root)
    refuse_inside_repo(out)
    for needed in ("meshes.json", "materials.json", "textures.json", "meshes"):
        if not (export_root / needed).exists():
            sys.exit(f"{export_root} is missing {needed}; run medieval_town_export.py first")
    pieces = load_recipe(args.recipe.expanduser())
    if args.only:
        wanted = [p.strip() for p in args.only.split(",") if p.strip()]
        missing = [p for p in wanted if p not in pieces]
        if missing:
            sys.exit(f"the recipe has no piece {', '.join(missing)}")
        pieces = {p: pieces[p] for p in wanted}

    # Remove only prior derived house outputs. The current Rust recipes
    # regenerate them after the piece build, including their atlas palette.
    for pattern in ["house-*-near.gltf", "house-*-near.bin", "house-*-middle.gltf", "house-*-middle.bin", "house-*-far.gltf", "house-*-far.bin", "house-atlas-*.png"]:
        for file in out.glob(pattern):
            file.unlink()
    export = Export(export_root)
    out.mkdir(parents=True, exist_ok=True)
    built, skipped, textures_used = {}, {}, set()
    for piece_id in sorted(pieces):
        try:
            built[piece_id] = build_piece(export, out, piece_id, pieces[piece_id], textures_used)
        except SkipPiece as error:
            skipped[piece_id] = str(error)
            for stale in (out / f"{piece_id}.gltf", out / f"{piece_id}.bin"):
                stale.unlink(missing_ok=True)
            print(f"skipped {piece_id}: {error}", file=sys.stderr)

    by_name = {name: asset for asset, name in export.texture_names.items()}
    textures = {}
    for name in sorted(textures_used):
        source = export.root / export.textures[by_name[name]]["png"]
        target = out / name
        if not target.is_file() or target.read_bytes() != source.read_bytes():
            shutil.copyfile(source, target)
        width, height = png_size(target)
        textures[name] = {
            "source": export.textures[by_name[name]]["png"],
            "width": width,
            "height": height,
            "bytes": target.stat().st_size,
        }

    if any(p.get("far_triangles") for p in pieces.values()):
        blender = args.blender or shutil.which("blender")
        mac = "/Applications/Blender.app/Contents/MacOS/Blender"
        if blender is None and Path(mac).is_file():
            blender = mac
        if blender is None:
            sys.exit("Far levels need headless Blender; set BLENDER or --blender")
        subprocess.run([blender, "-b", "--factory-startup", "-t", "4", "--python-exit-code", "1", "--python",
                        str(REPO / "scripts/blender/medieval_kit_far.py"), "--",
                        str(out), str(args.recipe.expanduser().resolve())], check=True)

    summary = {
        "pieces": len(built),
        "skipped": len(skipped),
        "triangles": sum(p["triangles"] for p in built.values()),
        "textures": len(textures),
        "texture_bytes": sum(t["bytes"] for t in textures.values()),
        "geometry_bytes": sum(p["bytes"] for p in built.values()),
    }
    summary["bytes"] = summary["texture_bytes"] + summary["geometry_bytes"]
    document = {
        "schema": BUILD_SCHEMA,
        "pieces": built,
        "skipped": skipped,
        "textures": textures,
        "summary": summary,
    }
    (out / "build.json").write_text(json.dumps(document, indent=1, sort_keys=True) + "\n")
    print(json.dumps(summary, sort_keys=True))
    for piece_id, reason in sorted(skipped.items()):
        print(f"skipped {piece_id}: {reason}")


if __name__ == "__main__":
    main()
