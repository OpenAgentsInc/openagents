"""Admit the generated models into the Everglade pack's sources.

Run from the repository root with Python 3, NumPy, and Pillow:

    python3 scripts/blender/everglade_admit.py [--kit KIT_DIR]

The pack compiler reads glTF with separate `.bin` buffers and PNG images, and
only base-color images it has admitted. The generated models are binary glTF
with their own JPEG textures, most of them recolored copies of the Medieval
Village MegaKit's images. This script converts each one to
`assets/verse/everglade/generated/<name>.gltf` and `<name>.bin` without
touching its geometry, and points every textured material at the village
set's admitted image instead of carrying its own copy:

- Plaster and non-red roof tiles use two neutral images this script derives
  from the kit (`T_Plaster_Luma.png`, `T_RoundTiles_Luma.png`): the kit
  image's luminance, the way `buildings.py` recolors it. The material's
  base-color factor carries the building's color.
- Timber, brick, rock trim, metal, and the uneven brick use the kit's own
  image, with a factor for darkened timber.

Each factor is the ratio of the generated image's mean linear color to the
admitted image's, so the converted model keeps its look within the
renderer's 0 to 1 factor range. Images, the emissive-strength extension,
and the embedded image buffers are dropped.

The script then writes both sets' manifests: the generated set's, which
records each converted file's source `.glb` digest and the conversion, and
the village set's, with the three derived images. KIT_DIR defaults to
`~/Downloads/Medieval Village MegaKit[Standard]`; it is read only for the
derived images.
"""

import argparse
import hashlib
import io
import json
import os
import struct
import sys

import numpy as np
from PIL import Image

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
SOURCES = os.path.join(ROOT, "assets", "verse", "generated")
EVERGLADE = os.path.join(ROOT, "assets", "verse", "everglade")
OUT = os.path.join(EVERGLADE, "generated")
VILLAGE = os.path.join(EVERGLADE, "village")
KIT = os.path.expanduser("~/Downloads/Medieval Village MegaKit[Standard]")

# Source glb (relative to assets/verse/generated) and the admitted name.
MODELS = [
    ("observatory.glb", "observatory"),
    ("fountain.glb", "fountain"),
    ("bandshell.glb", "bandshell"),
    # One stall; `layout::paint` gives each placed stall its awning's colors.
    ("market_stall_red.glb", "market_stall"),
    ("buildings/townhouse_jettied.glb", "townhouse_jettied"),
    ("buildings/townhouse_balcony.glb", "townhouse_balcony"),
    ("buildings/row_townhouse.glb", "row_townhouse"),
    ("buildings/library.glb", "library"),
    ("buildings/tavern.glb", "tavern"),
    ("buildings/market_hall.glb", "market_hall"),
    ("buildings/corner_shop.glb", "corner_shop"),
    ("buildings/l_house.glb", "l_house"),
    ("buildings/cottage_tower.glb", "cottage_tower"),
] + [
    (f"buildings/{name}.glb", name)
    for name in [
        "music_hall",
        "meeting_hall",
        "boathouse",
        "boardwalk_cafe",
        "bakery",
        "smithy",
        "windmill",
        "greenhouse",
        "clock_tower",
        "guild_hall",
        "lookout",
        "log_cabin",
        "gazebo",
        "farmhouse",
        "cottage_thatch",
        "hip_house",
        "gambrel_barn",
        # The sixth round's lighter town houses (`town_houses.py`).
        "shop_house",
        "gambrel_house",
        "stone_cottage",
        "brownstone",
        "timber_house",
        "lantern_inn",
        # The seventh round's outbuildings (`town_houses.py`).
        "garden_shed",
        "woodshed",
    ]
] + [
    ("kit/roof_round_tiles_8x10.glb", "roof_round_tiles_8x10"),
] + [
    (f"street/{name}.glb", name)
    for name in [
        "lamp_post",
        "barrel",
        "flower_box",
        "well",
        "stone_wall",
        "hedge",
        "hand_cart",
        "signpost",
        "lily_pads",
        "footbridge",
        "wildflowers",
        "bunting",
        "pine_low",
        "oak_low",
    ]
] + [
    (f"town/{name}.glb", name)
    for name in [
        "statue",
        "sculpture",
        "sundial",
        "planter",
        "garden_arch",
        "picket_fence",
        "garden_gate",
        "flower_bed",
        "veg_bed",
        "rail_fence",
        "haystack",
        "hay_bales",
        "beehives",
        "rowboat",
        "dock",
        "reeds",
        "fallen_log",
        "mossy_rock",
        "mushrooms",
        "stump",
        "cafe_table",
        "birch_low",
        "poplar_low",
        "spruce_low",
        "fruit_tree",
        "bush_round",
        "flower_patch_spring",
        "flower_patch_summer",
        "flower_patch_autumn",
        "lantern_string",
        "lamp_double",
        "shop_sign",
        "park_bench",
        "fruit_tree_bloom",
        "cafe_umbrella",
        "produce_stall",
        "crate_stack",
        "street_bin",
        "water_pump",
        "flower_cart",
        "fountain_small",
        "boardwalk",
        "thicket",
        "young_trees",
        "copse",
        "footpath",
    ]
] + [
    (f"grove/{name}.glb", name)
    for name in [
        "grove_standing_stone",
        "grove_rune_stone",
        "grove_altar",
        "grove_oak",
        "grove_brazier",
        "grove_torch",
        "grove_campfire",
        "grove_archery_butt",
        "grove_training_ring",
        "grove_hanging_lantern",
    ]
]

