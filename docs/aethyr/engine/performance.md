# Performance budgets and measurement

Status: prototype with [measured engine baselines](representative-benchmarks.md).
The measured Town baseline falls below 60 completed frames per second. The
[sustained qualification matrix](performance-qualification.md) defines the
required current checks; no supported 60 Hz profile has passed that whole matrix.
The targets below remain acceptance goals, not measured guarantees.

## Define what becomes faster

Track independent spans: input-to-UI feedback; command interpretation; validation;
document commit; incremental compile; candidate upload; first submitted frame;
observed visible change; durable local save; remote sync; publish admission; and
player release resolution. A single elapsed “AI edit” number cannot locate a
bottleneck. CPU command submission is not GPU completion or display presentation.

## Qualification profiles

| Profile             | Pinned test environment requirement                                                 | Steady-play goal                                    |
| ------------------- | ----------------------------------------------------------------------------------- | --------------------------------------------------- |
| Desktop integrated  | 16 GiB system memory, modern integrated GPU, exact model/OS/browser/driver recorded | 60 Hz target, p95 frame interval ≤20 ms             |
| Desktop discrete    | 16 GiB system memory, 6 GiB-class GPU, exact configuration recorded                 | 120 Hz optional profile; p95 ≤10 ms on its workload |
| Mobile              | Physical supported phone/tablet, exact device/OS/browser, sustained thermal run     | 30 Hz baseline, 60 Hz qualified quality profile     |
| Constrained network | 10 Mbit/s downstream, 5 Mbit/s upstream, 80 ms RTT, loss profile recorded           | Required cold-load/publish test condition           |
| Local/offline       | Cached engine plus required project artifacts, no service reachable                 | Direct authoring and saved-branch recovery work     |

These are workload-specific profiles, not a promise that every modern phone or
browser meets them. The first implementation gate selects at least two GPU
vendors, a physical mobile device, browser revisions, and exact hardware. Store
that manifest with evidence before claiming a supported profile.

## Interaction and iteration targets

All latency targets below are p95 over at least 100 edits after a defined warm-up,
with at least three process restarts. Record p50/p95/p99 and maximum, including
timeouts and refusals. Cold startup is measured separately.

| Operation                       | Boundary                                                                             | Desktop target           | Mobile target            |
| ------------------------------- | ------------------------------------------------------------------------------------ | ------------------------ | ------------------------ |
| Direct tool feedback            | Input event to selected/highlighted/drag feedback                                    | ≤50 ms                   | ≤80 ms                   |
| Cached object transform         | Validated local command to first observed updated frame                              | ≤50 ms                   | ≤100 ms                  |
| Small structural edit           | Local accepted document to complete changed render/collision/nav generation observed | ≤150 ms                  | ≤350 ms                  |
| Material constant               | Validated edit to observed uniform update                                            | ≤50 ms                   | ≤100 ms                  |
| WGSL pipeline edit              | Save to validated candidate observed; valid reference shaders                        | ≤500 ms                  | ≤1,000 ms                |
| Small behavior edit             | Save to checked/migrated bytecode active in same play                                | ≤250 ms                  | ≤500 ms                  |
| Natural-language instant action | Submit to accepted/applied local action, including inference and save                | ≤500 ms regional service | ≤800 ms regional service |
| Warm reopening                  | Open cached project to controllable first frame                                      | ≤1 s                     | ≤2 s                     |
| Cold first play                 | Navigation to controllable first area under constrained network profile              | ≤5 s                     | ≤8 s                     |

Driver pipeline compilation, cold model loading, imports, and large generated
changes may exceed instant budgets. They must show progress, retain the running
world, support cancellation, and record the miss. Quality must not be reduced
silently to disguise a regression. These targets require experiments and may
need an explicit profile revision when physical limits invalidate them.

The semantic instant profile is bounded in [AI authoring](ai-authoring.md).
Parsing an exact registered command locally avoids inference, but never bypasses
authorization, context capture, or validation. A newly generated behavior or asset
uses the reviewed job path. An unlimited engineering team cannot make arbitrary
generation a guaranteed sub-frame operation.

## Reference workloads

Create owned/publicly redistributable seeded workloads, not licensed vendor
captures. Pin source digest, toolchain, engine ABI, profile, viewport, resolution,
and thermal conditions for each run.

- **W1, editable village:** 20 semantic buildings, 200 rooms, 2,000 walls, 10,000
  placed objects with repeated meshes, 50 active behaviors; add room, cut opening,
  remove partition, update prefab, and enter each changed path.
