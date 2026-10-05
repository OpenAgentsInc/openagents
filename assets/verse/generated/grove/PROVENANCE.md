# Generated Grove props

The ten models in this folder were generated on October 5, 2026, by
`scripts/blender/grove_props.py` in Blender 5.2.2 LTS, run headless:

```sh
B=/Applications/Blender.app/Contents/MacOS/Blender
$B -b --factory-startup --python scripts/blender/grove_props.py -- \
    assets/verse/generated/grove
```

They're admitted into the Everglade pack's `generated` set by
`scripts/blender/everglade_admit.py`, which converts each `.glb` to
`assets/verse/everglade/generated/<name>.gltf` and `.bin` without touching
the geometry:

```sh
python3 scripts/blender/everglade_admit.py
python3 scripts/blender/tower_admit.py
```

The second command restores the concrete tower's entries, which the first
rewrites the manifest without.

## Mode and source

Mode: **Reference**. Every model is built from primitives in code, in the
flat-shaded low-poly style of `town_props.py`. No kit piece or kit image is
used, so no kit license applies; the models are CC0 1.0, like the rest of
the generated models. They carry no textures and no texture coordinates:
color comes from material base colors, which share `town_props.py`'s names
and values (`Prop_Stone`, `Prop_Moss`, `Prop_Bark`, and the rest), so the
pack keeps one copy of each. Randomness is seeded per model, so a rebuild
gives the same models.

Glowing materials are named with an `Emit` prefix (`EmitFlame`,
`EmitEmber`, `EmitRune`, and `EmitLantern`), and their base color is the
glow's color. The Everglade pack carries no emission, so Verse reads the
glow from the name. They also carry glTF emission, which admission drops.

## Models

| Model | Triangles | Blocks walking (footprint boxes) |
| --- | ---: | --- |
| `grove_standing_stone` | 310 | `menhir` |
| `grove_rune_stone` | 236 | `stone` |
| `grove_altar` | 764 | `altar` |
| `grove_oak` | 1,588 | `trunk` (the trunk only) |
| `grove_brazier` | 556 | `brazier` |
| `grove_torch` | 194 | `post` |
| `grove_campfire` | 642 | `fire` |
| `grove_archery_butt` | 476 | `butt` |
| `grove_training_ring` | 308 | `post0` to `post4`, `rails` |
| `grove_hanging_lantern` | 252 | none |

Each footprint is `<name>.footprint.json`, in glTF axes: +Y up, +Z front,
origin at the base center.
