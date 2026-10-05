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

## Textures

Each model packs its images into its glb as PNG.

From the admitted CC0 1.0 Quaternius kits (Credit: Quaternius),
downscaled to 256 pixels and tinted through the base color factor:

| Image | Kit | Source file | License file SHA-256 |
| --- | --- | --- | --- |
| `T_Brick_BaseColor.png` | Medieval Village MegaKit (Standard) | `assets/verse/everglade/village/T_Brick_BaseColor.png` | `ec6fd5004514cb0515a7dc1065f474644d31698861597b32e1745945ffec71de` |
| `T_RockTrim_BaseColor.png` | Medieval Village MegaKit (Standard) | `assets/verse/everglade/village/T_RockTrim_BaseColor.png` | `ec6fd5004514cb0515a7dc1065f474644d31698861597b32e1745945ffec71de` |

Authored in this script with NumPy from seeded value noise, so a
rebuild gives the same pixels: `T_Lab_Wood` (oak boards),
`T_Lab_Iron` (iron with rust), `T_Lab_Brass` (tarnished brass),
`T_Lab_Flag` (worn flagstone), `T_Lab_Effigy` (pale carved stone),
`T_Lab_Linen` (a shroud), `T_Lab_Parchment`, `T_Lab_Page` (a written
page), `T_Lab_Label` (a jar label), `T_Lab_Bone`, and `T_Lab_Rug` (a
wool rug's field, border, and medallion).

## Light

The hall is closed. Flames, embers, glowing liquids, and the
crystal carry glTF emission in cd/m² (`KHR_materials_emissive_strength`),
and the capture places a point light at each source. One barred
window in the far gable lets in a shaft of moonlight.

| Model | Triangles |
| --- | --- |
| `crypt_hall.glb` | 10708 |
| `slab_table.glb` | 2242 |
| `cauldron_green.glb` | 3210 |
| `cauldron_red.glb` | 3210 |
| `cauldron_amber.glb` | 3210 |
| `candelabrum_tall.glb` | 2876 |
| `candelabrum_short.glb` | 1066 |
| `floor_candles.glb` | 2992 |
| `ritual_rug.glb` | 708 |
| `specimen_jar.glb` | 996 |
| `specimen_jar_bones.glb` | 1228 |
| `alchemy_bench.glb` | 3112 |
| `bone_scatter.glb` | 4646 |
| `cobweb.glb` | 851 |
| `bookshelf.glb` | 3218 |
| `jar_shelf.glb` | 3502 |
| `writing_desk.glb` | 1830 |
| `lectern.glb` | 694 |
| `chained_skeleton.glb` | 4328 |
| `hanging_chains.glb` | 4436 |
| `brazier.glb` | 1810 |
| `crate.glb` | 264 |
| `barrel.glb` | 912 |
| `iron_cage.glb` | 2332 |
| `sarcophagus.glb` | 2204 |
