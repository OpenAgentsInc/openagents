# Lighting P1.6: the realms read the resolved environment (GitHub #13066)

Date: 2026-11-08. Plan: `docs/verse/lighting/README.md`, Phase 1 step 6. Baseline:
`docs/verification/aethyr-lighting-p1-baseline-2026-11-08/` (P1.1).

## What changed

- **One record a frame.** `aethyr/render/environment-frame.coax` (`#authority`) resolves the
  camera's environment once a frame and writes `env_gpu_pack`'s 108 words at word 208 of the
  frame's uniform block. To do that it admits the realm's set whole (`env_region_index_replace`),
  takes the day from `aethyr/world/day.coax`, and runs `env_resolve_camera` at the eye. On a
  refusal it writes the last record it held, or the glade's noon if it holds none.
- **The binding.** The `u` uniform grew from 26 or 52 vec4s to 79 in `gpu/everglade2.bindings`,
  `gpu/charactercreator.bindings` and `gpu/fivelab.bindings`.
- **The spatial design.** The frame carries one record, resolved at the camera. Surfaces do not
  carry per-tile candidates yet. Region bands are 44 to 60 m wide, so the camera's record
  changes smoothly as it moves, and one uniform read costs nothing at 60 FPS. Per-surface
  records (`env_resolve_at` from a stored tile) wait for a phase that needs light to differ
  within one view.
- **The shaders.** `aethyr/render/painterly.coax` adds `paint_env_*`. They use the constant
  versions' arithmetic word for word, but read the record. The `cg_*` delegates now take the
  block: `cg_sun_dir`, `cg_sun_colour`, `cg_hemisphere`, `cg_irradiance`, `cg_air_depth`,
  `cg_haze_window`, `cg_atmosphere`, `cg_film`, `cg_output`, `cg_sky` and `cg_cloud`. The
  cloud-shadow flag bit gates `cg_cloud`. No enabled lit path holds its own key, ambient or fog
  numbers. The WGSL is Coax-emitted (`wgsl-module --site --check`: 56 modules, 0 stale).
- **Character Create and Everglade II** (`apps/openagents-web/everglade2.coax`):
  - The set is #13065's `e2_environment_set` (the glade, the sunlit path and the deep canopy).
  - The day is the room's: the W header's day (`everglade2-net.coax` `w_day`) goes through
    `day_from_room`. Before a room is heard, and in Character Create, the day is the glade's
    fixed noon.
  - `E2_SUN` is gone. The shadow tiles look along the record's key, which is bit-equal to the
    old normalized `E2_SUN` in the glade.
  - Ordinary avatars and detailed characters are lit by the same `cg_*` path (`cc_player_light`
    now takes the block).
  - Exceptions: the sky plate's halo position stays authored (`ENV_FRAME_HALO`). The cast
    puffs and the portal light are unlit emitters and read no light.
- **Experimental and 5E Lab** (`fivelab.coax`) author no environment. They resolve the painterly
  default once at the page's fixed noon and write the same words every frame. A test checks that
  these words equal what the resolver writes at the eye.
- **Made painterly realms** (`myrealm.coax`; Aethyrforge's realms are these). A realm with no
  `time` line and no mist writes its style block through `verse_style_glade_env`
  (`verse/style-preset.coax`). The block starts as the glade-lit preset. Every word that means
  the same in both models comes from the resolved painterly compatibility profile at noon,
  which is the glade:
  - the key's direction and colour;
  - the sky and ground ambient;
  - the haze's air and sun tint;
  - the sky's zenith and horizon;
  - the saturation.

  The style shader's own units (the key's strength, the haze reach and the exposure) stay the
  preset's.

## Identity check

```
coax/web/tool.sh coax/web/lighting-baseline.coax OUT --compare docs/verification/aethyr-lighting-p1-baseline-2026-11-08 --build --serve 8987
```

The harness now binds the page's live uniform block (captured from `writeBuffer`) when it
evaluates the GPU probes. It also reads back the environment words every frame (op `E`, written
to `env.tsv`). Output: `check.log`, `compare.tsv`, `probes.tsv`, `run.tsv`, `env.tsv`,
`contact-sheet.jpg`.

