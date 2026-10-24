# C/Wasm Medieval Town slice: implementation and measurements

Status: experimental implementation with native, Node, and browser measurements,
October 23, 2026. The C path works, matches the baseline's fixed-camera images,
and supports live edits and compatible module replacement. It reduces CPU work
and leaf rebuild time; it does not show a reliable submission-rate improvement
for the measured town. This is an exploratory run on one busy Mac, not a
production language migration or a finished C creation engine.

The experiment implements a real C/Wasm path through the existing Medieval Town:
movement, resident source-instance visibility and compaction, and clustered-light
preparation. It also implements controls that remove repeated validation and
allocation without changing everything to C. The important comparison is between
those complete paths, with the same assets and GPU work, rather than between an
isolated C loop and an entire editor.

## Scope and frozen comparison

The source baseline is `ae4ac9a794940309cbba5aa244d8dbd447e7f8ad`, with the
experimental changes added in a separate checkout. The baseline Rust module is
rebuilt from that source; its SHA-256 is
`24d0cf3815ac19978a3d26edd96eaa631f15e34f057f4602ba10515a655552b2`.
The experiment excludes the original checkout's then-uncommitted character and
triangle-collision controller work. That work subsequently landed on main and
is preserved. The four-way numbers do not compare that newer controller.
Prepared assets contain character/collision links that the frozen code ignores.

Day timing bundles were frozen at `87ae58c`; subsequent capture-corrected bundles
were frozen at `eac0437`. The latter changes capture invalidation, verification,
and compatibility guards, without changing the measured live rendering
algorithm. The result summaries identify the harness checkout; the frozen bundle
manifest identifies the emitted runtime. Documentation and verification tooling
were edited independently of the served production bundles.

Before publication, Git agent attribution was corrected to Codex. The
[commit map](c-town/results/2026-10-23-commit-map.json) binds
original local measurement IDs to published commits with identical source trees.
Raw evidence hashes and numerical measurements are unchanged; public records
preserve original identities alongside the published source references.

The [artifact record](c-town/results/2026-10-23-artifacts.json)
contains emitted bundle/module hashes and the private asset-manifest hash. The
private snapshot contains 1,414 files totaling 1,725,660,343 bytes; only the
requested subset is counted in the startup table below.

The comparison authority has its own `public/c-town-authority.wasm` artifact;
rebuilding the current product's `public/kernel.wasm` does not replace it.
[The frozen-authority builder](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/experiments/c-town/build-frozen-authority.mjs)
extracts only the pinned workspace manifests, toolchain file, and kernel sources
into an operator-selected private scratch directory, builds with the pinned
toolchain and shared source-keyed Cargo state, and records source/output hashes.
Simulation conformance tests and CPU comparisons explicitly load that frozen
artifact. Their scope remains the pinned profile after newer controller work
lands on `main`.

One copied private asset tree supplies every variant. The CPU run records these
scene-document hashes:

| Scenario | Instances | Primitive draws | Source lights | Wind instances | Scene SHA-256 |
| --- | ---: | ---: | ---: | ---: | --- |
| Day | 8,118 | 1,952 | 0 | 76 | `7ab12bf14bd41bf647cadc7bf43557c285c1e62a3e2c4e05c855692afbb4f83a` |
| Night | 8,124 | 1,956 | 488 | 76 | `60d423d0445270f4d693228d09f7401e078bce54b8a87d14fcab16a0ce662660` |

Licensed assets, camera positions, captures, and raw browser traces stay outside
Git. The public summaries retain bounded statistics, counts, compiler details,
and content hashes. The [experiment guide](c-town/README.md)
contains the build and reproduction commands.

## Implemented variants

The build-time backend selection cannot be changed by world content.

| Variant | Movement | Source visibility and compaction | Clustered lights | Authoring |
| --- | --- | --- | --- | --- |
| `baseline` | Existing Rust kernel through JSON | Existing TypeScript | Existing TypeScript | Existing Rust kernel |
| `rust-packed` | Matched packed Rust module | Existing TypeScript | Existing TypeScript | Existing Rust kernel |
| `ts-packed` | Matched packed Rust module | Resident, pooled TypeScript | Existing TypeScript | Existing Rust kernel |
| `c` | Packed C module | Resident C module | C core with shared TypeScript input validation | Existing Rust kernel |

