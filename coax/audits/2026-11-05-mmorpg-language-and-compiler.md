# Coax for an entire MMORPG: language and compiler audit

Date: 2026-11-05. Source baseline: `a2e6d3ef99` on `origin/main`.

This is an engineering assessment of the checked-in book and implementation,
not a claim to have qualified a new game, benchmarked production capacity, or
completed the language roadmap. Proposed APIs below are requirements, not
accepted syntax. No language, protocol, authority, or deployment changes are
made by this audit.

## 1. Assessment

I would build an MMORPG in Coax, but I would budget substantial language and
toolchain work alongside the game. The strongest foundation is the combination
of explicit bounded state, caller-owned views, checks beside the code, generated
native interfaces, and shared gameplay arithmetic. Those are valuable for an
authoritative game whose failures can duplicate an item, leak hidden state, or
corrupt a character. The existing game is much more than a language demo.

The largest risk is **making every subsystem hand-maintain the same lifetime,
storage, schema, and scheduling conventions**. Those conventions can work in a
small bounded room; hundreds of interacting gameplay systems and years of live
content make them an expensive safety boundary. I would prioritize enforceable
ownership and read access, useful storage libraries, composable error states,
incremental compilation, and a small deterministic job model before broader
syntax or a new machine-code backend.

An entire MMORPG does not require every proposed feature before its first
release. One authoritative process per bounded instance is a good starting
architecture. The priority changes when that process must host dense encounters,
stream persistent world cells, or keep player transactions correct across crashes
and ownership transfers. Cross-process consistency cannot be supplied by a
language keyword.

## 2. Scope, evidence, and what “MMORPG” means here

The hypothetical product includes persistent characters, inventory and crafting,
quests and dialogue, guilds and parties, trade, instanced dungeons and connected
outdoor regions, thousands of concurrent players across the service, crowded
encounters, desktop/mobile/browser clients, streamed assets, moderation, live
content updates, and recovery after crashes or deploys. Any real payment remains
Bitcoin-only under repository policy. An item ledger is not itself a payment rail.

For design pressure, assume a **proposed** 30 Hz authoritative simulation, up to
256 players and 10,000 active simulated entities in a stress region, and 10,000
concurrent players distributed over many regions. These are deliberately larger
than today's room profile, not measurements or commitments. At 30 Hz the entire
tick has 33.3 ms; choose a provisional p99 simulation budget of 20 ms to leave
headroom. A 64-byte component for 10,000 entities occupies 640,000 bytes before
history, indexes, replication, or other components. A million-byte value copied
through several helpers is a different problem from a small scalar kernel.

I inspected the book's current-status catalogue and roadmap, memory, sums,
generics, layout, jobs, asynchronous work, determinism, GPU, builds, debugging,
agent tools, compiler internals and physical-world chapters; then followed the
contracts into compiler and engine source. Important evidence anchors:

| Evidence | What it establishes for this audit |
| --- | --- |
| [Book status and registry](../book/README.md), [catalogue](../book/appendix-a-keywords.md), [specification](../spec.md) | Current bounded features are separate from designed syntax; compiler release is recorded as 0.48.0. |
| [Compiler tour](../book/ch27-inside-coax0.md), [checker](../lib/compiler/checker.coax), [native bridge](../src/check.cpp), [C emitter](../src/emit_c.cpp) | Coax lexer/parser/checker, C++ evaluator/emitter and graph transport; explicit capacity reporting and native lowering. |
| [Memory chapter](../book/ch06-memory.md), [returned-borrow contract](../verification-return-borrows.md), [retained-store contract](../verification-retained-stores.md) | Useful synchronous borrow analysis exists, with incomplete relations and bounded refusal; it is not a general ownership system. |
| [Tagged-value contract](../results.md), [generic checker](../lib/compiler/checker/generics.coax), [bounded container](../lib/bounded.coax) | Results and fixed-storage generic values ship; important containment and borrow-containing template restrictions remain. |
| [Entity store](../../aethyr/entity.coax), [clock](../../aethyr/loop.coax), [replay](../../aethyr/replay.coax) | Generations, stable session IDs, deferred structural changes, fixed steps and external-event recording already exist. |
| [Interest implementation](../web/room/room-interest.coax), [replication chapter](../../aethyr/book/ch091-one-record-for-every-replicated-entity.md) | Spatial interest and acknowledged-baseline replication are delivered facilities, not missing language features. |
| [Current invariants](../../INVARIANTS.md), [travel](../web/room/room-travel.coax), [checkpoint contract](../../docs/protocol/character-checkpoint-v2.md) | Current ownership fencing, travel and durable character recovery are more advanced than some older chapter summaries. |
| [Earlier MMO analysis](../../aethyr/book/appendix-mmo-gap-analysis.md) | Useful game-level context, explicitly containing a historical snapshot and subsequent updates; not a current inventory by itself. |