# Far levels of detail that `town_houses.py` builds itself, admitted into
# the pack's `lod` set as `lod/generated.<name>`, beside `everglade_lod.py`'s.
LOD = os.path.join(EVERGLADE, "lod")
FAR_MODELS = [
    "shop_house",
    "gambrel_house",
    "stone_cottage",
    "brownstone",
    "timber_house",
    "lantern_inn",
]

# Derived village images: file name, kit source, edge, and how it is made.
LUMA_MEAN = {"T_Plaster_Luma.png": 0.92, "T_RoundTiles_Luma.png": 0.80}
DERIVED = {
    "T_Plaster_Luma.png": ("T_Plaster_BaseColor.png", 512),
    "T_RoundTiles_Luma.png": ("T_RoundTiles_BaseColor.png", 512),
    "T_UnevenBrick_BaseColor.png": ("T_UnevenBrick_BaseColor.png", 512),
}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def linear(srgb):
    srgb = np.asarray(srgb, dtype=np.float64)
    return np.where(srgb <= 0.04045, srgb / 12.92, ((srgb + 0.055) / 1.055) ** 2.4)


def png_bytes(image):
    out = io.BytesIO()
    image.save(out, format="PNG", optimize=True)
    return out.getvalue()


def derive(kit):
    """Writes the derived village images; returns {name: (bytes, kit digest, transform)}."""
    made = {}
    for name, (source, edge) in DERIVED.items():
        raw = open(os.path.join(kit, "glTF", source), "rb").read()
        image = Image.open(io.BytesIO(raw)).convert("RGB").resize((edge, edge), Image.LANCZOS)
        if name in LUMA_MEAN:
            px = np.asarray(image, dtype=np.float64) / 255.0
            lum = px[..., 0] * 0.299 + px[..., 1] * 0.587 + px[..., 2] * 0.114
            gray = np.clip(lum / lum.mean() * LUMA_MEAN[name], 0.0, 1.0)
            image = Image.fromarray(np.round(gray * 255.0).astype(np.uint8))
            transform = (
                f"the luminance of {source}, downscaled to {edge} px with Pillow's "
                f"Lanczos filter and scaled to a mean of {LUMA_MEAN[name]}, as a "
                "neutral image the generated models tint by material "
                "(scripts/blender/everglade_admit.py)"
            )
        else:
            transform = (
                f"downscaled to {edge} px on the long edge with Pillow's Lanczos filter "
                "(scripts/blender/everglade_admit.py)"
            )
        data = png_bytes(image)
        open(os.path.join(VILLAGE, name), "wb").write(data)
        made[name] = (data, sha(raw), transform)
    return made


