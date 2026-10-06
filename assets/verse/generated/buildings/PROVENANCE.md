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

The second round, generated on October 5, 2026, for the buildings the city
map names. These lean on cheaper generated parts where the kit's pieces cost
the most: octagonal and tapered drums, cone and pyramid roofs, roofs of two
textured slabs (`slab_roof`), thatch with courses and a ridge roll, round
logs, a bread oven, a forge, windmill sails, and glazing.

| Model | Triangles | Plaster, roof, timber | What it is |
| --- | ---: | --- | --- |
| `music_hall.glb` | 7,783 | White, teal, dark | Octagonal hall of tall arched windows on a stone plinth, a tiled cone, and a lit lantern cupola |
| `meeting_hall.glb` | 12,555 | Sage, slate, dark | Tall stone hall under a broad gable, a columned porch, and a bell-cote |
| `boathouse.glb` | 2,570 | White, brown, dark | Timber boathouse on a stone footing, its arch to the water, a side door, and a slab roof over board gables |
| `boardwalk_cafe.glb` | 7,281 | Butter, red, mid | One-storey café with wide windows under a green awning, on a railed plank deck |
| `bakery.glb` | 9,492 | Butter, red, mid | Two storeys with a round brick bread oven on its side, a tall stack, awnings, and a sign |
| `smithy.glb` | 6,184 | White, charcoal, dark | Stone smithy with an open lean-to forge, a glowing hearth, a chimney, an anvil, and a trough |
| `windmill.glb` | 3,019 | White, brown, mid | Tapering tower mill on a stone foot, a tiled cap, and four canvas sails |
| `greenhouse.glb` | 790 | White, red, light | Glasshouse on a brick base with white glazing bars, see-through glass (glTF `BLEND`), and seedlings |
| `clock_tower.glb` | 10,269 | White, slate, dark | Stone clock tower with four clock faces, an open belfry, and a slate spire, before a small hall |
| `guild_hall.glb` | 19,224 | Lilac, plum, dark | Stone ground floor, jettied upper floor, guild banners, dormers, and a round corner turret |
| `lookout.glb` | 555 | White, green, mid | Timber lookout tower on four legs with a ladder, a railed platform, and a flag |
| `log_cabin.glb` | 1,086 | White, brown, mid | Round-log cabin with a porch, a stone chimney, and a slab roof |
| `gazebo.glb` | 378 | White, teal, light | Open octagonal gazebo with railings and a tiled roof |
| `farmhouse.glb` | 5,486 | White, thatch, dark | Long timber-framed farmhouse under thatch |
| `cottage_thatch.glb` | 3,645 | Terracotta, thatch, mid | Small thatched cottage with shutters |
| `hip_house.glb` | 10,770 | Sky, slate, dark | Two storeys under a hipped roof, with a chimney and a door lantern |
| `gambrel_barn.glb` | 374 | Barn red, charcoal | Board barn under a gambrel roof with big doors, a hayloft door, and a cupola |

The sixth round, generated on October 5, 2026, by
`scripts/blender/town_houses.py` in Blender 5.2.2 LTS, run headless:

```sh
$B -b --factory-startup --python scripts/blender/town_houses.py -- \
    assets/verse/generated/buildings [NAME ...] [--kit KIT_DIR]
```

These are Reference mode: built from boxes, slabs, and triangles in the
kit's style, not from its wall pieces, using only the kit's base-color
images (`T_Plaster_BaseColor`, `T_RoundTiles_BaseColor`,
`T_WoodTrim_BaseColor`, `T_UnevenBrick_BaseColor`, `T_RockTrim_BaseColor`,
and `T_MetalOrnaments_BaseColor`), recolored as above, plus the kit's
`Roof_Support2` under the hanging signs. Each costs a quarter to a third of
a kit-built house's triangles, so they replace most of the city's kit-built
houses. Their plaster and tile materials are named `HousePlaster` and
`HouseTiles`, so the zone paints each placed house in its own colors; the
colors below are a house's own. The script also writes each house's far
level of detail to `far/<name>.glb`, with each window and door one pane and
without the small details, which `everglade_admit.py` admits into the pack's
`lod` set.

| Model | Triangles | Far level | Plaster, roof, timber | What it is |
| --- | ---: | ---: | --- | --- |
| `shop_house.glb` | 2,598 | 752 | Rose, red, dark | A narrow shop under a front gable: two display windows whose alcoves show goods on shelves through clear glass, a painted shop front and fascia, a striped awning, a hanging sign, and a jettied, half-timbered upper floor with window boxes |
| `gambrel_house.glb` | 1,716 | 486 | Sage, charcoal, dark | A gambrel roof with its gable to the street, windows in the gable, a porch over the door, and a stone plinth |
| `stone_cottage.glb` | 974 | 314 | Cream, brown, dark | A stone cottage under a steep hipped roof, a dormer over the door, and a stone chimney stack up its side |
| `brownstone.glb` | 2,310 | 706 | Brown stone, flat roof | Three storeys over a raised basement, a stoop with iron rails, a hooded door, an areaway rail, and a bracketed cornice; the stone is the uneven brick recolored brown |
| `timber_house.glb` | 2,246 | 728 | White, brown, dark | A stone ground floor under a jettied, timber-framed upper floor with St Andrew's crosses, a cross gable, and a balcony |
| `lantern_inn.glb` | 3,254 | 802 | Butter, red, dark | The Lantern Quarter's inn: lamplit windows, a double door, wall lanterns, a hanging sign, a timber-framed upper floor, and a hipped roof with two dormers |
| `garden_shed.glb` | 418 | | Cream, red, dark | The seventh round's: a plastered garden shed on a stone plinth under a tiled gable, with a plank door, a shuttered window, and a water butt, placed as a prop in back gardens; no far level |
| `woodshed.glb` | 478 | | Cream, brown, dark | The seventh round's: an open-fronted woodshed, a tiled lean-to on four posts over a plank back wall, with three courses of split logs and a chopping block, placed as a prop beside the cabins; no far level |

Each `<name>.footprint.json` lists axis-aligned collision boxes in the glb's
frame: 1 unit = 1 m, +Y up, +Z out of the front door, and the origin on the
ground at the center of the main front wall. Steps, a library entrance bay,
and the cottage's tower stand in front of that wall, at negative depth. The
second round's files also give `roofs` (the landing roofs), `front` (where
a walk ends outside the door), and `inside` (a point past an open doorway),
in glTF x and z, as `verse::zones::everglade::layout::generated::Model` takes
them.
