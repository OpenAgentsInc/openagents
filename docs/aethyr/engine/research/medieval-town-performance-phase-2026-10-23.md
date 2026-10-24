# Medieval Town performance phase wrap-up

Status: this phase ended at the owner's request on October 23, 2026. Sustained
smooth 60 FPS across the required workloads **has not been achieved**. The
[qualification contract](../performance-qualification.md) and its thresholds
remain unchanged. No further benchmark or implementation job is scheduled.

## Changes retained on main

- Allocation reductions in camera math, particles, lighting preparation, and
  source-draw preparation preserve the compared arithmetic and output.
- Planar reflections shade a conservative sampling rectangle. Unsupported
  projections retain the full pass.
- Canonical mirrored-mesh indices reduce pipeline switches. Prepared clustered
  lights reuse validated source records while rebuilding view memberships.
- Editor mesh inspection and material editing load separately from delivered
  players. Stock pipeline selection and planar-lighting change detection reuse
  bounded caches or numeric storage.
- Failed indirect-buffer uploads retry before a cached bundle can be reused.
- The optimized Wasm kernel is 768,330 bytes. The latest Town JavaScript is
  685,569 raw bytes against the unchanged 688,128-byte limit. All four player
  builds pass their delivery budgets and dependency audits.

The preceding [qualification notes](../performance-qualification.md#optimization-diagnostics-on-october-23)
and [derived evidence](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/experiments/kaigen/results/smooth-diagnostics-2026-10-23.json)
record the targeted tests, browser checks, exact-image comparisons, and earlier
full-suite results. These checks establish specific behavior and delivery claims;
they do not establish sustained frame pacing.

Command-pruning, expanded batching, custom-fragment culling, scheduling flags,
an iframe host prototype, and a backpressure-retry model remain private
experiments. None is presented as a qualified solution or enabled by this wrap-up.

## Final standalone-player test

Four consecutive windows used the immutable diagnostic Town player on Chrome
155 and the M5 Max host: night spawn, moving camera, 1280 × 720, eight live
knights, a 60 Hz attempt target, and full profiling. Each window had five seconds
of warmup followed by at least thirty seconds of measurement.

| Window | Attempts / accepted | Accepted submissions per second | Render CPU median | Wasm request median |
| --- | --- | --- | --- | --- |
| 1 | 1,801 / 1,801 | 60.021 | 3.1 ms | 0.7 ms |
| 2 | 1,800 / 1,800 | 59.995 | 3.2 ms | 0.7 ms |
| 3 | 1,800 / 1,774 | 59.126 | 4.0 ms | 0.9 ms |
| 4 | 1,801 / 1,591 | 53.010 | 6.8 ms | 1.5 ms |

CPU medians cover all render attempts, including rejected attempts; the nested
spans overlap and must not be summed. All four strict and application-pacing
verdicts fail. Even the first two windows have unacceptable completion tails.
Later windows also miss the required frame count. Submission rates and
queue-fence notifications are not measurements of physical display presentation.

Exact player images, camera settings, lighting parameters, native actor poses,
asset closure, and cleanup checks pass within this cohort. It is not an
identical-content comparison with the editor worker: the player includes two
quest props. The existing renderer disables baked probe GI and surface-sky
lighting globally when authored geometry is present, so the props also select a
different lighting path. The failed editor-image comparison is retained rather
than relabeled as a pass. This lighting behavior needs a separate validity audit.

## What the CPU probe establishes

An independent, allocation-free C/Wasm workload ran only between measurement
windows while the player was paused. Its module, input, memory, and checksum
stayed fixed. Median elapsed time rose from 4.4 to 10.2 ms. The corresponding
seven-call main-thread CPU bracket rose from 31.525 to 70.600 ms. Those brackets
include RPC and verification bookkeeping; they are not isolated native-loop CPU
timings.

The slowdown therefore also occurs outside the editor's DedicatedWorker and
includes increased accounted CPU execution time. Waiting or descheduling alone
cannot explain this probe result. It does not identify a clock, core-placement,
QoS, thermal, browser, or other host cause, and it is not a C-versus-JavaScript
speed comparison. Rewriting the renderer in C or moving it to the main thread is
not established as a remedy.

The owned Chrome process was confirmed as the native foreground application
before timing. The simulator was paused for about 149 seconds under the owner's
authorization, then automatically resumed and verified running. No Rust builds
were observed at the start. Other host activity was not eliminated. All owned
benchmark browser, context, server, worker, and observer processes were closed.

## Handoff

1. Explain and control the fixed-work CPU execution-rate drift before attributing
   a chronological FPS change to an engine optimization.
2. Audit baked-lighting validity for authored props, rooms, and fractured source
   geometry before comparing editor and player lighting paths.
3. Revisit command reduction or scheduling experiments only with a controlled
   baseline, exact visual checks, and bounded lifecycle behavior.
4. Once the difficult night-spawn workload stays within budget, run the complete
   repeated editor, player, controller, spell, destruction, knight, and aiming
   matrix at both required resolutions. The full matrix remains unqualified.

Private licensed images and assets remain outside Git. The committed derived
evidence retains measurements and hashes of the private raw records.
