"""Compact the Everglade pack's sources without changing what the pack draws.

Run from the repository root with Python 3 and Pillow:

    python3 scripts/blender/everglade_compact.py

The admit scripts (`everglade_admit.py`, `tower_admit.py`,
`everglade_lod.py`, and `foliage_admit.py`) call it when they finish, so a
rebuilt source is compacted again. It changes no geometry, color, or pixel
the pack compiler reads:

- In the sets these scripts build (`generated`, `foliage`, and `lod`), it
  rewrites each glTF and its buffer with only what the compiler reads:
  positions, normals, colors, indices, and texture coordinates where a
  material samples an image. A second coordinate set and the coordinates of
  an untextured material are dropped; the compiler gives an untextured
  primitive zero coordinates. Each accessor gets its own tightly packed
  buffer view, and the JSON is written without whitespace. The Quaternius
  kits' models stay byte for byte as the kits ship them.
- In every set, it recompresses each PNG losslessly with `oxipng`, keeping
  its 8-bit gray, RGB, or RGBA color type, so the compiler stores the
  smaller bytes in the pack as they are. It checks that every pixel decodes
  the same, and skips this step when `oxipng` is not installed
  (`cargo install oxipng`).

Each rewritten file's manifest digest is updated, and its transform records
the step. Running the script again changes nothing.
"""

import hashlib
import io
import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile

from PIL import Image

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
EVERGLADE = os.path.join(ROOT, "assets", "verse", "everglade")
# The sets whose models the admit scripts write; the kits' sets keep their
# models as shipped.
DERIVED = ["generated", "foliage", "lod"]
SETS = ["nature", "village", "props", "generated", "foliage", "lod"]
# Materials `zones::everglade::scene::PAINTED` gives a neutral image, so they
# keep their coordinates even without an image of their own.
PAINTED = ["MI_Plaster", "MI_RoundTiles", "HousePlaster", "HouseTiles"]
KEPT = ["POSITION", "NORMAL", "COLOR_0"]
GLTF_NOTE = (
    "then compacted by scripts/blender/everglade_compact.py: only the attributes "
    "the pack compiler reads, tightly packed"
)
PNG_NOTE = (
    "then recompressed losslessly by scripts/blender/everglade_compact.py with "
    "oxipng -o max --zopfli --nx --strip all"
)
OXIPNG_ARGS = ["-o", "max", "--zopfli", "--nx", "--strip", "all", "-q"]
COMPONENT = {5120: 1, 5121: 1, 5122: 2, 5123: 2, 5125: 4, 5126: 4}
WIDTH = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def textured(material):
    return "baseColorTexture" in material.get("pbrMetallicRoughness", {}) or any(
        name in material.get("name", "") for name in PAINTED
    )


def compact_gltf(text, buffers):
    """Returns the compacted glTF text and buffer, or None if it uses
    features this script does not rewrite."""
    doc = json.loads(text)
    if len(doc.get("buffers", [])) != 1 or any("sparse" in a for a in doc["accessors"]):
        return None
    materials = doc.get("materials", [])
    out = bytearray()
    views, accessors, moved = [], [], {}

    def move(index, target):
        if index in moved:
            return moved[index]
        accessor = dict(doc["accessors"][index])
        view = doc["bufferViews"][accessor.pop("bufferView")]
        size = COMPONENT[accessor["componentType"]] * WIDTH[accessor["type"]]
        stride = view.get("byteStride", size)
        start = view.get("byteOffset", 0) + accessor.pop("byteOffset", 0)
        data = buffers[view["buffer"]]
        packed = b"".join(
            data[start + i * stride : start + i * stride + size] for i in range(accessor["count"])
        )
        while len(out) % 4:
            out.append(0)
        new_view = {"buffer": 0, "byteOffset": len(out), "byteLength": len(packed)}
        if target is not None:
            new_view["target"] = target
        out.extend(packed)
        views.append(new_view)
        accessor = {"bufferView": len(views) - 1, **accessor}
        accessors.append(accessor)
        moved[index] = len(accessors) - 1
        return moved[index]

    for mesh in doc["meshes"]:
        for primitive in mesh["primitives"]:
            if "targets" in primitive:
                return None
            material = materials[primitive["material"]] if "material" in primitive else {}
            keep = KEPT + (["TEXCOORD_0"] if textured(material) else [])
            primitive["attributes"] = {
                name: move(index, 34962)
                for name, index in primitive["attributes"].items()
                if name in keep
            }
            if "indices" in primitive:
                primitive["indices"] = move(primitive["indices"], 34963)
    while len(out) % 4:
        out.append(0)
    doc["accessors"] = accessors
    doc["bufferViews"] = views
    doc["buffers"][0]["byteLength"] = len(out)
    return json.dumps(doc, separators=(",", ":")).encode(), bytes(out)


