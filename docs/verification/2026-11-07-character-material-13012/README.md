# Character material record (#13012)

Date: 2026-11-07. Machine: Apple M-series Mac, headless Chrome, WebGPU (apple metal-3).

## What changed

- `aethyr/character/material.coax` is the CPU material record. Each colour layer gets a kind
  (skin, cloth, hair, eye or metal), a colour, roughness, metalness, a normal map, an ORM map,
  a strength and a sheen. A bad table is refused whole, and every layer then uses the defaults.
- `aethyr/render/character-material.coax` is the GPU shading: cotangent-frame normal mapping,
  GGX, cloth sheen, skin wrap, Kajiya-Kay hair and rough-metal reflection.
- `verse/tools/humans/pack_material_layers.coax` adds 14 linear data layers to the Everglade II
  pack: Knight, Noble, Peasant, Ranger and Wizard normal and ORM maps, and the base bodies' normal
  and roughness maps. The host uploads them as `rgba8unorm`, never sRGB-decoded.
- `cc_player_fs` (`apps/openagents-web/gpu/everglade2.coax`) samples the maps live in Character
  Create and Everglade II. `?film` adds skin wrap, cloth sheen and hair highlights. `?maps=0`
  turns the maps off.
- Live and film use one `cc_albedo`, which applies dyes in display space.

## Matte rule

Roughness never goes below 0.55 for dielectrics or 0.7 for metal, and the GGX peak stays at or
below the old Blinn highlight. That keeps the toned-down wetness and glints from c90c2b0a20.
Rough metal reflects the light around it, so gold trims stay gold, not dark brown.

## Checks

| Check | Result |
| --- | --- |
| `coax/bin/coax test aethyr/render/character-material.coax --library` | 18 of 18 pass |
| `coax/bin/coax test aethyr/character/material.coax --library` | 4 of 4 pass |
| `coax/bin/coax test aethyr/character-appearance.coax --library` | 3 of 3 pass |
| `coax/bin/coax test apps/openagents-web/everglade2.coax --library` | 383 of 383 pass |
| `coax/bin/coax test aethyr/forge/pack.coax --library` | 37 of 37 pass |
| CPU and GPU agreement (`aethyr/tests/character-material-gpu.coax`) | all 8 functions agree on 64 cases; worst error 8.9e-07 |
| `coax/web/tool.sh coax/web/wgsl-module.coax --site --check` | 52 modules, 0 stale |
| Character Create, 1440x900 | 60 FPS with the maps, with `?maps=0` and with `?film` |
| Everglade II player, 1440x900 | 60 FPS |

## Budget

The data layers add 4.3 MB to the pack, which grows from 43 MB to 47 MB (budget 64 MiB). The
range-loaded `everglade2.vpk3` that `build.sh` writes from it grows from 21.7 MB to 26.0 MB. The
data layers sit between the colour layers and the sounds, the order `aethyr/forge/pack.coax` now
admits (kind 6, with a `D` catalog line each). On the
GPU they take about 19 MB with mips (14 layers at 512 px). They are already at the 512 px phone
size. A pack without data layers binds a 1x1 flat texture, and the shading falls back to the
record's own roughness.

## Shots

- `create-before.jpg` and `create-after.jpg`: Character Create, default Druid, before and after.
- `torso-before-create.png`, `torso-after.png`, `torso-aftermaps0.png`, `torso-afterfilm.png`:
  the same crop before, with the maps, without them and with `?film`. The maps add cloth folds
  and stitching; nothing is glossy.
- `everglade2-player.jpg`: the player in Everglade II.
