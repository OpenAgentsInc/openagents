# Stylized Nature MegaKit subset

This subset comes from Quaternius's Stylized Nature MegaKit (Standard),
downloaded by the owner from <https://quaternius.com>. The kit's
`License_Standard.txt` is kept here as `license.txt` and declares CC0 1.0.
`manifest.json` pins the creator, license, package, and the SHA-256 of every
admitted file, both as it shipped in the kit (`originals`) and as committed
(`files`), with the transform for each changed file.

The Everglade pack compiler (`verse::zones::everglade_pack::compile`) reads
these files, refuses any byte that differs from the manifest, and builds the
pinned zone pack. Only base-color textures are admitted, because the zone
renderer samples base color only.

## Admitted models

| Models | Use in the glade |
| --- | --- |
| `CommonTree_1`, `CommonTree_3` to `CommonTree_5`, `Pine_1`, `Pine_2` | The tree ring, varied by rotation and scale. `CommonTree_2` and `Pine_3` are left out to stay within the 30 MB budget. The twisted and dead trees are left out: the twisted trees cost about 10,000 triangles each and need their own 4.6 MB bark texture. |
| `Bush_Common`, `Bush_Common_Flowers`, `Fern_1`, `Plant_1`, `Plant_1_Big`, `Plant_7`, `Plant_7_Big` | Undergrowth at the clearing's edge and along the approach path. |
| `Grass_Common_Short`, `Grass_Common_Tall`, `Grass_Wispy_Short`, `Clover_1`, `Flower_3_Group`, `Flower_4_Group` | Ground cover in the clearing. |
| `Mushroom_Common`, `Mushroom_Laetiporus` | The lounge under the trees. |
| `Rock_Medium_1` to `Rock_Medium_3`, `Pebble_Round_1` to `Pebble_Round_3` | The proving-ground border and scattered stones. |
| `RockPath_Round_Small_1` to `RockPath_Round_Small_3`, `RockPath_Round_Thin`, `RockPath_Round_Wide` | The stepping-stone approach path. |

## Admitted textures

The base-color images those models use: `Bark_NormalTree`,
`Leaves_NormalTree_C`, `Leaf_Pine_C`, `Leaves_TwistedTree_C` (bushes),
`Leaves`, `Flowers`, `Grass`, `Mushrooms`, `PathRocks_Diffuse`, and
`Rocks_Diffuse`.

`Bark_NormalTree` keeps 1,024 pixels on its long edge and
`Leaves_NormalTree_C` keeps its native 1,024; every other image is at most
512 pixels, as the compiler's texture policy requires. Images larger than
their edge were downscaled with macOS `sips -Z`, as `transforms` records, to
keep the committed sources and pack within 30 MB. Bark normal maps and the
unused desert rock, dead-tree, and twisted-tree images are not admitted.
