# The dragon

`dragon.gltf` and `dragon.bin` are Shapechange's dragon: a Reference model built from primitives by
[`scripts/blender/dragon.py`](../../../scripts/blender/dragon.py) (with
[`kit.py`](../../../scripts/blender/kit.py)), in the style of the Quaternius kits, with nothing taken
from any kit. License: CC0 1.0 ([license.txt](license.txt)). Registered as the in-house package
`openagents-dragon` (`assets/registry/`).

- 3,260 triangles, flat-shaded; crimson hide, gold belly, wine-dark wing membrane with a warmer
  underside, ivory horns and claws, glowing amber eyes (materials `Dragon_Hide`, `Dragon_Belly`,
  `Dragon_Ridge`, `Dragon_Mouth`, `Dragon_Eye`, `Dragon_Horn`, `Dragon_Membrane`,
  `Dragon_MembraneUnder`).
- One lofted body, neck and tail tube skinned along the spine's bones (a deep keeled chest, a slim
  waist, a long tapered neck and tail); a wedge head with curved horns, cheek frills and a jaw with
  a hinge angle and chin; each leg one welded tube from hip to foot, the hind thighs heavier than
  the forelegs, under muscled shoulders and haunches; feet flat on the ground with three clawed toes
  and a dewclaw; a ridge of back plates; wings of arm, forearm and hand bones with four jointed
  fingers under a scalloped membrane (27 joints).
- A `saddle` joint on the chest at the base of the neck marks where a rider sits: (0, 3.62, 1.2) m
  in the model's frame (`DR_SADDLE_*` in `everglade/dragon-clips.coax`).
- About 5.8 m to the tips of its horns (7.9 m to its folded wings' tips), 14 m from snout to tail,
  22.5 m across its spread wings (over 1.5 times its length, to lift a rider); it faces +z with its
  feet at the origin's height.
- Clips: `idle`, `walk`, `fly`, `glide`, `bite`, `breath`, `roar`, `sweep`, and for a mount
  `idle_saddled`, `takeoff`, `hover`, `land`, `bank_left`, `bank_right`.
- Review stills: `docs/verification/druid-demo-2026-11-09/dragon-model-front.png`, `-side.png` and
  `-below.png`.

History: first made on 2026-10-05 for the Rust the OpenAgents Metaverse's Grove (commit `c8d1ba99fc`, 1,924
triangles, eight clips), removed with the Rust code in the Nuke (`0b401561a0`), restored on
2026-11-09 and rebuilt the same day with the head, limbs, wings, back plates, saddle and mount clips
above.

## Rebuild

```sh
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
  --python scripts/blender/dragon.py -- [OUT_DIR] [--stills DIR]
```

It writes `OUT_DIR/dragon.gltf` and `dragon.bin` (default `assets/metaverse/beasts`), prints one
`MODEL` line, and with `--stills` renders the three review stills into DIR. The committed files are
this script's output under Blender 5.2.2.

## Baked frames

Everglade's worlds draw the dragon as baked frames: `metaverse/tools/assets/dragon_bake.py` samples the
clips in `everglade/dragon-clips.coax`'s order and `character_glade_pack.coax --append-dragon`
appends them to Everglade Experimental's pack as `glade/dragon0` and on.
