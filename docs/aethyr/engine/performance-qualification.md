# Sustained frame-pacing qualification

Status: the recorder, strict evaluator, and matrix runners are implemented. Full
matrix results remain unqualified until the recorded windows pass. The earlier
[benchmark baseline](representative-benchmarks.md) measured Town below 60
submissions per second; those results never established smooth 60 FPS.

## Evidence and acceptance

Run one GPU world at a time. Keep original meshes, effects, materials, lighting,
sky, source camera lens and grade, and the actual render resolution. Qualify both
800 × 500 and 1280 × 720. A lower resolution or reduced quality is a separate
profile with a separate result.

Each permutation needs two windows of at least 30 seconds, each preceded by at
least 5 seconds of warmup. Short diagnostics retain the same raw observations and
can find failures, but cannot qualify sustained performance. Missing capabilities,
fixtures, or timestamp evidence remain explicit gaps.

| Observation                  | Acceptance at 60 Hz                                                                     |
| ---------------------------- | --------------------------------------------------------------------------------------- |
| Completed frame count        | At least `floor(actual window seconds × 60) − 1`; retain the actual unrounded rate      |
| Queue-fence coverage         | Exactly one successful completion for every accepted frame in the frozen window         |
| Fence interval               | p95 ≤ 17.5 ms; p99 ≤ 20 ms; maximum ≤ 33.34 ms                                          |
| Slow intervals               | At most 0.1% above 25 ms; none above 50 ms                                              |
| Queue latency                | p95 ≤ 33.34 ms; p99 and maximum ≤ 50 ms                                                 |
| Render/compute pass envelope | Available for every submitted sample; p95 ≤ 16.667 ms                                   |
| Timestamp validity           | The pass envelope contains its paired main HDR time, allowing 0.05 ms clock granularity |
| Simulation time              | No increase in kernel dropped time or host-clamped time                                 |

The one-frame count allowance handles window boundaries. It does not turn a
measured 59.9 into “60+.” Queue-fence callbacks measure when JavaScript observes
completed graphics work. Batched callbacks can make cadence evidence
insufficient; they do not prove that physical presentation was uneven. Compositor
presentation is being investigated by the separate [compositor tracing prototype](../../../apps/aethyr/tools/compositor-tracing.md);
physical displayed FPS remains unavailable. Reports distinguish
queue-qualified, GPU-budget-qualified, and the complete matrix verdict.

Main HDR timestamps retain the original HDR query. The wider GPU observation is
the envelope from the earliest valid beginning to the latest valid end of the
actual render and compute passes. It includes shadow, HDR, effects, and
postprocessing passes, with bounded pass coverage and explicit availability. It
excludes copies/uploads outside passes, query resolution/readback, and browser
composition. Synthetic marker passes are not used because independent compute
work can execute before graphics. Unavailable or invalid nonempty-pass timestamps
leave this envelope unavailable. CPU spans overlap and must not be summed. Captures and asset
hashing stay outside timed windows. Stop freezes the accepted frame range before
queue drain; late completions can finish without admitting later live frames.

Knight image comparisons use bounded, explicit capture references. A trusted
diagnostic can prepare native patrol start poses in a pristine world before
measurement, sample their source animation with isolated capture animators, and
submit a frame at time zero. Cross-instance checks require matching model, pose,
and actor-ID hashes. Later captures reuse that renderer's opaque reference;
foreign, replaced, or mismatched references fail. This setup is unavailable during
measurement or after combat and never clears building damage. Live captures remain
the default. Timed windows use live native actors: patrols move, and combat follows
their actual deaths, held corpses, and restoration. Capture references never
replace actors during measurement.

## Required matrix

