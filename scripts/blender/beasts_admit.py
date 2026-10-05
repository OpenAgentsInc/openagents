"""Admit the Wild Shape beasts into the Everglade pack's sources.

Run from the repository root with Python 3:

    python3 scripts/blender/beasts_admit.py

The pack compiler reads glTF with separate `.bin` buffers. The beasts are
binary glTF under `assets/verse/generated/`: the Giant Spider converted from
Quaternius's Easy Animated Enemy Pack by `enemy_pack.py`, and the stylized
bear, wolf, and eagle built by `animals.py`. Each is skinned, with its clips,
and carries flat base colors and no images. This script splits each one into
`assets/verse/everglade/beasts/<name>.gltf` and `<name>.bin` without
touching its geometry, skeleton, or clips, and writes the set's
`manifest.json`, which records each file's source `.glb` digest.

The compiler turns the set's skinned models into the pack's Wild Shape forms
rather than static models (`verse::zones::everglade_pack::compile`).
"""

import hashlib
import json
import os
import struct
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
SOURCES = os.path.join(ROOT, "assets", "verse", "generated")
OUT = os.path.join(ROOT, "assets", "verse", "everglade", "beasts")
LICENSE = b"""Wild Shape beasts for Verse

giant_spider: the Spider from Easy Animated Enemy Pack (January 2019) by
Quaternius (https://quaternius.com), converted to glTF by
scripts/blender/enemy_pack.py.

bear, wolf, eagle: generated from primitives by scripts/blender/animals.py.

songbird, duck, cat: Everglade's ambient wildlife, generated from
primitives by scripts/blender/wildlife.py.

rat, frog, snake, wasp: Everglade's ambient wildlife, lighter copies of the
Rat, Frog, Snake, and Wasp from the same Easy Animated Enemy Pack,
converted by scripts/blender/wildlife.py.

License:
CC0 1.0 Universal (CC0 1.0)
Public Domain Dedication
https://creativecommons.org/publicdomain/zero/1.0/
"""

# Source glb (relative to assets/verse/generated) and the admitted name.
MODELS = [
    ("giant_spider.glb", "giant_spider"),
    ("bear.glb", "bear"),
    ("wolf.glb", "wolf"),
    ("eagle.glb", "eagle"),
] + [
    (f"wildlife/{name}.glb", name)
    for name in ["songbird", "duck", "cat", "rat", "frog", "snake", "wasp"]
]


def sha(data):
    return hashlib.sha256(data).hexdigest()


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


def convert(source, name):
    raw, doc, blob = read_glb(os.path.join(SOURCES, source))
    assert not doc.get("images"), f"{source} carries images"
    blob += b"\0" * ((-len(blob)) % 4)
    doc["buffers"] = [{"uri": f"{name}.bin", "byteLength": len(blob)}]
    for key in ("extensionsUsed", "extensionsRequired"):
        doc.pop(key, None)
    for material in doc.get("materials", []):
        material.pop("extensions", None)
    text = (json.dumps(doc, sort_keys=True, separators=(",", ":")) + "\n").encode()
    open(os.path.join(OUT, f"{name}.gltf"), "wb").write(text)
    open(os.path.join(OUT, f"{name}.bin"), "wb").write(blob)
    return sha(raw), text, blob


def main():
    os.makedirs(OUT, exist_ok=True)
    files, originals, transforms = {}, {}, {}
    for source, name in MODELS:
        glb_digest, text, bin_bytes = convert(source, name)
        how = (
            f"split from assets/verse/generated/{source} by "
            "scripts/blender/beasts_admit.py: separate buffer, geometry, skin, and clips unchanged"
        )
        for file, data in [(f"{name}.gltf", text), (f"{name}.bin", bin_bytes)]:
            files[file] = sha(data)
            originals[file] = glb_digest
            transforms[file] = how
        print(f"{name}: {len(text)} + {len(bin_bytes)} bytes")
    license_text = LICENSE
    open(os.path.join(OUT, "license.txt"), "wb").write(license_text)
    files["license.txt"] = originals["license.txt"] = sha(license_text)
    manifest = {
        "schema": "openagents.verse.source-manifest.v1",
        "creator": "OpenAgents",
        "license": "CC0-1.0",
        "package": "Verse Wild Shape beasts and Everglade wildlife: creatures from Quaternius's Easy Animated Enemy Pack, and generated animals",
        "files": dict(sorted(files.items())),
        "originals": dict(sorted(originals.items())),
        "transforms": dict(sorted(transforms.items())),
    }
    path = os.path.join(OUT, "manifest.json")
    open(path, "w").write(json.dumps(manifest, indent=2, sort_keys=False) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