The conclusions below are source-grounded design judgments. “Missing” means a
general language/compiler facility is absent from the documented supported
profile, or that the cited implementation has a narrower contract than the
hypothetical workload needs. It does not mean there is no local implementation
anywhere in this large repository. Historical issue references are context, not
a live GitHub issue-state check.

## 3. Preserve what is already working

* **Bounded, explicit storage.** Fixed arrays and numeric views allow predictable
  admission and allocation-free ticks. Keep this path even when owners arrive.
* **Recoverable errors.** Nominal unions, optionals, exhaustive switches, direct
  handled Results, propagation and lexical `defer` already ship. “Add algebraic
  data types” would be an inaccurate recommendation.
* **Generic code and modules.** Procedures, fixed-storage templates, built-in
  constraints, qualified imports and explicit exports exist. The missing pieces
  concern expressiveness and safe composition, not basic modularity.
* **Engine identity and replay.** `entity.coax` separates generation handles from
  persistent/network IDs, retires exhausted generations, and flushes structural
  commands deterministically. `replay.coax` records accepted external events and
  checks step digests. Build on these rather than inventing another ECS.
* **Typed clocks as a library.** `coax/lib/clock.coax` supplies clock-tagged
  instants, explicit conversions, sample age and fixed-order schedules. Chapter
  30 distinguishes that delivered library from planned clock effects, rate
  syntax and adoption by the Verse and robot loops.
* **Real multiplayer contracts.** Everglade has acknowledged deltas, generation
  and warp handling, spatial interest and disclosure rules, bounded sends,
  reconnects and authoritative state. The current invariants also describe
  profile-2 complete character checkpoints, initial durable acceptance, a
  checkpoint-age pause rule, restore, travel and drain fencing. Persistence is
  not wholly absent. Item transactions and general world recovery remain a
  broader requirement than that character capture.
* **GPU compilation.** Scalar Metal is only one profile. Coax also has checked
  WGSL/SPIR-V graphics and Metal vertex/fragment emission, reflection, compute
  profiles and resource contracts. The early scalar tutorial's restrictions
  must not be generalized to all shader support.
* **Developer evidence.** JSON diagnostics, checked declaration queries and a
  validated query cache, source locations/debugging, compile budgets, provenance,
  native check emission and mutation support already provide a strong base.

## 4. Prioritized gap map

P0 means I would tackle it during the first serious MMO vertical slice; P1 means
before increasing density or the pace of live content; P2 is conditional on a
demonstrated bottleneck. Priority is an ordering recommendation, not a claim
that the current product is blocked.

| Gap | Primary home | Priority | First useful consumer |
| --- | --- | --- | --- |
| Read-only/exclusive regions and explicit retention contracts | Language + checker | P0 | Asset tables and queued network/world work |
| Controlled owners, pools and reusable collections | Language + library + host allocation effects | P0 | Region state and streamed cells |
| Stored fallible outcomes and generic borrowed carriers | Language + checker | P0 | Save/request queues and content loading |
| Nominal IDs and consistent critical integer semantics | Language + runtime | P0 | Character, item, owner and request identities |
| Content schemas and canonical generated operations | Compiler + Coax tools + protocol | P0 | Item/ability definitions and character saves |
| Measured incremental builds and capacity diagnostics | Compiler/tooling | P0 | Whole region server and content editor |
| Structured task scope, cancellation and backpressure | Library first; checker support | P0 | Disconnect, asset load and save lifecycles |
| Deterministic jobs, reductions and numerical profiles | Language + scheduler | P1 | NPC sensing, movement and combat batches |
| Data layout, copy visibility and portable SIMD | Compiler + library | P1 | Entity columns, animation and broad phase |
| Safe local content/code reload | Build/editor + engine | P1 | Quest and ability iteration |
| More complete target/debug/profile qualification | Compiler + hosts | P0/P1 | Same gameplay on Linux, mobile and browser |
| Native backend, broad reflection/macros, general atomics | Compiler | P2 | Only a measured consumer justifies them |

## 5. Storage and lifetimes: the first structural problem

### 5.1 Read-only views and exclusive write regions

