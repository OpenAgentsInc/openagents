# Greco-futurism models

Every model here is in Reference mode: original geometry built from boxes,
prisms, spheres, and flat strips by
[`scripts/blender/greco_futurism.py`](../../../../scripts/blender/greco_futurism.py),
with Blender 5.2.2 LTS. The style is defined in
[Greco-futurism](../../../../docs/verse/greco-futurism.md), from four
reference images the owner chose, the Civic Hall from a fifth, and the
belvedere from a sixth and a seventh; nothing from them ships.

Rebuild every model:

```sh
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
    --python scripts/blender/greco_futurism.py
```

Two textures are sampled from the Medieval Village MegaKit Standard by
Quaternius (CC0 1.0; credit: Quaternius), downloaded to
`~/Downloads/Medieval Village MegaKit[Standard]/`:

- `T_Plaster_BaseColor.png`, recolored as limestone (`T_Plaster_limestone`);
- `T_WoodTrim_BaseColor.png`, darkened to 42 percent as walnut
  (`T_WoodTrim_walnut`).

The Everglade pack doesn't carry these copies:
[`scripts/blender/greco_admit.py`](../../../../scripts/blender/greco_admit.py)
points them at the village set's admitted `T_Plaster_Luma.png` and
`T_WoodTrim_BaseColor.png` with base-color factors.

| Model | Triangles | What it is |
| --- | ---: | --- |
| `greco_house.glb` | 6,156 | The owner's house, with `greco_house.footprint.json` |
| `far/greco_house.glb` | 1,016 | Its far level of detail |
| `civic_hall.glb` | 7,817 | The Civic Hall, from a fifth reference image, with `civic_hall.footprint.json` |
| `far/civic_hall.glb` | 1,038 | Its far level of detail |
| `belvedere.glb` | 3,990 | The belvedere, from a sixth and a seventh reference image, with `belvedere.footprint.json` |
| `far/belvedere.glb` | 960 | Its far level of detail |
| `agora.glb` | 7,618 | The Agora, the sales floor's trading hall, with `agora.footprint.json` |
| `far/agora.glb` | 700 | Its far level of detail |
| `kit/column.glb` | 114 | Smooth column with a square capital |
| `kit/pier.glb` | 28 | Square pier |
| `kit/entablature_bay.glb` | 56 | 4 m of entablature with its panel frieze |
| `kit/stair_flight.glb` | 24 | Six shallow steps |
| `kit/planter_wall.glb` | 40 | Planter wall with a clipped hedge |
| `kit/circuit_door.glb` | 226 | Bronze double door with the machine glyph and amber panes |
| `kit/lattice_screen.glb` | 328 | Walnut lattice screen |
| `kit/coffer_bay.glb` | 46 | One bay of a coffered ceiling |
| `kit/pilaster.glb` | 22 | Marble pilaster |
| `kit/chimney.glb` | 32 | Chimney block |
| `kit/circuit_panel.glb` | 44 | Dark walnut wall with circuit lines |
| `kit/bench_long.glb` | 94 | Long low bench with two stools |
| `kit/planter.glb` | 64 | Planter with a clipped shrub |
| `kit/lamp.glb` | 40 | Bronze floor lamp with an amber shade |
| `kit/rug.glb` | 50 | Rug with a classical border |
| `kit/desk.glb` | 166 | Walnut desk and chair |
| `kit/sofa.glb` | 132 | Long sofa and low table |
| `kit/bookshelf.glb` | 176 | Walnut bookcase with books |
| `kit/bronze_column.glb` | 124 | Column on a high plinth with bronze bands |
| `kit/dentil_cornice_bay.glb` | 74 | 4 m of the deep dentil cornice |
| `kit/attic.glb` | 20 | Stepped attic block with its cap |
| `kit/circuit_portal.glb` | 384 | Copper portal with the seal and its open inset door |
| `kit/paired_window.glb` | 128 | Paired tall windows |
| `kit/bowl_planter.glb` | 106 | Walnut planter wall with a bronze bowl |
| `kit/council_ring.glb` | 560 | Tiered council benches in a ring |
| `kit/inlaid_pier.glb` | 48 | Marble pier inlaid with bronze lines, under a copper corbel |
| `kit/lintel_band.glb` | 148 | Red-brown lintel band with copper inlay |
| `kit/louver.glb` | 96 | Walnut slat louver |
| `kit/relief_panel.glb` | 166 | Copper relief of abstract figures |
| `kit/cushioned_bench.glb` | 60 | Marble bench with red cushions |
| `kit/urn_tree.glb` | 184 | Stone urn with an olive tree |
| `kit/threshold.glb` | 12 | Two-step marble threshold |
| `kit/mahogany_door.glb` | 176 | Mahogany double door with brass grids and medallions |
| `kit/stepped_surround.glb` | 110 | Stepped bronze inlay surround with wing brackets |
| `kit/meander_floor.glb` | 94 | Meander and arc inlaid in a floor |
| `kit/terracotta_pot.glb` | 144 | Terracotta pots with a tree and a shrub |
| `kit/trading_desk.glb` | 368 | Standing desk bank of three stations with screens, phones, and headsets |
| `kit/desk_phone.glb` | 22 | Bronze desk phone |
| `kit/headset.glb` | 44 | Headset on a hook |
| `kit/leaderboard_wall.glb` | 158 | Walnut leaderboard wall with its board |
| `kit/ticker_band.glb` | 28 | 6 m of the amber ticker band |
| `kit/bell.glb` | 158 | Bell on its stele and yoke |
| `kit/glass_partition.glb` | 66 | Glass office wall |
| `kit/whiteboard.glb` | 50 | Whiteboard |
| `kit/roleplay_booth.glb` | 306 | Role-play booth's lecterns and screen |
| `kit/pendant.glb` | 38 | Pendant lamp |
