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
giant spider, the bear, the wolf, the eagle, and Shapechange's dragon are
admitted as the pack's forms by `scripts/blender/beasts_admit.py`
([`assets/verse/everglade/beasts/`](../everglade/beasts/README.md)). The
training dummies and the sledgehammer are not admitted. Everglade's ambient
wildlife (`wildlife/`) is admitted as forms the same way.

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
| `market_stall_red.glb`, `market_stall_blue.glb`, `market_stall_green.glb`, `market_stall_gold.glb` | `market_stall.py` | 1,916 each | Everglade admits the red stall alone, as `generated/market_stall`, and paints each placed stall's awning red, blue, or gold (`layout::paint`) |
| `training_dummy.glb` | `training_dummy.py` | 936 | |
| `training_dummy_armored.glb` | `training_dummy.py` | 2,128 | |
| `training_dummy_warded.glb` | `training_dummy.py` | 1,860 | |
| `bear.glb` | `animals.py` | 1,254 | idle, walk |
| `wolf.glb` | `animals.py` | 1,242 | idle, walk |
| `eagle.glb` | `animals.py` | 854 | idle, flap |
| `dragon.glb` | `dragon.py` | 1,924 | idle, walk, fly, glide, bite, breath, roar, sweep |

The dragon is a Reference model: built from primitives in the style of the
Quaternius kits the pack carries (chunky low-poly solids, flat-shaded facets
for scales, a few flat colors), with nothing taken from any kit. Its body,
neck, and tail are one lofted tube skinned smoothly along the spine; its
head, legs, horns, spikes, and wings are weighted to one bone each. It
stands about 5 m to the top of its head and spans about 11 m across its
wings, at the size the Grove draws it. Rebuild it with
`Blender -b --factory-startup --python scripts/blender/dragon.py`, and add
`-- OUT_DIR --preview DIR` to render each clip for review.

The sledgehammer's origin is the handle's butt, with the handle along +Y, so
a hand can hold it. The fountain's water is the separate `Fountain_Water`
and `Fountain_Spill` materials, and the warded dummy's runes are the emissive
`Dummy_Rune` material.

## Wildlife

`scripts/blender/wildlife.py` builds Everglade's ambient creatures into
`wildlife/`. The songbird, the duck, and the cat are generated from
primitives on simple rigs, every solid weighted to one bone. The rat, frog,
snake, and wasp are lighter copies of the Easy Animated Enemy Pack's, made
by `enemy_pack.py`'s `convert` with smaller triangle budgets and only the
clips the town plays.

| File | Triangles | Clips |
| --- | ---: | --- |
| `songbird.glb` | 188 | idle, flap |
| `duck.glb` | 330 | idle, walk (paddling) |
| `cat.glb` | 448 | idle |
| `rat.glb` | 686 (from 4,004) | idle, walk, run |
| `frog.glb` | 686 (from 4,920) | idle, jump |
| `snake.glb` | 686 (from 1,618) | idle, walk |
| `wasp.glb` | 488 (from 3,736) | fly |

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
| `flower_patch_spring.glb`, `flower_patch_summer.glb`, `flower_patch_autumn.glb` | 440 each | Seasonal flower drifts on the commons, the town's lawns, and the woods' edges |
| `lantern_string.glb` | 552 | Paper lanterns across Lantern Road |
| `lamp_double.glb` | 176 | Two-armed lamps on Well Square and in the Lantern Quarter |
| `shop_sign.glb` | 168 | Signs before Main Street's shops |
| `park_bench.glb` | 156 | Well Square, Lantern Pond, and the commons walk |
| `fruit_tree_bloom.glb` | 356 | Orchards in spring blossom |
| `cafe_umbrella.glb` | 160 | Café parasols by Lantern Pond |
| `produce_stall.glb` | 372 | A trestle of produce crates under a striped canvas, on the Fountain Plaza |
| `crate_stack.glb` | 284 | Crates, a sack, and a keg by the plaza's stalls and the shops |
| `street_bin.glb` | 128 | Litter bins along Main Street, Library Way, Brownstone Row, and before the homes |
| `water_pump.glb` | 220 | Hand pumps over stone troughs on the plaza, Market Row, and Stoop Lane |
| `flower_cart.glb` | 712 | Hand carts of potted flowers on the plaza and before the shops |
| `fountain_small.glb` | 240 | Small fountains by Market Row and on the lawns south of the commons |
| `boardwalk.glb` | 340 | Plank decks with a rail on Lantern Pond's banks, for café tables |
| `thicket.glb` | 268 | Bushes round two saplings, in the wild ground between the town and the woods and the gaps between buildings |
| `young_trees.glb` | 156 | A young birch, spruce, and oak, in the wild patches and along the lanes toward the woods |
| `copse.glb` | 256 | Six young trees, two each of birch, spruce, and oak, most of the wild patches |
| `footpath.glb` | 60 | Three meters of path worn into the turf, for the trails from the town's edge toward the woods |

