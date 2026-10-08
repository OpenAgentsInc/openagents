# Water handoff

Status: W11 in progress, 2026-10-08. Phases W1 to W10 are landed. This page records what
landed, what remains open, and how to pick the work up again. The
specification is [Water](water.md), and the coastal zone's is
[The coast](coast.md).

## October 8 coordination checkpoint

W11 (#10783) remains open on the pushed branch
[`codex/water-w11-measurement`](https://github.com/OpenAgentsInc/openagents/tree/codex/water-w11-measurement).
Checkpoint `df50707aac` records the exact sources, measured cases,
artifact hashes, and resume commands. Its claim is released and the board
returns to Todo. Source `499b25d083` is rebased on main
`90f90cd4f2`; 61 focused water tests, shared shader validation, the WASM
check, and the release native and WASM precompiles pass. GPU-compute
parity tests are excluded from the Mac checks.

The measurements expose a missing scene bind group in the newly split
blended render pass. The branch fixes it with an explicit group-zero
binding. A corrected WebGPU Water Lab dry/wet pair renders without
device or unexpected log errors and retains 726 completed GPU timing
samples. Water GPU time averages 2.768 ms, with a 3.408 ms p95; main
elapsed time averages 1.113 ms. Browser elapsed time is not thread CPU
time. The old Medium targets are still exceeded after mirror, scene
copies, and screen-space reflection drop and the cadence reaches eight.
Do not treat this result as calibrated budgets or zero GPU cost.

The earlier frozen collection retains 15 native pairs and eight browser
cases. Four WebGPU cases render black because their command buffers are
rejected; four WebGL2 cases render correctly. Those failed images and
raw errors remain evidence, not passing browser checks. Corrected native
reruns retain zero valid GPU samples for Low pond-posts and waterline and
one for Medium pond-noon; two old-budget assertions still fail. A passing
waterline process with zero samples does not establish its GPU budget.
W11 still needs adequate timing samples, measured constants and the budget table
in [water.md](water.md#budgets-per-tier), the remaining corrected browser
matrix, and deployment from a normal production release image. The
benchmark's derived WASM is for measurement only.

On October 8, the idle Mac `~/work/openagents-target-agent12` cache is
reclaimed after free disk falls below 25 GB. Its two native water capture
executables and SHA-256 receipts remain in
`/Users/christopherdavid/.openagents/scratch/codex-01a119ab-cb4c-7331-b0dc-8ddce4fb09a0/preserved-water-harnesses/`.
The source checkout, measured evidence, and remote target remain intact;
recreate the Mac target when a later check needs compilation.

This run stops new assignments at 89 percent shared usage. W11 saves a
resumable checkpoint; #10919 and coast C1 through C6 are unassigned.
Recheck each issue and claim before resuming. The sections below retain
the original October 7 plan.

For resumption, use one Cargo command at a time through
`openagents lease build --keep-target-dir -- cargo ...`, with the agent's
target under `~/work/openagents-target-agentN`. Run checks and filtered
tests only, exclude GPU parity on the Mac, and use remote placement for
GPU compute and ray tracing. The original sweep and capture command list
below is historical; it does not override those limits.

## What landed

Each phase closed its issue with the commit listed. The phase's status
paragraph in [water.md](water.md) gives the details and measurements.

| Phase | Issue | Commit | What landed |
| --- | --- | --- | --- |
| W0 | — | `b427c1a995`, `7eef579b7d`, `713304d336`, `75eabb1ac7`, `5e1601dbff` | The Water Lab (`crates/verse-zone-water`): a cove with a water pass, a spell hotbar, the Water Orb, and the Thunderbolt |
| W1 | [#10773](https://github.com/OpenAgentsInc/openagents/issues/10773) | `b399fa2dbe` | `physics::water`: water bodies, a Gerstner surface that is exact at any tick, buoyancy, drag, currents, and river flow grids |
| W2 | [#10774](https://github.com/OpenAgentsInc/openagents/issues/10774) | `32e4d59fc2` | One water shader (`crates/verse-pbr/src/water/water.wgsl`) for both renderers and every tier, with Jerlov presets and shore foam |
| W3 | [#10775](https://github.com/OpenAgentsInc/openagents/issues/10775) | `03b7bbe2b0` | Everglade's four ponds and Glade Run are swimmable, with SRD 5.2.1 breath and suffocation |
| W3 fix | [#10892](https://github.com/OpenAgentsInc/openagents/issues/10892) | `fa5320828a` | Everglade's dirt sheet drew over the water; the ponds and Glade Run now draw as water, with underwater fog and a waterline |
| W4 | [#10776](https://github.com/OpenAgentsInc/openagents/issues/10776) | `cad6dfab6a` | Spectral FFT waves, whitecaps, and surf |
| W5 | [#10777](https://github.com/OpenAgentsInc/openagents/issues/10777) | `a6f138a248` | Scene copies, refraction, a planar mirror, and screen-space reflection on Medium and High |
| W6 | [#10778](https://github.com/OpenAgentsInc/openagents/issues/10778) | `3fc0dd6486` | Ripple and foam field, Kelvin wakes, splashes, floating debris, and boardable rowboats |
| W7 | [#10779](https://github.com/OpenAgentsInc/openagents/issues/10779) | `4d205ad1b7` | Underwater view from below, with caustics on every submerged surface |
| W8 | [#10780](https://github.com/OpenAgentsInc/openagents/issues/10780) | `6366e2df13` | Every spell's effect on water from SRD 5.2.1, plus our lightning conduction rule (`verse_world::spells::water`) |
| W9 | [#10781](https://github.com/OpenAgentsInc/openagents/issues/10781) | `c844a4863e` | A deterministic weather schedule, rain, puddles, and wet surfaces |
| W10 | [#10782](https://github.com/OpenAgentsInc/openagents/issues/10782) | `b69b981095` | The ocean on a geometry clipmap with a streamed field, a shared world tick, and water events for multiplayer |
| Sea states | [#10918](https://github.com/OpenAgentsInc/openagents/issues/10918) | `f35d16bad0` | Calm, moderate, and storm seas are visibly distinct (Cox and Munk slopes, JONSWAP fetch laws, denser whitecaps, crest spray) |

The coast specification issue,
[#10796](https://github.com/OpenAgentsInc/openagents/issues/10796), closed
with `c8c9f7d02e`, which split the coast into C1 to C6.

## What is open

| Issue | Remaining work | Blocked by |
| --- | --- | --- |
| [#10919](https://github.com/OpenAgentsInc/openagents/issues/10919) Everglade pond reflections | Not started. Find why Everglade's ponds read as flat green from above (`everglade-water/lantern-above.png`) when the harness ponds (`water-screen/`) show reflection and refraction. Candidates: the preset, Fresnel at that angle, the bed color, the scene copy and mirror not running in Everglade, or Everglade's tier defaults. Fix it, commit before and after captures from the same viewpoints, and add a test that catches the cause. | Nothing |
| [#10783](https://github.com/OpenAgentsInc/openagents/issues/10783) W11 measurement and budgets | In progress on `codex/water-w11-measurement`. Source and measurement harness checkpointed. Remote verification started; native desktop audio lacks `alsa.pc`, so use the capture consumer check and cover default features on the Mac later. No measurement has run yet. Measure per-tier GPU time, GPU memory, and CPU for water on the development Mac and on `everglade-web` with WebGPU and WebGL2; put device runs in `NEEDS_OWNER.md`. Replace the target budgets in [water.md](water.md#budgets-per-tier) with measured ones and set the constants and overrun behavior in `verse_engine::quality`. Known overruns: W10's floating-bodies view at 4.29 ms against High's 4 ms. | Nothing (W7 to W10 are closed) |
| [#10784](https://github.com/OpenAgentsInc/openagents/issues/10784) umbrella | Close when W11 closes and [water.md](water.md) has the measured budgets. | #10783 |
| [#10885](https://github.com/OpenAgentsInc/openagents/issues/10885) C1 zone shell | Not started. `ZoneId::Coast`, the plaza's west arch, generated terrain and bathymetry, the ocean with a tide, the harbor shelter mask, the estuary, spawn, and zone tests. Start from W10's coastal test scene, `verse_zone_water::coast`, and extend the bed to the horizon, as [coast.md](coast.md) says. | Nothing (W3, W4, and W10 are closed) |
| [#10886](https://github.com/OpenAgentsInc/openagents/issues/10886) C2 kits and pack | Blender kits and the pinned coast pack, through `openagents artifact submit`. | C1 |
| [#10887](https://github.com/OpenAgentsInc/openagents/issues/10887) C3 harbor and islands | Rowboats on swell, Gull Island, the sandbar, and the bounds current. | C2 |
| [#10888](https://github.com/OpenAgentsInc/openagents/issues/10888) C4 diving | The reef, the wreck, and tide pools, with caustics from W7. | C2 |
| [#10889](https://github.com/OpenAgentsInc/openagents/issues/10889) C5 transitions | Zone-to-zone portals, Everglade's estuary gate, and the Grid's coast gate. | C1 |
| [#10890](https://github.com/OpenAgentsInc/openagents/issues/10890) C6 climate and measurement | Weather and spells at sea, multiplayer checks, and per-tier measurement. | C3, C4 |

## Tidewater harvest items

[Tidewater for Verse water](research/tidewater.md#harvest-list) ranks 15
techniques to take from Tidewater (MIT). Each is assigned to an issue:

| Issue | Harvest items (rank) |
| --- | --- |
| W7 #10779 (closed) | Caustics splatted from the wave surface (4), closed-form in-scattering (6), shadows where the refracted sun ray enters (8), lens droplets (13). W7's commit covers the refracted-ray shadows; the others did not land and move to C4 or a new issue. |
| W9 #10781 (closed) | Sea detail: gusts, slicks, and windrows (7); lens droplets in rain (13). Neither landed; they move to C1 or a new issue. |
| W11 #10783 | Mipmapped cascades sampled at the mesh's spacing (3), reflections below the horizon darkened by slope (10), a per-view GPU timing bench (14), and frustum culling of clipmap blocks. The issue's comment has the details. |
| C1 #10885 | Wave travel-time field (1), analytic breakers with swash run-up (2), mipmapped cascades (3), wind sea plus distant swell (5), sea detail (7), foam lace and a beach foam and wetness field (9), breaker spray (11). |
| C3 #10887 | Breakers (2), keel cross-flow drag and a resistance hump for boats (12). |
| C4 #10888 | Splatted caustics (4), refracted-ray shadows (8). |
| C6 #10890 | Breaker spray (11), surf sound driven by the drawn waves (15). |

## W11 checkpoint

The source checkpoint adds CPU-built box mips, vertex sampling at the
clipmap spacing, slope-aware reflection darkening, conservative clipmap
block culling, owned-resource accounting, and a sustained-overrun policy.
GPU timestamps have delayed readback slots and run continuously on supported
Medium and High physical renderers. Normal Low keeps its fused pass; its
isolated GPU probe is diagnostic only. Low and unsupported APIs keep
runtime water GPU time absent and rely on whole-frame quality fallback
where available; water residency and CPU admission remain active.
Queue-fence estimates remain separate. Native CPU clocks measure
thread CPU time. Worker cost counts completed synthesis jobs over the
measurement interval, including superseded results; per-job duration is
reported separately. The native bench paces actual display intervals at
60 Hz, so back-to-back frame bursts cannot hide worker cost. Missing GPU
results neither advance nor reset the GPU overrun streak.
Native correctness captures read the final measured 1920 × 1080 texture;
they do not render a smaller viewport that could re-enable dropped optics.
Pass timestamps isolate mirror, color and depth copies, and surface work.
Shared opaque underwater shading and implicit queue texture uploads are
covered only by the separate wet-minus-dry fence estimate.

Resume from `codex/water-w11-measurement`; acceptance is still pending.
The first remote native check compiled the edited physics, engine, and
PBR crates, then stopped on the unrelated default desktop audio dependency:
`alsa.pc` is absent from coderos-4080's pkg-config path. Use Verse's
`--no-default-features --features capture` consumer there; check native
default features in the later Mac window. Do not change system packages.
At source `97ab779f788948dc42ed1dca38ad58749a5ef6e0`, the remote
`cargo check --locked -p verse --no-default-features --features capture -j4`
and `cargo check --locked -p everglade-web --target wasm32-unknown-unknown -j4`
passed. The latter uses `CC_wasm32_unknown_unknown=clang`. Pure water tests
and shared shader validation remain pending; P3 currently holds the team
Cargo token.
The stable remote source is
`/home/christopherdavid/.openagents/scratch/process-2198653/water-w11-agent12`,
with its own `~/work/openagents-target-agent12`. Advance only that checkout
between commands; do not compile the SHA-named placement snapshots.
The fixed-view ignored raster test is `w11::water_w11_fixed_views` in
`verse-pbr`'s `water_capture` example. Compile that filtered test with
`--release --no-run`, then run its executable under a quiet lease with
`WATER_W11_OUTPUT` set to scratch. The browser harness is
`bench/verse/2026-10-08/water-w11/browser.py`; run it through
`openagents browser run` against the candidate WASM output. It measures
Low on WebGL2 and Medium on WebGPU; the browser platform does not admit
High. Measurements, captures, budget updates in [water.md](water.md), and
phone steps in `NEEDS_OWNER.md` are still pending. No issue has closed.
Compile the browser candidate with the existing `presence_ui::tests` filter,
`--lib --target wasm32-unknown-unknown --release --no-run --message-format=json`.
`stage.py` selects its exact executable from Cargo's output, checks that
matched `wasm-bindgen` 0.2.128 retains the app's start export and removes
the Rust test harness entry, and records source, WASM, glue, and input
digests. It stages only in scratch. The browser check requires the
physical renderer's admitted tier and successful pinned pack responses.
The private kit input is
`~/.openagents/verse/private/medieval-town/packs/dae1612d4c22438a933c27b406c1e18fe134b13eab8eb5240ddcf5506ffb0b93.vtp`
(10,238,689 bytes). The public source pack is
`a82df378ca7d06d9c755ae24076c89270d8a8097509c54a166d941da05f9de2f`
(10,636,202 bytes). Recheck both pins if the source is rebased; keep all
licensed input bytes outside Git. Actual artifact and page checks have
not run yet.

The coordinator holds the team build token. Request a window before
Cargo or measurement, use at most four jobs for W11, and preserve the
terminal's quiet soak. Do not run `water::parity` on the Mac: those tests
use GPU compute. Run them only on `coderos-4080` through a remote lease.
The required browser consumer check is
`cargo check -p everglade-web --target wasm32-unknown-unknown`.

## Build, test, and capture commands

The machine is shared, so run every Cargo command through the build lease
with your own long-lived target directory and at most seven jobs:

```sh
export CARGO_TARGET_DIR=$HOME/work/openagents-target-agentN
openagents lease build --keep-target-dir -- cargo test -p physics water -j 7
```

Check `df -h ~` first, and stop if fewer than 25 GB are free. Run timing
under `openagents lease quiet --receipt FILE -- CMD`.

The sweep to run on `main` before starting water work:

```sh
cargo test -p physics water
cargo test -p verse-pbr --lib water -- --skip water::parity
cargo test -p verse-world water
cargo test -p verse-zone-water
cargo test -p verse-zone-everglade water
cargo test -p verse zones::everglade
cargo test -p verse gles
cargo check -p everglade-web --target wasm32-unknown-unknown
cargo check -p coder-mobile
```

Capture examples (release examples through the lease; never a release
build of the `verse` binary):

| Example | What it captures |
| --- | --- |
| `cargo run --release -p physics --example water_buoyancy` | Buoyancy timing for 64 bodies |
| `cargo run --release -p verse-pbr --example water_capture` | Harness views per tier and renderer, including the sea states; filter with `WATER_CAPTURE_VIEWS`, `WATER_CAPTURE_TIERS`, and `WATER_CAPTURE_IMPORTED=0` |
| `cargo run --release -p verse-pbr --example water_fft_cost` | FFT cost |
| `cargo run --release -p verse --example water_capture` | The Water Lab, including spell casts (`--spells`) |
| `cargo run --release -p verse --example everglade_water_capture` | Each pond and Glade Run from above, at eye level, and swimming |
| `cargo run --release -p verse --example everglade_wake_capture` | Wakes, rowboats, and floating debris |
| `cargo run --release -p verse --example everglade_underwater_capture` | Underwater views and caustics |
| `cargo run --release -p verse --example everglade_weather_capture` | Weather, rain, and wet surfaces |
| `cargo run --release -p verse-zone-water --example coast_capture` | W10's coastal test scene and clipmap rings |

## Known issues

- Puddles read as shadows in stills.
- Rain on Low is thin.
- The storm sea's color needs work; it should read darker and greyer.
- No water audio plays: no splashes, rain, or surf sounds.
- Everglade's ponds lack the harness ponds' reflection and refraction
  (#10919).
- The ripple field runs at the wave array's resolution (64² on Low and
  Medium, 128² on High), below the specification's, because WebGL2 has one
  free sampled-texture slot left in the water pipeline.
- The imported renderer draws water in W2's two halves on every tier and
  has no clipmap ocean.
- The planar mirror skips streamed content.
- On High, W10's floating-bodies view measured 4.29 ms of water time
  against a 4 ms budget.
- The Water Lab draws a flood as a rise of the whole bay, and one ice disc
  for the strongest patch; the rules track every patch.
- Where Glade Run leaves Reed Pond, a bright strip with a hard edge shows,
  and the stream's edges are jagged from above.
- Two-client checks for rowboats, water events, and the sea are tests over
  encoded frames, not live relay runs.

## Owner steps

These are in the workspace `NEEDS_OWNER.md`:

- Water on a phone (#10774).
- Swimming in Everglade on a phone (#10775).
- Water refraction and reflection on a phone (#10777).
- Rowboats and wakes on a phone and with two players (#10778).
- Underwater on a phone and in the browser (#10779).
- Spells on water in the Water Lab on a phone (#10780).
- Weather and rain on a phone (#10781).
- The clipmap ocean on a phone (#10782).

Open questions from the umbrella: compute shaders on desktop, and whether
to license Water Pro for side-by-side comparison.

## Captures

All under `bench/verse/2026-10-07/`:

| Directory | Phase |
| --- | --- |
| `water/` | W2: harness views per tier and renderer, and the browser |
| `everglade-water/` | W3 and #10892: ponds, Glade Run, wading, swimming, and diving |
| `water-w4/` | W4: spectral seas |
| `water-screen/` | W5: refraction and reflection, with W2 comparisons |
| `everglade-wake/` | W6: wakes, rowboats, and debris |
| `water-w7/` | W7: underwater and caustics |
| `water-w8/` | W8: spells on water |
| `everglade-weather/` | W9: weather, rain, and wetness |
| `water-w10/` | W10: the clipmap ocean and coastal scene |
| `sea-states/` | #10918: the three sea states before and after (`sheet-high.png`, `sheet-low.png`) |