After merging the newer character work, current main adds a distinct `c-frame`
option. It uses the **current Rust/JSON controller** and C render preparation;
the druid, third/first-person camera, triangle collision, and authoring remain
current Rust behavior. This is explicitly a different integration profile.
Current town workers reject the older packed-movement profiles instead of
ignoring triangles or inventing missing character state. The complete historical
C player remains reproducible at `eac0437`; it is not relabeled as a port of the
new controller.

`rust-packed` isolates the movement boundary. `ts-packed` adds a useful control
for scene residency, validation frequency, and output reuse. There is no matched
Rust culling or clustering implementation in this slice. The C clustering result
therefore does not isolate C versus Rust, and the full C variant changes more
than one CPU subsystem at once.

All variants retain React DOM, Effect Atom, and Effect 4 for the editor. The
worker's frame loop stays independent of React renders and atom subscribers.
TypeScript still owns browser and WebGPU objects, asset loading, source parsing,
GPU resource creation, render-pass encoding, shader compilation, and submission.
The WGSL, materials, lighting profile, shadows, source effects, resolution, and
quality settings are shared. No Kaigen engine, binary, or SDK is linked; the
experiment applies its data-layout and subsystem-boundary ideas to owned code.

### C render preparation

[The frame core](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/experiments/c-town/c/frame.c) stores source-instance
bounds, model matrices, wind parameters, draw ranges, visibility, compacted
outputs, and source-index mappings in linear Wasm memory. Bounds and camera
calculations use the selected precision needed to reproduce the current control;
GPU-facing model and wind buffers remain float32.

Installation validates complete scene arrays and copies them into resident
storage. A live culling call receives the camera matrix, performs sphere/AABB
frustum rejection, and stably compacts visible models, wind parameters, source
indices, and per-draw instance ranges. Fixed-size source-transform updates refresh
the existing handle rather than allocating another scene. Returned typed arrays
borrow Wasm storage and must be consumed or copied before their next overwrite.

The clustering adapter preserves authored-light validation and packs the same
records as the control. C performs cluster-bound calculation, counting, prefix
storage, and light-index placement. Overflow and work-limit paths retain the
complete all-light fallback; they do not silently remove lights. The renderer
continues to consume the existing packed cluster format and GPU programs.

This is resident render preparation, not a C asset compiler or general streaming
system. The frame module has a bounded bump arena, initially 2 MiB and capped at
256 MiB. It does not implement general scene unloading or arena reclamation.
The existing TypeScript source representations and GPU copies are still present.

### Packed movement and matched Rust control

[The C controller](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/experiments/c-town/c/simulation.c) and
[packed Rust control](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/experiments/c-town/rust-control/src/lib.rs) implement
the frozen kernel's scalar movement algorithm with the same packed layout:

- Fly and walk modes, normalized diagonal movement, sprinting, and camera angles.
- Fixed 60 Hz ticks, at most four ticks per call, elapsed-time clamping, and
  explicit dropped-time accounting.
- Gravity, jump edges, queued jumps between ticks, support following, and movement
  substeps.
- AABB blockers, transformed local wall solids, floor support, and authored ramps.

The movement ABI uses a 32-byte input record and a 64-byte state record. State
includes position, yaw, pitch, vertical velocity, ground height, fractional tick
time, dropped time, tick count, grounded/mode flags, and held/pending jump flags.
Neither controller currently has random-number state. C and packed Rust use the
same host sine, cosine, and float-remainder imports. That holds their math
boundary constant; the original Rust kernel retains its existing math path.

The bounded tables reserve 32,768 AABBs, 131,072 transformed solids, 65,536
supports, and 8,192 ramps. The C module initially reserves 16 MiB. The packed
Rust module's observed linear memory is 11,468,800 bytes. These capacities are
implementation choices, not evidence that C needs less runtime memory. The
packed tick counter is `u32`, compared with the original kernel's `u64`; it wraps
after roughly 828 days of uninterrupted 60 Hz simulation.

The town's frozen playable state currently uses an empty semantic collision
world in fly mode. Rendering thousands of buildings does not make their source
meshes walkable automatically. The collision stress measurement is separately
labeled and is not the newer triangle-controller implementation.