Today a view descriptor is immutable as a parameter, but its elements are
mutable. Strings are read-only bytes; that does not supply a general read-only
view of an asset table or entity component. The memory chapter explicitly
requires copying an immutable fixed-array parameter before borrowing it. The
[Unicode normalizer notes](../lib/text/README.md) describe local table copies
needed because mutable views cannot borrow constants; one optimized native
build removes them, but that is not a portable language guarantee.

An MMO reads far more data than it changes: collision meshes, skeletons, item
definitions, navigation tiles and animation clips. I want a general immutable
borrow, a distinct exclusive writable borrow, explicit reborrowing, and
invalidation rules. Merely adding `const` spelling is insufficient: it must
compose through records, returned carriers, helper writes and foreign calls.

Start with synchronous regions and split contiguous slices. Reject overlapping
writable regions before enabling parallel execution. Keep overlapping ordinary
sequential views valid where their current contract permits them. Do not
retroactively call existing borrow provenance analysis a race detector.

Acceptance: an animation query borrows immutable clip storage without an
authored full-table copy; two disjoint output ranges can run concurrently;
overlapping writes and an escaping scratch borrow are refused with both sites
named. Unsupported relations must remain explicit, never become a false proof.

### 5.2 Explicit owners and reusable storage

Heap arrays, context allocators and language handles remain planned. Existing
hosts can allocate backing and hand Coax views; that is enough to run a game,
but it leaves ownership and growth coordination outside the language.
`coax/lib/sys/README.md` even describes a 1 GiB lazily committed tool stack
because buffers are local. That is a tooling accommodation, not a suitable
default for thousands of tasks or a client worker stack.

I would add a narrow region/arena owner before arbitrary pointer graphs:
fallible reserve, explicit byte and element capacity, stable allocation identity,
reset only after borrows end, and transfer of ownership at explicit boundaries.
Growth must state whether views relocate or are forbidden while borrowed.
Host C performs allocation effects; Coax decides capacity, reuse and refusal.
No hidden heap allocation in an ordinary collection operation.

Build reusable Coax libraries for bounded rings, queues, sparse sets, maps,
priority queues, bitsets, string builders and pools. Existing specialized stores
and `Bounded(T,N)` are starting points, not evidence of a universal container
API. Include collision/adversarial-input policy, stable iteration options,
full-capacity behavior, scratch requirements, invalidation and complexity in
every contract. A hash table's iteration must not accidentally define combat
order or a save's canonical representation.

Acceptance: load/unload cells through a bounded pool, hold stable references
across unrelated allocations, refuse exhausted capacity without partial state,
and detect stale references after reuse. Measure committed memory, stack peaks
and copy volume on the real region workload.

### 5.3 Resource ownership must cover asynchronous retention

The current returned-borrow and retained-store checks catch important dangling
views, including through supported sums and helpers. Their contracts explicitly
exclude general async retention, complete recursive fixed points and some
helper-store composition. Copying an owner-shaped struct does not make it a
linear owner. The book's typed System One ownership protocol is useful evidence
for a library discipline, not a general compiler guarantee.

Network sends, database replies, pathfinding requests and GPU completions all
outlive a call. Require an explicit retained owner/token with generation and
epoch, transfer or share semantics, and a defined release point. Borrowed
request bytes must either be copied into owned storage or proven live until
completion. Foreign declarations need audited retention/effect summaries.

Do not add general reference counting or tracing GC by default. Both can be
useful tools, but their pauses, atomic costs and authority implications require
a consumer. A bounded owner pool and cancellation scope fit the current design
better as the first extension.

## 6. Types and control flow for a large gameplay codebase

### 6.1 Finish composable fallible state

[The R2 contract](../results.md) forbids a Result transitively inside arrays,
views, records, unions, optionals or another Result. Generic unions are also
outside the profile. Direct Results deliberately preserve handling obligations;
this is a sound initial restriction, but it prevents a natural queue of pending
or completed fallible jobs. Authors otherwise translate errors back into manual
status fields and payload arrays, losing the benefit at the busiest boundaries.

Add a deliberately designed stored-outcome model: distinguish an unhandled
obligation from a persistent outcome already accepted into a queue, or extend
obligation tracking through storage with explicit extraction/disposal rules.
Specify replacement, overwrite, partial initialization, loops and cleanup
before relaxing containment. Generalize borrowed generic carriers only alongside
their lifetime rules; current generic record bindings reject views and
view-containing values. User constraints should first enable useful operations
such as hashing and key comparison, without opening arbitrary compile-time code.

