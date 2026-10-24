# Behaviors, scripting, and hot replacement

Status: proposed specification. This is a deliberate restricted TypeScript
authoring language and an owned runtime contract, not an implementation or a
promise to run arbitrary JavaScript packages. [Security](security.md) owns
origin isolation and trust; [runtime](runtime.md) owns tick ordering and live
state; [contracts](contracts.md) owns message compatibility.

[Behavior bytecode v1](behavior-bytecode.md) defines the concrete module layout,
instruction families, verifier, metering, default limits, and conformance vectors.

## Language and execution choice

Creators write typed behavior modules with editor completion, diagnostics, and
source-level debugging. An Aethyrforge compiler accepts a documented TypeScript
subset and emits versioned, verified bytecode for an owned Rust/Wasm behavior
VM. Each VM instance executes inside an isolated script worker on the preview
origin. It cannot access the editor's JavaScript realm, Wasm memory, GPU device,
credentials, or document store.

The safe profile supports finite numeric values, booleans, bounded strings and
arrays, records, tagged unions, lexical functions, conditionals, bounded
iteration, and explicit module imports. Computation uses specified numeric
semantics; nonfinite results fail the invocation. Recursion has a strict depth
limit. Static imports resolve only through a pinned module manifest. Supported
syntax and library functions form a versioned conformance suite. Unsupported
features fail with source-linked diagnostics, rather than switching to native
JavaScript execution.

The safe profile excludes `eval`, `Function`, ambient globals, DOM access,
prototype mutation, dynamic property execution, dynamic imports, threads,
unrestricted regular expressions, native modules, and arbitrary npm dependency
execution. Asynchronous actions use host request handles, not free-running
promises or wall-clock timers. TypeScript types help authoring but do not grant
authority or replace runtime checks. The
[TypeScript handbook](https://www.typescriptlang.org/docs/handbook/2/everyday-types.html)
describes compile-time types and assertions; Aethyrforge adds its own bytecode
verification and checked host boundary.

This choice exchanges full JavaScript compatibility for bounded execution,
inspectable effects, and predictable reload. Effect remains the trusted
application orchestration layer; behavior modules do not inherit its full
environment or service graph. Trusted engine extensions require review and a
separate installation contract. They are not a hidden escape hatch in the safe
profile. Heavy navigation, animation, physics, and geometry work use coarse
trusted Rust kernels instead of millions of interpreted scalar instructions.

## Module contract and state

Each module declares identity, source and bytecode hashes, language and SDK
versions, state schema version, required capabilities, and resource limits.
Entry points are initialize, update, event, migrate, and dispose. Initialize
returns initial state. Update receives the previous complete tick's read-only
view, fixed timestep, seeded random stream, and explicit state. It returns
replacement state and a bounded list of proposed runtime actions.

Persistent state contains checked scalars, bounded collections, stable entity
IDs, and typed resource references. Functions, closures, raw pointers, DOM
objects, GPU objects, and unresolved host call objects are not persistent state.
The engine serializes state outside the guest VM at accepted tick boundaries.
An entity reference may resolve as absent after an edit; code must handle that
typed result. Persisted state has per-module and per-world byte budgets.

A script can propose movement intent, animation parameters, bounded spawning,
audio cues, and registered game events within its granted scope. It does not
directly write physical transforms or the authored world document. The host
validates every action and state result before applying them atomically for
that invocation. Invalid output leaves the prior state and unapplied actions
intact. Competing valid proposals use the runtime's stable ownership and
ordering rules; iteration over an unordered hash table cannot decide gameplay.

Each invocation targets a tick and a deadline. The runtime never blocks rendering
or waits without a bound for a script response. A missed deadline preserves that
instance's accepted state and produces no actions for the missed tick; a late
result is discarded. The next invocation receives the current logical tick and
missed-tick count. Pending events have bounded queues and explicit expiry. A
profile defines how repeated misses disable an instance, and records these
misses separately from fuel exhaustion.

## Capabilities and asynchronous work

A capability identifies an operation, permitted resources, rate limits, and
expiry. Importing a function is not permission to invoke it. The host checks
capability scope at every request and issues opaque handles that contain no
editor token. External access goes through a separate broker with explicit
destinations and data limits. The default behavior profile has no network or
persistent storage access.

Host requests include script instance, invocation, generation, request, and
target tick identities. Responses become events at a later defined tick. A
retry uses the same request identity; an old response cannot enter a replaced
instance. Removing a capability revokes pending handles. Irreversible actions
require an idempotent broker contract and the relevant product approval; a
script reload cannot retract a payment or an already delivered message.

The trusted supervisor and untrusted worker communicate through a validated
channel with size and rate limits. The
[WHATWG channel model](https://html.spec.whatwg.org/multipage/web-messaging.html#message-channels)
provides transport primitives; Aethyrforge still authenticates its handshake,
validates messages, and enforces capabilities. Wasm isolation limits guest
memory access but does not establish the safety of imports supplied by the
host. [WebAssembly security model](https://webassembly.org/docs/security/)
describes this embedding boundary.

## Bounded execution and interruption

The bytecode verifier rejects invalid control flow, unknown instructions,
unchecked host calls, and invalid stack use. Every instruction consumes fuel;
allocation, string processing, and host calls charge size-sensitive costs.
Each invocation also has limits for stack depth, live memory, output bytes,
entity queries, and actions. Reaching a limit aborts that invocation before
state or actions commit. Native host functions must be bounded, cancellable
jobs or charged bounded queries; a nominally cheap instruction cannot hide an
unbounded native operation.

A watchdog outside the worker enforces a wall-time deadline. On missed
heartbeats it revokes the worker's capabilities, terminates the worker, and
restores the last accepted state in a replacement VM. Termination is a recovery
backstop, not a hard real-time browser scheduling guarantee. Repeated offenders
are disabled with a diagnostic and manual retry. Other behaviors and the
editor remain usable. Debugging can increase budgets explicitly, but such a
session cannot qualify a release's normal execution profile.

## Live replacement

The compiler stages a candidate without disturbing the active module. It checks
bytecode, imports, state schema, resource profile, and the requested generation.
At a tick boundary the supervisor checkpoints the active instance and runs a
fuel-limited migration on a copy of that state. Migration has no external
effects. Unchanged schemas preserve state directly; changed schemas require an
explicit migration or a creator-selected reset of that behavior instance.

After successful validation the supervisor atomically installs the module and
migrated state, revokes old handles, and resumes at the next tick. Camera,
player position, unrelated entity state, and other modules survive compatible
changes. Compilation or migration failure retains the previous module and
state. Stale candidates cannot replace newer source. Removing a module cleans
up subscriptions and owned runtime objects under an explicit ownership policy.
Undoing source does not silently rewind all gameplay state; a separate replay
or checkpoint restore performs that action.

## Qualification

Tests exercise infinite loops, recursive calls, oversized allocations, forged
handles, malicious bytecode, stale async replies, worker death during migration,
action conflicts, and thousands of reloads. Differential tests compare supported
language semantics against the documented subset. Source maps must identify the
creator's line on traps. [Performance](performance.md) defines budgets and
[verification](verification.md) requires evidence before the safe profile ships.

- **SCRIPT-01:** Forbidden syntax, forged bytecode, and missing capabilities
  fail without entering trusted editor or kernel authority.
- **SCRIPT-02:** Fuel exhaustion and watchdog termination preserve accepted
  state, discard incomplete proposals, and leave unrelated behaviors usable.
- **SCRIPT-03:** Compatible hot replacement preserves typed state; failed
  compilation or migration keeps the active module, and old replies stay revoked.
