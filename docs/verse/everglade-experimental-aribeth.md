# Lady Aribeth in Everglade Experimental: private local specimen

On 2026-11-08 the owner's installed Neverwinter Nights: Enhanced Edition model
`c_aribeth` was decoded and rendered beside the path in the actual
`/worlds/everglade-experimental` realm, using its existing `fivelab` engine and
Character Glade shader. This is the original character mesh and texture, not a
new likeness. The experiment uses a private copy of the scene pack; no NWN
payload or public-world replacement is committed here.

## What is rendered

- 18 rigid/dangly rest-geometry mesh parts, 1,969 vertices, 1,242 triangles.
- Original `c_aribeth` 128×128 texture, decoded from the installed TGA.
- Hierarchical static node transforms, rotated from NWN Z-up into world Y-up.
- Placement `(1.8, 0, -2)`, yaw 180°, with the lowest model vertex aligned to ground.
- Existing environment draw, material kind 6, texture layer 93 in this base pack.

She is a static visual specimen: no inherited animation playback, cloth,
collision, conversation, AI, equipment logic or combat has been ported. The
renderer uses the glade's lighting/material behavior, not NWN lighting parity.

NWN stores her animations in supermodels. The installed chain is
`a_fa → a_ba → a_ba_non_combat → a_ba_med_weap → a_ba_custom → a_ba_casts`,
with 359 animation declarations across those providers, including overrides.
An animated reconstruction should evaluate the inherited node tracks rather
than assume the local model's zero animation count means animations are absent.

## Provenance and research tools

The tools and full campaign-content documentation are in the sibling `ore` repo:

- `tools/nwn_model.py`: bounded MDL rest-pose/texture exporter and supermodel-header inventory.
- `tools/nwn_experimental.py`: append a private model to an existing experimental pack.
- `docs/neverwinter-nights/aribeth.md`: models, appearances, campaign placements,
  dialogue, voice, portraits, items, journals, scripts and known model reuse.
- `docs/neverwinter-nights/aribeth-model/reconstruction.md`: decoding limits and render record.

The native model is `data/models_01.bif:228`, SHA-256
`30c8b97ea0962c12ce1ed697c9af133f127cd40fe726d4c9ce3931ed89831186`.
The TGA is `data/textures_01.bif:44`, SHA-256
`672cea56fdaa5b00efeee88b7d7201633163e8bed991436d89dfa9c139c7488f`.
Steam app 704450, installed build 20277208. Original resources and converted
GLB/PNG/scene packs remain under `~/ore-nwn-private/aribeth/`.

The [render/preservation receipt](../verification/nwn-aribeth-2026-11-08/receipt.json)
records exact base/output pack identities, source spans and screenshot hash.
It preserves all 93 existing image-layer/provider entries and all 270 existing
catalog draw streams. The append inserts indices after the environment draw,
shifts subsequent draw starts, retains their vertex-word origins, and rebuilds
the container offsets. It does not overwrite an existing texture or mesh.

## Reproduce locally

From an `ore` checkout, with the Steam installation already present and fresh
private output paths:

```sh
python3 tools/nwn_model.py --model c_aribeth \
  --out "$HOME/ore-nwn-private/aribeth/rest-new"
python3 tools/nwn_experimental.py \
  --base-pack /path/to/openagents/apps/openagents-web/realms/everglade-experimental/assets/glade.pack.gz \
  --model-dir "$HOME/ore-nwn-private/aribeth/rest-new" \
  --out "$HOME/ore-nwn-private/aribeth/experimental-new/glade.pack.gz"
```

Serve a private local page using the normal experimental HTML with only its
`pack:` URL changed to the generated pack. The verified preview is at
`http://127.0.0.1:8949/worlds/everglade-experimental` while this experiment's
loopback server is running. The complete local preview setup is documented in
ore's reconstruction note. No new shader or runtime import API is needed.

## Verification and limits

The actual world reached `coaxial.ready` and `coaxial.sceneReady` in muted,
headless Chrome on Apple Metal, at 1600×1000. Visual inspection confirms her
textured full body beside the spawn path. The independent GLB was also imported
and rendered in background Blender, confirming its geometry and texture outside
the pack renderer.

Twelve synthetic model/texture tests and eight pack-append tests pass. All
preexisting pack layers/draw ranges pass preservation checks. This is not a
production release qualification: the pinned checkout's release `fivelab` build measured
2,594,021 bytes against its existing 2,585,000-byte budget. A local development
build was used; no gate was relaxed and no public deployment was made.

Upstream subsequently raised that size budget to 2,602,000 in `b0651d5bac`.
The size observation above records the original capture checkout; this experiment
did not change the budget.
