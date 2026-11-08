# Aethyr lighting P1.5: compatibility profiles and Everglade II deltas (2026-11-08)

Receipt for issue #13065 (umbrella #13068). This step adds data only. Nothing reads it
until #13066 routes the realms through the resolver.

## What was added

- `verse/style-preset.coax`: `verse_style_compat_profile(preset)`. It turns the preset's four
  clock keys into a four-knot environment profile.
  - Painterly (`STYLE_ENV_PAINTERLY`, id 2): the noon knot is the glade's own knot. Dawn,
    dusk and night are deltas on the glade. They change the key, the ambient, the sky
    colours and the haze colours. They keep the glade's film, exposure, saturation, cloud
    shapes and haze shape.
  - Lowpoly (`STYLE_ENV_LOWPOLY`, id 3): its own profile, built from `lowpoly_key`, with the
    key direction that `verse_style_at` gives.
  - Flat, anime and photoreal have no profile. The material response (bands, rim, dab,
    `shadow_cool`, outlines) stays in the preset, and a test shows that material changes
    never change a profile.
- `verse/everglade2/environment.coax`: `e2_environment_set()`, for `env_region_index_replace`.
  - The open glade: the map default, `env_glade_profile()` itself. It covers the arrival,
    the glade's open ground and the clearing at the path's end.
  - The sunlit path (profile 11, region 21): a prism along `e2_road_x` from z -42 to -270,
    with 22 m bands. It is a subtle delta: the key at 2.05 instead of 2.15, a slightly
    greener and dimmer ambient, haze density 0.0060, and a near field of 28 m.
  - The deep canopy (profile 12, region 22, higher priority): a prism from z -235 to -395,
    with 30 m bands, gone by z -425, before the clearing. The key is 1.60. The sky light and
    ground bounce are cooler and dimmer. The haze is denser (0.0085) and lower (5 m falloff,
    4 m start, 22 to 55 m window).
  - Town, interior and river deltas wait until Everglade II has those places.
- `verse/everglade2/environment-probe.coax`: the probe that writes `probes.tsv`.

## Identity bound (numerical)

Nothing is routed yet, so the bound is shown on the records:

- `coax/bin/coax test verse/style-preset.coax`: painterly's compatibility profile at noon
  resolves bit-equal to `env_glade_profile()` in every field except its own id and segment
  (`env_resolved_same`), on both tiers. 144 checks pass.
- `coax/bin/coax test verse/everglade2/environment.coax`: Everglade II's set resolves
  bit-equal to the glade everywhere in the glade's open ground, on the path's first metres
  and in the clearing. It is exactly the sunlit profile from z -66 to -199 and exactly the
  canopy profile from z -270 to -355. A walk of the whole path in 0.5 m steps changes the
  key's luminance by less than 0.01 per step and never steps. 101 checks pass.
- `glade-identity-compare.tsv`: this build's three Everglade II captures against the P1.1
  baseline. All three have 0 moved blocks.

## Probe tables

`probes.tsv`, from `coax/bin/coax run verse/everglade2/environment-probe.coax`, has two parts:

- For each variant at each baseline camera (`?at=1`, 60 and 300): the camera's resolved
  profile and weights, the key, the ambient, a lit and a shaded mid-grey ground through the
  output, the haze at 20, 40 and 80 m, and the sky. The `ld_*` functions are
  `aethyr/render/painterly.coax`'s arithmetic, reading a resolved environment, and a check
  holds them equal to `paint_*` on the glade.
- A walk every 10 m from z 5 to -475: each region's weight, the key and ambient luminance,
  and the haze density and near field.

## Review captures

These captures come from the real Everglade II renderer, at the baseline's cameras and pins
(`coax/web/lighting-baseline.coax --only ...`, headless, fixed clock, clip mode). For each
variant, a local patch that was never committed set the glade constants in
`aethyr/world/environment.coax` to that profile's values. The run then regenerated
`everglade2.wgsl` and `everglade2-fx.wgsl` and captured the frames. For the compatibility
keys, the run also set `E2_SUN` in `everglade2.coax` and rebuilt the wasm, so the shadows
follow the sun. Each frame is lit wholly by one profile, with no region blend. Afterwards
the run restored every file.

| File | What it shows |
| --- | --- |
| `glade--e2-*.jpg` | The open glade (identical to the baseline) |
| `sunlit-path--e2-*.jpg` | The sunlit-path delta at each camera. Compare `sunlit-path--e2-sunlit-path.jpg` with `glade--e2-sunlit-path.jpg` |
| `deep-canopy--e2-*.jpg` | The deep-canopy delta at each camera. Compare `deep-canopy--e2-deep-canopy.jpg` with `glade--e2-deep-canopy.jpg` |
| `painterly-dawn--e2-arrival.jpg`, `painterly-dusk--…`, `painterly-night--…` | Painterly's compatibility keys at the arrival |

What the review found:

- The sunlit path is barely different from the glade, as intended.
- The deep canopy reads as the same glade, deeper in the wood. It is a little dimmer and
  cooler, with a soft low haze down the path. The colours, film and saturation are the
  glade's.
- Dawn and dusk give a warm, low sun with a peach haze glow.
- Night darkens the land and the key, but Everglade II's sky backdrop stays daylight cyan.
  The page draws that backdrop from its own sky texture (the `s.local.x < 0.5` branch of
  `e2_environment_fs`), not from the profile's sky colours. The profile carries the night
  sky. Making the backdrop read it is routing work for #13066.

Run identity: Mac17,6, Apple M5 Max, Chrome 157 headless, WebGPU hardware (metal-3).
`run.tsv` is the deep-canopy run.
