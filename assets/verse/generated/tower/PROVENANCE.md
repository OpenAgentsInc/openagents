# Generated concrete tower

`concrete_tower.glb` was generated on October 5, 2026, by
`scripts/blender/concrete_tower.py` in Blender 5.2.2 LTS, run headless:

```sh
B=/Applications/Blender.app/Contents/MacOS/Blender
$B -b --factory-startup --python scripts/blender/concrete_tower.py -- \
    assets/verse/generated/tower
```

It's admitted into the Everglade pack's `generated` set by
`scripts/blender/tower_admit.py`
([`assets/verse/everglade/generated/`](../../everglade/generated/README.md)):

```sh
python3 scripts/blender/tower_admit.py
```

## Mode and source

Mode: **Reference**. The tower is built from boxes in code. No kit piece or
kit image is used, so no kit license applies; the model and its texture are
CC0 1.0, like the rest of the generated models.

The texture, `T_Concrete_BaseColor` (256 by 256 pixels, tileable, covering
2.4 m), is baked procedurally by the same script with NumPy: tileable
mottling and grain, two 1.2 m pour lifts with dark pour lines, 1.2 m form
panels with fainter 0.3 m board seams, form-tie holes every 0.6 m with
drips, and vertical water streaks. Its randomness is seeded from the model
name, and the script encodes the PNG itself, so a rebuild gives the same
image. The admission script writes that embedded PNG unchanged as
`assets/verse/everglade/generated/T_Concrete_BaseColor.png`.

## Model

| Model | Triangles | Materials |
| --- | ---: | --- |
| `concrete_tower.glb` | 2,022 | `Concrete`, `ConcreteDark` (the same image at a 0.75 factor), `Glass`, `Door`, `Metal` |

A slim brutalist lookout: a hollow shaft 5.6 m square and 28.5 m tall with
0.35 m walls, real window openings with reveals and dark glass set 0.15 m
in, floor slabs inside whose tops are at 5, 10, 15, 20, and 25 m (each
0.25 m thick, below that height), a plinth, a recessed door on the front
(+Z), and a roof platform at 28.5 m with a parapet, a stair housing, and a
mast. It spans x and z from -3.1 to 3.1 and stands 33.68 m to the mast's
tip (31.45 m to the housing's cap). The script's header comment lists every
measurement.

`concrete_tower.footprint.json` gives the frame, the triangle count, and
one collision box for the shaft, in glTF terms: 1 unit = 1 m, +Y up, +Z out
of the front door, the origin on the ground at the center of the base.
