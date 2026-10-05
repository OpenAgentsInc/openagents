# Generated village buildings

These buildings were generated on October 5, 2026, by
`scripts/blender/buildings.py` in Blender 5.2.2 LTS, run headless:

```sh
B=/Applications/Blender.app/Contents/MacOS/Blender
$B -b --factory-startup --python scripts/blender/buildings.py -- \
    assets/verse/generated/buildings [NAME ...] [--kit KIT_DIR]
```

They are admitted into the Everglade pack's `generated` set by
`scripts/blender/everglade_admit.py`
([`assets/verse/everglade/generated/`](../../everglade/generated/README.md)).

## Source

Every piece and texture comes from Quaternius's Medieval Village MegaKit
(Standard), downloaded by the owner from <https://quaternius.com> and read from
`~/Downloads/Medieval Village MegaKit[Standard]/glTF`. The kit's
`License_Standard.txt` (SHA-256
`ec6fd5004514cb0515a7dc1065f474644d31698861597b32e1745945ffec71de`) declares
CC0 1.0, so the generated models are CC0 1.0 too. Credit: Quaternius.

## What the script uses

Kit pieces, placed whole and sometimes stretched along a wall or roof:

- Walls: `Wall_Plaster_Straight`, `Wall_Plaster_Straight_Base`,
  `Wall_Plaster_Straight_L`, `Wall_Plaster_Straight_R`,
  `Wall_Plaster_WoodGrid`, `Wall_Plaster_Window_Wide_Round`,
  `Wall_Plaster_Window_Wide_Flat`, `Wall_Plaster_Window_Thin_Round`,
  `Wall_Plaster_Door_Round`, `Wall_Plaster_Door_Flat`, `Wall_Arch`, and the
  matching `Wall_UnevenBrick_*` pieces for stone ground floors.
- Openings: `Window_Wide_Round1`, `Window_Wide_Flat1`, `Window_Thin_Round1`,
  `WindowShutters_Wide_Round_Open`, `WindowShutters_Wide_Flat_Open`,
  `WindowShutters_Thin_Round_Open`, `Window_Roof_Wide`, `Door_1_Round`,
  `Door_8_Flat`, `DoorFrame_Round_WoodDark`, and `DoorFrame_Flat_WoodDark`.
- Frame and floors: `Corner_Exterior_Wood`, `Corner_ExteriorWide_Wood`,
  `Roof_Support2` (jetty joist ends), `Floor_WoodDark`, `Floor_WoodDark_Half3`,
  `Floor_UnevenBrick`, `Balcony_Cross_Straight`, and `Stairs_Exterior_Straight`.
- Roofs: `Roof_RoundTiles_4x4`, `4x6`, `4x8`, `6x6`, `6x8`, `6x10`, `8x8`,
  and `8x10`; `Roof_Front_Brick4`, `6`, and `8` (the gables);
  `Roof_Dormer_RoundTile`, `Roof_Wooden_2x1_L`, and `Roof_Wooden_2x1_R`.
- Props: `Prop_Chimney`, `Prop_Chimney2`, `Prop_Crate`, and
  `Prop_ExteriorBorder_Straight1`.

The script thins the densest pieces with Blender's collapse decimation:
`Roof_RoundTiles_*` to 60 percent, `Roof_Dormer_RoundTile` to 65 percent, and
the arched windows to 75 percent. It drops the kit's second UV map and its
normal, roughness, and ORM images.

Textures, from the kit's base-color images: `T_Plaster_BaseColor`,
`T_RoundTiles_BaseColor`, `T_WoodTrim_BaseColor`, `T_Brick_BaseColor`,
`T_RockTrim_BaseColor`, `T_UnevenBrick_BaseColor`, and
`T_MetalOrnaments_BaseColor`. Plaster and tiles are recolored per building
(the texture's luminance mapped onto a new color, with roof moss kept), and
timber is darkened. They're embedded in each glb as JPEG, at 1,024 pixels for
wood and 512 pixels for the rest.

Generated solids, textured with the same images or flat colors: plinths and
steps, beams, the round tower and its cone roof, the clock turret, the tavern
and shop signs, wall lanterns, and the striped awnings.

## Models

| Model | Triangles | Plaster, roof, timber |
| --- | ---: | --- |
| `townhouse_jettied.glb` | 13,266 | Cream, red, light |
| `townhouse_balcony.glb` | 12,893 | Ochre, brown, mid |
| `row_townhouse.glb` | 10,447 | Rose, slate, dark |
| `library.glb` | 17,826 | White, slate, mid |
| `tavern.glb` | 16,074 | Ochre, green-grey, dark |
| `market_hall.glb` | 15,264 | White, red, mid |
| `corner_shop.glb` | 11,112 | Rose, brown, light |
| `l_house.glb` | 13,660 | Cream, green-grey, mid |
| `cottage_tower.glb` | 7,805 | White, brown, dark |

Each `<name>.footprint.json` lists axis-aligned collision boxes in the glb's
frame: 1 unit = 1 m, +Y up, +Z out of the front door, and the origin on the
ground at the center of the main front wall. Steps, a library entrance bay,
and the cottage's tower stand in front of that wall, at negative depth.
