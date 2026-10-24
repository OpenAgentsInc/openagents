# Runtime and simulation

Status: proposed specification. No runtime exists in this repository. Performance
budgets are qualification targets, not measured results. [Architecture](architecture.md)
defines the execution boundaries; [contracts](contracts.md) owns wire formats
and compatibility, and [world model](world-model.md) owns authored data.

## Execution and ownership

The editor shell uses React DOM, Effect Atom, and Effect 4 on its trusted
origin. React DOM renders controls and accessible panels. Effect Atom exposes
selected service state to those controls; Effect 4 owns asynchronous service
lifetimes, cancellation, bounded queues, diagnostics, and disposal. These tools
do not schedule individual entity updates, vector operations, or simulation
ticks. A project-scoped Effect 4 service owns the preview connection and worker
lifetime independently of panel subscriptions. Its resource scope must also
release resources that finish acquisition after cancellation.
[Effect 4 resource scopes](https://effect.website/docs/v4/resource-management/scope)
describe the lifetime mechanism; the implementation pins its exact version.

A preview on a separate origin owns one trusted preview worker for simulation
and GPU command encoding, plus an isolated script worker. The preview worker
calls the Rust/Wasm kernel and owns an OffscreenCanvas. Compilation uses a
bounded worker pool or the Rust build service. A supervisor detects missed
heartbeats and can replace it. Workers execute independently of interface code,
but consume memory and scheduling resources; the engine uses a small pool,
never a worker per object. [WHATWG worker processing model](https://html.spec.whatwg.org/multipage/workers.html)
defines the underlying lifetime and termination behavior.

The Rust/Wasm kernel owns live entity components, physics, spatial indexes,
navigation, animation state machines, and simulation snapshots. The preview
worker owns GPU objects. The editor owns the authored document; the simulation
cannot mutate it by changing a live entity. Runtime changes become authored
changes only through an explicit bake command and the same validated
transaction path used by other edits.

## UI and project lifecycle boundary

DOM controls publish typed commands and coalesced input batches through the
trusted broker. Command validation, captured context, origin separation, and
capability checks remain mandatory. Coalescing accumulates pointer deltas and
preserves button transitions; it cannot discard a release event or reinterpret
an authored command against a later selection. Inputs expire on focus loss or
disconnect rather than leaving movement held indefinitely.

The preview worker advances simulation, camera integration, physics,
animation, and GPU encoding using its own clock and queues. It never waits for
a React render, React effect, or Effect Atom notification before advancing a
tick or presenting a ready frame. A busy, suspended, or unmounted inspector
cannot become the simulation scheduler. Page visibility and explicit pause
commands follow the runtime policy below; React's panel state is not that policy.

The engine reports bounded selected summaries, command receipts, diagnostics,
and rate-limited telemetry to the project service. Effect Atom holds those
summaries, not a per-frame reactive copy of every transform, collider, animation
value, or GPU resource. Selection and command results publish promptly;
numerical inspector values and profiler counters use coalesced per-view update
budgets. Full snapshots remain explicit bounded inspection/export operations.
Dropping an intermediate UI summary never drops an engine tick or a command.
Command outcomes remain recoverable from durable receipts/status reads. Bounded
lifecycle streams carry cursors and expose gaps; history/current-state recovery
and explicit diagnostic truncation follow [frontend](frontend.md#independent-frame-loop).
No reliable UI notification queue can block ticks or grow without a limit.

Panels attach and detach subscriptions without creating, resetting, or disposing
the project engine. The preview surface has a stable project lifetime; replacing
its presentation attachment uses an explicit handshake and preserves compatible
simulation state. Project close or restart explicitly stops input admission,
cancels pending work, saves/checkpoints according to policy, revokes broker
ports and capabilities, and stops the frame loop before disposing GPU resources
and workers. Shutdown has a deadline and reports unresolved durability or a
forced stop. Unmounting a React panel performs only its own subscription and
DOM cleanup. This project service is not a browser ServiceWorker or permission
to move untrusted code into the editor origin.

## Kernel boundary

[Runtime ABI v1](runtime-abi.md) fixes the baseline exports, binary layouts,
handle ownership, limits, error codes, and upgrade fixtures. Higher-level host
adapters cannot silently widen that contract.

The kernel exposes versioned batch operations: create session, stage generation,
validate migration, activate generation, submit input batch, advance tick,
read snapshot, restore checkpoint, and dispose. Each request includes a session
identity, request sequence, ABI version, active generation, and bounded payload
length. A response reports the applied tick or a typed failure. Stale, duplicate,
and out-of-order requests have explicit outcomes; a retry never applies input
twice. Entity IDs remain stable while internal dense indexes may change.

Packed component tables use little-endian scalars, explicit alignment, offsets,
strides, counts, and format versions. JavaScript never holds Rust object
pointers. The kernel validates offset arithmetic, lengths, finite numbers,
entity handles, and maximum counts before reading a batch. A snapshot is
immutable until its receiver releases its sequence. One writer owns each slab.

Inside the preview worker, JavaScript borrows checked Wasm views only until
their explicit release boundary. Cross-worker messages transfer reusable
ArrayBuffers containing snapshots and inputs.
The cross-origin-isolated profile, the primary web performance profile under
[decision 0006](decisions-history/0006-cross-platform-engine-core.md), uses a bounded shared-memory ring
with atomic publication and independent read/write cursors; a full ring applies
backpressure rather than overwriting unread state. Neither profile assumes
that Wasm memory is GPU memory. GPU upload is an explicit accounted operation.
Memory growth occurs only at a stopped boundary; the host refreshes its views
after growth and rejects stale view generations. The
[WebAssembly JavaScript interface](https://webassembly.github.io/spec/js-api/index.html#memories)
defines memory interaction; memory sharing is a separately qualified platform
capability, not a prerequisite for correctness.

Separate render and simulation workers are a later qualified profile, requiring
measured latency benefit and bounded snapshot age. The baseline needs no shared
memory. World transforms use 64-bit CPU coordinates; active physics and GPU
data use 32-bit local coordinates. Each profile bounds its active region,
origin-shift threshold, and shift quantum. A tick-boundary shift moves all
local positions and indexes together; snapshots include their origin identity.
Global entity IDs, authored integer-millimeter topology, and saved world poses
do not change. Large worlds stream local regions instead of extending one
low-precision physics space indefinitely.

## Time, input, and order

Simulation uses 60 fixed ticks per second. Inputs carry sequence numbers and an
assigned tick. A local session consumes at most four overdue ticks per render
opportunity, records dropped wall time, and resumes without a burst of old
inputs. A hidden local preview pauses gameplay and clears its accumulator;
resume requires fresh input. An authoritative server keeps its own monotonic
tick and overload policy. Clients never accelerate the server by advancing
their wall clocks.

Within each tick the order is: accepted generation activation; input sampling;
script proposals; validation and application of proposals; navigation steering;
character and rigid-body simulation; animation state evaluation; event emission;
snapshot publication. Script queries read the completed previous tick, avoiding
order-dependent reads of partially updated entities. Rendering interpolates the
two most recent complete snapshots. Camera look may use a predicted presentation
pose; authoritative collision uses the kernel pose.

Script batches target an assigned tick and must arrive before its deadline;
missing batches follow the bounded skip policy in [scripting](scripting.md).
The simulation never waits indefinitely for another worker.

The engine records random seeds, input order, engine build, generation hashes,
and tick numbers. It does not promise floating-point equality across browsers,
CPUs, compiler versions, or GPU results. Replays qualify identical pinned
environments; authoritative networking corrects clients. Competitive rules
cannot depend on GPU simulation or client-reported collisions.

## Simulation systems

The physics/controller implementation is owned Rust behind the versioned
kernel contract. An independent library can serve as a differential test
oracle; it does not own Aethyrforge bodies, scene topology, state, or ABI.
The initial rigid-body profile uses a dynamic AABB-tree broad phase, static
triangle BVHs, GJK/EPA convex contact generation, persistent contact manifolds,
and a warm-started sequential-impulse solver with eight velocity and three
position iterations. Convex dynamic hulls have at most 64 vertices. Static
concave meshes are supported; dynamic concave shapes, deformable bodies,
cloth, and vehicle systems need separate later profiles. Material friction
combines geometrically; restitution uses the larger value clamped to [0,1].
Gravity defaults to (0,-9.81,0) meters per second squared.

The walking controller owns swept capsule movement, contact sliding, grounded
state, and step-up/step-down tests. Doorways, floor support, and clearance use
the same compiled collision generation as body queries. A character transform
is its foot position; capsule standing height includes both hemispheres.
Continuous casts have
a maximum of 16 conservative-advancement iterations; failure to converge
clamps movement to the last verified safe position and reports the limit,
rather than disabling collision. Physics solver and contact-order versions
are pinned in checkpoints. Independent reference cases compare contact
separation, energy bounds, resting stability, and traversal properties rather
than assuming unrelated solvers produce identical trajectories.

Physics separates static compiled collision, kinematic characters, dynamic
bodies, triggers, and query-only shapes. Broad-phase indexes update only dirty
regions. Swept character movement and continuous collision modes prevent
supported fast objects from tunneling; configured speed, scale, and timestep
limits define that support. Sleeping bodies retain stable IDs. Collision events
have tick-local IDs and enter/exit semantics; replacing geometry cannot replay
old contact rewards. Negative or nonuniform scale requires a compiled collider
variant or is rejected for that body type.

The initial walking profile uses the world model's 0.3-meter capsule radius,
1.8-meter standing height, and 0.25-meter step. Initial building qualification
covers one storey; upper floors require tested stairs, holes, and clearance.

Navigation uses tiled compiled traversability, explicit portals, agent-radius
profiles, and live obstacle overlays. A path references its tile generation;
an affected path becomes invalid when that tile changes. Queries return bounded
partial paths or a reason for failure. Agents stop safely at an unready boundary
instead of walking through an unloaded or invalid region.

Animation state and gameplay events advance on the CPU tick; skinning and
visual interpolation may run on the GPU. Root motion is an input to collision,
not an unchecked render transform. Audio events have stable deduplication IDs,
positions, and target times. Browser Web Audio schedules playback; custom DSP
uses AudioWorklet with preallocated buffers and no compilation or network work
in its callback. The audio device clock and simulation clock are distinct.
[Web Audio processing model](https://www.w3.org/TR/webaudio/#rendering-loop)
governs the audio rendering thread. A user gesture enables audio, and suspension
or device changes do not replay stale sounds.

## Activation and recovery

Each candidate includes render, collision, navigation, script, and selection
outputs from one world generation. The supervisor stages all required outputs,
checks capacity and state migration, and activates at a tick boundary only
after both simulation and rendering staging succeed. The renderer presents
the matching generation and snapshot at its next frame boundary. It may
hold the previous complete frame while waiting; it never presents new walls
with old collision. [Asset compilation](asset-compilation.md) defines dirty
regions and artifact reuse.

An edit that encloses a player, deletes occupied support, or removes a live
script dependency needs an explicit safe migration. The default rejects
activation and leaves the working world running; relocation requires an
authored destination and creator approval. Compiler failure changes diagnostics,
not active state. Kernel traps stop that worker, retain the last checkpoint,
and offer recovery with the offending change disabled. Recovery must disclose
any lost unsnapshotted runtime time.

## Qualification

Every trace separates input delay, script time, kernel time, interop bytes,
snapshot age, and rendering delay. Tests cover long pauses, saturated rings,
Wasm growth, stale handles, tick overload, worker crashes, and occupied topology
edits. [Performance](performance.md) sets budgets;
[verification](verification.md) defines required evidence. A Rust rewrite is
accepted only with measured workload benefit or a necessary correctness
boundary, never an assumed language-wide speedup.

- **RUNTIME-01:** A fixed-tick input replay reproduces the qualified pinned
  environment, and exposes overload or discarded wall time.
- **RUNTIME-02:** Occupied topology edits activate only with a valid migration;
  failure preserves the complete previous generation and live state.
- **RUNTIME-03:** ABI fuzzing, stale views, origin shifts, worker replacement,
  and queue saturation cannot corrupt state or repeat an accepted input.
- **RUNTIME-04:** Repeated panel mount/unmount, delayed React rendering, and
  saturated UI subscriptions neither duplicate engine workers nor gate ticks;
  explicit project shutdown releases resources and records its save outcome.
