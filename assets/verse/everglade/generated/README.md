# Generated models

These models are built by scripts under `scripts/blender/`, run headless in
Blender 5.2.2 LTS, and committed as binary glTF under
[`assets/verse/generated/`](../../generated/PROVENANCE.md), whose
`PROVENANCE.md` files name each model's script. This set holds them as the
Everglade pack compiler reads them: glTF with a separate `.bin` buffer.

`scripts/blender/everglade_admit.py` converts each committed glb without
changing its geometry. It drops the glb's embedded JPEG images and points
each textured material at the village set's admitted image instead
(`../village/<file>.png`), with a base-color factor that keeps the model's
color, so the pack holds one copy of each kit image. Plaster and roof tiles
in colors other than the kit's own use two neutral images the script derives
from the kit, `T_Plaster_Luma.png` and `T_RoundTiles_Luma.png`, admitted in
the village set with `T_UnevenBrick_BaseColor.png`. It also writes this
set's `manifest.json`: for each converted file, its digest, the digest of the
glb it came from (`originals`), and the conversion (`transforms`).

To readmit after rebuilding a model, run from the repository root:

```sh
python3 scripts/blender/everglade_admit.py
cargo run --release -p verse --example everglade_pack -- assets/verse/everglade
```

then repin the pack (`verse::zones::everglade_pack`).

## Contents

| Models | Script | Use in Everglade |
| --- | --- | --- |
| `townhouse_jettied`, `townhouse_balcony`, `row_townhouse`, `library`, `tavern`, `market_hall`, `corner_shop`, `l_house`, `cottage_tower` | `buildings.py` | Buildings in the city's districts (`layout::city::STAND_INS`) and the Stacks |
| `observatory`, `fountain`, `bandshell`, `market_stall_red`, `market_stall_blue` | `observatory.py`, `fountain.py`, `bandshell.py`, `market_stall.py` | Observatory Hill, the Fountain Plaza, the commons, and the markets |
| `lamp_post`, `barrel`, `flower_box`, `well`, `stone_wall`, `hedge`, `hand_cart`, `signpost`, `lily_pads`, `footbridge`, `wildflowers`, `bunting` | `street_props.py` | Street furniture, ponds, and meadows |

## License

The buildings are assembled from Quaternius's Medieval Village MegaKit
(Standard), and the observatory and the bandshell sample its brick image;
the kit is CC0 1.0, and `license.txt` is its license text. The other models
are generated from primitives. OpenAgents releases every model here under
CC0 1.0 too.