| Group | Result |
| --- | --- |
| Character Create (`cc-glade`, `cc-closeup`, `cc-turn-0..3`, `cc-phone-glade`) | pass: 0 blocks moved, peak block at most 1.64/255, 24 GPU probes in bound |
| Hand-over (`handover-0..3`) | pass: 0 blocks moved |
| Hand-over (`handover-4`) | moved by the authored delta: 125 blocks, peak 13/255. The walk has entered the sunlit path's band, and 13 light words are off the glade's noon |
| Everglade II arrival (`e2-arrival`) | pass: 0 blocks moved |
| Everglade II path and canopy (`e2-sunlit-path`, `e2-deep-canopy`) | moved by the authored delta: 18 and 21 light words off the glade. The canopy is cooler and dimmer with denser low haze (`authored-deltas.jpg`: baseline left, now right) |
| Experimental (`exp-combat`) | lighting identical: 0 light words off the glade, 24 GPU probes in bound. 93 blocks moved, all in the top right: HP 7/7 against 4/7 and a monster's position, from the Experimental ranged-attack fixes that landed since the baseline (`exp-combat-pair.jpg`: baseline top, now bottom) |

Only the Everglade II path and canopy captures moved, plus the hand-over capture that walks into
the path. They moved by #13065's authored deltas. Character Create is unchanged within the P1.1
bounds.

**Readback.** The GPU probes evaluate the page's own compiled `cg_*` functions on the live block.
On both Character Create and Experimental they return the resolved glade values (`probes.tsv`):

- key colour 2.15, 1.512, 1.122, and key direction -0.426, 0.618, -0.661;
- ambient up 0.49, 0.55, 0.62, and ambient down 0.32, 0.34, 0.16.

`env.tsv` shows every load writing the record each frame. Day 0.5 means a fixed noon.

**The hand-over, probed every frame.** Over 788 frames from Character Create, through the
hand-over, and up the path, the page wrote 335 distinct records. No frame changed any light word
by more than 0.0101, while the total change over the walk was 2.0. The harness fails the
hand-over if one frame moves a word by more than 0.02 (`LB_ENV_JUMP`).

## Performance and size

- **Frame rate.** `coax/web/tool.sh realm-perf http://127.0.0.1:8987 --only everglade2`
  (`realm-perf.log`). The second run held 60.0 FPS with p95 16.7 ms and no frame over 20 ms at
  1440x900 dpr 1, at dpr 2 and on a 390x844 phone. The first run, at load average 11 from other
  agents, dropped one or two frames on two cases.
- **Wasm sizes.**
  - everglade2: 676,355 bytes, and 687,298 after the rebase onto #13046. The budget was raised to 720,000 (from #13046's 680,000) in
    `wasm-budgets.txt`, with the reason.
  - fivelab: 2,598,953 bytes, under its 2,602,000 budget. It caches its fixed record instead of
    carrying a whole frame.
  - myrealm: 634,134 bytes, under its 660,000 budget.

## Checks

- `coax test`, with results:
  - `aethyr/render/environment-frame.coax`: 185 checks;
  - `aethyr/render/painterly.coax`: 108;
  - `verse/style-preset.coax`: 145;
  - `apps/openagents-web/everglade2.coax`: 1170;
  - `fivelab.coax`: 1135;
  - `myrealm.coax`: 599;
  - `charactercreator.coax`: 377;
  - `gpu/everglade2.coax`: 206;
  - `gpu/charactercreator.coax`: 177;
  - `gpu/fivelab.coax`: 176;
  - `gpu/character-glade-light.coax`: 112;
  - `verse/everglade2/environment.coax`: 101;
  - `coax/web/lighting-baseline.coax`: 356.

  All pass.
- `coax mutate --root-only aethyr/render/environment-frame.coax`: 102 mutants, 102 killed, 0
  survived (`mutate-environment-frame.log`).

## Not covered here

- **The made painterly realm** is checked at the style block, not in a browser. The check
  confirms that the words its shader reads equal the glade's resolved values (`myrealm.coax`:
  "a made painterly realm with nothing authored renders under the glade"). The static page
  server has no realm API to serve a made realm's spec, so the harness has no capture of one.
- **`verse/tools/assets/glade_light_check.coax`** is a legacy probe. It binds the glade module
  without the uniform block it now needs. It is not part of any gate.