Acceptance: a batch asset loader holds success/error outcomes for every request,
handles all of them on shutdown, cannot silently overwrite a pending failure,
and cannot retain bytes from a dead decode scratch region. Nested errors and
generic records must work under both evaluator and native checks.

### 6.2 Nominal IDs beyond physical quantities

The engine's handle and ID distinction is excellent, but its public helpers
return ordinary `int`. A million-line game will mix character IDs, entity IDs,
asset IDs, request IDs, owner generations, ticks and database revisions unless
the type system helps. Nominal physical quantities and framed spatial values
already establish part of the design vocabulary; physical f32/f64 quantities
are not a replacement for integer identity domains.

Add lightweight nominal integer wrappers with explicit construction, equality,
selected ordering and codec conversion. Put checked generation handles and
world/region namespaces in libraries over those types. Deserializing an integer
does not prove a handle is live or that the caller has permission to use it.
Persistent IDs need cross-region uniqueness; the entity store's session-local
counter is not a global ID service. Measure the linear `entity_find` scan and
add an indexed lookup where needed rather than assuming generic syntax fixes it.

Acceptance: passing a QuestId as an ItemId is a compile error; a stale handle
fails after reuse; a network ID resolves only in the admitted owner/world scope.

### 6.3 Critical integer behavior cannot depend on optimization

`emit_c.cpp` explicitly uses trapping add/subtract/multiply in debug and wrapping
forms in release; bounds, division, shifts and casts retain fatal checks. This
avoids C signed-overflow undefined behavior, but a character revision or item
count must not have different acceptance behavior when `--release` is selected.

Provide named checked, wrapping and saturating integer operations with stable
semantics, then an explicit critical-arithmetic profile or type discipline.
Before changing defaults, preserve existing profiles and migrate with diagnostics.
Economy code needs recoverable preflight failures and whole transitions rather
than a process trap after partial writes. Keep hashing's intentional wrapping
arithmetic separate from counters that must retire before wrap.

Acceptance: identical near-boundary item, revision and generation cases refuse
without mutation in debug/release/evaluator/native execution. Test the emitted
optimized artifact, not only the compile-time evaluator.

### 6.4 Ergonomics to add only after these foundations

Expression-oriented exhaustive matching, explicit error conversion helpers,
better generic diagnostics and useful user-defined constraints would remove
boilerplate. General operator overloading, implicit conversions, wildcard cases
and exceptions are lower priority. Exhaustiveness is especially valuable when
a new quest state or ability event must update all consumers. Avoid convenience
features that hide allocation, effects or a forgotten authority decision.

## 7. Parallel simulation and deterministic execution

[Chapter 14](../book/ch14-jobs.md) correctly explains the missing alias proof:
two named views can refer to overlapping storage. `job`, `pure` and capture
lists remain reserved/design. `src/jobs.cpp` concerns the machine-wide compiler
worker budget; its existence does not mean gameplay jobs have shipped.

For a dense region I want field/region read-write summaries propagated through
calls, disjoint chunk writes, bounded command buffers for structural changes,
and deterministic merge order. Use entity flush's existing ordering as the
starting contract. Jobs emit commands; they do not mutate membership underneath
another job's iteration. Foreign calls, allocation and retained views participate
in effects. The first implementation can deliberately exclude shared atomics.

Implement a single-thread reference scheduler first. Then use work stealing only
for independent chunks; command IDs, tie breaks and reductions must be stable
across completion order. NPC sensing, navigation tile work and animation are
good first consumers. A distributed shard is an owner/lease and persistence
problem, not a remote extension of a shared-memory job.

Determinism also needs an explicit numerical profile. Current source-order
evaluation and rounding rules are valuable, and Aethyr has fixed-step clocks
and replay; `strict`, `deterministic`, fixed-point types and a general portable
replay guarantee remain planned. Qualify FMA, denormals, NaN, transcendental
functions, float/integer quantization and reductions for each target. Pin PRNG
algorithm/state and separate per-system streams so an extra cosmetic draw does
not alter loot. Choose fixed-point or explicit quantization where it buys a
clear contract; do not force rendering and all physics into one representation.

The proposed replay artifact should bind simulation profile, compiler/build,
rules/content versions, initial snapshot, PRNG streams and admitted external
events. Report the first divergent component/field, not just a final hash.
`aethyr/replay.coax` uses a compact 32-bit mix for diagnostics; do not treat it
as collision-resistant evidence of save authenticity. Cryptographic identities
and debug fingerprints serve different purposes.