| Host and content                 | Permutations                                                                                                                                                                                       |
| -------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Editor, Medieval Town            | Source spawn camera in both day and night, static and moving routes; all 16 implemented source views: MT-02, MT-03, MT-05 through MT-18, with their original source scenario and grade             |
| Editor interaction               | Day/night command-panel and performance-panel churn; typed prop addition, repeated transforms, removal, and a failed WGSL edit during live rendering                                               |
| Owned room and character players | Static and moving render routes; actual controller movement; first person, plus third person when supported; staged key/exit/reset cycles                                                          |
| Published Town player            | Source spawn camera in both day and night, static and moving routes; all 16 source views and routes; first/third person; controller and quest rows; source-bound spells and persistent destruction |
| Druid gameplay                   | Lightning, meteor shower, and thunderbolt preparation/casting/effects; persistent damaged geometry after debris settles                                                                            |
| Castle, when present             | Both source scenarios; barrage preparation and active effects; cancellation during preparation and during a live barrage; restore; complete destruction with settled persistent source rubble      |
| Knights, editor and Town player  | Day/night native eight-actor patrol movement; Lightning, Meteor, and Thunderbolt against live knights; death animation, held corpses, and restore; rejected sky cast and intact-arch cover         |
| Editor aiming                    | Day/night first- and third-person locked and unlocked aim; real V/number keys, cursor movement, pointer lock, click/Enter casting, world-ring projection, and rejected sky aim                     |
| Empty player                     | Idle browser/UI callback responsiveness and existing clear-image checks; continuous engine rendering is not applicable                                                                             |

Knight windows retain native actor IDs, positions, death clocks, cast outcomes,
and timed phase markers. Patrol polling accumulates path distance so a completed
loop cannot appear stationary. Combat preparation, live impacts, the death clip,
held corpses, and restore occur inside the measured window. Editor combat also
checks that a rejected WGSL edit retains the defeated actors, working shader, and
player. The player has no shader authoring surface; its rows use its existing
native cast API. Aiming rows operate the actual editor canvas and controls,
independently project the displayed native target, and verify that the impact
reaches that point without changing pointer lock or moving the player. Native
inspection at up to four samples per second is included in these workloads.

The suite requires these outcomes as well as pacing. A feature label or an
optimistic aggregate flag cannot replace the saved native phase evidence. The
latest Town must contain all eight knights; an older Druid-only artifact leaves
these required rows unqualified.

Every row uses both resolutions and two repetitions. Spell windows include the
actual preparation call and subsequent live effects; they do not describe a
continuous 30-second barrage unless the recorded state shows that. Castle phase
markers retain their actual timing so preparation or restore stalls remain
visible. Quest camera positions are staged through the same Rust controller; this
is not a continuous navigation claim. MT-04 is the asset overview, not one of the
16 implemented scenic views.
The Town spawn rows use the cooked scene's camera position and target, including
the existing `[6, 4, 20]` baseline camera. They use the day/night reference grade
and default camera lens. Controller movement rows use the separate player spawn.
Passing the scenic gallery cannot replace the required spawn rows.

An editor-only verdict does not qualify players or gameplay. A player report with
a Castle capability cannot qualify its complete matrix without matching Castle
fixtures. Earlier Druid breach/walking checks remain separate from a new
performance-only aiming probe when collision assets have changed.

## Run

Use the isolated browser launcher and an output directory outside the checkout:

```sh
openagents browser run -- env \
  AF_CAPTURE_DIR="$OPENAGENTS_SCRATCH/town-pacing" \
  AF_VERIFY_WORLD=medieval-town \
  AETHYRFORGE_PRIVATE_ASSETS="$AETHYRFORGE_PRIVATE_ASSETS" \
  node tools/qualify-runtime.mjs
```

For players, first build diagnostic artifacts with
`VITE_PLAYER_DIAGNOSTICS=1 npm run build:player`. Normal player builds leave GPU
timing disabled. Then run:

```sh
openagents browser run -- env \
  AF_CAPTURE_DIR="$OPENAGENTS_SCRATCH/player-pacing" \
  AF_PLAYER_ROOT="$PWD/dist/player" \
  AF_PLAYER_PROFILES=empty,room,character,town \
  AF_DRUID_FIXTURES="$AF_DRUID_FIXTURES" \
  AF_CASTLE_FIXTURES="$AF_CASTLE_FIXTURES" \
  node tools/qualify-player.mjs
```

Town must be included in the player build from the same private source. Druid
fixtures must match its exact gameplay and collision resources. A new
performance-only fixture additionally pins its private native-aim proof and the
current kernel SHA. It grants no older breach or walking claim. Castle fixtures
pin both scenario manifests, exact node totals, and source placement IDs.

`AF_PERFORMANCE_MODE=diagnostic` selects shorter, explicitly unqualified windows.
`AF_RUNTIME_WINDOW_MS`, `AF_RUNTIME_WARMUP_MS`, `AF_PERFORMANCE_REPEATS`,
`AF_PERFORMANCE_VIEWPORTS`, `AF_PERFORMANCE_SHOTS`, and `AF_RUNTIME_ROUTES` can
narrow a diagnostic. Incomplete selections cannot establish full qualification.
The default Town shot selection includes `spawn` and all 16 gallery IDs. Selecting
`AF_PERFORMANCE_SHOTS=spawn` measures both source scenarios; narrowing
`AF_RUNTIME_SCENARIOS` can further restrict these spawn diagnostics.
Use `AF_PERFORMANCE_OBSERVERS=off,on` only with diagnostic mode to compare normal
rendering against optional CPU/GPU profiling at the same view, resolution, and
quality. Repeated pairs reverse order. Both cohorts retain the common callback
and completion recorder; the off cohort has unavailable GPU timings. Reports
retain actual completion rates, latency, and exact cross-cohort source pixels.
Qualification continues to require profiling on and complete GPU evidence.
The normal frame loop follows display callbacks. Benchmark requests default to a
60 Hz render-attempt profile; `AF_RUNTIME_TARGET_HZ=display` explicitly selects
uncapped display callbacks for those checks. The frame loop uses elapsed
time across intentionally paced callbacks and never issues catch-up bursts.
Reports distinguish raw callbacks, intentional pacing, suspended callbacks, render
attempts, and genuine GPU backpressure. Pacing alone cannot make an over-budget
frame finish sooner or qualify a workload.
Editor features can be selected with `AF_PERFORMANCE_FEATURES`; player features
use `AF_PLAYER_PERFORMANCE_FEATURES`. Both accept `enemies`, `enemy-lightning`,
`enemy-meteor`, `enemy-thunder`, and `enemy-miss-cover`. The editor additionally
accepts `aim-third-locked`, `aim-third-cursor`, `aim-first-locked`,
`aim-first-cursor`, and `aim-sky`. Editor aiming requires `AF_DRUID_FIXTURES` whose
camera probe matches the exact gameplay and collision resources. Raw callback, completion, and GPU arrays are
written to separate private per-window files, linked by SHA-256 and byte count.
Source, kernel, working shader, quality settings, asset closure, browser, adapter,
viewport, and build identity remain pinned by the surrounding benchmark reports.

Index the resulting native reports with `node tools/performance-suite.mjs
--editor-starter STARTER_REPORT --editor-town TOWN_REPORT --players
PLAYER_REPORT --out PRIVATE_INDEX.json`. The index rehashes raw frame/image files
and reevaluates pacing. Missing workloads, insufficient repeats, or an incomplete
Castle matrix prevent an all-product verdict.

## Optimization diagnostics on October 23

The [short diagnostic summaries](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/experiments/kaigen/results/smooth-diagnostics-2026-10-23.json)
retain actual completion counts and durations, plus hashes of private raw evidence.
The full-quality MT-11 night view measured about 19–22 completed Hz at the two
resolutions. Disabling volume lighting measured about 20–26 Hz; replacing material
lighting with a flat diagnostic measured about 25–29 Hz. Both ablations alter
quality, remain outside product code, and cannot qualify performance. Other
pre-existing film/game GPU clients were active on the host. These short runs cannot
separate renderer cost from resource contention or establish sustained performance.

