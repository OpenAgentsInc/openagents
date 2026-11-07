# Echo rendering follow-up — November 7

The rendering changes, deterministic orbit comparison and compatibility checks
pass. The afternoon follow-up below attributes the live 60 FPS miss to another
app's GPU work, measures the scene's own GPU cost per frame, re-records the
orbit on current `main`, and closes #12984. No production deployment is claimed
by this record.

The source base is `7d153a7c146e2a7658f562dc434082a185b41d4b`.
[performance-source.json](performance-source.json) identifies each changed code
file, compiler and actual Wasm. The final 2,040,495-byte Wasm is below the
2,150,000-byte budget; its build runs all 1,443 contextual checks successfully
in 385.7 seconds. Individual commands remain below 15 minutes.

## Rendering work removed

- `aethyr/render/grass-root.coax` builds conservative row ranges against the
  existing grass shader's root-clip planes. Four stored terrain heights bound
  each one-metre root cell. Accepted cells retain every original instance ID,
  density, seed, wind response and material. The square-grid profile requires
  finite admitted views/heights, integer metre grid spacing/origin, radii at
  most 32 m and densities at most 64; it is qualified for Everglade's bounded
  coordinates, not arbitrary far-origin arithmetic. Unsupported grid alignment
  keeps the full original draw. At most 128 ordered ranges are produced.
- The optional grass-range v1 browser ABI executes those ranges. The observed
  staged frame submits 35,824 rather than 104,576 grass instances in 36 draws.
  Old producers retain their single full-domain draw. Opaque geometry precedes
  grass so hidden fragments can fail depth early; a producer with the existing
  blend ABI retains its previous ordering. Native/shared-graph adapters still
  use their existing full-domain route.
- Lamp and contact-shadow bounding boxes reject exact-zero contributions before
  distance/lighting work. Ordinary lamps with exact-zero daylight gain are
  skipped before the trailing spell lamps. Frozen pre-change sums check exact
  CPU float bits, including night/mixed lighting and overlapping shadow levels.
- Unused water vertices return before sampling any nodes. A red check with empty
  node views exposes the old read-before-rejection; live-domain vertices retain
  every frozen output field.
- The background triangle computes inverse projection at its three vertices.
  Homogeneous world points interpolate, followed by the original per-pixel
  division and normalization. Rotated/translated camera checks compare this
  with direct per-pixel unprojection at a 0.0002 component tolerance. The real
  WebGPU pipeline and a separate SPIR-V emission accept the added varying.

These changes keep the existing two-frame admission limit, raster, graphics
quality ladder, groom levels, eight skin influences and lighting parameters.
Experiments with a longer queue and late background rendering were rejected;
those product changes are absent.

## Correct measurements

An unobserved trace callback (`-1`) is no longer classified as a submission.
The lifecycle refusal path explicitly records a held callback. Held callbacks
still accumulate into the next drawn frame's gap. Current qualification also
requires actual world draw calls and a fresh hero submission in every sampled
frame; stale character readiness cannot qualify a fast incomplete scene.

The strict rule is unchanged: more than 30 frames, p95 at most 17.5 ms, and
no frame over 33 ms. The existing ready/groom/full-raster requirements remain.
The new provenance boundary kills all 30 mutants; grass admission/access kills
all 206. Both have zero survivors or equivalents. The shader suite passes all
113 checks. The rebuilt producer's 1,443 contextual checks pass.

Actual SDK checks cover range execution, old-ABI fallback, synthetic blend-ABI
ordering and GPU replacement. All pass with zero console errors. Removing the
range ABI while demanding range execution fails as an independent negative
control. The synthetic blend export returns opaque profile 0; this verifies
ordering compatibility, not a new translucent art profile.

## Fixed-epoch image comparison and recording

Fixed animation steps previously left weather tied to epoch time. The capture
fixture now freezes the page epoch to `1794009600000` (2026-11-07 00:00 UTC),
uses zero-dt readiness polling, and prefixes a clip room's worker clock when
that worker exists. A clock assertion records whether a local worker is present.
The staged orbit has no room worker. Real-time FPS commands do not load this
clock fixture.

Two complete 217-frame orbits at 1920×1080 use the same epoch,
`clock=0.95&weather=rain`, asset packs and clip steps. The baseline uses the
original host/WGSL with the compatible current producer; the candidate uses
the optimized rendering. Capture precedes the trace-only provenance addition.