Acceptance: one recorded encounter matches at 1/2/4/8 workers and supported
optimization profiles; shuffled worker completion and equal-priority commands
cannot change the state. Cross-target bit equality is claimed only for a
qualified named profile. A server-authoritative MMO need not use global client
lockstep to benefit from reproducible server replays.

## 8. Schemas, content, persistence and authority

### 8.1 A shared authored model needs several explicit layouts

CPU reflection, declaration identities, layout reports and protobuf support
exist. `type_info` is not general procedural reflection, and `#derive`, typed
macros and compiler plugins remain planned. A large MMO needs definitions of
items, spells, quests, factions and components to drive validation, editing,
canonical saves and replication without maintaining each field list by hand.

Start with a restricted, checked schema tool written in Coax. Generate versioned
codecs, validators, inspector metadata, canonical hashing, field-level diffs,
and replication descriptors from explicit annotations. Prefer this constrained
workflow over an unrestricted macro system. All generated inputs and tool
versions belong in a manifest; malformed annotations must fail atomically.

Keep CPU ABI, GPU layout, wire encoding and durable format separate. Native
LP64 padding is not serialization. Sums' inactive bytes are unspecified.
Declaration IDs are not automatically stable content IDs, global entity IDs,
or published protocol field numbers. Assign durable identities explicitly and
retain migration rules for renamed or removed definitions. Published contracts
get a new version instead of being rewritten in place.

Acceptance: adding an ability field updates its intended editor/codec consumers;
an omitted required field is rejected; independent decoders agree on canonical
bytes; old saves remain readable through an explicit migration; an unknown
required version refuses rather than silently loading a different character.

### 8.2 Durable gameplay transactions belong above the compiler

The checkpoint and owner-generation invariants already solve important pieces
of crash recovery. They do not automatically make a trade, guild bank withdrawal,
auction result or cross-region item transfer atomic. A full economy needs an
operation ID, expected revisions, conserved item/value transitions, durable
result recording, retry semantics and an explicit commit point. A receipt must
mean committed state, not merely successful transport.

Implement these as Coax authorities plus storage effects, with server-derived
identity and capability checks. Model check duplicate, reordered, conflicting
and stale-owner requests; inject a crash at every durable transition. Avoid an
“exactly once network” claim: retryable operations can have one recorded effect
when the storage transaction and deduplication result are coupled.

Neither `#authority` nor a job's `write` effect grants authorization. The former
enforces check coverage; the latter would describe memory access. Actual money,
loot, disclosure and admission decisions still need authenticated inputs,
validated scope and runtime policy.

### 8.3 Untrusted content needs an execution boundary

First-party compiled content and player-authored scripts have different trust
requirements. Compile-time evaluation has budgets, but that does not establish
a runtime sandbox for arbitrary native content. A future player script profile
needs restricted capabilities, instruction/fuel limits, memory quotas,
deterministic host imports and a validated format. Prefer declarative quest
graphs or a small bounded VM until a real scripting consumer needs more. A
single malformed mod must not trap the authoritative server process.

## 9. Async work, networking and streaming

I would introduce a structured scope library before `async`/`await` syntax:
request identity, parent scope, deadline, cancellation reason, generation/epoch,
owned result and a bounded completion mailbox. Cancelling a scope must define
what happens to physical work, its output and its reservation. At minimum a late
completion cannot publish into a replacement owner. A syntax transformation is
useful only when the generated state machine obeys this contract.

The [shared host transport](../../aethyr/host/transport.coax) already models
concrete asynchronous owners and bounded slots. Generalize measured patterns
from it instead of inventing a second platform-wide task runtime. Define
overflow behavior per queue: coalescing is appropriate for superseded transforms,
but not for trade commits or entity lifecycle. Separate byte limits, queue age,
work per tick and admission priority. Recoverable exhaustion must be observable.

World streaming needs a cell lifecycle: requested, loading, validated, resident,
retiring and released. Navigation, rendering, audio and simulation may pin
different resources. A cell can unload only after those owners release it;
callbacks from its previous generation must be harmless. Content-addressed
immutable assets and bounded decode work belong in libraries and host effects,
not in a new language-level world-cell keyword.

WebSocket framing and interest policies already exist. Alternate datagram
transport, compression, baseline windows, adjacent-region authority and
cross-region handoff are protocol/engine decisions. Reuse the existing
[WebTransport evaluation](../../docs/verse/2026-11-05-aethyr-webtransport-evaluation.md)
before assuming a compiler feature can remove TCP head-of-line blocking.