## Editing authority and state preservation

[The kernel adapter](../../../../apps/aethyr/apps/preview/kernel.ts) keeps typed authoring,
validation, revisions, command-ID handling, undo, and redo in the existing Rust
kernel. The C frame loop does not serialize movement through that JSON bridge.
Both packed variants still download and instantiate the original authoring
module; their small movement binaries are additional modules.

Before an authoring request, the adapter synchronizes the exact active camera
fields into Rust. Rust therefore checks overlap and support against the current
player rather than a camera left behind when packed simulation started. For a
command, a Rust `prepare` result is checked against the packed-world admission
limits before the real command commits. Invalid semantic commands still reach
the real authority, preserving its failed-command and duplicate-ID behavior.
Preparation alone does not mutate the active world or history.

After an accepted edit, the adapter repacks the complete collision snapshot.
Ordinary authoring does not reset position, velocity, fractional ticks, or queued
jumps. Runtime replies use active packed player/tick data with authoritative
revision and history metadata. Errors report the active player and clock rather
than the authoring module's unused simulation clock.

A saved-world replacement applies yaw, pitch, and mode even when position stays
the same. It follows the existing authority's velocity/grounding reset while
preserving its fractional tick and jump-edge behavior. A fresh `init` resets the
clock and runtime state. Startup uses deployment-owned inputs; a complete
transactional rollback of an arbitrary Rust `init` that later cannot be packed
is not implemented. The tested preflight/rollback boundary applies to editing
commands, not hostile-world admission.

### Actual gameplay code replacement

The builder also produces a C movement candidate compiled with
`AF_SIM_SPEED_SCALE=1.1f`. This changes an actually used movement constant and
exports its selected scale. It is a separate verification candidate, never a
silent change to the default benchmark behavior.

Module replacement first instantiates a candidate while the old module keeps
running. Immediately before activation, it installs the **current** world and
copies the **current** 64-byte state in one JavaScript turn. Capturing state
before the asynchronous compile would lose movement made during compilation;
the implementation and tests explicitly avoid that bug.

Each request advances a desired-generation counter. An older completion cannot
replace a newer requested candidate. A failed newer request keeps the last
working module and invalidates an older pending candidate. Invalid Wasm,
unsupported state versions, and failed restoration leave the active module
unchanged. This is compatible replacement of the defined movement ABI, not a
general schema-migration engine or a preemptible untrusted-code sandbox.

## CPU measurement method

The [public CPU summary](c-town/results/2026-10-23-cpu.json)
records one exploratory run on an Apple M5 Max, 18 logical CPUs, 128 GiB memory,
Darwin 25.4.0, arm64, and Node `v24.13.1`. CPU work ran without a WebGPU context.
There are 15 measured batches of 128 calls after an unrecorded warmup batch.
Implementation order rotates between batches. The statistics describe batch
wall time divided by call count; their p95 values are **not individual frame
tails**. File caches are not flushed, and no multi-machine confidence interval
is established.

Before timing, 64 camera views per scenario compare source visibility, compacted
models, wind output, draw ranges, and clustered-light storage against the current
implementation. Simulation checks 128 differential frames per profile and
reports zero position difference. Separate conformance tests cover longer
collision, jump, and editing sequences.

### Source visibility and compaction

Times below are milliseconds per call, using the median of the 15 batch averages.

| Scenario | Current TypeScript | Pooled TypeScript | C/Wasm | Current/C ratio | Pooled TS/C ratio |
| --- | ---: | ---: | ---: | ---: | ---: |
| Day | 0.29005 | 0.09290 | 0.06522 | 4.45× | 1.42× |
| Night | 0.29122 | 0.09120 | 0.06460 | 4.51× | 1.41× |

Moving validation to installation and reusing buffers already gives the
TypeScript control a 3.12–3.19× improvement over the existing path. C saves about
0.225–0.227 ms against that existing path, but only 0.0266–0.0277 ms against the
pooled TypeScript control. The 4.5× headline therefore combines architectural
changes with the remaining implementation difference. It is not a measured
4.5× language advantage or a 4.5× frame-rate improvement.

### Clustered-light preparation