def mean_linear(data):
    image = Image.open(io.BytesIO(data)).convert("RGB")
    image.thumbnail((256, 256))
    px = np.asarray(image, dtype=np.float64) / 255.0
    return linear(px).reshape(-1, 3).mean(axis=0)


def target(image_name):
    """The admitted village image a generated image maps to."""
    n = image_name
    if os.path.isfile(os.path.join(VILLAGE, n + ".png")):
        return n + ".png"
    if n.startswith("T_Plaster"):
        return "T_Plaster_Luma.png"
    if n.startswith("T_RoundTiles"):
        return "T_RoundTiles_BaseColor.png" if n == "T_RoundTiles_red" else "T_RoundTiles_Luma.png"
    for prefix, file in [
        ("T_WoodTrim", "T_WoodTrim_BaseColor.png"),
        ("T_Brick", "T_Brick_BaseColor.png"),
        ("T_RockTrim", "T_RockTrim_BaseColor.png"),
        ("T_MetalOrnaments", "T_MetalOrnaments_BaseColor.png"),
        ("T_UnevenBrick", "T_UnevenBrick_BaseColor.png"),
    ]:
        if n.startswith(prefix):
            return file
    raise SystemExit(f"no admitted image for {image_name}")


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


def convert(source, name, village_means, out=None):
    raw, doc, blob = read_glb(os.path.join(SOURCES, source))
    images = doc.get("images", [])
    image_views = {img["bufferView"] for img in images if "bufferView" in img}
    # Each generated image's mean, against the admitted image it maps to.
    remap = []
    for img in images:
        view = doc["bufferViews"][img["bufferView"]]
        start = view.get("byteOffset", 0)
        data = blob[start : start + view["byteLength"]]
        file = target(img["name"])
        # A kit image under its own name is the admitted image itself.
        if file == img["name"] + ".png":
            ratio = np.ones(3)
        else:
            ratio = mean_linear(data) / village_means[file]
        remap.append((file, ratio))
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
    doc["buffers"] = [{"uri": f"{name}.bin", "byteLength": len(bin_bytes)}]
    # One image and texture per admitted village file this model uses.
    files = sorted({file for file, _ in remap})
    doc["images"] = [{"name": f[:-4], "uri": f"../village/{f}"} for f in files]
    doc["textures"] = [{"source": i} for i in range(len(files))]
    doc.pop("samplers", None)
    for material in doc.get("materials", []):
        material.pop("extensions", None)
        material.pop("emissiveTexture", None)
        material.pop("normalTexture", None)
        material.pop("occlusionTexture", None)
        pbr = material.setdefault("pbrMetallicRoughness", {})
        pbr.pop("metallicRoughnessTexture", None)
        info = pbr.get("baseColorTexture")
        if info is None:
            continue
        source_image = doc_textures_source(raw_doc_textures(raw), info["index"])
        file, ratio = remap[source_image]
        factor = pbr.get("baseColorFactor", [1.0, 1.0, 1.0, 1.0])
        rgb = [min(1.0, float(round(factor[c] * ratio[c], 4))) for c in range(3)]
        pbr["baseColorFactor"] = rgb + [factor[3]]
        pbr["baseColorTexture"] = {"index": files.index(file)}
    for key in ("extensionsUsed", "extensionsRequired"):
        doc.pop(key, None)
    text = (json.dumps(doc, sort_keys=True, separators=(",", ":")) + "\n").encode()
    out = out or OUT
    open(os.path.join(out, f"{name}.gltf"), "wb").write(text)
    open(os.path.join(out, f"{name}.bin"), "wb").write(bin_bytes)
    return sha(raw), text, bin_bytes


def raw_doc_textures(raw):
    # The source document's textures, before conversion replaced them.
    length = struct.unpack_from("<I", raw, 12)[0]
    return json.loads(raw[20 : 20 + length]).get("textures", [])


def doc_textures_source(textures, index):
    return textures[index]["source"]