After the owner approved killing three orphaned film/game jobs, two duplicate
Town previews were also closed for measurement. The later full-quality night
diagnostic with eight live knights completed about 60 Hz at 800 × 500 and
59.1–59.3 Hz at 1280 × 720. These five-second windows remain unqualified. The
different host activity and asset closures prevent attributing that difference
to engine changes alone. Specialized opaque depth captures matched the preceding
source captures exactly.

Implemented optimizations include stock material and facing specialization,
derivative-safe early rejection, pooled source culling including destroyed masks,
exact probe sampling and selective evaluation, conservative tighter light lists,
per-ray fog calculation reuse, pooled particle writes, idle gameplay visual sync,
and cooperative fracture preparation. Stock depth and color stages use smaller
interfaces when their data contract permits. Custom shaders retain the general
path. MASK fringes retain depth writes. Successful and failed shader edits are
checked after timing, including player continuity and exact source restoration.
Known opaque depth draws omit unnecessary fragment work. Stable per-view source
buffers and bounded render bundles reuse indexed indirect commands when the
adapter supports `indirect-first-instance`; other adapters retain direct draws.

Spell forecasting now advances an invisible native candidate in small requests,
yielding between batches. Live simulation continues. Fragment preparation runs
before final native revalidation; pending impacts that advance in the meantime
restart the candidate. A validated commit reuses its prepared requirements rather
than running the entire forecast again. The synchronous API retains its timing
checks. Native and actual Wasm tests compare all four spells with the original
ordering and exercise cancellation, stale handles, live movement, and in-flight
impacts. Individual cloning, casting, and support steps are still indivisible;
synthetic timings do not establish their largest possible browser cost.

Prepared debris geometry now resides in bounded GPU arenas. Dirty updates upload
ordered packing jobs; a compute pass fills the existing visible slabs before
shadow and color passes in the same submission. Whole and moving geometry retain
their stored bits. GPU rubble transforms use f32 arithmetic and require a bounded
comparison against the former CPU expression, rather than an exact-pixel claim.
The standalone `tools/verify-debris-packing.mjs` runner checks real GPU readback at
the existing slab capacities without loading a world. CPU and GPU fallback costs
remain visible. The complete sustained qualification matrix has not passed.

Restore now keeps the admitted source topology, fracture cache, and GPU slabs.
An idle restore avoids rebuilding source geometry and shadows; a damaged restore
invalidates them once. Stock shader restoration retains its original pipeline
family. A visible-browser check passes exact original, failed-edit, accepted-tint,
restored-stock, and repeated-restored captures. The existing image assertions
remain exact.

Visible Chrome exposes this display's roughly 120 Hz callback cadence. The first
display-driven diagnostic submitted about 61–90 Hz across its four windows, with
many genuine backpressure skips. A later five-second 60 Hz diagnostic submitted
only 21.39 Hz and failed pacing despite passing image checks. Its GPU envelope
and initialization were also substantially slower. These results do not isolate
host contention, GPU scheduling, or observer cost; neither run qualifies smooth
60 FPS. Balanced visible runs and source-attributed compositor evidence are still
required before claiming smooth 60 FPS across the complete matrix. The production
60 Hz target reduces deliberate over-submission on high-refresh displays; it does
not establish that qualification.

The kernel build now applies pinned Binaryen 132 optimization without changing
the player byte limits. The resulting kernel is 768,330 bytes, compared with
1,008,424 bytes before this optimization. Kernel behavior tests exercise the
optimized artifact as well as the native implementation.