| Scenario | Current TypeScript median, ms | C median, ms | Current/C ratio | Absolute median difference, ms |
| --- | ---: | ---: | ---: | ---: |
| Day, zero local source lights | 0.01571 | 0.00804 | 1.95× | 0.00767 |
| Night, 488 source lights | 0.65895 | 0.39636 | 1.66× | 0.26259 |

The night workload has a useful absolute CPU reduction in this run. Light
validation and record packing remain in the measured C adapter. There is no
pooled TypeScript or matched Rust clustering control, so this result leaves
allocation policy, storage layout, algorithm expression, and language mixed.
It says nothing directly about GPU lighting execution time.

### Movement boundary

These values use **microseconds** per complete input-write, step, and player-reply
call. They are not bare Wasm-export timings.

| Profile | Existing Rust/JSON | Packed Rust | Packed C | JSON/C ratio | Packed Rust/C ratio |
| --- | ---: | ---: | ---: | ---: | ---: |
| Fly, empty semantic world | 4.109 | 0.309 | 0.301 | 13.63× | 1.025× |
| Walk, 1,024 synthetic source AABBs | 8.374 | 7.477 | 6.732 | 1.244× | 1.111× |

Packed Rust removes almost all of the empty-world JSON overhead too: 13.30×
against the original JSON path. Its difference from C is about 0.0075 µs in that
profile, far too small for a strong general-language conclusion from one local
run. With collision work included, the C median is about 0.745 µs lower than
packed Rust. The stress blockers come from source bounds; they are synthetic
AABBs rather than a qualified walkable-town mesh.

Observed movement linear memories are 1,179,648 bytes for Rust/JSON in the empty
profile, 2,818,048 bytes for its stress profile, 11,468,800 bytes for packed Rust,
and 16,777,216 bytes for C. The frame core separately grows from 6,750,208 bytes
after day work to 13,041,664 bytes after night work in the CPU tool. None of these
figures is total browser RAM, GPU memory, or an engine-wide peak allocation count.

## Build and payload measurements

The [leaf-build results](c-town/results/2026-10-23-builds.json)
change a used speed constant in private source copies and require five distinct
output hashes for each language. One warmup build is excluded. The sample order
alternates. C uses Clang 23.1.3, C17, `-O3`, no fast math, no floating-point
contraction, and freestanding Wasm linking. Rust uses `rustc 1.98.1`, release
optimization level 3, thin LTO, one codegen unit, aborting panics, and incremental
compilation disabled.

| Changed dependency-free movement module | Samples | Median build, ms | Range, ms |
| --- | ---: | ---: | ---: |
| C compilation and Wasm link | 5 | 57.273 | 56.399–58.448 |
| Rust crate compilation and Wasm link | 5 | 461.613 | 455.573–468.925 |

The ratio of these medians is 8.06×. This is useful evidence for a small leaf
module's source iteration, with process startup and linking included. It
excludes asset compilation, whole-engine rebuilding, browser fetch/compile,
state transfer, GPU work, and time to see the changed behavior. An unchanged
Cargo invocation is a different boundary and must not be compared with a fresh
C compilation as if both rebuild code.

The measured modules' payload sizes are:

| Module | Built bytes | Gzip bytes |
| --- | ---: | ---: |
| Existing Rust authoring/simulation kernel | 611,816 | 184,721 |
| Packed Rust movement control | 56,335 | 22,196 |
| Default C movement | 5,778 | 2,613 |
| C render preparation | 7,129 | 3,423 |

The default C variant still needs the 611,816-byte authoring kernel. Its three
Wasm payloads total 624,723 built bytes before HTTP compression, not 12,907 bytes
for a complete engine. These sizes also exclude JavaScript, WGSL, textures,
meshes, and other town assets. Custom-section removal and gzip analyses in the
summary are separate payload experiments; the application downloads the actual
built modules. The Rust control has not received a dedicated size-optimization
pass, so these are current build outputs rather than minimum language sizes.

## Correctness and safety checks

The targeted simulation and integration command passes **16 tests with zero
skips** when the experiment and current Rust artifacts are built. The integrated
JavaScript/TypeScript suite passes **529 tests with zero skips**; the current
Rust kernel passes **37 native tests**. TypeScript and documentation checks pass.