- **W2, streamed region:** 100,000 placed objects across cells, at most 10,000
  resident visible/simulation-relevant objects; traverse boundaries under cold
  cache and constrained bandwidth. Counts do not require simulating all distant
  objects every tick.
- **W3, behavior stress:** 1,000 small behavior instances with bounded action/query
  budgets, plus a deliberately runaway guest; unrelated input stays responsive.
- **W4, collaboration:** ten active creators in one region/branch, 50 presence
  subscribers, conflicting topology edits, reconnects, and independent dragging.
- **W5, multiplayer:** 32 players in one release/session, recorded server tick
  cost, prediction correction, packet loss, and session migration failures.
- **W6, iteration endurance:** 1,000 alternating valid/invalid structural, shader,
  and behavior edits; canceled jobs, cache pressure, device loss, and reopen.

Each workload has a baseline visual-quality manifest. Performance improvements
must report changed quality, supported semantics, and total resource use.

The [Medieval Town parity effort](references/medieval-town.md) adds a private
licensed comparison workload: fixed recipes for 16 scenic references, an asset
overview, and a day/night comparison. Report still matching and interactive
performance separately. Pin lights, shadows, sky/clouds/fog, textures/LOD, water,
particles, antialiasing, and source/engine identities; lowering those settings is
a different quality profile. Public benchmark distributions use eligible original
equivalents rather than bundling purchased assets or the downloaded gallery.

## Memory and first-view budgets

Initial desktop admission budgets: 512 MiB engine-owned CPU buffers/heaps and
512 MiB GPU allocations. Mobile starts at 128 MiB CPU and 128 MiB GPU. These are
engine accounting limits; total process/driver memory is measured separately.
Candidate activation, captures, import jobs, and caches count toward peak budgets.

Target a ≤3 MiB compressed first-play working set for the small entry workload:
selected JS/Wasm modules, first area geometry/textures, fonts, and metadata.
Defer editor tools and nonvisible cells from the player startup path. The full
engine or project can be larger; lazy loading cannot delay required collision or
rules after a player enters an area. Record cold bytes, requests, decompression,
Wasm compilation/instantiation, GPU uploads, and first stable input separately.

Steady state should avoid unbounded allocations and unnecessary per-entity bridge
calls. At identical world/quality, resource counts and resident bytes after W6
must return within a documented tolerance of baseline after cleanup. “One pipeline”
is not a universal goal: correct bounded material variants and pass budgets matter.

Measure React DOM commits, atom recomputation/notification, and Effect 4 service
work separately from kernel ticks and GPU encoding. Repeat the reference runs
with inspector churn, rapid UI updates, slow status consumers, and no runtime-data
subscribers. The [frontend contract](frontend.md) forbids a UI wait dependency in
the frame loop, per-entity reactive mirrors, and unbounded summary queues. Its
ten-Hz continuous diagnostic-view default does not throttle input, selection,
command receipts, or engine frames. Shared browser-resource contention still
counts toward the integrated latency/frame budget.

## Publishing and build budgets

With blobs already uploaded, target ≤2 s regional release admission/channel update
and ≤5 s healthy-client discovery at p95. End-to-end publishing additionally counts
upload and missing-artifact build time. Report those costs; 10 MiB cannot upload
instantly over a 5 Mbit/s link. New clients pin exact releases; running sessions
follow the migration policy rather than switching versions mid-frame.

Small TypeScript/editor changes target ≤250 ms warm rebuild. A small isolated
Rust/Wasm kernel leaf targets ≤5 s warm rebuild; global optimization or ABI
changes have separate measured budgets. These are developer targets, not reasons
to compile engine code for room or asset edits. Incremental caches must account
for their memory, disk, correctness, and cold-machine cost.

## Evidence and decision rules

Collect CPU spans, queue delays, frame cadence, allocation/GC events, GPU timestamp
queries where supported, memory high water, transferred bytes, and device-loss
events. When GPU timestamps are unavailable, mark GPU timing unavailable rather
than inferring it from requestAnimationFrame. Keep local preview, remote commit,
and server tick measurements distinguishable.

An optimization qualifies only after equivalent outputs, failure behavior, and
resource budgets pass across the supported profiles. Record a hypothesis, input,
baseline, change, results, uncertainty, and regressions. Faster Rust, GPU-driven
passes, shared memory, or another renderer architecture are workload decisions,
not conclusions inferred from a language or library name.
