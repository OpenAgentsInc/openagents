# Generated and converted models

Every model here was written by Blender 5.2.2 LTS, run headless
(`Blender -b --factory-startup --python SCRIPT -- ARGS`), from a script under
`scripts/blender/`. The script is the model's source; rebuild them all with
`scripts/blender/build-models.sh`. Blender's glTF export isn't byte-identical
across versions, so compare rebuilds by their previews, not their bytes.

The models aren't admitted into any zone pack yet. Admission is a later step
that follows [the pipeline](../../../docs/verse/blender-pipeline.md).

All models use 1 unit = 1 m and glTF's +Y up, with the origin at the center
of the base. A model's front faces glTF +Z.

## Converted models

Source: Quaternius, *Easy Animated Enemy Pack* (January 2019), file
`Easy Animated Enemy Pack - Jan 2019.zip`, SHA-256
`a97f38b981fec2f42b263fe92828a7bf73f9da1228d5aac906fe354cd2b21004`.
License: CC0 1.0 (public domain), as published by Quaternius.

Script: `scripts/blender/enemy_pack.py`, which imports each FBX through
`convert_fbx.py` (making the pack's alpha-0 materials opaque), scales it,
renames its clips to lowercase, and decimates it to its triangle budget.

| File | Source FBX | Size | Triangles | Clips |
| --- | --- | --- | --- | --- |
| `giant_spider.glb` | `Spider.fbx` | 2 m across the legs | 2,712 | idle, walk, attack, jump, death |
| `rat.glb` | `Rat.fbx` (decimated from 4,004) | 0.45 m long | 2,352 | idle, walk, run, attack, jump, death |
| `frog.glb` | `Frog.fbx` (decimated from 4,920) | 0.22 m | 2,352 | idle, jump, attack, death |
| `snake.glb` | `Snake.fbx` | 0.6 m, reared | 1,618 | idle, walk, attack, jump |
| `wasp.glb` | `Wasp.fbx` (decimated from 3,736) | 0.3 m | 2,350 | fly, attack, death |

## Generated models

These have no source asset; each is built from primitives by its script.
The observatory and the bandshell sample the village kit's
`T_Brick_BaseColor.png` (Quaternius, Medieval Village MegaKit, CC0 1.0; see
`assets/verse/everglade/village/`), downscaled to 256 pixels and embedded.

| File | Script | Triangles | Clips |
| --- | --- | --- | --- |
| `sledgehammer.glb` | `sledgehammer.py` | 928 | |
| `fountain.glb` | `fountain.py` | 2,460 | |
| `observatory.glb` | `observatory.py` | 2,888 | |
| `bandshell.glb` | `bandshell.py` | 1,380 | |
| `market_stall_red.glb`, `market_stall_blue.glb` | `market_stall.py` | 1,916 each | |
| `training_dummy.glb` | `training_dummy.py` | 936 | |
| `training_dummy_armored.glb` | `training_dummy.py` | 2,128 | |
| `training_dummy_warded.glb` | `training_dummy.py` | 1,860 | |
| `bear.glb` | `animals.py` | 1,254 | idle, walk |
| `wolf.glb` | `animals.py` | 1,242 | idle, walk |
| `eagle.glb` | `animals.py` | 854 | idle, flap |

The sledgehammer's origin is the handle's butt, with the handle along +Y, so
a hand can hold it. The fountain's water is the separate `Fountain_Water`
and `Fountain_Spill` materials, and the warded dummy's runes are the emissive
`Dummy_Rune` material.
