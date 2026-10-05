# Far levels of detail

Lighter copies of the Everglade pack's heaviest models, which the city draws
instead of a model once its cell is 60 m or more from the eye
([`zones::everglade::detail`](../../../../crates/verse/src/zones/everglade/detail.rs)).
`scripts/blender/everglade_lod.py` makes each one in Blender 5.2.2 LTS from
the model's admitted glTF in a sibling set, and the pack compiler admits it
as `lod/<set>.<name>`:

| Models | Recipe |
| --- | --- |
| Generated buildings and landmarks | Parts under 0.35 m dropped, nearly flat faces dissolved within each texture island, collapsed to 15 percent of the triangles |
| Kit pieces that kit-built houses repeat, and the lighter round-tile roof | Parts under 0.2 m dropped, flat faces dissolved, collapsed to 8 to 40 percent |
| Nature kit trees and bushes | Twigs dropped and bark collapsed to 45 percent; half of the leaf cards kept, each grown to keep the canopy's cover |

Each file names its images as the source set's admitted files
(`../<set>/<file>.png`), so the pack holds one copy of each image, and its
materials are the source model's, so far and near levels match in color.
`manifest.json` records each file's digest, the digest of the source glTF it
came from, and its recipe.

To rebuild after changing a source model, run from the repository root:

```sh
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
  --python scripts/blender/everglade_lod.py
cargo run --release -p verse --example everglade_pack -- assets/verse/everglade
```

then repin the pack (`verse::zones::everglade_pack`).

## License

The sources are Quaternius's CC0 kits and models OpenAgents generated from
them. OpenAgents releases every model here under CC0 1.0 too; `license.txt`
states it.
