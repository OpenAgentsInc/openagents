# Generated and converted models

Every model here was written by Blender 5.2.2 LTS, run headless
(`Blender -b --factory-startup --python SCRIPT -- ARGS`), from a script under
`scripts/blender/`. The script is the model's source; rebuild them all with
`scripts/blender/build-models.sh`. Blender's glTF export isn't byte-identical
across versions, so compare rebuilds by their previews, not their bytes.

The fountain, the observatory, the bandshell, the market stalls, the
buildings, the street furniture, the town pieces, and the kit roof are
admitted into the
Everglade pack's `generated` set by `scripts/blender/everglade_admit.py`
([`assets/verse/everglade/generated/`](../everglade/generated/README.md)),
following [the pipeline](../../../docs/verse/blender-pipeline.md). The
giant spider, the bear, the wolf, and the eagle are admitted as the pack's
Wild Shape forms by `scripts/blender/beasts_admit.py`
([`assets/verse/everglade/beasts/`](../everglade/beasts/README.md)). The
rat, frog, snake, and wasp, the training dummies, and the sledgehammer are
not admitted.

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

## Street furniture

`scripts/blender/street_props.py` builds each piece from primitives in flat
colors and writes it to `street/`:

| File | Triangles | Use in Everglade |
| --- | ---: | --- |
| `lamp_post.glb` | 172 | Lamps along the streets and lanes |
| `barrel.glb` | 176 | Stock by the stalls and the tavern |
| `flower_box.glb` | 452 | Under the shop and townhouse fronts |
| `well.glb` | 284 | Wells on the greens |
| `stone_wall.glb` | 440 | The orchard's dry-stone wall, 2 m a piece |
| `hedge.glb` | 254 | Between Main Street and the commons, 2 m a piece |
| `hand_cart.glb` | 378 | The Fountain Plaza |
| `signpost.glb` | 64 | The crossings |
| `lily_pads.glb` | 268 | The ponds |
| `footbridge.glb` | 408 | Brownstone Row over Glade Run |
| `wildflowers.glb` | 364 | The meadows |
| `bunting.glb` | 240 | Over the plaza and the streets |
| `pine_low.glb` | 88 | The forest belt and the woods' stands |
| `oak_low.glb` | 184 | The forest belt and the woods' stands |

## Town, park, and woodland pieces

`scripts/blender/town_props.py` builds the second round of small pieces the
same way, in the street furniture's flat colors and materials, and writes
them to `town/`:

| File | Triangles | Use in Everglade |
| --- | ---: | --- |
| `statue.glb` | 540 | A bronze robed figure raising a lamp, on the Sculpture Walk |
| `sculpture.glb` | 328 | A bronze ring on a cairn, on the Sculpture Walk |
| `sundial.glb` | 296 | The Sculpture Walk |
| `planter.glb` | 442 | The Fountain Plaza, the clock tower, and the glasshouse |
| `garden_arch.glb` | 1,044 | The community garden's gate, under roses |
| `picket_fence.glb` | 222 | Round the community garden, 2 m a piece |
| `garden_gate.glb` | 404 | The orchard wall's gate |
| `flower_bed.glb` | 780 | The community garden |
| `veg_bed.glb` | 736 | The community garden and the farmhouse |
| `rail_fence.glb` | 48 | The farm's paddock, 2 m a piece |
| `haystack.glb` | 156 | The farm |
| `hay_bales.glb` | 324 | The farm |
| `beehives.glb` | 488 | The beekeeper's hut |
| `rowboat.glb` | 248 | Moored at the jetties and in the boathouse |
| `dock.glb` | 380 | Jetties on Lantern Pond, Reed Pond, and the Thinking Pond |
| `reeds.glb` | 232 | Round the ponds' banks |
| `fallen_log.glb` | 262 | Walden Woods and Fernhollow |
| `mossy_rock.glb` | 112 | Walden Woods and Fernhollow |
| `mushrooms.glb` | 524 | Walden Woods and Fernhollow |
| `stump.glb` | 148 | The woods and the woodcutter's cottage |
| `cafe_table.glb` | 296 | The Boardwalk Cafés' decks and the Fountain Plaza |
| `birch_low.glb` | 118 | Walden Woods, Fernhollow, and the forest belt |
| `poplar_low.glb` | 96 | The forest belt |
| `spruce_low.glb` | 112 | Walden Woods, Fernhollow, and the forest belt |
| `fruit_tree.glb` | 282 | The orchard |
| `bush_round.glb` | 126 | The woods and the clearing's edge |

## Kit pieces

`scripts/blender/kit_lod.py` imports the Medieval Village MegaKit's
`Roof_RoundTiles_8x10` (Quaternius, CC0 1.0) and thins it with collapse
decimation to `kit/roof_round_tiles_8x10.glb`, 2,464 triangles of the
original's 4,480. Its images are thumbnails that only name the kit's images,
which the admitted copy samples from the village set.