```sh
npx tsx --test --test-concurrency=1 \
  tests/c-town-kernel.test.ts tests/c-town-simulation.test.ts
```

[Simulation tests](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/tests/c-town-simulation.test.ts) cover 2,020 movement
frames across fly, walk, jump, dropped ticks, AABB blockers, transformed walls,
floors, and an actual ascent to a 1.5 m landing. C and packed Rust state records
match byte for byte through the longer differential sequences; the baseline
player comparison uses explicit 0.0001/0.0003 tolerances. Additional checks cover
invalid geometry/input/poses, exact state restoration, queued jumps, movement and
world changes while module compilation awaits, stale candidates, failed newer
candidates, and activation of the genuinely recompiled 1.1-speed module.

[Kernel integration tests](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/tests/c-town-kernel.test.ts) run the same editing
sequence through baseline, packed Rust, and C. They check nonmutating preparation,
atomic failed edits, failed-command ID conflicts, duplicate successful commands,
undo/redo, metadata, a queued jump across edits, save replacement at an unchanged
position, and initialization resets. A separate fixture proves that a world
accepted by the existing Rust authority but unrepresentable by this packed ABI
is rejected before the authoring revision or history changes.

[Frame tests](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/tests/c-town-frame.test.ts) verify stable visibility and
compaction, empty views, malformed-input rejection, source-transform updates
without memory growth, exact clustered-light words, complete overflow fallback,
authored-light validation, and owned cache metadata surviving borrowed Wasm
storage overwrite/growth. [Metrics tests](../../../../apps/aethyr/tests/c-town-metrics.test.ts)
check explicit backend selection and bounded sample/request behavior. TypeScript
checking passes with `npm run check`.

### Native sanitizers

The [native controller driver](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/experiments/c-town/c/simulation-native-check.c)
executes the same C source in one translation unit, accessing its state directly
instead of truncating native 64-bit pointers through the Wasm pointer ABI. Native
math substitutes for the three browser imports. Apple Clang 21.0.0 builds it with
`-O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer`.

With seed `859a2bc1`, **200,000 bounded movement frames and 4,652 invalid-input/
table-count batches pass**, exit code 0, with no sanitizer stderr. Checks include
containment within independent boundary walls, ground and angle limits,
nonpenetration, finite state, bounded tick work, retained state after rejected
input/counts, and a queued jump surviving state transfer. The
[public sanitizer result](c-town/results/2026-10-23-sanitizers.json)
records compiler details and source hashes.

The run uses `ASAN_OPTIONS=detect_leaks=0:halt_on_error=1` and
`UBSAN_OPTIONS=print_stacktrace=1:halt_on_error=1`. This Apple runtime aborts before
the test when `detect_leaks=1` is enabled; leak detection is unsupported here.
Passing native address/undefined-behavior checks is bounded evidence, not a proof
of all C memory safety, Wasm host-boundary safety, or production correctness.

## Browser method and limits

The [day results](c-town/results/2026-10-23-browser-day.json)
and [night results](c-town/results/2026-10-23-browser-night.json)
use Chrome 155.0.8059.85 on macOS 26.4, Apple M5 Max, 128 GiB, connected to AC
power. Chrome identifies its hardware GPU through ANGLE/Metal as Apple M5 Max;
this is not SwiftShader. WebGPU timestamp queries were available.

Each variant has three fresh, isolated browser-context trials per scene, with
five seconds of warmup and 20 seconds of measured live rendering. Variant order
rotates between trials. Production bundles run on separate loopback editor and
preview origins. The worker applies the same continuous 20-second camera sway;
ordinary simulation keeps stepping. The viewport and drawing surface are
1280×720. Fixed-time readbacks occur after timing, never inside the measured
interval. Only one experiment context renders at a time.

These are exploratory measurements on a shared development machine. Other Chrome
GPU processes and an unrelated Rust build were observed during the session.
GPU times drift substantially between trials. Temperature, clock frequencies,
exclusive GPU utilization, power consumption, and physical display presentation
were not measured. The proposed five-session, 120-second, two-device qualification
protocol has **not** been completed. Mobile, low-end hardware, production-network
latency, and a standalone editor-free player are not qualified here.

