# Greco-futurism models

Every model here is in Reference mode: original geometry built from boxes,
prisms, spheres, and flat strips by
[`scripts/blender/greco_futurism.py`](../../../../scripts/blender/greco_futurism.py),
with Blender 5.2.2 LTS. The style is defined in
[Greco-futurism](../../../../docs/verse/greco-futurism.md), from four
reference images the owner chose; nothing from them ships.

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
| `greco_house.glb` | 4,802 | The owner's house, with `greco_house.footprint.json` |
| `far/greco_house.glb` | 924 | Its far level of detail |
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