The sixth round's pieces (`produce_stall` to `boardwalk`) were added on
October 5, 2026, in Reference mode: built from primitives in the same
shared flat-color materials, with nothing from a kit. The seventh round's
(`thicket`, `young_trees`, `copse`, and `footpath`) were added the same way the same
day.

## Kit pieces

`scripts/blender/kit_lod.py` imports the Medieval Village MegaKit's
`Roof_RoundTiles_8x10` (Quaternius, CC0 1.0) and thins it with collapse
decimation to `kit/roof_round_tiles_8x10.glb`, 2,464 triangles of the
original's 4,480. Its images are thumbnails that only name the kit's images,
which the admitted copy samples from the village set.

## Coast kit (C2)

Mode: **Reference**. `scripts/blender/coast_common.py` and the coastal
family scripts generate original geometry and material colors from
primitives, without downloaded kits, Fab content, or external textures.
The source scripts use meter units, ground-centered origins, and fronts
facing Blender -Y (glTF +Z). Per-model source records retain the Blender
version, triangle budget, output digest, and generation result. Models are
reviewed before admission to the separate coast pack.

### Coast C2 model inventory

All models below are original procedural Reference-mode assets by OpenAgents,
dedicated under CC0-1.0. External inputs: none. Blender 5.2.2 LTS.
The adjacent source records retain triangle budgets and source digests.
`coast_admit.py` preserves geometry, skin, and clips in the admitted sources.

