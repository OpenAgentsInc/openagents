"""Admit the Greco-futurism buildings into the Everglade pack's sources.

Run from the repository root with Python 3, NumPy, and Pillow:

    python3 scripts/blender/greco_admit.py [NAME ...]

NAME is `greco_house` (the owner's house), `civic_hall` (the Civic
Hall), or `belvedere` (the belvedere); with none, all three.
`scripts/blender/greco_futurism.py` writes each to
`assets/verse/generated/greco/NAME.glb` and its far level of detail to
`far/NAME.glb`. This script converts them with
`everglade_admit.py`'s conversion, which keeps the geometry and points the
limestone and walnut images at the village set's admitted plaster and wood
trim images with base-color factors:

- `assets/verse/everglade/generated/NAME.gltf` and `.bin`;
- `assets/verse/everglade/lod/generated.NAME.gltf` and `.bin`, the
  far level of detail Everglade draws beyond 80 m.

It adds only those files to each set's `manifest.json`, leaving every other
entry as it is, so it runs beside `everglade_admit.py` without rebuilding
the other models.
"""

import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from everglade_admit import LOD, OUT, VILLAGE, convert, mean_linear, sha, write_manifest  # noqa: E402

NAMES = ["greco_house", "civic_hall", "belvedere"]
VILLAGE_FILES = [
    "T_Plaster_Luma.png",
    "T_RoundTiles_Luma.png",
    "T_RoundTiles_BaseColor.png",
    "T_WoodTrim_BaseColor.png",
    "T_Brick_BaseColor.png",
    "T_RockTrim_BaseColor.png",
    "T_MetalOrnaments_BaseColor.png",
    "T_UnevenBrick_BaseColor.png",
]


def admit(source, name, out, how):
    means = {f: mean_linear(open(os.path.join(VILLAGE, f), "rb").read()) for f in VILLAGE_FILES}
    glb_digest, text, bin_bytes = convert(source, name, means, out=out)
    path = os.path.join(out, "manifest.json")
    manifest = json.load(open(path))
    for file, data in [(f"{name}.gltf", text), (f"{name}.bin", bin_bytes)]:
        manifest["files"][file] = sha(data)
        manifest["originals"][file] = glb_digest
        manifest["transforms"][file] = how
        print(f"{file}: {len(data)} bytes")
    for key in ("files", "originals", "transforms"):
        manifest[key] = dict(sorted(manifest[key].items()))
    write_manifest(path, manifest)


def main():
    names = sys.argv[1:] or NAMES
    for name in names:
        if name not in NAMES:
            print(f"unknown model {name}; expected one of {NAMES}")
            return 1
    for name in names:
        source, far_source = f"greco/{name}.glb", f"greco/far/{name}.glb"
        admit(
            source,
            name,
            OUT,
            f"converted from assets/verse/generated/{source} by scripts/blender/greco_admit.py with "
            "everglade_admit.py's conversion: separate buffer, embedded images replaced by the village "
            "set's admitted images with base-color factors",
        )
        admit(
            far_source,
            f"generated.{name}",
            LOD,
            f"far level of detail of generated/{name}, built by scripts/blender/greco_futurism.py and "
            f"converted from assets/verse/generated/{far_source} by scripts/blender/greco_admit.py",
        )
    return 0


if __name__ == "__main__":
    status = main()
    if not status:
        # Compact the glTF this script wrote (`everglade_compact.py`). It
        # writes no image, so the sets' PNGs aren't recompressed again.
        import everglade_compact

        for name in ("generated", "lod"):
            everglade_compact.compact_set(name, None)
    sys.exit(status)
