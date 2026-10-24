# Runtime ownership and CPU worker experiment

Status: implemented prototype for [issue #4](https://github.com/OpenAgentsInc/aethyrforge/issues/4).
This document describes the current Town runtime. The broader
[runtime specification](runtime.md) still includes future systems.

## Owners and lifetimes

| Data | Owner | Lifetime and mutation |
| --- | --- | --- |
| Authored world and history | Rust kernel in the preview worker | Validated commands; candidate preparation does not commit |
| Player, ticks, collision, Druid damage | Same Rust kernel | Fixed-step simulation and explicit gameplay requests |
| Input, camera, clock, command queue | Preview worker | Session; input clears on focus loss and project close |
| Verified asset bytes | `CellStream` | Bounded cache; latest candidate controls activation |
| Prop matrices, bounds, selection poses | `RuntimePropTable` | Resident tables and reserved candidate scratch; generation-checked handles |
| Source bounds and render resources | `Renderer` | Renderer instance; replaced at a stream activation boundary |
| Frame scratch and GPU uploads | Renderer in the preview worker | Frame or prepared candidate; one GPU queue owner |
| CPU experiment bounds | Probe coordinator and up to two helpers | Immutable owned copies; install replaces a dataset |
| Helper output | One helper partition, then coordinator | Transfer on completion; merge in original source order |
| UI state | React and Effect Atom services | Bounded summaries and intentions; no frame scheduling authority |

The normal frame remains synchronous: Rust step, live gameplay and character
update, CPU render preparation, then GPU command encoding. Input callbacks and
accepted edits run between frame callbacks. Compilation and asset preparation
may wait asynchronously while the previous complete world continues to render.
They do not borrow live frame scratch across an `await`.

`TaskOwnership` checks declared resource reads and writes, owner identity,
conflicting task groups, synchronous borrow lifetimes, and structural safe
points. Town frames declare mutable kernel and renderer access. Authored
activation declares the same writes and runs at a safe point. Stream swaps
replace resource generations at a safe point. Probe snapshots declare renderer
reads. Helpers declare their culling inputs and exclusive output access.

The checker applies to trusted engine code. It is not a sandbox or deep proxy:
a callback must not retain a raw object or typed array after `withView` returns.
The checked borrow rejects later access through its wrapper. Generation changes,
cancellation, exceptions, and shutdown invalidate those wrappers. Tests also
reject missing declarations, conflicting writers, and asynchronous callbacks.

## Bounded worker trial

`CpuCullExperiment` executes the existing `frustumVisibility` algorithm in the
coordinator or one/two resident browser workers. It clones source bounds before
transfer, installs each partition once, sends a small matrix per measurement,
and merges masks in source order. The renderer's live arrays are never detached.
Generation, sequence, partition, lengths, finite values, and output masks are
checked. A stalled helper times out and terminates; cancellation, supersession,
and disposal release listeners, timers, and helpers.

The supported path uses ordinary `ArrayBuffer` transfer and needs no
`SharedArrayBuffer`, cross-origin isolation, or threaded Wasm. The serial
control remains available without any helper. Live Town culling remains serial:
probe results are frozen measurement data and never become visibility masks for
a later camera or capture. There is no frame wait for a helper.

The benchmark-only `cpuCullProbe` request snapshots actual Town bounds and the
current view at a safe point, checks results against an untimed serial reference,
and reports installation, computation, dispatch, round trip, merge, explicit
buffer copies/transfers, partial owned-buffer peaks, and concurrent frame counts.
It permits one active probe, at most two helpers, and 100 jobs per request.
Cancel bypasses pending edits. `EditorService.close()` sends zero input and
unloads the preview, which stops workers and releases its graphics context.

## Reproduce

Run `npm test`, `npm run check`, and `npx vite build`.
Run `tools/verify-cpu-cull.mjs` through `openagents browser run`, with an external
`AF_CAPTURE_DIR` and `AF_VERIFY_WORLD=starter` or `medieval-town`. The private Town
profile needs `AETHYRFORGE_PRIVATE_ASSETS`.

The tool runs CPU-only seeded 1k/10k/100k bounds first, then opens one GPU world.
It uses balanced trial orders, verifies exact masks outside timed kernel work,
and measures idle/serial/one-helper/two-helper frame windows with actual input.
Startup and installation stay separate from resident jobs. Shader quality and
the live render algorithm do not change. Callback cadence, accepted submissions,
GPU pass timing, and input response are separate measurements; none alone proves
displayed frame rate. All measurements are local to the recorded browser and
device. Raw reports and private captures stay outside Git.

## Recorded result

The [scrubbed result](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/experiments/kaigen/results/issue-4.json) records 50 browser
checks in each world, including real add/transform/undo/redo/remove activation,
cancellation, held-input shutdown, and released workers. All serial/helper masks
match exactly. Type checking, 597 TypeScript tests, the production build, and
spec checks pass.

On the recorded Town dataset of 8,118 instances, resident job median total time
is approximately 0.1 ms serial, 0.2 ms with one helper, and 0.1 ms with two.
The balanced seeded 100k workload measures 1.5 ms serial, 2.6 ms with one helper,
and 1.6 ms with two. Helper installation adds milliseconds and duplicates resident
bounds. Timer resolution limits the smaller measurements.

Four balanced one-second Town windows per condition record 33–35 accepted GPU
submissions and similar CPU preparation costs for idle, serial probe, and helper
probe conditions. The main HDR GPU pass remains about 22 ms in a representative
idle window. This does not demonstrate an integrated worker benefit or displayed
frame rate. The default remains serial; the measured experiment stays available.

After merging current castle destruction support, the complete Town verifier
passes again against the rebuilt kernel: 602 TypeScript tests and 60 Rust tests
pass. The result records that source commit and Wasm identity separately from
the initial comparison. Castle-specific browser qualification belongs to the
castle verifier; this run uses the frozen Druid asset fixture.
