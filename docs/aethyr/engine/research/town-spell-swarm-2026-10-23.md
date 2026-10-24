# Medieval Town with 100,000 GPU spell creatures

Status: this phase stopped at the owner's request on October 23, 2026.
The spell is implemented and browser-verified. **Consistent smooth 60+ FPS was
not achieved.** No further optimization or benchmark job is scheduled.
The [performance thresholds](../performance-qualification.md) remain unchanged.

## What shipped

The Town editor has a floating **Summon 100,000** / **Clear spell swarm** control.
Manual and agent actions use the same validated command. The internal command
also accepts 10,000 and 200,000 creatures; those populations were not benchmarked
in Town during this phase. The swarm is a session effect, not a saved world edit.

The [Boids Lab](boids-lab-2026-10-23.md) simulation runs on persistent GPU buffers:
clear cells, aggregate their occupants, then steer and integrate. One instanced
HDR draw renders all 100,000 creatures, each with the complete animated
310-triangle mesh: **31 million additional triangles per submitted Town frame**.
Readbacks outside timed windows verify finite state and advancing positions.
There is no per-frame CPU flocking loop or state readback.

The existing Town resolution, geometry, materials, 4× MSAA, lighting, shadow
resolutions, reflections, fog, bloom, and output passes remain enabled. Eight
native knights remain active. No LOD or reduced-resolution setting was used.
This is a different workload from the standalone underwater lab; its results
cannot establish a Kaigen, Unity, or C-versus-TypeScript speed ratio.

## Rendering and ownership changes

- Source render bundles omit commands for source IDs never encountered by that
  view. Fixed 256-ID pages let newly visible geometry rebuild only affected
  pages. Pages execute in original source order and retain the full indirect
  command table. Failed uploads or page recordings cannot publish partial work.
- Stock near, far, and distant sun shadows reuse source bundles for their static
  and moving partitions. Original shaders, indices, winding, materials, wind,
  formats, and sample counts remain. Authored geometry, rooms, debris, custom
  shaders, and unsupported adapters retain their established drawing paths.
- GPU orientation preparation computes each creature's basis and seed once per
  frame, then shares it across vertices. It adds 4.8 MB at 100,000 creatures.
  Mesh detail, animated tails, and fragment shading remain enabled.
- Local lighting rejects clearly out-of-range squared distances before taking a
  square root and loading the remaining record. It retains the original range
  test and contributing-light arithmetic. Nearest-light selection reuses bounded
  storage while preserving the previous eligibility, ordering, and stable ties.
- Normal rendering follows display callbacks. The explicit 60 Hz test profile
  remains available, and outstanding submissions remain bounded to two frames.
