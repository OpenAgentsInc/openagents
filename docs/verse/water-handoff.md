# Water handoff

Status: W11 is code-complete and deployed, 2026-10-08. Phases W1 to W10 are landed. This page records what
landed, what remains open, and how to pick the work up again. The
specification is [Water](water.md), and the coastal zone's is
[The coast](coast.md).

## October 8 coordination checkpoint

W11 lands on main `1d126aad2b` and production revision
`coder-web-w11-1d126aad2b-20261008202124`. Native query resolution waits for render submission
completion without blocking the frame thread. All 15 Apple M5 Max and all
15 RTX 4080 fixed views return 96/96 valid GPU samples. Empty ripple kernels
are skipped without losing clock phase, and spectrum integrals are cached.
The 59 focused water tests and explicit browser-target check pass.

`docs/verse/water.md` now records the measured tier costs and admission
caps: GPU 3.5/3.5/4 ms, memory 8/32/64 MiB, unchanged main-thread CPU caps,
and worker CPU 0.5/1/2.5 ms. All 15 RTX views pass again under those caps,
with 96/96 valid samples and mean costs within bounds. Storm views still
reduce effects; pond views retain full effects. Original-cap failures and
the failed Metal experiments remain in the evidence. This is not a claim
that every scene runs all effects within budget on every device.

Evidence is under `bench/verse/2026-10-08/water-w11/` in
`metal-deferred-f176dfa0c8/`, `vulkan-deferred-f176dfa0c8/`, and
`calibrated-965e57a09a/`. All eight refreshed browser cases pass on the frozen timer build
`40284fdcc7`, retained in `browser-deferred-40284fdcc7/`; integration retains both B4's
`offline_light` and W11's water telemetry. Physical phone steps are in
`NEEDS_OWNER.md`. Final integration checks pass. A normal release module
from `1d126aad2b` passes four production browser cases: Water Lab and
Everglade on WebGPU and WebGL2. Both town cases load the pinned bake;
WebGPU reports real timestamps and WebGL2 leaves them unavailable.
All 12 delivery checks and artifact hashes pass before and after promotion.
Receipts and private capture hashes are in `production-1d126aad2b/` beside
the measurement records. No new bake runs. #10783 and umbrella #10784 are
code-complete; phone qualification remains owner-only.

The coordinator now holds #10919 on `codex/water-pond-optics`. Its initial
CPU regression confirms mirror admission for all four ponds on Medium and
High from above and at eye level; visual diagnosis remains open. Coast
C1 through C6 remain separate work.

