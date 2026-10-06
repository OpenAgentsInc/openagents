# Foliage

Every model here is a **Reference**-mode model
([asset runbook](../../../../docs/verse/asset-runbook.md)): built from
primitives by `scripts/blender/foliage.py`, run headless in Blender 5.2.2 LTS:

```sh
Blender -b --factory-startup --python scripts/blender/foliage.py [-- OUT_DIR NAME...]
```

Nothing from a kit's models ships. The models sample only images already
admitted in Everglade's nature set (`assets/verse/everglade/nature/`): the
bark (`Bark_NormalTree.png`), the broadleaf cluster
(`Leaves_NormalTree_C.png`), the leaf atlas (`Leaves.png`: fern frond,
clover), the fir branch (`Leaf_Pine_C.png`), the flowers (`Flowers.png`),
the grass strip (`Grass.png`), and the rocks (`Rocks_Diffuse.png`). Those
images are from the Stylized Nature MegaKit (Standard) by Quaternius
(<https://quaternius.com>), downloaded by the owner to
`~/Downloads/Stylized Nature MegaKit[Standard]/`, licensed CC0 1.0; the
admitted `license.txt` has SHA-256
`120710e542c3ebaf83856c4eb55b4a1e680593b39302eba7fdc5d942dfe78c58`. The glb
files carry 16 px stand-ins of those images, which
`scripts/blender/foliage_admit.py` replaces with the admitted files when it
writes the pack's `foliage` set
([`assets/verse/everglade/foliage/`](../../everglade/foliage/README.md)).

The kinds of objects, and how they group in a wood, were chosen by studying a
local list of a commercial game client's woodland file names: which kinds of
canopy, roots, stumps, logs, bushes, rocks, and clearing props a forest
holds. Only the file names were read. No mesh, texture, or image of that
client was opened, and nothing here is derived from one or named after one.

All models use 1 unit = 1 m and glTF's +Y up, with the origin at the center
of the base and the front facing glTF +Z. The wall-hung pieces (`ivy_wall`,
`window_box`, `rose_trellis`) have a kit wall piece's origin instead: on the
wall's center line at its base, with the wall's face 0.09 m toward +Z.

| Model | Triangles | What it is |
| --- | ---: | --- |
| `oak_forked` | 720 | Broadleaf forking low into two leaders, each with its crown |
| `beech_tall` | 636 | Tall straight broadleaf, high oval crown over a clear bole |
| `linden_broad` | 854 | Short trunk under a wide, low dome of a crown |
| `willow_weeping` | 396 | Pond-side willow with curtains of leaves |
| `oak_old` | 1,006 | Gnarled old oak: massive bole, crooked limbs, lumpy crown |
| `fir` | 504 | Tall fir: tiers of drooping needled branches |
| `snag` | 252 | Leafless dead tree with a broken top |
| `roots_spread` | 144 | Exposed roots at a trunk's foot |
| `root_arch` | 530 | Old roots arching out of a mossy bank |
| `stump_mossy` | 305 | Wide stump with moss, root feet, and bracket fungi |
| `stump_broken` | 188 | Tall snapped stump with a splintered top |
| `log_hollow` | 156 | Fallen hollow log with moss along its back |
| `log_broken` | 124 | Fallen trunk broken in two |
| `shrub_mound`, `shrub_tall`, `shrub_flowering` | 126, 84, 124 | Low mound, upright oval, and flowering shrubs |
| `bramble` | 326 | Sprawling bramble with canes and red berries |
| `fern_clump` | 78 | A dozen arching fern fronds |
| `grass_tall` | 84 | Tuft of tall meadow grass |
| `wildflower_clump` | 98 | Grass with wildflowers on stems |
| `mushroom_ring` | 664 | Fairy ring of caps |
| `ivy_wall` | 250 | Ivy climbing a wall, 2.6 m wide and up to 3 m high |
| `ivy_low` | 92 | Ivy over a low wall or fence |
| `rose_trellis` | 304 | Timber trellis with climbing roses |
| `hedge_long` | 320 | Clipped hedge, 4 m |
| `hedge_gate` | 702 | Hedges either side of a 2.6 m gateway under a clipped arch, gates open |
| `planter_overflow` | 118 | Timber planter overflowing with leaves and flowers |
| `window_box` | 72 | Window box with trailing stems |
| `standing_stone`, `standing_stone_squat` | 56, 80 | Weathered standing stones |
| `cliff_rock` | 480 | Craggy outcrop with ledges and moss |
| `boulder_cluster` | 356 | Boulders lying together with a fern |
| `campfire` | 386 | Fire in a stone ring under a teepee of logs, three log seats |
| `cascade` | 712 | Stream spilling over a weir of stones, with foam |
| `birch_trio` | 414 | The eighth round's: three slender birches from one root, pale bark with dark knots, small light crowns |
| `rowan` | 646 | The eighth round's: a rowan about 5 m tall, an open oval crown, and clusters of red berries |
| `shrub_hazel` | 244 | The eighth round's: a hazel's fan of thin stems, leafy above head height, with catkins |
| `shrub_elder` | 172 | The eighth round's: an elder in flower, with flat cream umbels over its leaves |
| `fern_bank` | 156 | The eighth round's: a 3 m drift of three fern clumps among grass |

`scripts/blender/everglade_lod.py` makes far levels of detail for the five
broadleaf trees, the birch trio, and the rowan (`lod/foliage.*`). `scripts/blender/foliage_views.py` renders
any admitted model beside a 1.8 m figure for review.