def note(transforms, file, text):
    before = transforms.get(file)
    if before is None:
        transforms[file] = text[len("then ") :]
    elif text not in before:
        transforms[file] = f"{before}; {text}"


def pixels(data):
    image = Image.open(io.BytesIO(data))
    return image.mode, image.size, image.tobytes()


def oxipng():
    found = shutil.which("oxipng")
    if found:
        return found
    cargo = os.path.expanduser("~/.cargo/bin/oxipng")
    return cargo if os.path.isfile(cargo) else None


def recompress(path, tool):
    """Returns the smaller lossless PNG for `path`, or None."""
    before = open(path, "rb").read()
    with tempfile.TemporaryDirectory() as scratch:
        copy = os.path.join(scratch, "image.png")
        open(copy, "wb").write(before)
        subprocess.run([tool, *OXIPNG_ARGS, copy], check=True)
        after = open(copy, "rb").read()
    if len(after) >= len(before):
        return None
    if pixels(after) != pixels(before):
        raise SystemExit(f"oxipng changed the pixels of {path}")
    return after


def compact_set(name, tool):
    directory = os.path.join(EVERGLADE, name)
    path = os.path.join(directory, "manifest.json")
    manifest = json.load(open(path))
    transforms = manifest.setdefault("transforms", {})
    saved = 0
    for file in sorted(manifest["files"]):
        full = os.path.join(directory, file)
        if name in DERIVED and file.endswith(".gltf"):
            text = open(full, "rb").read()
            doc = json.loads(text)
            uris = [b["uri"] for b in doc.get("buffers", [])]
            buffers = [open(os.path.join(directory, uri), "rb").read() for uri in uris]
            compacted = compact_gltf(text, buffers)
            if compacted is None:
                continue
            new_text, new_bin = compacted
            for target, old, new in [(file, text, new_text), (uris[0], buffers[0], new_bin)]:
                if new != old:
                    open(os.path.join(directory, target), "wb").write(new)
                    manifest["files"][target] = sha(new)
                    note(transforms, target, GLTF_NOTE)
                    saved += len(old) - len(new)
        elif file.endswith(".png") and tool:
            smaller = recompress(full, tool)
            if smaller is not None:
                saved += os.path.getsize(full) - len(smaller)
                open(full, "wb").write(smaller)
                manifest["files"][file] = sha(smaller)
                note(transforms, file, PNG_NOTE)
    manifest["transforms"] = dict(sorted(transforms.items()))
    with open(path, "w") as f:
        f.write(json.dumps(manifest, indent=2) + "\n")
    return saved


def main():
    tool = oxipng()
    if tool is None:
        print("oxipng is not installed; PNGs are left as they are (cargo install oxipng)")
    total = 0
    for name in SETS:
        saved = compact_set(name, tool)
        total += saved
        print(f"{name}: {saved} bytes smaller")
    print(f"compacted: {total} bytes smaller")
    return 0


if __name__ == "__main__":
    sys.exit(main())
