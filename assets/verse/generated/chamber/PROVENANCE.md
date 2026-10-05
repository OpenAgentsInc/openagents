# Crypt lab models

Mode: **Reference**. These models are original solids built from
primitives. A private 1.12.1 Scholomance laboratory view was studied
for the kinds of objects in the room and where they sit. No mesh,
texture, font, or UI file from that view is in this folder, and the
models are not copies of those silhouettes.

Script: `scripts/blender/chamber_lab.py`

Command:

```sh
Blender -b --factory-startup --python scripts/blender/chamber_lab.py -- \
    assets/verse/generated/chamber
```

Blender 5.2.2 LTS.

The hall is an open ruin. A closed ceiling would leave the floor in
shadow under the zone renderer's directional light.

| Model | Triangles |
| --- | --- |
| `crypt_hall.glb` | 1376 |
| `slab_table.glb` | 104 |
| `cauldron_green.glb` | 392 |
| `cauldron_red.glb` | 392 |
| `cauldron_amber.glb` | 392 |
| `candelabrum_tall.glb` | 776 |
| `candelabrum_short.glb` | 264 |
| `floor_candles.glb` | 576 |
| `ritual_rug.glb` | 24 |
| `specimen_jar.glb` | 544 |
| `specimen_jar_bones.glb` | 708 |
| `alchemy_bench.glb` | 224 |
| `bone_scatter.glb` | 116 |
| `cobweb.glb` | 72 |