The earlier checkpoint below remains historical evidence.

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
| [#10783](https://github.com/OpenAgentsInc/openagents/issues/10783) W11 measurement and budgets | Checkpointed on `codex/water-w11-measurement`; claim released and board Todo at the usage stop. The corrected WebGPU Water Lab pair renders and has 726 valid GPU samples. Three corrected native reruns retain two old-budget failures and zero/one valid GPU samples. Native timing validity, complete calibration, measured constants in [water.md](water.md#budgets-per-tier) and `verse_engine::quality`, and production verification remain pending. See the checkpoint below. Device runs belong in `NEEDS_OWNER.md`. | Nothing (W7 to W10 are closed) |
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

Resume branch: `codex/water-w11-measurement`. The frozen measured source is
`499b25d0839a01dbbbc524e1374b1ee5c3e52644`, rebased on main
`90f90cd4f2ac8d6f175906f31ab1b3aa72709144`. The feature branch is pushed;
#10783 remains open, with its claim released and board status Todo at the
usage stop. Nothing from W11 has landed or deployed. No W11 commands,
watchers, profiles, or loopback servers remain running. Preserve the
unfinished checkout, scratch evidence, and warm agent12 targets.

The source adds CPU-built cascade box mips, vertex sampling at
`log2(spacing / texel) + 0.7`, slope-aware reflection darkening,
conservative clipmap block culling, owned-resource accounting, and a
sustained-overrun policy. These are original Rust and WGSL implementations
from the public MIT Tidewater sources cited in the source commits. No
licensed shaders, defaults, textures, or asset bytes are in Git.
Worker cost counts actual completed synthesis jobs, including superseded
results, and reports per-job cost separately. Native clocks measure thread
CPU time. Browser intervals measure elapsed time; waves run inline there,
so a separate worker CPU clock is unavailable.

Supported Medium and High physical renderers collect bounded delayed GPU
timestamps continuously. Missing results neither advance nor reset the
completed-observation overrun streak. Normal Low keeps its fused pass and
has no dedicated runtime water GPU controller; CPU and residency admission
remain active, with whole-frame fallback where available. Low's isolated
probe splits opaque, water, and blended passes with attachment load/store,
so its GPU cost does not directly measure the normal fused path. Mirror,
color/depth copies, and surface timestamps exclude shared opaque underwater
work and implicit queue uploads. Wet-minus-dry fence wall time is a
separate estimate. Memory is declared owned water GPU resources and CPU
wave/mip/ripple storage; shared resources and driver padding are excluded.
Requested cadence alone does not establish removed work for a pond with
no spectral jobs or admitted optics.

At the frozen source, scoped PBR formatting passes. Remote focused water
library tests pass: engine 6 and PBR 55, with `water::parity` excluded.
Shared shader validation passes both default and GLES variants. The
explicit `everglade-web` wasm32 check passes. Both the Mac release filtered
native example and remote release filtered WASM artifact compile with
`--no-run --message-format=json`. The earlier no-default-features Verse
capture consumer check passes; remote default desktop audio requires
absent `alsa.pc`, so do not install system packages for this task.

The previous full collection and its rendering diagnosis remain in
[`frozen-732c49a582`](../../bench/verse/2026-10-08/water-w11/frozen-732c49a582/receipt.json).
All four old WebGPU captures were black: the fresh `verse neon blended`
pass lacked scene bind group 0, invalidating dry and wet command buffers.
Timestamp mappings succeeded but returned zero counters from rejected
submissions. The corrected source binds group 0 inside `draw_blended`.
The browser harness now retains device errors and `Log.entryAdded`, rejects
blank captures, saves failures, and closes every tab.

The corrected 2026-10-08 evidence is
[`corrected-499b25d083`](../../bench/verse/2026-10-08/water-w11/corrected-499b25d083/receipt.json).
It includes original compressed reports, compiler JSON, source/artifact/input
identities, command and lease receipts, and exact orchestration scripts.
At 1920 × 1080 on Chrome WebGPU, the Water Lab dry/wet pair renders
correctly with no device errors or loss. The wet interval has 726 valid GPU
samples: mean 2.767768 ms, p95 3.407872 ms. Main-thread elapsed mean is
1.112534 ms, p95 1.6 ms; this is not thread CPU time. It completes 90 inline
synthesis jobs averaging 0.431111 ms elapsed per job. Steady declared GPU
residency is 4,768,708 bytes and CPU wave/ripple storage is 1,329,792 bytes.
Copies, mirror, and SSR are absent in the final plan, with refresh cadence
8. The old Medium GPU and CPU targets still miss despite reduced optics.
The separate signed fence estimate is +1.991598 ms. Browser High is not
admitted. This establishes the WebGPU binding fix and query plumbing only.

The corrected native reruns use Apple M5 Max Metal, 1920 × 1080, 60 Hz,
and 96 steady samples. All three captures render the pond or waterline.
All complete zero spectral jobs, use 131,072 bytes of CPU ripple storage,
and retain their original plan and cadence.

| Native case | Exit | Valid GPU samples; mean | Main-thread CPU mean / p95 | GPU bytes | Separate fence estimate |
| --- | --- | --- | --- | --- | --- |
| Low pond-posts | 101, old budget assertion | 0; unknown | 0.143280 / 0.182750 ms | 989,164 | 1.995709 ms |
| Low waterline | 0 | 0; unknown | 0.133705 / 0.165875 ms | 989,164 | 0.617709 ms |
| Medium pond-noon | 101, old budget assertion | 1; 2.687709 ms | 0.181507 / 0.244750 ms | 28,855,068 | 0.111208 ms |

Low waterline's exit `0` is not a GPU-budget pass. Zero/one observations
cannot calibrate native GPU budgets or prove sustained GPU degradation.
The decoder currently rejects `end <= start` for every masked pass.
Do not relax it from the old browser's rejected submissions or invent zero
GPU costs. The next diagnosis must retain raw native slot intervals,
submission/device errors, mapping completion, and rejection reasons, then
distinguish legitimate zero-duration ancillary passes from invalid water
intervals or rejected submissions. Add a focused decoder fixture only if
those counters establish the required behavior.

Mac scratch is
`~/.openagents/scratch/codex-01a119ae-08ea-73f2-9344-ec959f74a795`.
The unfinished checkout is its `water-w11` directory. Corrected raw reports
and captures are under `w11-metrics-499b25d0839a01dbbbc524e1374b1ee5c3e52644/`:
`browser/`, `native-low-pond-posts/`, `native-low-waterline/`, and
`native-medium-pond-noon/`. The corrected browser wet capture is
`browser/water-webgpu.png`. Original compiler artifacts are retained under
`w11-native-499b25d0839a01dbbbc524e1374b1ee5c3e52644/` and
`w11-wasm-499b25d0839a01dbbbc524e1374b1ee5c3e52644/`.
The native executable SHA-256 is
`431de7347c7f73cf93bb60562245abded436a7f12ec7af38fae077d27a3de9d3`
(8,715,152 bytes). Original remote WASM SHA-256 is
`1bd632682a1ad6faf4e7f7e8f0e36b5e4b7bc3eff02f930316dcb3446b06b763`
(37,439,093 bytes); served WASM is
`f348b3670781e6eb2d2a50957ea2751e867f267f4b9dbcd7874f63005b04b1ba`.
Matched `wasm-bindgen` 0.2.128 staging removes only the `main` export from a
derivative and verifies normal startup reaches the app once, with no test
entry exports or implicit start. Original bytes and Cargo JSON remain
unchanged. This test-compiled derivative is for measurement only;
production requires the normal repository release image build.

The public input is `a82df378ca7d06d9c755ae24076c89270d8a8097509c54a166d941da05f9de2f`
(10,636,202 bytes). The private kit is
`dae1612d4c22438a933c27b406c1e18fe134b13eab8eb5240ddcf5506ffb0b93`
(10,238,689 bytes), in `~/.openagents/verse/private/medieval-town/packs/`.
Keep private input bytes and captures outside Git. Recheck both pins on a
fresh rebase; never alter a pin without its artifact submission queue.

Resume after the coordinator grants a window:

1. Read #10783 and its claim status before claiming it; set the board to
   In progress. Diagnose native timing validity before calibration.
2. Keep the remote checkout at
   `/home/christopherdavid/.openagents/scratch/process-2198653/water-w11-agent12`
   and its target at `~/work/openagents-target-agent12`. Advance only the
   clean checkout between commands. On the Mac, use the same numbered
   target. Run one Cargo command at a time, four jobs, through build leases,
   with the 25 GB disk floor. Preserve other agents' quiet leases.
3. For a justified source fix, rerun only affected water tests
   (`cargo test --locked -p verse-engine -p verse-pbr --lib water -j4 -- --skip water::parity`),
   shared shader validation, and the explicit wasm32 consumer check. Compile
   native `water_capture water_w11_fixed_views` and WASM `presence_ui::tests`
   release artifacts with filtered `--no-run --message-format=json`;
   derive exact paths from Cargo JSON and verify their hashes before staging.
4. Run the compiled native `w11::water_w11_fixed_views --ignored --exact
   --test-threads=1 --nocapture` with `WATER_W11_CASE=tier/view` and
   `WATER_W11_OUTPUT` in new scratch under quiet and GPU leases. Browser
   checks use `openagents browser run`, the staged candidate, and
   `WATER_W11_BROWSER_CASES=water-webgpu,everglade-webgpu,water-webgl2,everglade-webgl2`
   as granted. Use fresh receipts and 1080p buffers, retain every failure,
   and keep CPU elapsed and GPU estimates labeled. The retained scratch
   wrapper is locked to clean HEAD `499b25d083`; a new invocation must
   freeze its own source and output directory without overwriting evidence.
5. Review adequate per-tier native and browser data before replacing the
   old target constants or [water.md](water.md#budgets-per-tier). Prove an
   actual removed effect or work reduction under an overrun. Add meaningful
   policy regressions and put phone/device runs in `NEEDS_OWNER.md`.
6. Rebase on current main without overwriting scene-lit particles or owner
   steps. Review the remaining image/deployment scope with the coordinator,
   use a normal release build, and verify the deployed candidate before
   closing #10783. Do not start #10919, coast work, or close #10784 here.

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

The October 8 Metal timer diagnosis retains the two failed experiments in
`bench/verse/2026-10-08/water-w11/metal-timer-diagnosis/`. Deferring query
resolution until submission completion, without blocking the render thread,
passes all 15 Mac and all 15 RTX 4080 fixed views with 96/96 valid samples
per view. The records are in `metal-deferred-f176dfa0c8/` and
`vulkan-deferred-f176dfa0c8/` under `bench/verse/2026-10-08/water-w11/`.
Budget calibration and the refreshed browser check remain pending.
