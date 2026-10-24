# Representative engine benchmarks

Status: implemented prototype for [issue #7](https://github.com/OpenAgentsInc/aethyrforge/issues/7).
The shared runner extends the existing C-town tools. It keeps simulation CPU,
integrated rendering, startup, and edits separate, and runs one GPU world at a
time. The Town now exposes a small performance popup from its existing frame
counter, including attempts, submitted and skipped frames, dropped simulation
time, capped long-frame time, and partial graphics-memory accounting.

## One evidence contract

[`measurement.mjs`](../../../apps/aethyr/experiments/c-town/measurement.mjs) owns versioned
`aethyrforge.measurement.v1` reports, interpolated distributions, balanced trial
orders, frame summaries, native evidence references, and byte-budget checks.
Its [contract notes](research/c-town/MEASUREMENT.md) and
[JSON schema](../../../apps/aethyr/experiments/c-town/measurement.schema.json) describe the fields.
The original C-town `statistics` API keeps its median field and interpolation.
New reports use the same interpolation and preserve unavailable observations as
`null`, separately from measured zero.

A report pins actual source files and uncommitted changes, dependencies and build
flags, the kernel binary, working WGSL, asset inventory, quality, browser, OS,
GPU, viewport, cache conditions, warmup, and actual trial order. Historical
streaming, runtime-table, worker, agent, and player results retain their original
schema, source, timing boundaries, and limitations behind exact file hashes.
They are context rather than paired current-source candidates.

The runtime asset inventory reuses the deployment dependency resolver. It hashes
every selected scene, referenced document, geometry buffer, texture, collision
resource, rule, and character artifact before and after a run. Missing or changed
content fails qualification. It does not read assets during timed windows or
create another packager. The preflight read warms the OS file cache, so even the
fresh-context startup is not a cold-disk measurement.

## Workloads and independent checks

| Workload           | Measured work                                                                    | Correctness outside timing                                                                             |
| ------------------ | -------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| Owned starter      | Current Rust controller, direct WebGPU, static and moving camera                 | Known owned geometry, actual submissions, fixed pixels, typed edits and failures                       |
| Active 1k/10k/100k | Float32 Euler movement and enclosure bounce, followed by separate derived bounds | Independent object reference, exact state/bounds, all entities moving, five rehashed negative controls |
| Private Town       | Same renderer and controller, day/night, static/moving camera                    | Non-flat submitted geometry, fixed source pixels before/after, causal prop edits and shader safety     |
| Player profiles    | Exact delivered graph and content, empty/room/character                          | Shared artifact, exact editor/player views, real movement and independent quest rules                  |

The active-entity workload has a fixed algorithm, seed 12999, Float32 timestep,
and arithmetic order. It allocates resident arrays before timing and updates all
entities. It is a transform/physics micro-workload, not 100k complete game
entities with scripts, collisions, AI, rendering, networking, and persistence.
The independent reference and hashing do not enter its measured intervals.

The browser CPU runner uses five warmup steps and batches of 32 measured steps.
The integrated runner warms each window, then records one second of live work.
Both use paired, reversed instrumentation-on/off schedules. Raw samples and
failures remain outside Git. Slow windows stay in the result. One device cannot
distinguish shared-machine contention from every engine or driver cause.

After render timing, the harness checks the visible performance popup, performs
accepted add/transform commands through the normal authority, verifies causal
geometry and pixels, rejects rehashed black/missing/stale/wrong-camera images,
and confirms a failed shader retains player, world, code, and image. Source
quality checks are separate from timed work. Closing each context releases its
preview workers before another scene opens; cleanup failure fails the suite.

## Timing and memory boundaries

- `stepMs` includes the JSON/Wasm request, gameplay synchronization, and character
  camera updates. The bridge separates encoding, the Wasm request, decoding, and
  remaining overhead. Wasm request time includes Rust JSON decoding/encoding and
  simulation; it does not isolate arithmetic alone.
- `renderCpuMs` includes render preparation and submission. Culling, light-list
  preparation, and command encoding are nested spans. Command encoding can also
  contain preparation callbacks. These spans must not be added as disjoint work.
- Available GPU timestamps cover the main HDR pass. Other passes, isolated
  upload time, compositor presentation, and physical displayed FPS stay
  unavailable. Worker callbacks, accepted GPU submissions, and skipped attempts
  are distinct. Actual camera and animation time accompany every observation.
- Dropped-time fields are cumulative counters; window summaries subtract the
  first from the last. Long-frame capping in the host and dropped kernel time
  remain separate. Intentional pause/capture time is outside live windows.
- Accounted buffers, GPU resources, Wasm linear memory, cache high-water values,
  and heap snapshots are partial observations. They are not process-memory or
  total GPU-memory peaks.
- The off condition disables optional bridge, culling, light, encoding, and GPU
  timing. It retains the common recorder and normal product counters. Therefore
  the comparison estimates optional timer overhead above that common work, not
  the observer's entire cost. Tests prove disabled bridge/light clocks do not run
  and measured results preserve the actual Rust world and uploaded light data.

Fresh-context startup and same-context reloads are labeled separately. Chrome,
Wasm, shader, driver, OS, and disk caches are not reset. A faster later profile
cannot establish that adding its features improved startup. Build byte ceilings
are local regression checks, separate from proposed device or network targets.

## Reproduce

Build the kernel and run `npm run check`, `npm test`, and `npx vite build`.
Then run through an isolated browser profile:

```sh
openagents browser run -- node tools/benchmark-suite.mjs \
  --out "$OPENAGENTS_SCRATCH/fresh-engine-benchmark" \
  --town "$AETHYRFORGE_PRIVATE_ASSETS"
```

Omit `--town` for the public CPU and owned-world checks. Output must be a fresh
empty directory outside the checkout. The default includes static and moving
camera routes. Individual runners are `tools/benchmark-active-entities.mjs` and
`tools/benchmark-runtime.mjs`. The suite writes native and common reports and
links the earlier workstreams without changing their original measurements.

`npm run build:player` now enforces each profile's
[byte ceilings](../../../apps/aethyr/experiments/kaigen/player-budgets.json) after auditing all
served code and resources. Private Town has its own content budget; it is never
compared with the empty or owned room asset budget.

## Recorded baseline: October 23, 2026

The [scrubbed report](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/experiments/kaigen/results/issue-7.json) pins measured
implementation `4f412be`, including the newer building work from main. It records
24 balanced CPU samples and 48 integrated windows: 16 owned-world windows and
32 Town windows. All independent state, image, edit, shader-error, asset-stability,
and cleanup checks passed. Raw reports, captures, failed attempts, and earlier
runs remain private, referenced by exact hashes rather than pooled together.
Validation also passed 680 TypeScript tests, 64 Rust tests, and seven asset
inventory tests. A further eight-window owned-world run and empty-player check
verified the corrected network summary without pooling their timings into this
baseline.

This run used one Apple M5 Max Mac, Darwin 25.4.0, Chrome 155.0.8059.85, and its
Metal WebGPU adapter. Rendering used an 800 by 500 viewport; fixed checks used
512 by 320 pixels. The build was Vite development mode on localhost, without
network throttling. Asset hashing warmed the OS file cache before navigation.

| Active entities | Uninstrumented batch-average step, p50 | Instrumented batch-average step, p50 |
| --------------- | -------------------------------------- | ------------------------------------ |
| 1,000           | 0.0281 ms                              | 0.0281 ms                            |
| 10,000          | 0.2422 ms                              | 0.2437 ms                            |
| 100,000         | 2.1938 ms                              | 2.1531 ms                            |

Each count has four off and four on batches. The 100k workload uses 12.4 MB of
resident typed arrays and allocates no typed arrays in its timed loop. Its
instrumented simulation and derived-bounds medians were 0.50 and 1.65 ms. These
are partial storage and transform-workload measurements. Timer resolution limits
small inner spans; the negative apparent overhead at 100k is treated as variation,
not evidence of a speedup.

The owned world submitted 60–61 frames per one-second window. Town submitted
20–25 by day and 16–19 by night across the measured routes. These are window
counts, not displayed FPS. Town's on/off median submission-rate ratios ranged
from 0.950 to 1.040; this short run does not isolate a stable instrumentation cost.
The per-window stage distributions and slower observations remain in the report.

Town's controller-ready metric was 4.01 seconds in the fresh context and 4.14
seconds on reload; navigation-to-ready was 4.47 and 4.31 seconds. Its pinned asset
closure contains 465 files and 630,832,452 raw bytes. The live popup counted about
1,077 MiB of graphics resources. These observations justify tracking asset,
graphics, startup, and simulation costs separately; they do not establish a
language or threading winner. The frozen Druid Town fixture does not qualify the
separately added castle assets.

Chrome returned invalid negative response sizes during the owned-world reload.
The shared summary now reports that complete byte total as unavailable, retaining
known partial bytes and the raw observations separately. The publication records
this postprocessing fix without rewriting the measured source identity or raw
reports. Browser response sizes are not physical wire-byte measurements.

All four current player profiles passed byte ceilings and 43 browser checks,
including real input, quest behavior, artifact rejection, avatar switching, and
exact editor/player images. Empty delivers 5,917 gzip bytes of JavaScript and no
Wasm; room delivers 172,364, and character/Town deliver 179,279. Nonempty profiles
use the same 917,167-byte Wasm binary. The full Town content remains substantial;
removing editor dependencies does not shrink those assets.

No independent second device, displayed presentation timing, complete memory
peak, or isolated upload timing was available. Those measurements remain required
before broader hardware or language claims.

## Sustained qualification

The longer [frame-pacing qualification](performance-qualification.md) extends
these observations with frozen queue-completion ranges, full-command GPU timing,
source reference images, two resolutions, and live interaction/gameplay rows.
A successful short benchmark remains separate from a sustained matrix verdict.