| Model | Triangles | GLB SHA-256 |
| --- | ---: | --- |
| `beach/barrel` | 200 | `47b6bda0661bcef57d690391251017d891d409d5fac5894e788aff01687421ad` |
| `beach/beach_grass` | 88 | `99ef29d4c548ef38b067f6c11ae312d8d95a1afac5f6831f5ee2a688380e9bdf` |
| `beach/crate` | 60 | `3cc551b6c1c93e3b2b3fdf636ffff67178673a93f707495be8078575b88614f3` |
| `beach/driftwood` | 48 | `68f7f10a5f1e3759b05f515a9a46c8fdd780fcbd26ed6f60ddfdedfcad88c701` |
| `beach/dune_fence` | 108 | `884316a1e2035b319fae045c742db1432eb4317e70e9a97636bfbfe0391731c7` |
| `beach/net` | 112 | `eb701af5c77affcd8d9cbe3913755b9e13245ed6f3773a897a273480cca4f5a8` |
| `beach/rope` | 216 | `115cbe1b48cd55c5e9580ffa1eafe5a5b3d457d0d72028ff594e89bf6d2c2621` |
| `beach/seaweed` | 88 | `0329cf8f16c8b26acbabb5e01df390ef38a8fdf368d8c6708dd3289215577185` |
| `beach/shell` | 108 | `d332b4d088202d31d4c7f60ff8ee152344b109ab3288dc85c4254b9ca4928875` |
| `boats/oars` | 128 | `8cfd2fe33e90cb040931953aa63292041084099fbef36e8220ef75f9f73fb52c` |
| `boats/sloop` | 521 | `56d612909d2b80a7b2931b3efc48a546e025b78a755a8b68a8f34fdddcfd332a` |
| `boats/wreck_bow` | 220 | `3d544287693b60973cebbff9b0f79133c75c7ce484d7de5b938a8667b763abd3` |
| `boats/wreck_stern` | 220 | `85d92e51a3f49178b61e0433cf631604f0249f119319c4a3bffa980842c49806` |
| `harbor/boathouse` | 146 | `2b648daddcc34e48908aa691d0eb7ab9399def63f938a242b27ba0e330975427` |
| `harbor/bollard` | 84 | `66b85a7faf6e5093a9b7bed4530f9202c72e8817be384f2ac71bdbf7e978eab7` |
| `harbor/breakwater` | 320 | `aa6b90caeea5b6afbc8ca504bcb05082fd3adf7b30e13aafea0aaa58de87bbb5` |
| `harbor/buoy` | 66 | `65704eb055b858cce8bd82b5a9b4e8223d91a7d1ca5e1a50ba7bc6d5e3e84ee4` |
| `harbor/hand_crane` | 196 | `7fa47686fcc0a3622a42846c898ff8ed7489c4b9a0954b9fedfb78fc81b6feb3` |
| `harbor/mooring` | 156 | `8afcc7e2ed523be510dd0e65f1093c663694a6cd6a0a7a83219667f6f54bf776` |
| `harbor/pier` | 328 | `d916fb1f67a7d3a906bc00d3725258f30f770673964e5fe65f5e48b6abec39b0` |
| `harbor/piling` | 36 | `0b9947a8493d65660f5707fcd14f1822f4a57b92edde5ae8807cc5db95bec46f` |
| `lighthouse/fog_bell` | 284 | `11760eea1f50768826d866da500738d5b1764999a05a6ac7473bb7571a9eef76` |
| `lighthouse/keeper_cottage` | 146 | `f6f174d21202ac5a2b31027f7682dbd4ffb198acbe2e28ae9ded6e138a6731be` |
| `lighthouse/lighthouse` | 1394 | `1b66464d3f40a0f1b9b123f1e9782d4e344f3b5d321dfe944e322f2973002d64` |
| `lod/beach.barrel.lod1` | 108 | `f99ad696425f40d2394d5c1d5ffc0e44755e3a2778e14df49534017ef9b1676b` |
| `lod/beach.barrel.lod2` | 64 | `2c3d27536067c33dcaf7555542fa109fb07ca544f90f8b60604a637820cd8d95` |
| `lod/beach.beach_grass.lod1` | 88 | `7be8c6115dab8376e28358c3e6e3dc534779ae6c984f97f8a15373fb6d926d14` |
| `lod/beach.beach_grass.lod2` | 88 | `7be8c6115dab8376e28358c3e6e3dc534779ae6c984f97f8a15373fb6d926d14` |
| `lod/beach.crate.lod1` | 60 | `3cc551b6c1c93e3b2b3fdf636ffff67178673a93f707495be8078575b88614f3` |
| `lod/beach.crate.lod2` | 60 | `3cc551b6c1c93e3b2b3fdf636ffff67178673a93f707495be8078575b88614f3` |
| `lod/beach.driftwood.lod1` | 24 | `1b87d2080b6be5a11d3077ac335aa540e14b462843df042279fcf097d6ec1350` |
| `lod/beach.driftwood.lod2` | 10 | `52096615459cf9fb42e5febea4efb76f49db456d0a9e1702c2ac5984d487a976` |
| `lod/beach.dune_fence.lod1` | 108 | `884316a1e2035b319fae045c742db1432eb4317e70e9a97636bfbfe0391731c7` |
| `lod/beach.dune_fence.lod2` | 108 | `884316a1e2035b319fae045c742db1432eb4317e70e9a97636bfbfe0391731c7` |
| `lod/beach.net.lod1` | 112 | `eb701af5c77affcd8d9cbe3913755b9e13245ed6f3773a897a273480cca4f5a8` |
| `lod/beach.net.lod2` | 112 | `eb701af5c77affcd8d9cbe3913755b9e13245ed6f3773a897a273480cca4f5a8` |
| `lod/beach.rope.lod1` | 114 | `f208517a642aca2ebe662eabf7ccf880d7d6ee471bb43e85916107a5c8a5f106` |
| `lod/beach.rope.lod2` | 70 | `7a2a8f06cff5c640cae8fbbc1d19024d952a2f8f87af4887c98de93139a237c2` |
| `lod/beach.seaweed.lod1` | 88 | `0329cf8f16c8b26acbabb5e01df390ef38a8fdf368d8c6708dd3289215577185` |
| `lod/beach.seaweed.lod2` | 88 | `0329cf8f16c8b26acbabb5e01df390ef38a8fdf368d8c6708dd3289215577185` |
| `lod/beach.shell.lod1` | 108 | `d332b4d088202d31d4c7f60ff8ee152344b109ab3288dc85c4254b9ca4928875` |
| `lod/beach.shell.lod2` | 108 | `d332b4d088202d31d4c7f60ff8ee152344b109ab3288dc85c4254b9ca4928875` |
| `lod/boats.oars.lod1` | 48 | `a5b0f843e34dbd794559886dd30ba595d048e78fa77ff7308fa038b7df495d5e` |
| `lod/boats.oars.lod2` | 20 | `0d4fc6e808a2d18a3d01c0941070628613c56a64886147a8b7200ae9bd71def7` |
| `lod/boats.sloop.lod1` | 315 | `90757eb77ffc7761bcee34597d8a516722d10d593bbe28f672596f585a3370c3` |
| `lod/boats.sloop.lod2` | 189 | `f682b9fd926639101c52f55afe3407e118d344d82474dd6b32e684f47375d86d` |
| `lod/boats.wreck_bow.lod1` | 124 | `ad387b19341b66f3a5f5a155c665db297708435f691f1d2bdfd603bcf1f8373e` |
| `lod/boats.wreck_bow.lod2` | 66 | `2de487479a1dad210b35b090fb26df5260796c38aeb5ccbbb6e33f0636e1221b` |
| `lod/boats.wreck_stern.lod1` | 124 | `854e57540e7c74e47222f1ea45953c800b4ae7e5fa2b39ac59bfcdcc1d79062c` |
| `lod/boats.wreck_stern.lod2` | 66 | `ec8af8ed7fca557ee86ec51eb4ed58b01ed0824f5484c3daf35b1628588fe435` |
| `lod/harbor.boathouse.lod1` | 146 | `2b648daddcc34e48908aa691d0eb7ab9399def63f938a242b27ba0e330975427` |
| `lod/harbor.boathouse.lod2` | 146 | `2b648daddcc34e48908aa691d0eb7ab9399def63f938a242b27ba0e330975427` |
| `lod/harbor.bollard.lod1` | 42 | `8da8d3fcc4bc20bad2b88bb6f84d047b309c551ac14924945df88147addc4e5c` |
| `lod/harbor.bollard.lod2` | 18 | `63d3f2b82c92b14b94ca377f9f91fed58ffda6f02cb252fa53a85a28f57e9111` |
| `lod/harbor.breakwater.lod1` | 176 | `02d04483f84202fbf517e48bee1c925942f2b77f84fffcda78698ffcc3e4fd07` |
| `lod/harbor.breakwater.lod2` | 80 | `7e733cfd508a75f28e5f59255a61211cdd53fe7d938b43e5433cf1be40acd88e` |
| `lod/harbor.buoy.lod1` | 38 | `10d119729e0ddde65c1d256ee08839ac3e3f2e7666fc8905870c179e8d884635` |
| `lod/harbor.buoy.lod2` | 22 | `09649f2b3cfc9031c1f1bff1ba1e6370151d929807e0bf4e04cf5edc1d56adc3` |
| `lod/harbor.hand_crane.lod1` | 108 | `5d99f2f7781f03c5496e6cfa1c5569b981c44630832a554a088d7de2189ebaa5` |
| `lod/harbor.hand_crane.lod2` | 54 | `6871ff0efd1fc8bf72cd3624b38bf7a3faaacbb7a807b61f652016431ab658f2` |
| `lod/harbor.mooring.lod1` | 80 | `03c5cd8532a48c681d0e75aef17adbc0ee049671a6ecdc195bd217277953e27b` |
| `lod/harbor.mooring.lod2` | 44 | `6e24369f90acd3c83da9be94ae9c4411454d1d6879ba914248c6f88e44758590` |
| `lod/harbor.pier.lod1` | 272 | `41cf022e7e6ec347abc7cb5053920cc3ba1fa0ffb79f624cefa70efb7304a5c9` |
| `lod/harbor.pier.lod2` | 240 | `56c08d53e488e8efec4f23a645a5f126a79391a4f5176bcdd5d14b04f32808ff` |
| `lod/harbor.piling.lod1` | 18 | `4145ba64e5b9194550e7cbff9d296e76266fff229f19335683f2d8a23f43eca8` |
| `lod/harbor.piling.lod2` | 8 | `2c87e8916ce1d70774556660aa53fc962828b5bdb1cbc70ae5197d7d2c8b365c` |
| `lod/lighthouse.fog_bell.lod1` | 156 | `4c81ee970ab01cd771bfc9bb959b14be0a05af20e4d567171d8a1b65a7e112da` |
| `lod/lighthouse.fog_bell.lod2` | 76 | `2e005406e9a59f61cac5c88fef8cf5d58881d5998cb0748002c5abd6624804cb` |
| `lod/lighthouse.keeper_cottage.lod1` | 146 | `f6f174d21202ac5a2b31027f7682dbd4ffb198acbe2e28ae9ded6e138a6731be` |
| `lod/lighthouse.keeper_cottage.lod2` | 146 | `f6f174d21202ac5a2b31027f7682dbd4ffb198acbe2e28ae9ded6e138a6731be` |
| `lod/lighthouse.lighthouse.lod1` | 874 | `708b523c24469e669d81c1e85eed95c55824b1530a3ae6e652b0ce9c42d9edde` |
| `lod/lighthouse.lighthouse.lod2` | 564 | `3341cf16c9ec7a37269685a456f3c55476abea4e82782696a2163ffcbaccde8b` |
| `lod/reef.anemone.lod1` | 158 | `8a6b2b62944c91567cf172eecf4c9be50f0d6113a5c13a937f9d04e921b9dc2b` |
| `lod/reef.anemone.lod2` | 150 | `21ba794a816eb7a3997a54d9e7c08e3ef47cdaa6088c96e0d247254c1f06481b` |
| `lod/reef.kelp.lod1` | 13 | `ab26e8155ed82682fd8ec1e76a16d0a0c98257f567977b5b02564bc46a7367df` |
| `lod/reef.kelp.lod2` | 6 | `52180d3f4677be4b6c6522d2d824840878a119c768cb890f66b788cdcc0487b7` |
| `lod/reef.reef_cluster.lod1` | 30 | `f82246c9aea59d75e12cb312955a7b4ca8260f8f6e138b67a650f76a7714b432` |
| `lod/reef.reef_cluster.lod2` | 12 | `a5727ec0c73b9974880579f47c7afe5a25e1a6ac8c2f298873d19ad6711dfb03` |
| `lod/rocks.boulder.lod1` | 176 | `bfc55c16a91fafca9ad31b9830e671bda645df95660f8cbf6f4a8c1b539f79df` |
| `lod/rocks.boulder.lod2` | 80 | `6492160573c29253c018a6b5ce1b6d9bf885c9c998bf6ee9802e5c5cc4ea136c` |
| `lod/rocks.cliff_corner.lod1` | 220 | `269aeeb7cc4ee73220586f7c36d9565dd7da80cddc714b8bfb09afa7fb9aa103` |
| `lod/rocks.cliff_corner.lod2` | 100 | `2359c11d28b6861de33d208c5b9183e4ede9f01a0738a3f7340ce121979fb0bb` |
| `lod/rocks.cliff_inlet.lod1` | 308 | `6f536d8fa01d0d24d515fe978f00e7a18e183f091ccf7d19eb15608f24267bab` |
| `lod/rocks.cliff_inlet.lod2` | 140 | `8bff0cd760bfa807fe8019370a970b8e493741617555cf0a9c683e017bc41c80` |
| `lod/rocks.cliff_straight.lod1` | 220 | `128d9475dd0443575ad4de8a6ab4f91ec45790e878aa3cd44ea37742a90ff01c` |
| `lod/rocks.cliff_straight.lod2` | 100 | `86aa65a211339428bdb167de56d235a783dd9c79dce50665bb177a2c6ccd8415` |
| `lod/rocks.reef_rock.lod1` | 44 | `42051fd180c4b4b064670ec7b1893b15923b5346aa40c7bbd1e62402ec8c7b47` |
| `lod/rocks.reef_rock.lod2` | 20 | `1815dd25b1ec9af44c9e5c8c76ff758cde45ca6d044fc6996524a983b1e92644` |
| `lod/rocks.sea_arch.lod1` | 308 | `2ea43e07e393e800845c1bbb7c3cd5133c6a3e695bdf572a5f8c2dbd31a72360` |
| `lod/rocks.sea_arch.lod2` | 140 | `267f8417cba82dbeb4264af65ac78d789fd68662b23865735588a88ae9be8de4` |
| `lod/rocks.sea_cave.lod1` | 308 | `2987a8a98ee8e6a256e39e60c1ab55cd90eea6730110c2f18217b482dca32baa` |
| `lod/rocks.sea_cave.lod2` | 140 | `53b39bf86ea1a3c30cf249a89af8246a938d0fd84a5c3027c01ca4ab7fdd1212` |
| `lod/rocks.sea_stack.lod1` | 176 | `2a2d02f2c7b387f92101f6f47b0e34bebdfdbe13a8ba661be2fe28cd4c8e6989` |
| `lod/rocks.sea_stack.lod2` | 80 | `b1859591c6f9827be74283bc97ec1000d5aea1f1a5e088cdab0922bf8ec38a00` |
| `lod/rocks.tide_pool_shelf.lod1` | 1584 | `e674a80d0d71101af77f6613bc9526e6072c8647225b3059965f288a42e18996` |
| `lod/rocks.tide_pool_shelf.lod2` | 720 | `1f31351b91f28e76a904037aaf0dbf80741958a1ab7bccc424c8c8058059ce9a` |
| `reef/anemone` | 172 | `f2941892f2036016afaae199fbe0de8e3f0815fb1857d42bc6596e73abe5ec6b` |
| `reef/kelp` | 24 | `5363d5d8a3aa6a95c74debbb7f83b0281826aab0923a88672278d02a1d21e7d6` |
| `reef/reef_cluster` | 60 | `72bc92dd02718ca7b97c012c7500b05bfb200a9269099b18ae8662eca3d38447` |
| `rocks/boulder` | 320 | `a315306df3e68a7d7f080df67deb51bababde9c80092d1845fed9dd4c799cf6f` |
| `rocks/cliff_corner` | 400 | `c65440705455583776b93b04b493333af66c0efed0640b8a9faf49d62c9f4c4d` |
| `rocks/cliff_inlet` | 560 | `1d8311d952e29ffeca4a50b41dfaf75bdb4bf2e064034cbe9a4eefbfd074768b` |
| `rocks/cliff_straight` | 400 | `c1f2dec4db350ec132a9d9167a2aec8e88eee17c1efa9a16d2121baed6bd0aeb` |
| `rocks/reef_rock` | 80 | `2539d93052bc2d69fc96edbf48625cd991b65783a230762e9cd3c7b18b561170` |
| `rocks/sea_arch` | 560 | `533df3aa15725dd3a18346fe1b0853b9743de1149e6c465d9b761ffacbea5eb1` |
| `rocks/sea_cave` | 560 | `a5470deb09ae2cebd5852315774a61f180c96374a98b8952cb2dbe034f46e8c4` |
| `rocks/sea_stack` | 320 | `b34963dd2c92ce8b704b7534ad10289bc3eb7db07a1bfd4d3f4e04bfbb6d750c` |
| `rocks/tide_pool_shelf` | 2880 | `0de5dd2457bd8d5849828768f11540c88bcd15c4ba39c26e22d6f4dc735916de` |
| `wildlife/crab` | 216 | `0ba0c6d2ab11849825933989eec6e28f9045ea418f0125d9fb54ac491f6534a5` |
| `wildlife/fish` | 72 | `d981253891148ab399f6cb812a9e81bb3664274aa57b3c270cd6ef59acd3af37` |
| `wildlife/gull` | 236 | `cdd2d29f0e2ac6e3cf1f0414fe1e110fceb80e825b2687f2370a24c690e344fe` |
| `wildlife/seal` | 272 | `1f7184c521e217b2b731549d759811398fc62ef975c9a800203f339c05916a11` |

The original `fx/surf_spray` and `fx/sea_mist` sheets each contain 16 frames,
with four columns of 64-pixel cells. Their adjacent JSON files retain digests
and premultiplied color encoding. They are generated on the CPU; no bake runs.