Acceptance: disconnect while loading a cell, transferring a character and
awaiting a save; deliver every late callback after slot reuse; fill every
mailbox; prove no stale publish, double release, lost authoritative commit or
unbounded tick work. Exercise actual adapters as well as the pure policy.

## 10. Code generation, performance and target support

### 10.1 Make expensive values visible

Fixed arrays and structs are values. Copying and returning them is legitimate,
and C optimization can remove copies, but a large game should not discover a
megabyte temporary only through a stack overflow or a missed tick. Add optional
reports for large by-value parameters/returns, aggregate temporaries, maximum
individual stack frames, and hot call chains. Distinguish emitted potential
copies from measured physical copies. Hidden copy elision must not change
observable value semantics or turn a safe copy into an alias.

The existing layout chapter explains hand-authored SoA; automatic SoA storage,
hot/cold splitting and portable lane types remain planned. Use library column
stores first. Add typed gather/scatter, SIMD masks and vectorized math only
after profiling movement, skinning, collision or navigation. Benchmark cache
misses, memory bandwidth, allocations and p95/p99 tick cost, not just arithmetic
throughput on one friendly kernel.

### 10.2 GPU work needs resource graphs more than new shader syntax

The shipped shader emitters and reflection are substantial. The remaining MMO
pressure is reliable resource lifetime, upload/streaming budgets, compute-to-draw
dependencies, layout validation and reproducible specialization. Chapter 42's
device graphs are still planned; the shared renderer's live-handle ledger and
fence contracts are a concrete base. General graph support must prevent reuse
until actual completion and preserve ownership across device loss.

Do not prioritize automatic differentiation or logical tensor syntax solely
because they are on the simulation roadmap. They can serve Psionic/physics
consumers, but are not prerequisites for ordinary combat, quests or a renderer.

### 10.3 Treat each target as a contract

`native-lp64` remains the named layout-query target. Working Wasm applications,
quantities on native C, and supported shader subsets are different facts;
one does not qualify all forms on another backend. Establish a feature matrix
for Linux server, Apple clients and wasm32 covering sums, borrowed carriers,
integer widths, alignment, headers, math, source debugging, traps and resource
recovery. Refuse unsupported types before writing artifacts.

Extend actual ABI observations, independent codec oracles and replay fixtures
on each supported path. Start Linux debugger/profiler qualification from the
existing source-mapped native approach; do not infer it from Mac LLDB evidence.
Protect C as the reference backend. LLVM or another direct backend is P2 until
phase measurements show C compilation or its generated code is the bottleneck.
Moving the evaluator/emitter into Coax is useful maintenance work, but not a
functional requirement for an MMO launch.

## 11. The development loop is a product feature

The compiler remains single-shot for compilation. The repository already has
validated compiler-artifact reuse, bootstrap gate caches, tool caches, and a
checked-query cache. “No caching” would be wrong. What remains is reusable
semantic/compilation work across edits and controlled reload in a running world.

Instrument parse, instantiate, borrow/retain analysis, checks, emit, host C
compile, link, asset generation and startup separately. Keep source graph,
compiler/SDK, target, semantic profile, descriptors and generation inputs in
cache identities. Reuse a passing result only when its complete inputs match.
A warm query cache is not proof that an edited program compiled correctly.

The native checker already reports node storage, generic instance usage,
nesting, borrow work, queries and origins. Build on those reports: expose
machine-readable per-declaration costs, high-water marks, the exhausted resource,
and a practical reduction suggestion. Generic instance bounds and E0381
refusals are important protection, but a large generated content catalogue
must not require repeatedly raising global limits without understanding cost.
Borrow/retained analysis should remain conservative and bounded while gaining
better summary reuse and completeness reporting.

Provisional iteration goals, to be ratified on named hardware: a warm small
gameplay edit visible in a local realm within 2 s median/5 s p95; a content-only
edit within 1 s p95 when no schema changes; ordinary cold client/server build
within 2 minutes. These are targets for measurement, not observed current
failures. Record source size, machine, build profile and cache state with them.
Split any individual check command above the repository's 15-minute limit.

Hot reload should arrive in this order:

1. Immutable asset replacement under versioned handles, with old readers pinned.
2. Validated quest/ability data replacement at a tick boundary.
3. Procedure-body replacement for an unchanged ABI/effect/layout profile.
4. Explicit state migration into a private destination, with rollback on failure.

