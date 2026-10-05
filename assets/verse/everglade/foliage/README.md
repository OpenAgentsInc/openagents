# Foliage

Everglade's trees, undergrowth, deadwood, rocks, and garden greenery:
broadleaf, weeping, gnarled, and dead trees, exposed roots, stumps and logs,
shrubs and brambles, ferns, tall grass, wildflowers, a mushroom ring, ivy,
climbing roses, hedges, planters, window boxes, standing stones, cliff rocks
and boulders, a campfire, and a cascade.

`scripts/blender/foliage.py` builds each model from primitives in Blender,
in the style of the Stylized Nature MegaKit, and writes it as binary glTF
under [`assets/verse/generated/foliage/`](../../generated/foliage/PROVENANCE.md).
`scripts/blender/foliage_admit.py` converts each one to the glTF and `.bin`
the pack compiler reads, without changing its geometry, and points every
image at the nature set's admitted file of the same name
(`../nature/<file>.png`), so the pack holds no new image. Leaf, fern, and
flower materials are alpha-masked and double-sided. The script writes this
set's `manifest.json`: each file's digest, the digest of the glb it came
from (`originals`), and the conversion (`transforms`).

To rebuild and readmit, run from the repository root:

```sh
$B -b --factory-startup --python scripts/blender/foliage.py
python3 scripts/blender/foliage_admit.py
$B -b --factory-startup --python scripts/blender/everglade_lod.py
cargo run --release -p verse --example everglade_pack -- assets/verse/everglade
```

then repin the pack (`verse::zones::everglade_pack`).