A visible 72-window sweep covers both spawn scenarios and all sixteen gallery
views, static and moving routes, at both resolutions. All 72 source-image pairs
match exactly. These two-second diagnostics have a median of 59.907 completed Hz
and a minimum of 40.323 Hz; only 5 pass every strict diagnostic check. Several
night views and the day spawn remain too slow. The source hashes and every
window's counts, duration, and failures remain in the diagnostic summary.

The planar reflection now bounds fragment shading to the texels that visible
fountain basins can sample. Conservative projection, floating-point margins,
normal distortion, and filtering padding determine the rectangle. The pass keeps
its original camera, viewport, MSAA, scene draws, and shading. Unknown or unstable
projections and custom scene shaders use the full pass. All sixteen spawn
comparison captures match the preceding source pixels exactly; a second
eight-capture control also passes. Spawn performance improves substantially, but
the moving 1280 × 720 night case still completes about 56 Hz in the ten-second
control. This optimization has not qualified the full matrix.

A separate early local-light rejection experiment also preserves the compared
pixels, but does not establish a consistent improvement. It remains outside the
product. Short comparisons with different route durations cannot isolate shader
cost, host activity, or changing scene visibility.

The worker CPU profile identifies allocation-heavy matrix inversion, particle
sampling, water-light packing, and decal projection. Their replacements preserve
the original arithmetic, source ordering, and packed output bits. Focused CPU
benchmarks improve these functions; the subsequent 72-window browser scan still
has a minimum of 39.382 completed Hz and a median of 59.840 Hz. Four windows pass
the strict diagnostic and fifteen pass the separate application diagnostic.
All 72 before/after image pairs match; 142 cross-source images also match under
identical capture conditions. Two MT-06 images have different camera rounding
and remain noncomparable. A separate controlled recapture at the original
float64 camera, environment, and actor pose matches exactly. None of these short
windows qualifies the complete sustained matrix.

`tools/application-pacing.mjs` now reports application scheduling separately
from the unchanged strict fence-cadence evaluator. It checks accepted callback
spacing, successful queue observations, complete GPU envelopes, and authoritative
start/stop time-loss counters. A new `submitReturnedAt` observation includes the
synchronous encoder finish and submit cost in callback deadlines. Historical
`submittedAt` and latency fields keep their original pre-finish origin. Saved
raw evidence is checked again by the suite, which requires repeated complete
coverage. Compositor and physical presentation remain unavailable.

New captures also record `callbackEnteredAt` from `performance.now()` at actual
callback entry. Application pacing v2 uses that observation for CPU ordering and
entry-to-submit deadlines, while scheduled rAF timestamps still determine
opportunity cadence. A scheduled timestamp can precede the previous callback's
finish even though the next callback has not entered yet. Legacy captures retain
their original v1 evaluations; the strict evaluator and thresholds are unchanged.
This correction does not change the failed Town completion counts or cadence.


A same-context direct/bundle/bundle/direct diagnostic preserves all compared
pixels but completes about 57.2, 44.5, 40.0, and 38.8 Hz respectively. Render CPU
time and Wasm execution time both increase across the run. These results do not
establish a stable submission-policy winner. A separate native graphics-process
sample shows indirect encoding and Metal submission waits; stripped Chrome
symbols and sleeping-thread stack counts prevent attributing the main bottleneck
to Dawn bundle replay. The full source and raw evidence hashes remain recorded.

Two thirty-second standalone Town night-spawn windows complete about 49.8 and
50.7 Hz at 1280 × 720. All thirteen functional and fixed-image checks pass,
including exact player/editor pixels. Render-only coverage does not qualify the
full standalone workload matrix. These tests also ran while a separate Medieval
Town app was active in the iOS simulator; host contention remains uncontrolled.
The product still needs additional optimization and complete sustained checks.

Moving camera and reflection views now reuse their own source-draw records.
The scoped mocked-upload benchmark saves about 0.03 ms per preparation while
removing thousands of repeated draw-object allocations; this does not establish
an FPS improvement. Failure tests cover allocation and upload retries, including
returning to an earlier camera after a failed preparation.