Client/server/content compatibility must be pinned; live rollout cannot mix
arbitrary definitions in an encounter. Checkpoints need the exact rules/content
identity needed to restore them. Replacing a procedure address alone does not
solve in-flight tasks, retained resources or a save made by old code.

For human and agent authors, extend current declaration queries and JSON errors
into editor navigation, safe rename, richer formatting and multi-location
ownership diagnostics. A bounded content inspector and replay field diff will
often save more debugging time than another language construct.

### Library work that should not become language syntax

An MMO also needs a wide reusable library surface. Audit and consolidate existing
subsystems before creating replacements; the following are expansion requirements,
not assertions that every primitive is absent:

| Subsystem | What I would require from its Coax interface |
| --- | --- |
| Navigation and NPC work | Incremental, cancellable searches with bounded expansions, stable tie breaks, cell/version ownership and explicit partial/no-route results; behavior trees or state machines can remain ordinary data. |
| Large-world coordinates | Typed local/global frames, checked cell transitions, floating-origin conversion and unit-aware transforms; current native affine types are a start, not a complete streamed-world transform graph. |
| Text and social UI | UTF-8 validation, grapheme iteration, cursor movement, localization/plural rules, text shaping/bidi integration, IME and accessibility effects; the existing NFKC normalizer is useful but is not a complete international text stack. |
| Assets | Deterministic manifests, dependency hashes, validation, packing, bounded decompression and resumable streaming; immutable asset IDs must survive source-file moves. |
| Time and services | Adopt chapter 30's existing clock-tagged instants and schedules where appropriate, and specify persisted expiry and service deadlines; neither a device clock nor a simulation pause should silently decide a trade expiry. Loop adoption and compiler-enforced clock-read effects remain separate work. |
| Diagnostics | Per-tick spans, queue ages, allocation/copy counters and replay event IDs with bounded overhead and explicit redaction of secrets; collection is an effect, interpretation is Coax. |

These libraries are also the best tests of proposed generics and ownership.
If a common navigation frontier, localized text table or asset cache needs
bespoke host-side product logic, use that concrete friction to choose the next
language extension. Do not put navmeshes, guilds or UI widgets into the grammar.

## 12. Verification needed beyond small pure checks

Keep check-first development and zero-survivor mutation for new authorities.
However, exhaustive finite `#forall` does not explore arbitrary distributed
schedules, real storage commit behavior, native memory use or target numerical
differences. Introduce reusable scenario testing and minimized event traces;
build them in Coax under the repository's tools rule. Independent oracles remain
independent, with a documented firm reason where a non-Coax harness is necessary.

| Scenario | Required observation |
| --- | --- |
| Entity/component churn and exhausted generations | No stale reference revival, deterministic structural flush, bounded memory |
| Duplicate trade, concurrent withdrawal, owner transfer | One durable recorded result, conservation, stale writer refusal |
| Crash before/after append, sync, commit and receipt | Replay yields an admitted whole state; lost response retries safely |
| Loss, reorder, duplication, long stall, reconnect | Correct baseline recovery; no old owner/socket input; bounded queue age |
| Cancel/unload/device loss with late completions | No stale publish, use-after-free, leaked reservation or duplicate release |
| 1/2/4/8 simulation workers | Matching canonical state and stable command/reduction order |
| Debug/release/evaluator/native and supported targets | Matching specified acceptance and numeric behavior; unsupported forms refuse |
| Large content and generated type graphs | Measured cold/warm latency, capacity headroom, comprehensible bounded refusal |
| Dense encounter and login surge | p99 tick, memory, bytes, queue age and refusal metrics stay within named budgets |

Use sanitizers on native ownership boundaries and original-source native checks
where supported; retain the evaluator baseline and evidence identity. Add
structure-aware fuzzing for codecs and untrusted content, model-based tests for
storage and state machines, and crash/fault tests for adapters. A passing mutation
score is not a substitute for these dimensions. Keep functional refusals,
inconclusive budget exhaustion and infrastructure failures distinct.

## 13. A delivery sequence with concrete exits

### Slice A: one durable adventurer, one streamed region

Build on current room/checkpoint contracts. Add nominal IDs, stable checked
critical arithmetic, immutable borrowed content, owner-backed bounded storage,
and a queue that can hold fallible completion state. Add an authored item/quest
schema tool and a single inventory transaction authority. Capture build-phase,
stack and memory baselines at the same time.

Exit: a player loads two cells, acquires an item, changes an ability definition,
disconnects and restores after injected crashes. Every old callback and stale
writer is refused; no duplicated item; capacity refusal leaves a complete
state. This exposes language needs before a general runtime is designed.

