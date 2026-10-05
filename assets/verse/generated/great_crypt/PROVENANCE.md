# Great crypt models

Mode: **Reference**. These models are original solids built from
primitives, a larger sibling of the crypt lab in
`assets/verse/generated/chamber/`. A private 1.12.1 Scholomance view
was studied for the kinds of spaces a cult's crypt holds: a vaulted
nave, aisles behind an arcade, side chapels, and a raised dais. No
mesh, texture, font, or UI file from that view is in this folder, and
the models are not copies of those silhouettes.

Script: `scripts/blender/great_crypt.py`, which reuses the materials
and helpers of `scripts/blender/chamber_lab.py`.

Command:

```sh
Blender -b --factory-startup --python scripts/blender/great_crypt.py -- \
    assets/verse/generated/great_crypt
```

Blender 5.2.2 LTS.

## Textures

The same images as the crypt lab: `T_Brick_BaseColor` and
`T_RockTrim_BaseColor` from the admitted CC0 1.0 Quaternius Medieval
Village MegaKit (Credit: Quaternius, `assets/verse/everglade/village/`),
downscaled to 256 pixels and tinted, and `T_Lab_Flag`, `T_Lab_Wood`,
and `T_Lab_Iron`, which `chamber_lab.py` authors with NumPy from
seeded value noise. Each model packs its images into its glb as PNG.

## Props

The crypt's furniture is the crypt lab's 24 props, reused and
multiplied by `verse_world::great_crypt::LAYOUT`.

## Collision

`great_crypt_hall.footprint.json` lists explicit boxes rather than
each part's bounds, so arches and chapel openings stay open.

| Model | Triangles |
| --- | --- |
| `great_crypt_hall.glb` | 21784 |
| `summoning_circle.glb` | 2240 |
| `broken_pillar.glb` | 532 |
| `rubble_pile.glb` | 276 |