A mirrored-mesh index optimization keeps shaders, draw order, and geometry
counts unchanged while reducing pipeline switches. All six compared night-spawn
image pairs match exactly. Its short balanced comparison completes 48.16 Hz with
the original indices and 52.71 Hz with the candidate. The later original arm is
slower than the first, so host drift remains a limitation. This candidate still
falls short of 60 Hz. A subsequent quality-only run passes all 36 camera and
resolution cases, including 108 full-resolution captures, with exact pixels and
actual camera, lighting, and actor metadata. Wire overlays, custom fragments, and
stock shadow and reflection restoration also match. MT-07 uses the app's existing
fallback camera because it has no authored source-camera mapping. The bounded
optimization is now enabled for stock full-player/editor views; unsupported or
oversized cases retain their original path.

The clustered-light builder now retains validated source records while the source
array and its explicit revision stay unchanged. Camera memberships and shadow
assignments still rebuild. All 51 authored-camera comparisons retain identical
output bits. In a paired Node microbenchmark, the 488-light night preparation
changes from about 0.645 to 0.356 ms per build. This CPU measurement does not
establish an FPS improvement.

Fresh Empty, Room, Character, and Town builds pass the existing delivery limits
and dependency audits. Real browser checks pass all 46 functional checks,
including exact player/editor images and wire-overlay restoration. The first
combined attempt exposed a missing editor mesh hook when opening published
Rooms; that regression is fixed and verified through the real command path.
Town JavaScript is 687,441 raw bytes against the unchanged 688,128-byte limit.
These integration checks contain no timed FPS windows; sustained qualification
remains outstanding.

Four subsequent 30-second night-spawn windows at 1280 × 720 complete about
44.3, 43.9, 41.6, and 38.8 Hz (static, static, sway, sway). Two separate
15-second sway diagnostics with optional profiling disabled complete about
39.7 and 45.3 Hz. They are separate contexts without a matched baseline; neither
set establishes a causal speedup or smooth gameplay. All quality checks pass,
window time-loss counters stay unchanged, and actual callback-entry ordering
checks pass. Completion count and cadence still fail. CPU timings drift while
accounted GPU bytes stay bounded. The separate simulator Town remains active,
and physical presentation remains unmeasured. Additional draw-call reduction
and complete sustained qualification are still required.

Material staging, image overrides, and scalar editing now load only with editor
features. All four delivered players exclude those modules and pass another 46
browser checks, including exact player/editor images and wire restoration. An
owned Starter fixture passes twelve additional GPU checks for staging,
activation, discard, failed images, scalar reset, and original-image restoration
through the published Room host. Town and Character JavaScript now contain
684,317 raw bytes; the unchanged limit is 688,128. The full Node suite passes
995 tests with two optional private-fixture skips, and type checking and the
editor production build pass. This reduces delivery size; these checks contain
no timed FPS windows and do not qualify sustained performance.

Stock pipeline choices are now cached for bundle recording and direct authored,
room, and debris draws. A paired Node command-sink benchmark reduces its isolated
resolver-heavy loop by about 44–47%; this is not browser FPS. Cached source-bundle
reuse already performs zero resolver calls, including damage-only indirect
updates. The cache cannot solve the steady undamaged Town bottleneck. Another
46 delivered-player checks and twelve owned material GPU checks pass with exact
images. All delivery budgets pass; Town JavaScript is 685,232 raw bytes. The
1,004-test suite passes 1,002 tests with two optional private-fixture skips.

A separate private batching experiment preserves the original shader and fixed
pixels while reducing source draw commands by 29.27%. It adds about 12 MB of GPU
data and submits 0.417% more source indices before hardware clipping. Its four
30-second windows complete about 55.0, 52.6, 50.6, and 43.8 Hz in
original/batched/batched/original order. Balanced rates are 49.44 versus 51.61 Hz,
but CPU costs drift substantially and another task's Rust compiler is active.
The roughly 4.4% aggregate difference is confounded; GPU pass envelopes stay
roughly steady. This candidate remains outside product code. No threshold or
quality requirement was relaxed, and the complete matrix still needs work.