The timing boundaries are deliberately separate:

- Instrumented CPU is simulation plus the CPU `Renderer.render()` call. It
  excludes other browser work. Culling and cluster times are nested inside render
  CPU and must not be added again.
- CPU tables below use submitted frames only. The renderer declines new work
  when two submissions remain in flight; skipped callbacks otherwise dilute CPU
  medians with near-zero render work.
- Main HDR GPU time is an asynchronous start/end timestamp pair for that pass.
  It excludes other passes, presentation, and input-to-photon latency. It is
  elapsed GPU time under the observed contention, not an isolated device study.
- Accepted submissions/s comes from submitted sample timestamps. Animation
  callbacks often remain near 60 Hz while the engine submits much less often.
  Neither counter establishes physically displayed FPS.
- Browser clocks quantize short CPU spans around 0.1 ms here. A reported zero
  movement span is below that observation resolution, not zero work. The Node
  batch measurements resolve that small boundary separately.

The profiler uses four asynchronous timestamp slots without per-frame queue
waits. Missing final query results remain missing. Its own overhead was not
independently estimated. Full per-run p50/p95/p99, skipped submissions, request
counts, and resource estimates remain in the public JSON; no confidence interval
is invented from thousands of correlated frame samples.

### Live frame results

Each cell below is the median of three **run-level** statistics. Bracketed ranges
are the minimum–maximum across the three runs. CPU values are p50/p95/p99 in ms;
GPU values are main HDR p50 in ms. This does not pool frames as independent trials.

| Day variant | Submitted CPU p50 / p95 / p99 | HDR GPU p50 [range] | Accepted submissions/s [range] |
| --- | ---: | ---: | ---: |
| baseline | 2.40 / 3.38 / 4.50 | 31.00 [26.15–41.71] | 26.22 [20.42–31.43] |
| rust-packed | 2.40 / 3.40 / 4.26 | 30.80 [25.49–35.78] | 26.97 [22.94–32.23] |
| ts-packed | 2.10 / 3.02 / 3.80 | 30.41 [25.72–33.55] | 27.17 [24.82–31.80] |
| c | 1.90 / 3.04 / 3.60 | 34.34 [27.59–35.29] | 24.12 [23.42–29.63] |

| Night variant | Submitted CPU p50 / p95 / p99 | HDR GPU p50 [range] | Accepted submissions/s [range] |
| --- | ---: | ---: | ---: |
| baseline | 4.00 / 5.30 / 6.15 | 40.76 [35.78–40.80] | 19.52 [19.48–22.57] |
| rust-packed | 4.00 / 5.26 / 6.71 | 41.94 [37.62–42.30] | 19.47 [16.66–21.32] |
| ts-packed | 3.70 / 4.66 / 4.90 | 41.55 [38.40–41.62] | 19.43 [19.00–21.12] |
| c | 3.55 / 4.70 / 5.30 | 42.50 [40.34–43.19] | 18.78 [16.56–20.42] |

The integrated C variant saves about 0.50 ms of submitted CPU median in day and
0.45 ms in night against the current path. The pooled TypeScript control already
recovers much of that saving. None of these runs demonstrates a C submission-rate
win. C's lower observed rate must not be promoted into a general C disadvantage
either: the GPU workload is unchanged and uncontrolled run variation is large.
Under these measured conditions, CPU preparation is much shorter than the GPU
interval; changing the CPU implementation cannot remove the unchanged GPU work.

Median submission intervals are roughly 33.4 ms for day baseline/C and 50 ms for
night baseline/C, despite animation callbacks near 16.7 ms. Median skipped-attempt
fractions are 55.1%/58.0% in day and 62.5%/63.1% in night. These counters explain
why a callback-based “60 FPS” result would have been misleading.

### Sustained night pair

The [longer pair](c-town/results/2026-10-23-browser-sustained.json)
runs C then baseline for 120 seconds each after ten seconds of warmup, using the
same corrected frozen bundles and night sway. Both runs complete without page
errors, and their fixed-time images are byte-identical.