def admit_far(means):
    """Converts each of `FAR_MODELS`' far levels into the `lod` set and
    adds them to its manifest, leaving `everglade_lod.py`'s entries as they
    are."""
    path = os.path.join(LOD, "manifest.json")
    manifest = json.load(open(path))
    for name in FAR_MODELS:
        source = f"buildings/far/{name}.glb"
        lod = f"generated.{name}"
        glb_digest, text, bin_bytes = convert(source, lod, means, out=LOD)
        how = (
            f"far level of detail of generated/{name}, built by "
            "scripts/blender/town_houses.py and converted from "
            f"assets/verse/generated/{source} by scripts/blender/everglade_admit.py"
        )
        for file, data in [(f"{lod}.gltf", text), (f"{lod}.bin", bin_bytes)]:
            manifest["files"][file] = sha(data)
            manifest["originals"][file] = glb_digest
            manifest["transforms"][file] = how
        print(f"{lod}: {len(text)} + {len(bin_bytes)} bytes")
    for key in ("files", "originals", "transforms"):
        manifest[key] = dict(
            sorted((f, v) for f, v in manifest[key].items() if os.path.isfile(os.path.join(LOD, f)))
        )
    write_manifest(path, manifest)


def write_manifest(path, manifest):
    open(path, "w").write(json.dumps(manifest, indent=2, sort_keys=False) + "\n")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--kit", default=KIT)
    args = parser.parse_args()
    os.makedirs(OUT, exist_ok=True)
    derived = derive(args.kit)
    village_files = {
        f: open(os.path.join(VILLAGE, f), "rb").read()
        for f in [
            "T_Plaster_Luma.png",
            "T_RoundTiles_Luma.png",
            "T_RoundTiles_BaseColor.png",
            "T_WoodTrim_BaseColor.png",
            "T_Brick_BaseColor.png",
            "T_RockTrim_BaseColor.png",
            "T_MetalOrnaments_BaseColor.png",
            "T_UnevenBrick_BaseColor.png",
        ]
    }
    means = {f: mean_linear(d) for f, d in village_files.items()}
    files, originals, transforms = {}, {}, {}
    for source, name in MODELS:
        glb_digest, text, bin_bytes = convert(source, name, means)
        how = (
            f"converted from assets/verse/generated/{source} by "
            "scripts/blender/everglade_admit.py: separate buffer, embedded images "
            "replaced by the village set's admitted images with base-color factors"
        )
        for file, data in [(f"{name}.gltf", text), (f"{name}.bin", bin_bytes)]:
            files[file] = sha(data)
            originals[file] = glb_digest
            transforms[file] = how
        print(f"{name}: {len(text)} + {len(bin_bytes)} bytes")
    license_text = open(os.path.join(VILLAGE, "license.txt"), "rb").read()
    open(os.path.join(OUT, "license.txt"), "wb").write(license_text)
    files["license.txt"] = originals["license.txt"] = sha(license_text)
    # Keep the entries other scripts admit into the set (`tower_admit.py`),
    # while their files are there.
    path = os.path.join(OUT, "manifest.json")
    if os.path.isfile(path):
        before = json.load(open(path))
        for file, digest in before["files"].items():
            if file not in files and os.path.isfile(os.path.join(OUT, file)):
                files[file] = digest
                originals[file] = before["originals"][file]
                if file in before.get("transforms", {}):
                    transforms[file] = before["transforms"][file]
    write_manifest(
        os.path.join(OUT, "manifest.json"),
        {
            "schema": "openagents.verse.source-manifest.v1",
            "creator": "OpenAgents",
            "license": "CC0-1.0",
            "package": "Verse generated models (scripts/blender), built with the Medieval Village MegaKit Standard",
            "files": dict(sorted(files.items())),
            "originals": dict(sorted(originals.items())),
            "transforms": dict(sorted(transforms.items())),
        },
    )
    admit_far(means)
    path = os.path.join(VILLAGE, "manifest.json")
    village = json.load(open(path))
    for name, (data, kit_digest, transform) in derived.items():
        village["files"][name] = sha(data)
        village["originals"][name] = kit_digest
        village["transforms"][name] = transform
    for key in ("files", "originals", "transforms"):
        village[key] = dict(sorted(village[key].items()))
    write_manifest(path, village)
    return 0


if __name__ == "__main__":
    status = main()
    if not status:
        # Compact what this script wrote (`everglade_compact.py`).
        import everglade_compact

        everglade_compact.main()
    sys.exit(status)