### Slice B: a reproducible crowded encounter

Add read/write region contracts and the reference job scheduler, then parallel
chunks and deterministic merging. Introduce the first qualified strict numeric
profile and component-level replay diagnostics. Optimize column access and
copies based on actual measurements.

Exit: the proposed 256-player/10,000-entity synthetic stress region meets its
named tick/memory/network budgets, or produces measurements that justify a
smaller supported profile. Replays agree across worker counts. Smaller shipped
rooms remain supported; the stress target is not a reason to overcomplicate them.

### Slice C: content production and deploy recovery

Add schema compatibility/migration tooling, warm compile reuse, staged local
reload, bounded task scopes, and coherent version manifests. Extend durable
gameplay operations to two players and ownership handoff. Test actual storage
and host adapters, not just state-transition helpers.

Exit: authors can iterate within the ratified latency budget; an incompatible
update is rejected; a mid-trade or mid-travel deploy recovers one owner and one
recorded outcome. Recovery objectives state the outage/capacity assumptions.

### Slice D: expand only at a measured limit

Scale region placement, shared services and cross-region semantics separately.
Add portable SIMD, more complete target support, direct backends or a resident
compiler only where the data supports them. Leave general atomics, arbitrary
compile-time plugins and unrestricted player scripting out until their safety
and maintenance costs have a specific consumer.

## 14. Book changes I would want alongside implementation

The current book is careful about status, and its registry separates source,
target qualification and adoption. Preserve that discipline. Its next useful
MMO route should connect those small contracts into a durable region:

* Chapter 6: immutable/mutable borrows, owner-backed allocations, invalidation,
  stack/copy reports, and an async request that survives the submitting frame.
* Chapters 5/7/8: stored fallible outcomes, borrowed generic carriers and the
  negative examples that explain their limits.
* Chapters 13–16: one shared component store, effect-checked jobs, deterministic
  merges, a qualified math profile and a replay that identifies its first drift.
* Chapters 18/20/21: explicit wire/save/GPU/CPU identities, version migration and
  observed Linux/Apple/wasm32 boundaries.
* Chapters 19/22/41/45/46: measured cold/warm edit loops, cache invalidation,
  capacity diagnostics and failed transactional reload with rollback.
* A new worked capstone: **a crash-safe two-player item exchange while a region
  unloads**. Show the durable commit, lost receipt, retry, owner replacement,
  cancellation and restore. Include a hand trace, native adapter observations,
  an independent model, deliberate refusals and fault injection.

An MMORPG need not wait for all planned robot, spacecraft, tensor or derivative
chapters. The most valuable language extension is the one that removes an unsafe
or repeatedly hand-written contract from an actual game subsystem, demonstrates
its cost, and keeps its limits visible to the next author.

## 15. Audit validation scope

This change adds analysis only. A matching checked compiler build was installed
with `coax/build.sh` from cache fingerprint prefix `dbc5292e433c`; no fresh
full compiler gate is claimed. Temporary independent source probes against that
compiler produced these observations on this Linux machine:

| Probe | Command/profile | Observed result |
| --- | --- | --- |
| `Box :: struct { value: Result(int, Failure); }`, with `Failure :: enum u8 { bad; }` | `coax check` | Exit 1, E0900: “Result cannot be stored in a record field”. |
| Direct `Result(int,Failure)` returned and consumed with exhaustive `switch` | `coax check` | Exit 0. Control case distinguishes supported Results from the containment restriction. |
| `Box :: struct($T) { value: $T; }` instantiated as `Box([]int)` over a live local backing array | `coax check` | Exit 1, E0802: fixed storage cannot bind a borrowed view. |
| Local `values: [..]int;` | `coax check` | Exit 1, E0301: dynamic arrays not supported. |
| `add :: (a:u8,b:u8)->u8 {return a+b;}` called with 255 and 1 | `coax run` / `coax run --release` | Debug exits 134 with an integer-overflow diagnostic; release exits 0 and prints `0`. |

The probes were temporary and did not alter compiler or repository source.
They confirm narrow implementation facts, not the proposed ownership or MMO
design. Local Markdown links were checked for existing destinations;
`git diff --check` passed. `coax/web/tool.sh tools-check` passed with 1,786
tracked tool files, including 35 vendored vector files, and zero new files
without a firm reason. No new gameplay, concurrency, durability or performance
qualification is asserted. The cited baseline identifies the inspected code;
later merges can advance implementation beyond this dated assessment.