| Variant | Submitted CPU p50 / p95 / p99, ms | HDR GPU p50 / p95 / p99, ms | Accepted submissions/s |
| --- | ---: | ---: | ---: |
| C | 3.10 / 4.10 / 4.70 | 37.95 / 46.92 / 50.43 | 21.28 |
| Baseline | 4.70 / 6.60 / 7.30 | 39.26 / 47.95 / 52.17 | 20.41 |

C submits 2,554 frames and baseline 2,450. Both have a 50 ms median submission
interval and about 66.7 ms p95. The C frame arena is 13,041,664 bytes and movement
memory remains 16,777,216 bytes at the observation point. This is not a complete
peak-memory or leak study. The roughly 4% submission-rate difference in this
single sequential pair is smaller than the variation in the shorter runs and
does not establish a causal C frame-rate advantage.

### Startup and payloads

Fresh-context median readiness is 1.98 s baseline versus 1.74 s C in day, but
1.79 s versus 2.06 s in night. Warm-navigation results vary too. These opposite
orderings provide no consistent startup win. Context freshness does not flush
OS/file caches; localhost loading is not a production CDN or cold Internet test.

The first day trial requests these declared response-body bytes:

| Group | Baseline | C |
| --- | ---: | ---: |
| Private assets | 662,150,341 | 662,150,341 |
| Browser host, UI, and shader-containing resources | 965,044 | 965,030 |
| Wasm modules | 611,816 | 624,723 |

The private asset response bodies alone are about 631.5 MiB. The full private
snapshot is larger still; it contains unused and historical files. Neither
number is a claim about compressed CDN transfer or an optimized release package.
The measured C path adds 12,907 Wasm bytes and does not change the town's asset
bytes. Its accounted GPU resources remain about 1.067 GB, essentially unchanged.
C also adds its frame arena and movement memory while retaining the Rust
controller authority and TypeScript scene representations. Whole-process peak
RAM and driver-private allocations were not measured.

The dominant first-play opportunity is therefore asset delivery and residency,
not removing a few kilobytes of runtime code. A cooked, compressed, demand-loaded
runtime package deserves its own comparison with equivalent visible quality.
This experiment does not implement or benchmark that package.

## Browser correctness and actual iteration

The [browser verification summary](c-town/results/2026-10-23-verification.json)
records **22 passing checks**, 45 private captures, zero uncaught page errors,
and exact decoded RGBA equality for all **16 supported reference cameras** between
baseline and C, across day and night. This is parity with our baseline renderer;
it is not a new claim of parity with the vendor's reference images or video.

The functional path exercises the existing editor and preview services:

- Add a real catalog prop, observe the saved draft and changed image, then undo
  and recover the exact fixed-time image and draft.
- Change a prop's material red constant through the typed authoring path and
  undo it, with image change/restoration and unchanged player position.
- Add a semantic room, aim from inside at its east wall, remove that wall, undo
  it, and undo the room. Saved wall state and same-pose images verify the result.
  This does not pretend an imported decorative mesh is an authored wall.
- Reject an invalid authored transform without changing the saved world/player.
- Compile a valid WGSL candidate, reject invalid WGSL, and retain the working
  shader, same player, identical fixed-time output, and continued live rendering.
- Activate the actually recompiled 1.1-speed C movement module, reject malformed
  Wasm, retain exact paused player/tick/state, and restore the default module.
  Native differential tests separately verify the changed constant's movement.

Ten alternating prepared C module activations measure 2.47 ms median through the
editor/CDP API, p95 3.86 ms, range 1.81–4.66 ms. Worker fetch/instantiate/validate/
activation is 1.15 ms median, range 0.90–1.40 ms. These warm activations exclude
source compilation and first-visible-frame timing. Adding the independent leaf
compile median to this median would not create a measured end-to-end result.

Individual receipt observations, including the Node/CDP round trip, are:

| Action | Receipt time, ms |
| --- | ---: |
| Add prop | 115.23 |
| Material red constant | 14.77 |
| Material undo | 16.25 |
| Add authored room | 86.69 |
| Remove authored wall | 6.30 |
| Undo authored wall | 12.96 |
| Valid WGSL append/compile | 553.18 |
| Rejected WGSL | 20.13 |

These are single functional checks, not latency distributions or edit-to-photon
measurements. Separate captures prove that changes became visible; capture time
is excluded from the receipt spans. No model inference or code-generation delay
is included. Typed data changes, C compilation, WGSL compilation, module
activation, and AI interpretation are different iteration clocks.