The independent Pillow oracle compares all RGB8 channels. The oracle enforces limits of mean ≤0.25, worst-frame mean ≤0.5, p99 ≤2, and at most 0.01% of channels
differing by more than 2. Results:

| Measure | Result |
| --- | ---: |
| Mean absolute RGB8 difference | 0.000462784 |
| Worst frame mean | 0.002032054 |
| 99th-percentile channel difference | 0 |
| Fraction of channels differing by more than 2 | 0.000004687 |
| Maximum channel difference | 96 |

This is numerical image parity within the stated tolerance, not byte identity;
the sparse extreme tail is retained rather than hidden. A wrong-camera-frame
negative control fails. Earlier comparisons with unfrozen weather were discarded
because rain/wind differed between runs. No licensed pixels are committed.

The new private `orbit.mp4` is H.264, 1920×1080, 24 fps, 217 frames and
9.041667 seconds, recorded directly from the staged Everglade page. Encoding
only assembles the actual GPU frames; there is no matte or scene compositing.
Capture stepping waits for the GPU and does not establish real-time 60 FPS.

## Live performance status

All live runs use muted headless Chrome 157, hardware Apple Metal WebGPU,
DPR 1 and the actual requested 1920×1080 raster on AC power. Owner applications
remain running. No thermal/performance warning was reported by `pmset`.

Initial unpinned medium diagnostics improved from about 53 to 60 FPS. An
intermediate pinned rain/night run also reported 300 frames at 60 FPS while
shader replacement was in progress. That window is **not accepted** as final
qualification. Fresh final-artifact rain/night windows before the additional
provenance fields instead measured:

| Shot | Frames / FPS | p95 / max, ms | Held |
| --- | --- | --- | ---: |
| Wide raid, first cast | 192 / 38.4 | 33.335 / 50.000 | 108 |
| Medium, orbit held | 189 / 37.9 | 33.335 / 33.340 | 111 |

With every headless test browser closed, subsequent IORegistry observations
reported 89% and 99% device/renderer GPU utilization. This establishes competing
GPU work outside this qualifier, without identifying or modifying its owner.
The historic R4 qualification explicitly closed competing 3D tabs. The subsequent strict provenance runs confirm at least 94 world draws in every
wide frame and 81 in every medium frame, with the hero submitted in every frame.
They measure 37.5 FPS wide (p95 33.335 ms, max 50.005 ms) and 45.6 FPS medium
(p95/max 33.335 ms), both failing. These receipts include the final host and
qualification boundary; they are not fast empty-scene measurements. A matching
controlled window is still needed; it is recorded in `NEEDS_OWNER.md`.

## Afternoon follow-up: who uses the GPU, and the scene's own cost

Source: `main` at `a141f28617`, with the Everglade glade lighting (`361baffd85`)
and the flyover pass timer (`a141f28617`). The Everglade Wasm is 2,044,871 bytes.

**Who uses the GPU.** IORegistry keeps each Metal client's accumulated GPU time
(`AGXDeviceUserClient`, `AppUsage.accumulatedGPUTime`, in nanoseconds). Two
snapshots 5 s apart give each process's share of the GPU. With no qualifier
running, the owner's visible Chrome (two tabs of the 8861 dev server:
`/worlds/everglade` and `/worlds/character-creator`, both drawing 3D every
frame) used **84–92%** of the GPU and WindowServer 6–9%. That is the competing
load the earlier runs saw. Agents do not close the owner's tabs, so a fully
quiet window is not available from an agent session.

**The scene's own cost.** During each qualifier run the snapshots attribute the
new GPU load to the run; the owner's Chrome drops to its time slice. The run's
share of the GPU divided by the frames it drew gives its GPU time a frame. On a
time-sliced GPU, a GPU-bound client draws as many frames as its share pays for,
so this measures the work, not the slice. Cache contention can only inflate it.
The raw lines are in `gpu-share-*.log`. In three of them IORegistry lists the
run's Metal client under a process id that `ps` later shows as
`VTDecoderXPCService`; that client appears only while the run is open, and its
share is the run's share in the table.