- The capacity fence is registered before new profiler buffer mappings.
  [WebGPU's queue contract](https://gpuweb.github.io/types/interfaces/GPUQueue.html#onSubmittedWorkDone)
  includes prior mappings in work-done completion. These chronological tests do
  not isolate a causal performance benefit from changing that order.
- The editor owns the optional swarm host, cancellation, GPU cleanup, and memory
  accounting. Static players omit its implementation. Grass also loads through
  a declared editor feature instead of entering unused player profiles. Explicit
  audited private-name minification preserves existing delivery ceilings.
- Routine memory-status updates read the existing byte counter directly instead
  of constructing the complete benchmark report.

These are implemented structural changes. The measurements below do not prove
that each change independently improves FPS.

## Results and limits

The retained [measurement summary](data/boids/town-2026-10-23.json)
includes failures, source/report hashes, per-second counts, timing scopes,
quality checks, and cleanup outcomes. Private raw records and licensed images
remain outside Git.

Chrome 155 on the Apple M5 Max used an actual 1280 × 720 canvas, night spawn,
and a moving-camera route. Every row below contains all 100,000 creatures.

| Cohort | Recording / warmup | Accepted / completed | Rate | Full GPU p95 | Strict / application verdict |
| --- | --- | --- | --- | --- | --- |
| Initial integration | 3 s / 1 s | 180 / 180 | 59.936/s | 14.877 ms | Fail / fail |
| GPU orientation preflight | 5 s / 2 s | 325 / 325 | 64.958/s | 12.701 ms | Pass / fail; short diagnostic |
| Fence ordering, instrumented | 30 s / 5 s | 1,844 / 1,844 | 61.461/s | 14.352 ms | Fail / fail |
| Profiling disabled | 30 s / 5 s | 1,824 / 1,824 | 60.794/s | Unavailable | Fail / fail |
| Final paged bundles and light selection | 10 s / 5 s | 605 / 605 | 60.481/s | 14.418 ms | Fail / fail |

The last window has one-second accepted counts from **57 to 64**, a 58.4 ms
maximum accepted-opportunity interval, and a 64.1 ms maximum fence latency.
Its median render CPU time is 3.4 ms; full GPU time is 11.93 ms median and
14.42 ms p95. These nested execution spans overlap and must not be added.
An average above 60 is not evidence that the consistency goal was met.

The profiling-disabled run omits CPU/GPU stage timing. Existing verdicts still
require GPU evidence and are retained as failures; it is not a qualified result.
The legacy strict timeline check also compares enqueue time with scheduled rAF
boundaries, which can fail when CPU work extends beyond the next scheduled
callback. Actual accepted-cadence and latency failures remain in the evidence.

GPU timing covers the render/compute-pass envelope, excluding uploads, copies
outside that envelope, and browser composition. Submissions and fence callbacks
are not physical display presentation. Host activity was not fully isolated.
The earlier [phase wrap-up](medieval-town-performance-phase-2026-10-23.md) retains
independent evidence of fixed-work CPU execution-rate drift; its cause remains
unresolved. No individual optimization or profiling-overhead speedup is inferred
from these chronological cohorts.

## Quality and verification

The original Town night-spawn image remains pixel-identical at 1280 × 720 across
the initial integration, later optimizations, and final spell restoration:
`77d9650074b2d09be6321b542fe7d1bbf708fd5e5758c9bbd2b24140bb98a631`.
Captures pin camera, environment, animation time, and canonical native knight
poses. The complete purchased asset closure stays unchanged: 470 files,
637,679,875 bytes. Swarm-on captures independently establish visible added work.
The real Summon and Clear buttons pass the final browser check.

The 1,000-creature, 256 × 256 HDR orientation comparison is not bit-exact:
one color channel of one pixel differs by one half-float step
(9.5367431640625e-7). Coverage and alpha are identical. The fixture checks that
bound explicitly; it does not claim universal image identity. An intermediate
report incorrectly mixed ULP and absolute-error units in one diagnostic field;
that failed record is retained, and later reports correct the field.

Verification completed: 1,089 tests passed, two optional fixtures skipped, and
TypeScript checking passed. All four player delivery builds passed before the
final paging changes. Room and Character were rebuilt afterward and passed
unchanged ceilings at 653,887 and 687,621 JavaScript bytes respectively. The full
private Town closure was not rebuilt again after paging; no asset bytes changed.
Browser checks retain unchanged source/asset hashes, live GPU movement, complete
population/triangle counts, and empty app-error and cleanup-error lists.
The final ten-second diagnostic is not a sustained qualification.

## Future work

[Issue #8](https://github.com/OpenAgentsInc/aethyrforge/issues/8) tracks the
remaining work. It is open; this phase does not satisfy its acceptance criteria.
The later [renderer redesign plan](../town-renderer-redesign.md) organizes that
future work into phases, starting with compiled batches and specialized materials.
It adds no implementation or new benchmark result.

1. Explain and control fixed-work CPU execution-rate drift before attributing
   chronological FPS differences to engine changes.
2. Capture correlated worker CPU, Dawn/driver, upload/copy, GPU execution, and
   fence-delivery traces. A vacant queue slot does not by itself prove GPU idle.
3. Evaluate one coalesced render retry when GPU capacity returns, retaining the
   two-frame bound. Native rAF must consume input and advance simulation once;
   a retry may render only the newest prepared state. Record retry origins and
   actual entry times separately, with versioned metrics and cancellation tests.
4. Measure command-page size and broader geometry/material batching with exact
   source-image controls. Keep an optimization only with controlled evidence of
   useful headroom; do not infer a win from fewer JavaScript calls alone.
5. Rebuild all delivered profiles, then run repeated 30-second windows across
   day/night, static/moving cameras, controllers, destruction, spells, knights,
   and the editor/player resolutions required by the qualification contract.
   Retain every failed interval and verify compositor presentation separately.

To reproduce the targeted instrumented check, use prepared private assets and an
external capture directory:

```sh
AETHYRFORGE_PRIVATE_ASSETS=/path/to/purchased/project \
AF_CAPTURE_DIR="$OPENAGENTS_SCRATCH/town-swarm-check" \
AF_TOWN_SWARM_ORDER=100000 AF_TOWN_SWARM_WINDOW_MS=30000 \
AF_TOWN_SWARM_WARMUP_MS=5000 \
openagents browser run -- node tools/verify-town-swarm.mjs
```

`AF_TOWN_SWARM_INSTRUMENTED=0` omits optional stage timing; it does not qualify
GPU budgets. Scenario, shot, route, population order, and short diagnostic windows
are recorded explicitly. No further run is scheduled by this document.