A private scheduling control uses four fresh visible Chrome processes in
original/flag/flag/original order. The sole intended argument change is
`--disable-renderer-backgrounding`; source batching stays off. The thirty-second
night windows complete about 59.4, 47.5, 42.5, and 46.0 Hz. A fixed JavaScript
frustum probe slows after every window in both configurations. Source, assets,
browser arguments, document focus, visibility, and fixed images pass their checks.
The flag does not eliminate the drift and has no demonstrated benefit. The
chronological change and external activity prevent attributing the aggregate
difference to the flag. The probe shares renderer code and allocates temporary
arrays, so it cannot identify a CPU frequency, scheduling, JIT, or collection
cause. No browser flag or production scheduling policy changed.

A further two-window control adds an independent, allocation-free C/Wasm integer
probe. Its fixed input, checksum, module, and memory contents stay unchanged.
The first window's median probe time changes from 5.1 to 8.0 ms, alongside the
shared JavaScript probe's 5.2 to 10.5 ms; both stay near those slower rates in the
second window. A renderer-shared-function explanation is therefore insufficient
by itself. These are separate workloads, not a C-versus-JavaScript speed contest.
The scene completes about 42.24 and 41.67 Hz, with exact reference images.
Initial thread CPU brackets are valid; later brackets are unavailable because a
different owned renderer process exited. Seventy-nine aggregate hardware samples
cover both windows, but cannot locate the worker on a core or establish a
frequency, scheduling, thermal, or compiler-tier cause. Both windows include the
observers; their measured CPU cost does not establish their FPS impact.

Planar-lighting change detection now compares a fixed numeric snapshot instead
of constructing an array and string each eligible frame. Differential tests
retain the original revision behavior, including signed zero and nonfinite
values. The isolated detector saves about 0.009–0.014 ms per check in Node;
this small allocation reduction is not an FPS result. All four player builds
pass the existing delivery limits and dependency audits. Town JavaScript is
685,569 raw bytes, including the separate indirect-upload retry correction.

A private render-bundle experiment omits draw IDs that have never had a nonzero
instance count. It preserves the compared source pixels, geometry, shader,
compacted buffers, indirect offsets, and draw order while reducing the fixed
night capture's recorded source commands from 11,498 to 7,500. Moving views admit
41 further records and rebuild 60 pass bundles in each candidate window; that
cost remains in the results. This candidate is not enabled in product code.

The corrected four-window comparison fixes camera drift in its earlier runner
and pauses the separately owned simulator for 153 seconds, then verifies its
automatic resumption. No Rust builds are observed at the checked boundaries;
other system activity remains. Original/candidate/candidate/original windows
complete 60.014, 59.889, 54.045, and 47.407 Hz. Render CPU medians rise from
3.3 to 6.5 ms and native-request medians from 0.7 to 1.4 ms, while GPU envelope
medians change from 9.57 to 10.95 ms. The first window still fails the strict
fence-notification p95 check. Pausing the simulator does not remove the drift;
neither the command reduction nor a specific scheduling cause has been
established as a solution. The complete matrix remains unqualified.

The [phase wrap-up](research/medieval-town-performance-phase-2026-10-23.md)
records the final four-window standalone-player control and the stop requested by
the owner. Accepted rates are 60.021, 59.995, 59.126, and 53.010 Hz; all four
strict and application-pacing verdicts fail. A fixed C/Wasm probe also slows on
the player's main thread, with increased accounted CPU time. The cause remains
unproven. Player quest props select an existing global baked-lighting fallback,
so this is not an identical-content worker comparison. Exact player images and
cleanup pass. The simulator resumes automatically after measurement; no further
experiment is scheduled.
