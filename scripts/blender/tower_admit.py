"""Admit the concrete tower into the Everglade pack's generated set.

Run from the repository root with Python 3:

    python3 scripts/blender/tower_admit.py

`scripts/blender/concrete_tower.py` bakes its own concrete texture, so unlike
the models `everglade_admit.py` converts, the tower keeps its image instead
of pointing at a village kit image. This script converts
`assets/verse/generated/tower/concrete_tower.glb` to
`assets/verse/everglade/generated/concrete_tower.gltf` and `.bin` without
touching the geometry, writes the embedded PNG beside them as
`T_Concrete_BaseColor.png` (the glTF names it by that relative URI), and adds
only those three files to the set's `manifest.json`, leaving every other
entry as it is. The PNG is the glb's embedded bytes, unchanged, so its
original digest is its own and it needs no transform.
"""

import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from everglade_admit import OUT, SOURCES, read_glb, sha, write_manifest  # noqa: E402

SOURCE = "tower/concrete_tower.glb"
NAME = "concrete_tower"


def convert():
    raw, doc, blob = read_glb(os.path.join(SOURCES, SOURCE))
    images = doc.get("images", [])
    image_views = {img["bufferView"] for img in images if "bufferView" in img}
    # Write each embedded image as its own PNG.
    pngs = {}
    for img in images:
        view = doc["bufferViews"][img["bufferView"]]
        start = view.get("byteOffset", 0)
        data = blob[start : start + view["byteLength"]]
        assert img.get("mimeType") == "image/png" and data[:8] == b"\x89PNG\r\n\x1a\n", img
        pngs[img["name"] + ".png"] = data
    # Compact the buffer without the image views.
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
    doc["buffers"] = [{"uri": f"{NAME}.bin", "byteLength": len(bin_bytes)}]
    names = [img["name"] for img in images]
    doc["images"] = [{"name": n, "uri": f"{n}.png"} for n in names]
    doc["textures"] = [{"source": t["source"]} for t in doc.get("textures", [])]
    doc.pop("samplers", None)
    for material in doc.get("materials", []):
        material.pop("extensions", None)
        material.pop("emissiveTexture", None)
        material.pop("normalTexture", None)
        material.pop("occlusionTexture", None)
        material.get("pbrMetallicRoughness", {}).pop("metallicRoughnessTexture", None)
    for key in ("extensionsUsed", "extensionsRequired"):
        doc.pop(key, None)
    text = (json.dumps(doc, sort_keys=True, separators=(",", ":")) + "\n").encode()
    return sha(raw), text, bin_bytes, pngs


def main():
    glb_digest, text, bin_bytes, pngs = convert()
    path = os.path.join(OUT, "manifest.json")
    manifest = json.load(open(path))
    how = (
        f"converted from assets/verse/generated/{SOURCE} by "
        "scripts/blender/tower_admit.py with everglade_admit.py's conversion: "
        "separate buffer, its embedded image "
        "written beside it as a PNG"
    )
    for file, data in [(f"{NAME}.gltf", text), (f"{NAME}.bin", bin_bytes)]:
        open(os.path.join(OUT, file), "wb").write(data)
        manifest["files"][file] = sha(data)
        manifest["originals"][file] = glb_digest
        manifest["transforms"][file] = how
        print(f"{file}: {len(data)} bytes")
    for file, data in pngs.items():
        open(os.path.join(OUT, file), "wb").write(data)
        # Baked by scripts/blender/concrete_tower.py and written unchanged.
        manifest["files"][file] = manifest["originals"][file] = sha(data)
        manifest["transforms"].pop(file, None)
        print(f"{file}: {len(data)} bytes")
    for key in ("files", "originals", "transforms"):
        manifest[key] = dict(sorted(manifest[key].items()))
    write_manifest(path, manifest)
    return 0


if __name__ == "__main__":
    status = main()
    if not status:
        # Compact what this script wrote (`everglade_compact.py`).
        import everglade_compact

        everglade_compact.main()
    sys.exit(status)