| Shot, 1920×1080 | Run's GPU share | Frames / FPS (shared GPU) | GPU time a frame | Owner's Chrome share |
| --- | ---: | --- | ---: | ---: |
| Medium (morning source, `848cef5166`) | 32.0% | 258 / 51.6 | 6.2 ms | 58.4% |
| Medium (current `main`) | 32.6% | 230 / 46.0 | 7.1 ms | 57.4% |
| Wide raid, first cast (`848cef5166`) | 31.8% | 203 / 40.7 | 7.8 ms | 63.8% |
| Wide raid, first cast (`848cef5166`) | 31.4% | 189 / 37.9 | 8.3 ms | 58.9% |
| Wide raid, first cast (current `main`) | 18.9% | 119 / 23.9 | 7.9 ms | 36.6% (and 35% from a third client) |

The wide shot's cost stays at 7.8–8.3 ms a frame while its share of the GPU
falls from 32% to 19% and its frame rate from 41 to 24 FPS: the cost is the
scene's, and the frame rate follows the share other apps leave it.
Every run confirms a ready Echo with hero draws in every frame, at least 81
(medium) or 91 (wide) world draws a frame, the full 1920×1080 raster, hardware
Metal and no console errors. The CPU stays at 1.9–3.3 ms p50 a frame.

The heaviest shot needs at most about 8.3 ms of GPU a frame, half of the
16.7 ms a 60 FPS frame allows. This agrees with the historical R4 strict pass
(wide and medium at 60.0 FPS with zero held frames in 12 windows; wide 1080
submit-to-done p50 13.7 ms) taken with competing 3D tabs closed.
The wall-clock misses (24–52 FPS) follow from other apps taking 57–72%
of the GPU while the qualifier runs, not from the scene.

**Pass timer.** `everglade-echo-perf.coax` now also turns on the host's pass
timer (`coaxialPassTimer`, from #13017) and prints a second line: the first
pass's start to the last pass's end, and the passes' own times added up. The
line is a report; it does not change the 60 FPS judgment. On a shared GPU the
span includes the other app's slices (medium: p50 19.0 ms, p95 28.9 ms), and on
Apple's tiled GPU the summed pass times overlap, so neither is the scene's cost
when another app draws. On a quiet GPU the span is the GPU frame time.

**The recording on current `main`.** A fresh 217-frame orbit records straight
from `/worlds/everglade?stage=echo&clock=0.95&weather=rain` with the glade
lighting: every frame passes with no console errors, the pack loads in 20.3 s,
and the groom draws 36,764 strand segments. `orbit.mp4` is H.264, 1920×1080,
24 fps, 217 frames (9.04 s), under
`~/.openagents/verse/private-build/echo-12984-final/` (private; no licensed
pixels are committed). Frames show Echo in the rainy town square at night, lit
by the fountain brazier and the windows, with the player passing in front of
her and the grass and houses in front of and behind them both.

A strict wall-clock rerun with the owner's 3D tabs closed is listed in
`NEEDS_OWNER.md` as an optional check; the issue does not wait on it.

## Reproduction

Run from this worktree with its own local server; never open a visible browser.
All licensed inputs and rendered output remain in the existing private build
area. Build/source identities are in the JSON receipt.

```sh
coax/bin/coax test aethyr/render/grass-root.coax
coax/bin/coax mutate aethyr/render/grass-root.coax --root-only --jobs 4
coax/bin/coax test verse/everglade/gpu.coax
coax/bin/coax mutate coax/web/everglade-echo-proof.coax --library --root-only --jobs 4
coax/web/tool.sh coax/web/grass-root-check.coax "$base?stage=echo" ranges
coax/web/tool.sh coax/web/grass-root-check.coax "$base?stage=echo" legacy
coax/web/tool.sh coax/web/grass-root-check.coax "$base?stage=echo" blend
coax/web/tool.sh coax/web/grass-root-check.coax "$base?stage=echo" recover
# Expected exit 1:
coax/web/tool.sh coax/web/grass-root-check.coax "$base?stage=echo" negative
coax/web/tool.sh coax/web/everglade-echo-perf.coax \
  "$base?raid=preview&stage=echo&clock=0.95&weather=rain" wide 1080
coax/web/tool.sh coax/web/everglade-echo-perf.coax \
  "$base?stage=echo&clock=0.95&weather=rain" medium 1080
```

The capture tool's output directory must exist. `clip-clock.js` is a headless
fixture only; ordinary realms do not load it. The private paired images, image
JSON and encoded clip are under `~/.openagents/verse/private-build/echo-12984-opaque/`.