### The capture bug found by the experiment

The first longer day run produced five different PNG hashes, including changes
between repeats of the baseline. That failed result is retained in the day JSON;
it is not rewritten into a parity success.

The existing directional-shadow fit uses camera hysteresis: a nearby camera can
reuse an earlier live fit. A time-zero capture therefore inherited different
shadow anchors from the preceding sway. The fix invalidates directional/local
shadow history, wind-shadow planning, and planar views before an ordinary capture
and again on exit, including failures. Temporary object-transform captures retain
their existing symmetric invalidation. Live rendering behavior is unchanged.

Regression tests cover entry/exit and readback/render failures. After the fix,
the separate 16-camera decoded-pixel study passes, and all 12 corrected night
trial captures are byte-identical. This is a concrete lesson: resetting a clock
is not enough when a renderer retains view-dependent history.

### Current character-enabled integration

The [current qualification](c-town/results/2026-10-23-current-qualification.json)
uses separate production baseline/`c-frame` builds at `074f290`, with the current
661,865-byte Rust kernel. Both frozen scene documents contain existing character
and triangle-collision manifests; the loaded druid is observed in the browser.
This is a compatibility study, not a fifth row in the historical performance
comparison.

Both variants pass six live character checks: loaded third-person druid, active
rendering, keyboard/button perspective changes, orbit distance, walking/running,
and held-jump behavior. A separate seven-check `c-frame` run additionally rejects
WGSL while preserving player feet and rendering. A fresh Fly-mode comparison
matches all 16 gallery images exactly across 32 captures. Fly keeps elapsed
walking-animation state from contaminating the still-image comparison.

Later main-branch UI changes were merged at `9169b6e`. A rebuilt `c-frame` passes
the seven-check character/shader study again, including the real keyboard and
visible view controls. The earlier pixel comparison remains attributed to
`074f290`; the subsequent changes do not alter its rendering path. The final
suite also fixes an intermittent test-only rejection-handler race by attaching
expected reload rejections before awaiting another candidate. Runtime reload
behavior is unchanged.

No private source-wall/stair route fixture was supplied for this later check.
Those browser routes are explicitly untested here; native Rust collision tests
remain separate evidence. Keeping current Rust movement in `c-frame` avoids
claiming that the historical C controller implements those newer surfaces.

## Conclusions and next experiments

**Keep the packed architecture; do not approve a wholesale C migration from these
results.** The C implementation is usable and its small-module rebuilds are fast.
The measured town does not become reliably faster to render by changing these CPU
subsystems to C.

1. Make resident data, reusable output storage, validation at installation, and
   coarse typed boundaries the default design goals. The pooled TypeScript and
   packed Rust controls show that these gains are available without changing the
   whole engine language.
2. Keep current Rust editing, collision, and character behavior while trying
   `c-frame` render preparation. Its explicit profile lets ownership and memory
   decisions be evaluated without silently replacing a newer controller.
3. Keep the C leaf-module path for further iteration studies. Extend state
   migration, arena reclamation, and memory accounting before making it a general
   behavior runtime. The demonstrated 64-byte controller transfer is not a general
   engine hot-reload solution or an untrusted-code sandbox.
4. Profile and reduce actual GPU work at fixed quality: per-pass cost, visibility,
   draw batching, shadow/reflection work, foliage overdraw, and material cost.
   Change one algorithm at a time and retain the image comparison. A C-facing
   WebGPU host through Emdawnwebgpu is a separate unimplemented B1 experiment;
   this B0 result gives no measured reason to claim its encoding path will fix
   the GPU interval.
5. Reduce the initial asset closure and resident duplicates. Compare cooked
   packages, compressed payloads, staged quality, and demand loading, preserving
   the declared visual target. Runtime binary size is a very small part of the
   measured 662 MB town payload.
6. Before a stack decision, repeat with a quiet machine, independently estimated
   instrumentation overhead, the proposed longer balanced trials, a second GPU,
   and real mobile hardware where relevant. Measure process/GPU peaks and actual
   source-save-to-visible-frame latency. The present data identifies useful
   engineering work; it does not supply those missing qualifications.
