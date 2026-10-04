# Medieval Village MegaKit subset

This subset comes from Quaternius's Medieval Village MegaKit (Standard),
downloaded by the owner from <https://quaternius.com>. The kit's
`License_Standard.txt` is kept here as `license.txt` and declares CC0 1.0.
`manifest.json` pins the creator, license, package, and the SHA-256 of every
admitted file, both as it shipped in the kit (`originals`) and as committed
(`files`), with the transform for each changed file.

The Everglade pack compiler (`verse::zones::everglade_pack::compile`) reads
these files, refuses any byte that differs from the manifest, and builds the
pinned zone pack. The glTF files still name the kit's normal, ORM, and
roughness images; the compiler ignores those references, and the images are
not admitted, because the zone renderer samples base color only.

## Admitted models

The pieces build one timber-and-plaster workshop on the kit's 2 m grid, with
its yard.

| Models | Use in the workshop |
| --- | --- |
| `Wall_Plaster_Straight`, `Wall_Plaster_Straight_Base`, `Wall_Plaster_WoodGrid`, `Wall_BottomCover` | Hall walls. |
| `Wall_Plaster_Window_Wide_Round`, `Wall_Plaster_Window_Wide_Flat`, `Window_Wide_Round1`, `Window_Wide_Flat1`, `WindowShutters_Wide_Round_Open` | Wide windows. The windows carry the kit's alpha-blended glass material. |
| `Wall_Plaster_Door_Round`, `DoorFrame_Round_WoodDark`, `Door_4_Round` | The entrance; two leaves make the double doors. |
| `Wall_Arch` | The opening to the hall gallery. |
| `Corner_Exterior_Wood`, `Corner_Interior_Big` | Wall corners. |
| `Floor_WoodDark`, `Floor_Brick` | The hall floor and the hearth corner. |
| `Roof_RoundTiles_8x10`, `Roof_RoundTiles_6x8`, `Roof_Front_Brick8`, `Roof_Support2` | The round-tile roof, its gable, and supports. |
| `Prop_Chimney` | The hearth chimney. |
| `Stairs_Exterior_Straight`, `Stair_Interior_Solid` | The porch step and the gallery stair. |
| `Prop_WoodenFence_Single`, `Prop_WoodenFence_Extension1` | The yard fence and the notice board frame. |
| `Prop_MetalFence_Simple`, `Prop_MetalFence_Ornament` | The strongroom. |
| `Prop_Wagon`, `Prop_Crate` | The wagon by the gate. |
| `Prop_Vine1`, `Prop_Vine5`, `Prop_Support`, `Prop_ExteriorBorder_Straight1`, `Prop_ExteriorBorder_Corner` | Dressing for walls and the building's base. |

## Admitted textures

The base-color images those models use: `T_Plaster_BaseColor`,
`T_WoodTrim_BaseColor`, `T_RoundTiles_BaseColor`, `T_Brick_BaseColor`,
`T_RockTrim_BaseColor`, `T_MetalOrnaments_BaseColor`, and `T_VineLeaf_png`.

Plaster, wood trim, and roof tiles keep 1,024 pixels on their long edge; the
rest are at most 512 pixels, as the compiler's texture policy requires. Images
larger than their edge were downscaled with macOS `sips -Z`, as `transforms`
records, to keep the committed sources and pack within 30 MB. The uneven
brick and red brick pieces are left out so their textures are not needed.
