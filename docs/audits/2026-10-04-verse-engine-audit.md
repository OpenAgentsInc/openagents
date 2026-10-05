# Verse Engine audit for a AAA MMORPG

Date: October 4, 2026. Original source baseline:
[`e3d774841b39bca2a7a916ebe115e442bc7dffe2`](https://github.com/OpenAgentsInc/openagents/tree/e3d774841b39bca2a7a916ebe115e442bc7dffe2).
Scope: engine systems, Verse worlds, authoritative gameplay, multiplayer,
durability, content production, and desktop, mobile, and browser integration.
Upstream crowd-recovery evidence is refreshed through
[`148d2b6bcd`](https://github.com/OpenAgentsInc/openagents/commit/148d2b6bcd);
remediation status identifies subsequent issue work.

## Assessment

Verse has a credible foundation for a playable multiplayer vertical slice. It
has independently implemented combat, intent-only command admission, life and
control fencing, authenticated TLS transport, restart-safe character mutations,
validated assets, layered animation, continuous character queries, and a custom
GPU renderer. These are substantial implemented systems.

The audited code does not yet establish a AAA MMORPG engine. The largest gaps
are persistent service lifetime, movement quality under latency, replication
scale, unified world authority, content authoring, and operating a persistent
population. Rendering quality also needs measured budgets and a consistent
platform contract. The first priority is to make a durable multiplayer slice
reliable; adding visual features alone cannot establish MMORPG readiness.

The reward-history lifetime blocker (V01) is resolved in
[#10573](https://github.com/OpenAgentsInc/openagents/issues/10573). V02 remediation
in [#10574](https://github.com/OpenAgentsInc/openagents/issues/10574) adds ordered
background storage and bounded backpressure. V03 remediation in
[#10575](https://github.com/OpenAgentsInc/openagents/issues/10575) adds reviewed
offline migration, retained backups, and guarded rollback. V04 remediation in
[#10580](https://github.com/OpenAgentsInc/openagents/issues/10580) establishes a
bounded deterministic delayed-movement profile. V05 remediation in
[#10591](https://github.com/OpenAgentsInc/openagents/issues/10591) adds conservative
spatial relevance, acknowledged deltas, and bounded snapshot scheduling. V06
remediation in [#10593](https://github.com/OpenAgentsInc/openagents/issues/10593)
adds independent instances, exclusive leases, and atomic character transfer. V07
remediation in [#10596](https://github.com/OpenAgentsInc/openagents/issues/10596)
adds explicit hosted social profiles and a shared native projection. V08
remediation in [#10602](https://github.com/OpenAgentsInc/openagents/issues/10602)
adds transport admission partitions and shared request budgets. V09 remediation
in [#10603](https://github.com/OpenAgentsInc/openagents/issues/10603) adds stable
accounts, durable logout and selection, key replacement, and reward participation.
Two findings still deserve
immediate engineering attention:

1. A sustained 20-player/40-NPC battle fails performance acceptance, and a
   newer three-contact recovery failure stopped the whole host. Current code
   contains recoverable character/NPC movement failures; accepted crowded
   performance remains unproven.
2. Historical delayed battles retain failed correction and frame budgets,
   including a 6.5-meter outlier. V04 now passes a bounded deterministic profile;
   accepted crowded movement and hardware latency still require evidence.

The [engine roadmap](../verse/engine/roadmap.md) already names a battle with
about 20 authenticated players and 40 active NPCs. Treat that as the next
measured milestone. Neither a 64-player admission limit nor a video with two
players proves that workload. Realm population, concurrent nearby players,
instance density, minimum devices, and operating cost still need explicit
targets before planning broader MMORPG scale.

## Method and limits

This is a static architecture and code audit, supplemented by checked-in
measurement receipts. It follows execution paths across admission, simulation,
save, replication, presentation, and platform adapters, and inspects relevant
tests. It is comprehensive by subsystem, not a claim that every source line or
vendored upstream test received individual review.

The original audit includes no Rust code changes, new benchmark runs, GPU
captures, live host probes, penetration tests, or device runs. Remediation
updates below identify subsequent code changes and their targeted verification. Existing tests and
receipts are evidence of their recorded revision and scope, not fresh passes at
the source baseline. In particular, the delayed-network runs disable durable
storage, share one GPU between three clients, and delay TCP chunks rather than
simulate packet loss. Their CPU submission timings are not GPU execution times.

The review covers the following code surfaces. Counts include inline tests and
count Rust files under each crate's `src/`; they exclude examples and vendored
code. Counts describe scope, not coverage or quality.

| Surface | Rust files / lines | Reviewed responsibilities |
| --- | --- | --- |
| [`verse`](../../crates/verse/src/lib.rs) | 126 / 84,379 | Shared runtime, controllers, zones, renderer, shaders, assets, presence, chat, native chamber, and Agent Studio integration. |
| [`verse-engine`](../../crates/verse-engine/README.md) | 21 / 7,827 | Entity/life identities, clocks, packs, provenance, handles, presentation, animation graphs, sockets, lighting, render graph, audio, and quality tiers. |
| [`verse-world`](../../crates/verse-world/README.md) | 71 / 46,570 | Commands, combat, encounters, spells, movement, prediction, grants, TLS, client workers, replicas, character mutations, and recovery. |
| [`physics`](../../crates/physics/src/lib.rs) | 25 / 11,509 | Rigid bodies, contacts, warm starting, joints, CCD primitives, mesh queries, character movement, walkable navigation, lifetimes, and traces. |
| [`verse-lagrange`](../../crates/verse-lagrange/README.md) | 5 / 4,504 | Orbital mechanics, construction, fixed stepping, restorable zone state, and conservation fixtures. |
| [`verse-ruins`](../../crates/verse-ruins/README.md) | 3 / 1,354 | Adapter boundary and retained source/provenance; selected vendored collision, replication, and server schedule interfaces. |
| [`verse-wow` at the original baseline](https://github.com/OpenAgentsInc/openagents/blob/e3d774841b39bca2a7a916ebe115e442bc7dffe2/crates/verse-wow/src/lib.rs) | Compatibility adapter | Imported snapshots, numeric motion bindings, and separation from original content. |
| [`everglade-web`](../../crates/everglade-web/README.md) | 3 / 809 | Pinned pack fetching, local world mounting, input, and WebGPU/WebGL2 rendering. |
| [Mobile surface](../../crates/coder-mobile/src/verse_app.rs) and [OpenAgents wrapper](../../crates/openagents-mobile/src/verse.rs) | Integration review | Rust-owned state, injected identity, native surface lifecycle, and feature boundaries. |
| [Host example](../../crates/verse/examples/verse_host.rs), [remote client](../../crates/verse/src/imported/remote_window.rs), and [battle harness](../../scripts/bench/verse-battle-capture.py) | Execution-path review | Startup, content identity, configured rights, persistence selection, network workers, authenticated load, capture, and profiling. |
| [Assets](../../assets/verse) and [retained evidence](../../bench/verse) | Contract and receipt review | Original/retained content separation, manifests, character compilation, reloads, and measurement limitations. |

Other OpenAgents account, payment, host, and agent systems are integration
dependencies, not presumed implementations of MMO accounts, commerce, guilds,
or world services. Private reference repositories and Unreal source were not
read. Public primary documentation supports three specific recommendations:
replication interest, GPU timing, and texture color semantics; it supplies no
benchmark claim about Verse.

## What to preserve

- **Authority boundaries.** `verse-world` default builds are independent of
  rendering and network devices. Clients submit intents; the authority derives
  movement, resources, and damage. Spectators cannot submit player actions.
- **Lifetime correctness.** Instance, actor, life generation, control epoch,
  and command sequence distinguish ownership and respawn. Replicas, sockets,
  mounts, projectiles, and effects reject stale identities.
- **Durable acknowledgment.** The save path uses an exclusive writer, digested
  snapshots, file synchronization, atomic replacement, directory
  synchronization, and failure fencing. Character operations retain original
  receipts for exact retries. Preserve these guarantees when changing storage.
- **Content admission.** Pack loading bounds encoded and decoded data, verifies
  texture digests, checks references, and validates dependency/provenance
  closures before GPU allocation. Catalog generations fence reloads.
- **Animation and presentation.** Named motion states, local-space crossfades,
  graphs, markers, and mounts share final evaluated palettes. Rendering consumes
  immutable admitted frames instead of applying damage.
- **Physics evidence.** Mesh BVHs, capsule sweeps, moving supports, multilayer
  walkable cells, relative-motion projectile hits, contact warm starting, and
  conservation fixtures already exist. Do not replace these with a new solver
  merely because scale work remains.
- **Honest receipts.** Retained multiplayer runs report failed acceptance,
  capture drops, omitted observations, and measurement limits. Keep that
  distinction as the system improves.

## Priorities

P0 means a blocker before a persistent public world. P1 means required for a
convincing measured multiplayer slice or a safe production foundation. P2 means
required for broader AAA content, platforms, or MMO features after that slice.
These priorities describe the requested destination, not an assertion that an
existing production deployment is failing.

Evidence labels:

- **Code:** behavior follows from the inspected implementation.
- **Recorded:** a retained run demonstrates the result at its own revision.
- **Gap:** the reviewed execution paths do not implement the required capability.
- **Risk:** the implementation suggests a scaling or quality problem that still
  needs measurement.

| ID | Priority | Finding | Evidence | Owning boundary | Status |
| --- | --- | --- | --- | --- | --- |
| V01 | P0 | Reward history is archived without a transaction lifetime cap. | Code | Character storage and world service | Complete ([#10573](https://github.com/OpenAgentsInc/openagents/issues/10573)) |
| V02 | P0 | Full synchronous checkpoint commits occupy the tick loop. | Code, risk | World service persistence | Complete ([#10574](https://github.com/OpenAgentsInc/openagents/issues/10574)) |
| V03 | P0 | Content/rules changes lack a general durable migration path. | Code, gap | Content and save versions | Complete ([#10575](https://github.com/OpenAgentsInc/openagents/issues/10575)) |
| V04 | P1 | Confirmed movement intervals pass a bounded delayed profile. | Recorded, code | Movement and client replication | Complete ([#10580](https://github.com/OpenAgentsInc/openagents/issues/10580)) |
| V05 | P1 | Spatial replication bounds steady traffic in retained fixtures. | Recorded, code | World service replication | Complete ([#10591](https://github.com/OpenAgentsInc/openagents/issues/10591)) |
| V06 | P1 | Independent realm instances have durable placement, leases, lifecycle, and transfer. | Code | World hosting | Complete ([#10593](https://github.com/OpenAgentsInc/openagents/issues/10593)) |
| V07 | P1 | Presence and local zones do not share authoritative world state. | Code, gap | World rules and zone adapters | Complete: [#10596](https://github.com/OpenAgentsInc/openagents/issues/10596), bounded hosted profiles |
| V08 | P1 | Grant-based admission and transport work have bounded policies. | Code | World access and transport | Complete ([#10602](https://github.com/OpenAgentsInc/openagents/issues/10602)) |
| V09 | P1 | Persistent accounts own recoverable resident and dormant characters. | Code | Persistent character domain | Complete ([#10603](https://github.com/OpenAgentsInc/openagents/issues/10603)) |
| V10 | P1 | CPU submission measurements do not isolate GPU or input latency. | Code, recorded | Profiling and acceptance | Open |
| V11 | P1 | Renderer budgets and quality behavior differ by path. | Code, risk | Renderer and device capabilities | Open |
| V12 | P1 | Whole-pack preparation is not large-world asset streaming. | Code, gap | Content loading and residency | Open |
| V13 | P1 | Runtime mip generation ignores texture semantics. | Code | Content compiler and texture upload | Open |
| V14 | P1 | Spatial queries and rigid-body detection need scene-level scaling. | Code, risk | Shared physics | Open |
| V15 | P1 | Navigation needs tiled content and scheduled crowd work. | Code, gap | Navigation and AI | Open |
| V16 | P1 | Game rules and primary-player special cases limit reuse. | Code | World rules and ability adapters | Open |
| V17 | P1 | Engine boundaries remain intertwined with the Verse application. | Code | Engine extraction and host packaging | Open |
| V18 | P0 | Crowd recovery improves, but failure containment and scale acceptance remain. | Recorded, code | Movement failure handling and scale acceptance | Open |
| V19 | P1 | Persistent operations lack complete live diagnostics and recovery tooling. | Code, gap | World operations | Open |
| V20 | P2 | Content production still requires Rust implementation work. | Code, gap | Rust authoring tools | Open |
| V21 | P2 | Animation needs production locomotion and authoring support. | Code, gap | Animation and character content | Open |
| V22 | P2 | Lighting paths need a common visual and performance contract. | Code, risk | Rendering and art direction | Open |
| V23 | P2 | Audio is a bounded mixer, not a complete game audio system. | Code, gap | Audio and platform adapters | Open |
| V24 | P1 | Mobile/browser rendering does not establish authoritative game parity. | Code, gap | Platform world clients | Open |
| V25 | P2 | MMO social and progression systems need dedicated domains. | Code, gap | Verse game services | Open |
| V26 | P1 | Player-generated content needs publication and disclosure boundaries. | Code, gap | Content admission and product access | Open |
| V27 | P1 | Replay evidence needs explicit revision/platform guarantees. | Code, gap | Simulation and replay | Open |
| V28 | P1 | Status documentation trails the implementation. | Code | Runtime documentation | Open |

## Persistence, authority, and multiplayer

### V01: Reward history exhaustion is resolved

**Status:** Complete in
[#10573](https://github.com/OpenAgentsInc/openagents/issues/10573).
The original ledger refused new transactions after 4,096 receipts, and combat
reward failure could stop the serving loop. Hosted ledgers now retain at most
128 active receipts and bounded character state. Older receipts move into an
immutable, content-addressed
[`history` index](../../crates/verse-world/src/service/rewards/history.rs), with
bounded leaves and indexed lookup by actor and stable source. A batch clones
active state and its index root instead of lifetime history.

[`Store`](../../crates/verse-world/src/service/persistence.rs) commits that root
with the world and character state. Archive files are published and synchronized
before a checkpoint can reference them; abandoned batch or uncommitted writes
remain outside the committed root. Exact retries return original transactions
and revisions, and conflicting reuse cannot change balances. Network clients
accept revisions beyond 4,096 without weakening ownership or count validation.

Version-eight saves store character summaries, recent receipts, and the root
instead of copying every historical transaction into the checkpoint. Versions
one through seven remain recoverable and are archived on their next store
commit. Recovery validates referenced archive nodes and refuses missing or
corrupt history. Backups must retain `state_dir/rewards` with `chamber.json`
and, after V02, `journal.jsonl`.
Nondurable network hosts use an owned temporary history directory; standalone
chambers retain in-memory history until attached to a store or network host.

**Acceptance evidence:** Targeted `verse-world` tests cover 5,002 mixed reward,
quest acceptance/claim, inventory debit, equipment, and outfit operations;
bounded active receipts and a checkpoint under 64 KiB; old retries after
recovery; conflicting sources; failed cooperative batches; failed archive
writes; and persistent recovery above 4,096 operations that ignores later
uncommitted index writes. Missing committed history prevents startup.

**Remaining limits:** Disk history grows with mutations and can retain
unreferenced nodes. V02 moves durable archive publication to the writer; cold
indexed reads and nondurable temporary-history writes remain synchronous.
Maintenance and verified backup/restore tooling belong to V19. These tests establish transaction lifetime and bounded hosted
memory, not battle-scale performance.

### V02: Ordered storage runs outside the simulation loop

**Status:** Complete in [#10574](https://github.com/OpenAgentsInc/openagents/issues/10574).

The original host serialized, hashed, wrote, and synchronized a complete
checkpoint inside each 30 Hz authority tick. Remediation replaces that path with
an owned persistence copy and a dedicated
[storage writer](../../crates/verse-world/src/service/persistence/writer.rs).
The authority retains command dispatch; the copy contains no connection
challenges or dispatch interface. Encoding, structural diffing, hashing,
reward-history publication, and filesystem synchronization run on the writer.
Later world mutations cannot change a submitted copy.

The queue admits one active and one waiting copy, with up to 128 pending replies
in a batch and a bounded reward-history staging cache. Mutation and read replies
wait for ordered successful commits when state is new; reads share a pending
commit or use already committed state when no newer mutation is pending.
Sequential client helpers bound retries of explicit pre-admission storage
refusals, preserving operation fields; raw and pipelined requests expose the
refusal. Transport errors do not trigger uncertain command replay.
A failure stops the host and withholds its
uncommitted replies. When storage fills capacity, the host explicitly pauses
simulation and returns `storage_busy` using the last committed tick and control.
It records paused wall time separately and does not simulate it later. Shutdown
drains admitted copies and commits parked controls before releasing the lock.

A [bounded journal](../../crates/verse-world/src/service/persistence/journal.rs)
records revision, parent-state and resulting-state digests, structural changes,
and its own checksum. Atomic snapshots compact it after 256 records or 64 MiB.
Recovery validates and replays complete records, discards an unterminated final
append, and refuses complete corruption, broken chains, or missing history.
Compaction recovery accepts a validated prefix already covered by the snapshot.
Backups require the snapshot, journal, and rewards directory together.

**Acceptance evidence:** The
[ordered durability receipt](../../bench/verse/2026-10-04/ordered-durability/run.json)
retains local TLS cadence and growing-state simulation/capture/commit p50/p95/p99
measurements, a controlled writer stall, and recovery checks. A subprocess exits
without unwinding at nine storage boundaries, including journal synchronization,
snapshot staging/synchronization/rename, and journal truncation. Previously
acknowledged rewards retain their original receipt after each restart; uncertain
operations either recover whole or apply once under their original identity.
Other checks cover isolated persistence copies, compaction, incomplete appends,
complete corruption, duplicate revisions, signed-zero representation, queue
bounds, explicit refusals, withheld replies, and orderly writer drain.

**Remaining limits:** This is local fixture evidence, not a durable 20/40 battle
or a physical power-loss test. Full checkpoint encoding still runs on the writer,
and bounded copies and read-response construction consume authority CPU. Runtime
percentiles are fixed histogram upper bounds over all lifetime observations.
Cold historical receipt reads still perform bounded synchronous filesystem work;
nondurable hosts still write temporary history directly. Battle-scale CPU and
storage budgets and live operational visibility remain V18 and V19 work.

### V03: Ordinary content updates can make saves incompatible

[`save::decode`](../../crates/verse-world/src/service/save.rs) accepts several
save versions but requires the exact content digest. [`Game::restore`](../../crates/verse-world/src/play.rs)
admits the current rules revision and a narrow legacy revision.
[`Config::validate_recovered`](../../crates/verse-world/src/service/host.rs)
requires saved enrollment, catalogs, policies, and scene context to match
startup configuration. These checks correctly refuse accidental relabeling;
they also mean a routine content, catalog, or enrollment change needs more than
replacing files and restarting.

**Improve:** Version rules, character schemas, and world content separately.
Implement explicit offline migrations with source/target digests, backups,
validation, and rollback. Distinguish an instance's content pin from persistent
character data that must survive a patch. Add enrollment changes through an
audited operation rather than silently weakening recovery validation.

**Acceptance:** Upgrade a populated save across a real scene, item, quest, and
rules revision. Preserve character ownership, XP, equipment, receipt identities,
and completed objectives. A failed migration leaves the original save usable.

**Remediation:** [#10575](https://github.com/OpenAgentsInc/openagents/issues/10575)
adds [`persistence::migration`](../../crates/verse-world/src/service/persistence/migration.rs)
and the [`verse_migrate` command](../../crates/verse/examples/verse_migrate.rs).
A reviewed plan pins source commit/state, source and target content/rules/schema
identities, both configs, candidate state, and enrollment changes. Save version
nine adds character schema two and retained ownership independent of active
grants. Legacy versions remain explicit compatibility paths; arbitrary future
rules or schema revisions require implemented adapters.

Apply retains character IDs, XP, item balances, equipment/outfits, accepted
quest baselines, completed quests, and the immutable original receipt index.
It explicitly restarts world dynamics and encounters, respawns players, refills
target equipment limits, and advances player/NPC/prop fences. Catalog IDs and
slots remain stable. Unsupported changes to active quest semantics are refused.
Revoked owners retain their characters; re-enrollment preserves the actor and
can change spawn explicitly. Implicit ownership transfer is refused. Normal
startup continues to require the configured content, enrollment, and catalogs.

Synced before/after snapshots and a digested operation record live under
`migrations/`. The writer lock covers the entire operation. A pending marker
precedes active replacement; until the operation's target seal is synced,
interrupted apply restores its complete source, including acknowledged journal
changes. A sealed operation recovers its target. Rollback uses the same protocol,
retains its own record, increments the commit revision, and refuses any later
commit. Storage failure withholds success; recovery can resume when storage is
available. Backups retain receipt-history roots and require their shared files.

The [migration receipt](../../bench/verse/2026-10-04/content-migration/run.json)
records a populated version-eight/rules-v18 fixture upgraded across changed
scene actors, items, equipment, outfit, quest reward/goal, and rules. Regressions
cover exact archived grant/claim/use/equip retries, retained ownership and
re-enrollment, input drift, active-objective refusal, storage failure, later-commit
rollback refusal, deleted/reintroduced NPC and prop generations, seven apply
crash boundaries, and four rollback boundaries. These are local scratch-store
checks, not a power-loss or production rollout experiment. Instance-scoped
character capacity and global identity remain V09; verified operational backups
and retention remain V19. The asset-loading command still depends on the
renderer crate until V17 separates that boundary.

### V04: Confirmed intervals establish bounded delayed movement acceptance

Prediction is implemented in [`prediction`](../../crates/verse-world/src/prediction.rs)
and [`prediction::Local`](../../crates/verse-world/src/prediction/local.rs), with
shared collision, life/epoch fencing, applied-input baselines, coalesced input
history, and bounded replay. The [`worker`](../../crates/verse-world/src/service/worker.rs)
uses the duplex client pipeline; describing the current client as wholly
sequential or as having no prediction is incorrect.

The [timed delayed run](../../bench/verse/2026-10-04/prediction-delayed-timed/run.json)
at `93095163aee8e15031467c64ef41dc7e956aca4a` reports ordinary correction p95
of 0.853 m and 0.941 m. The code now shifts retained local input timing when
authority overtakes it. The newer
[rebased delayed receipt](../../bench/verse/2026-10-04/prediction-delayed-rebased/run.json),
compiled at `af701a4151c815b6e82d434616cb4cd1adfb9f69`, records improvement to
0.427 m and 0.302 m, with capture drops of 5/7/5 across the three clients.
It still reports failed acceptance: a 6.5 m correction outlier, frame-interval
p95 around 28–30 ms on the shared GPU, and no life-change coverage. Those
measurements demonstrate improvement, not accepted movement or isolated-client
rendering performance. A subsequent
[clock-alignment fix](https://github.com/OpenAgentsInc/openagents/commit/533d3b0cde)
prevents repeated late baselines from advancing the prediction clock or renewing
expired input holds. Its focused regressions pass; the later three-contact
battle stops before it can establish sustained prediction acceptance.

**Improve:** Specify exactly when a movement input becomes effective, which
physics steps an acknowledgment covers, and how coalesced inputs retain their
intervals. Separate camera smoothing from collision-correct predicted state.
Measure corrections caused by rejection, clock alignment, supports, remote
blockers, teleports, and life changes independently.

**Acceptance:** Repeated delayed runs on current code cover starts/stops,
diagonals, jumps, stairs, moving supports, collisions, cast interruptions,
death/respawn, reconnect, and teleports. Publish ordinary correction distributions
and input-to-display latency separately; intentional discontinuities never
count as prediction failures or hide them.

**Remediation:** [#10580](https://github.com/OpenAgentsInc/openagents/issues/10580)
adds [complete movement intervals](../../crates/verse-world/src/movement/frames.rs),
[authority admission](../../crates/verse-world/src/play/framed_movement.rs), and
[prediction binding](../../crates/verse-world/src/prediction/local_frames.rs).
Wire version 22 separates admitted command sequence from confirmed character
physics time and the world clock. Native clients send all elapsed intervals,
including held-input expiry and jump edges. The authority bounds packet size,
queued work, catch-up work, and time credit; it never rewinds combat or receipts.
Entry returns an initial snapshot without another network round trip; a
48-step startup allowance and a 32-step active lag bound cover that delivery.
The worker preserves the verified receipt age through pipeline handoff. The
serial SDK refreshes an explicitly stale command only when the returned control
proves no sequence was consumed, under a three-envelope/ten-second bound;
future ticks, gameplay refusals, changed controls, and uncertain IO are excluded.
Idle expiry, reconnect, handoff, respawn, and teleport clear the context. Current
collision and spell modifiers remain authoritative. Updated CLI/headless
clients keep their arrival-time profile; version-21 peers must upgrade.

The [deterministic receipt](../../bench/verse/2026-10-04/movement-intervals/run.json)
compares the actual authority and local motor under ordered input/acknowledgment
and snapshot delays. Arrival-time movement reproduces 0.213–0.427 m correction
p95 at 67–167 ms nominal RTT. Confirmed intervals produce 0 m p95 in the static
profiles, including delayed initial entry, bounded jitter, combined input,
diagonals/jumps, wall contact, and authored stairs. Each framed run contains 121 correction observations and
no command refusals. A separately measured translating support gives 0.026 m
p95 and 0.027 m maximum. Both are below the declared 0.10 m deterministic-profile
budget. Snapshot omissions are counted; unsupported cases are not discarded
into a passing percentile.

Primary/secondary lifecycle tests cover stale input, reconnect, death/respawn,
teleport, cast interruption, queued save restoration, invalid admission without
sequence consumption, and idle gravity recovery. TLS worker and native adapter
checks cover ordered binding, admission versus completed physics, and actual
native interval production. A local TCP proxy adds real 67 ms uplink and 100 ms
downlink delay to the TLS worker; its sustained interval stream starts without
a mode reset and records a separate correction distribution. Native recording retains reset reason counts and
separate discontinuity distances, including changes of life or epoch.

This completes the bounded movement-timing remediation, not MMORPG performance
acceptance. The retained historical failures remain failures. Network
loss/retransmission behavior, complex moving geometry and spell transitions,
crowded movement, isolated rendering, and input-to-display latency need their
own evidence. V10 owns hardware latency evidence, V14/V18 own collision cost and
crowded acceptance, and V24 owns mobile/web multiplayer integration. The recent
upstream containment changes are included in these checks; they do not turn a
failed battle performance receipt into a pass.

### V05: Spatial replication bounds steady traffic in retained fixtures

**Status:** Complete in [#10591](https://github.com/OpenAgentsInc/openagents/issues/10591).

**Original finding:** Native polling transferred complete snapshots every 50 ms,
including full collision descriptions and distant presentation. Client-side
collision compilation reuse reduced CPU work but did not reduce wire bytes.
The 16 KiB request and 2 MiB response bounds did not establish a steady traffic
budget. This was a code-level scaling gap.

**Remediation:** [#10591](https://github.com/OpenAgentsInc/openagents/issues/10591)
adds independent Rust relevance and delta contracts in
[`service::replication`](../../crates/verse-world/src/service/replication.rs).
Wire 23, the SDK, and the native worker use acknowledged replication requests.
The authority chooses a 64-meter presentation radius over reusable 32-meter cells;
collision uses an 80-meter radius with conservative bounds for rotated and large
geometry. Relevant projectile and telegraph endpoints remain life-bound. Owned
HUD, resources, and cooldowns stay private; spectator snapshots carry an empty
private projection. Near poses and owned collision follow the requested cadence.
Outer-band transforms update after six authority ticks; lifecycle, health,
equipment, and teleport changes bypass that hold. Unchanged fields are dormant
in the delta stream.

Two connection-scoped baselines, each bounded to 512 KiB, bind revision, authority
tick, and SHA-256 digest. The receiver acknowledges a state only after bounded
patch reconstruction, digest checking, and complete state admission. Unknown or
expired acknowledgements, control changes, and explicit client cache discard
produce a full baseline. Oversized deltas fall back to full packets; oversized
baselines are refused. Diagnostic reads and movement-mode entry retain full
snapshots. Signed zero has one canonical digest representation. Replica relevance
exit preserves generation/death fences and allows the same life to reenter.

The SDK bounds replaceable snapshots to one request in flight. The worker schedules
events and inventory separately, retains reliable command and cursor ordering,
and skips missed snapshot intervals. Shared public extraction and cell lists are
cached until a mutation or tick; baseline memory is released on disconnect,
supersession, and revocation. Host metrics retain packet counts, bytes, encoding
cost, resyncs, and acknowledgement age after disconnect.

**Acceptance evidence:** The retained
[replication receipt](../../bench/verse/2026-10-04/spatial-replication/run.json)
compares 0, 32, and 128 additional distant actors. Each fixture retains 15 relevant
actors and transfers the same 30,286 bytes across 59 delta samples, following a
22,745-byte full packet. A scratch
TLS proxy adds 67 ms upstream and 100 ms downstream delay. It verifies normal
deltas, unknown-ack full resync, application cache-discard resync, bounded proxy
queues, owned state admission, and baseline cleanup. Focused tests also cover
relevance entry/exit/reentry, skipped-tick frequency updates, conservative support
bounds, oversized geometry, malformed deltas, owner privacy, cache invalidation,
and independent reliable reads while a snapshot is pending. The final checks pass
424 world tests (two crash child hooks are intentionally ignored), 12 replication
tests, 14 native remote tests, portable compilation, and formatting. The TLS
receipt records nine deltas, three full packets, a six-tick maximum acknowledgement
age, one snapshot in flight, and zero retained baseline bytes after shutdown.

**Limits:** These fixtures establish bounded per-viewer steady transfer for the
declared populations. Per-viewer source cloning/filtering still scales with
bounded source lists; conservative large meshes can remain relevant to many
viewers. Existing instance/entity and 512-entry replica history bounds remain.
This is acknowledged polling, and TCP head-of-line blocking remains. Encoding
cost, real WAN loss, crowded movement, and hardware latency are not claims of
accepted AAA MMORPG capacity; V06, V10, V12, and V18 retain that work. The original
[replication graph reference](https://dev.epicgames.com/documentation/en-us/unreal-engine/replication-graph-in-unreal-engine)
remains background for spatial list reuse; the implementation shares no Unreal
code.

### V06: Independent realm lifecycle and transfer are implemented

Resolved for the independent-instance profile in
[#10593](https://github.com/OpenAgentsInc/openagents/issues/10593).
[`Realm`](../../crates/verse-world/src/service/realm.rs) owns actual `Gateway` and
`Game` instances behind an exclusive coordinator lock. One sealed manifest
records content-bound immutable world checkpoints, stable realm character IDs,
placement, capacity, phase, endpoint, and authority epochs. Host APIs create,
admit, drain, stop, restart, and lease instances. Every tick, connection, and
dispatch checks the explicit 30-second lease; acquisition and recovery park old
sessions. Recovery advances epochs and requires fresh acquisition. Host time
cannot regress. V06 initially capped 32 instances and 2,048 registered characters,
with a 64-player simulation limit; V09 moves dormant characters out of that table.

[`Transfer`](../../crates/verse-world/src/service/realm/transfer.rs) prepares and
validates both game copies before publishing. One atomic manifest rename selects
both checkpoints, the destination placement, and an immutable transfer retry
root. Before that seal, recovery chooses the source; after it, recovery chooses
the destination. Uncertain durability poisons the coordinator and withholds the
result until recovery. A bounded radix index retains original transfer outcomes
without a fixed operation lifetime cap. Changed retry arguments are refused.

V06 transfers retained health, mana, inventory, equipment, progression, and
remaining cooldowns across different world clocks for living additional
adventurers. Temporary world effects and input stopped, and both affected
instances required fresh authentication. V09 also supports primary characters
and preserves unrelated sessions. Compatible item, outfit, equipment, and progression catalogs are
required, and destination capacity and collision admission run before publication.
[`Receipt books`](../../crates/verse-world/src/service/rewards/books.rs) bind
history to the realm character rather than its local actor. Original item,
outfit, equipment, quest, and reward outcomes stay intact after repeated transfers;
exact retries return the original revision without another debit or restoration.
The legacy receipt root remains immutable and new per-character roots grow
independently. Save version 10, character schema 3, records these namespaces;
versions 1–9 retain their own schema admission rules.

[`Realm TLS hosting`](../../crates/verse-world/src/service/realm/net.rs) uses the
existing framed TLS transport and SDK across separate instance listeners. A
local operator handle routes, admits, drains, and transfers characters; clients
cannot request those operator actions through the wire. The coordinator thread
owns all mutable games and disk work, while socket tasks retain bounded queues
and existing protocol deadlines. Socket and dispatch limits are 128; timer work
coalesces and skipped elapsed time is recorded.

**Acceptance:** The [retained receipt](../../bench/verse/2026-10-04/realm-transfer/run.json)
covers two actual TLS instances, SDK reauthentication at the routed destination,
exclusive coordinator refusal, stale lease refusal, draining, restart, capacity,
incompatible catalogs, and repeated placement. Five subprocess termination
boundaries recover exactly one character owner and three remaining items. Both
old and new receipt namespaces preserve exact item-use retries across transfer,
return transfer, and restart. Twenty transfers exercise branching history nodes.

**Limits:** This is one coordinator over independent instances, with no claim of
distributed consensus, seamless cross-instance simulation, or accepted population
scale. The TLS adapter serializes checkpoint work and has not established the proposed
crowded 30 Hz throughput target; V08, V10, and V18 retain load and containment
acceptance. V06 initially refused primary-avatar transfer and reconnected both
worlds on transfer or admission. V09 removes those restrictions. Foreign
reward-history imports, online content migration for realm manifests, archive
garbage collection, and retention policy remain operator work under V19. The
receipt covers scratch loopback and process death; it does not test disk power
loss, remote host failover, WAN delivery, GPU rendering, or mobile transfer UI.

### V07: Local worlds and presence use different authority models

Original finding at the audit baseline:

[`WorldRuntime`](../../crates/verse/src/runtime.rs) and
[`zones::runtime`](../../crates/verse/src/zones/runtime.rs) own local plaza,
Everglade, Lagrange, Lab, and Ruins behavior. [`session`](../../crates/verse/src/session.rs)
shares publisher-authored presence through NIP-MV. Signatures validate publishers,
not legitimate movement. The presence budget is 54 events per minute despite a
desktop moving interval of 100 ms; that interval does not guarantee 10 Hz
delivery. Lagrange's restorable physics state is not a hosted zone service.

Everglade uses its own height/footprint/roof collision representation in
[`solids`](../../crates/verse/src/zones/everglade/solids.rs), while the chamber
uses shared capsule/mesh queries. Agent Studio seat motion is client presentation.
These paths cannot be assumed to agree across viewers or transitions.

**Improve:** Introduce a social world profile without mandatory combat actors,
with authority-owned movement, interaction, and seats. Adapt each hosted zone to
one command/snapshot lifecycle. Keep relay presence as discovery and ambient
presence, with hosted instance snapshots authoritative during play. Follow the
existing [networking convergence plan](../verse/networking.md).

**Acceptance:** Two viewers agree on actor positions, interactions, and zone
entry/exit. Late presence events cannot overwrite authority poses. Transitions
fence old commands and preserve character identity without transferring unrelated
host or studio permissions.

#### V07: Hosted social authority is implemented

[#10596](https://github.com/OpenAgentsInc/openagents/issues/10596) adds
[`play::social`](../../crates/verse-world/src/play/social.rs) to the existing
owned game, authenticated gateway, scoped replication, checkpoints, and realm
transfer. Profile revision 1 explicitly admits Plaza and Everglade social
variants with static boxes or terrain triangles and bounded interaction objects.
These worlds require no hostile actor or combat encounter. Client combat is
refused; movement uses the same authoritative capsule motor and collision
geometry that native presentation receives.

Seats have exclusive life-bound occupancy; switches have authoritative state.
Interactions require the session's character, current control epoch, fresh tick,
monotonic sequence, and proximity. Accepted interactions fence and stop queued
movement. Invalid interactions leave the world unchanged. Movement, disconnect,
revocation, respawn, and transfer release seat claims. A trusted host publishes
only typed public seat poses through the lease-checked realm control channel.
There is no wire operation for publishing those poses or accessing Studio tasks.

[`host::Config`](../../crates/verse-world/src/service/host.rs) accepts an optional
`social_profile`; host preparation and the CLI, host example, and offline
migration example bind its digest into content identity. Recovery refuses a
changed profile. Checkpoint rules advance to v21 and wire messages to version
24; earlier supported combat checkpoints remain readable, and older rule labels
cannot carry social state.

[`verse::hosted`](../../crates/verse/src/hosted.rs) connects the authenticated SDK
and validated replica to `WorldRuntime`. Explicit destination admission pins
instance and profile digest before replacement. The runtime draws authoritative
actors, public seat poses, occupancy, and switches; it disables local placement,
movement, navigation, zone installation, and Studio operations while attached.
`Session::tick_world` keeps relay discovery metadata while suppressing its
spatial crowd projection in hosted play. Local presence cannot replace the
admitted poses. A destination rejects the old source replica, and an explicit
exit drops the hosted projection before restoring the local runtime mode. A local
attachment revision fences retired clients before they send requests, including
replacement by another connection to the same instance.

The [social authority receipt](../../bench/verse/2026-10-04/social-authority/run.json)
retains focused TLS convergence/transfer and framed SDK/native viewer evidence,
source digests, and verification results. The TLS fixture covers two profiles,
a player and spectator, shared seat/switch/Studio state, spectator refusal,
reconnection after transfer, an unchanged realm character ID, old command
refusal, and durable recovery. Native viewers produce equal meshes and refuse
local movement, placement, transitions, Studio operations, and old destination
updates. Presence tests retain discovery without allowing late or replayed
publisher poses into hosted geometry. Final checks pass 446 world tests (two
crash child hooks are intentionally ignored), 511 Verse unit tests (11 ignored),
seven Verse integration tests, two final native authority/presence tests, three
CLI chamber tests, and the CLI REACH grant test. Host and migration examples
compile. The broader Verse check requested regeneration of the checked-in
Everglade pack; the previous content-addressed download remains available.

**Limits:** This is an explicit opt-in SDK and shared runtime profile. Existing
local plaza and imported Everglade artwork and rules are retained; the hosted
v1 renderer uses neutral geometry and generic figures. It does not automatically
convert those local asset packs or provide a desktop/mobile join screen. Ruins,
Lagrange, and Lab remain local-only and are refused as hosted social profiles.
Profiles permit at most 64 static shapes, 512 terrain triangles, 64 interaction
objects, and 64 public Studio poses; host JSON is also bounded to 64 KiB.
The V07 receipt predates V09's primary-character decoupling and preservation of
unrelated sessions; the concurrent resident limit remains 64. Hosts must already own or separately obtain observation
authority before producing public Studio poses; this change grants none.
The fixtures establish functional agreement, not crowded throughput, WAN latency,
GPU quality, or a browser/phone multiplayer acceptance result.

### V08: Admission and request work have bounded policies

**Status:** Complete in [#10602](https://github.com/OpenAgentsInc/openagents/issues/10602).

**Original finding:** The inspected host enrolled a static key list. Pending
sockets shared admitted-player capacity, request limits applied only per socket,
and worker outcomes disappeared. There was no principal/IP admission policy or
aggregate work budget. This was an overload risk, not a demonstrated exploit.

**Remediation:** Upstream already connects the real binaries to NIP-HOST `world`
grants, NIP-REACH channels, and owner-authorized world discovery. Grants bind a
device, host generation, and revocation epoch. The transport checks them at the
handshake, before every request, and while idle. A granted key outside the host's
role table joins as a spectator. Chamber challenges remain signed, single-use,
expiring, connection-bound, and optionally content-bound. World admission grants
no Studio observation or agent execution right.

[`net::admission`](../../crates/verse-world/src/service/net/admission.rs) now
limits total transport slots to 128, with at most 32 pending authentication and
eight pending per IP; IPv4 and mapped IPv6 addresses share that budget. A shared handshake bucket permits a 64-connection burst
and refills at 128 per second. Pending IP records disappear on promotion or
closure. Thirty seconds is an absolute chamber authentication deadline; a
`storage_busy` retry does not restart it. TLS handshakes retain their five-second
deadline. Supersession wakes and closes the previous transport, including an idle
one, and releases its capacity.

Verified principals share buckets across reconnects. Commands cost one token,
scoped replication eight, full snapshots 32, inventory eight, and events four.
A principal has independent command and projection buckets: capacities/refills
are 120/120 and 512/512 tokens per second. Aggregate buckets have capacities of
256 commands and 2,048 projection tokens, refilling at 16,384 and 65,536 per second.
Projection overload leaves command capacity available. At most 128 principal
records retain credit for 120 seconds after use; active records remain pinned.
Limits refuse new records when all records remain retained. Refused work never
reaches authority dispatch and consumes no game operation or command sequence.
The SDK receives `rate_limited` with its last acknowledged authority header.

The existing bounded FIFO dispatch retains one request in flight per connection;
principal supersession prevents one identity from multiplying current sockets.
Standalone TLS/REACH and all realm listeners share the same admission policy,
with realm limits shared across instances. Shutdown drains child tasks before
capturing counters. Aggregate statistics classify handshake, authentication,
grant, frame, rate, IO, supersession, and worker cancellation outcomes without
retaining principal keys or IP addresses. The CLI's stopped record and the host
example expose those counters.

**Acceptance evidence:** The
[admission receipt](../../bench/verse/2026-10-04/admission/run.json) retains source
identity, checks, and scratch TLS evidence. Stalled unauthenticated connections
and a snapshot flood leave accepted player commands live. Deterministic checks
cover pending partitions, aggregate projection overload, command independence,
and reconnect without credit refill. Transport checks cover the actual slow TLS
deadline, absolute authentication expiry during a storage retry, immediate
supersession, cleanup, and durable realm transfer. Grant fixtures refuse absent,
narrow, expired, revoked, and stale-epoch grants; existing authentication tests
refuse challenge replay and expiry. Native polling and quest flows pass under
the declared budgets. After upstream integration, 468 world tests pass (two
crash hooks are intentionally ignored). Four final admission tests, two real TLS
realm tests, four native hosted tests, three CLI chamber tests, and the CLI REACH
grant test pass. Host/migration examples compile, formatting passes, and document
links resolve. The receipt preserves the earlier failed budget check and its
correction; the final focused checks cover IPv4 mapping and adapter cleanup
after the broad world suite.

**Limits:** These are fixed process budgets and bounded functional acceptance.
Token prices bound request counts rather than measured CPU cost; a single
accepted request, world tick, or durable realm commit can still exceed a frame
budget. Hardware capacity and hostile multi-principal load remain acceptance
work under V10 and V18. Per-IP pending limits can refuse concurrent joins from a
shared network; new admissions have no progress guarantee during a distributed
connection flood. Principal retention and the existing role-table/character
bounds are explicit; V09 owns persistent character capacity. Idle encrypted
channel closure contributes to transport closure counters when the bridge does
not expose a more specific reason. Reliable operations require explicit caller
backoff after `rate_limited`; the SDK does not replay uncertain effects.

### V09: Persistent accounts and characters have a recovery contract

**Status:** Complete in [#10603](https://github.com/OpenAgentsInc/openagents/issues/10603).

The original finding identified chamber-scoped ownership and undefined MMORPG
participation. V06 supplied stable character receipt books and atomic placement
transfer. V09 adds independent accounts, explicit resident retirement, dormant
storage, key recovery, and authored reward participation.

[`realm::registry`](../../crates/verse-world/src/service/realm/registry.rs) stores
accounts, credential bindings, and character residence in immutable SHA-addressed
nodes selected by manifest version 2. It separates account and character IDs from
credential keys, instance actor slots, life generations, controllers, and render
actors. Accounts own at most eight characters and one resident at a time. Dormant
characters do not consume the 2,048-entry resident table. Nodes retain at most
eight records and 128 KiB; lookup has a 64-digit depth bound. Version-one heads
upgrade without relabeling their character IDs or reward receipts. Legacy v21
scene origins remain readable after the upstream field rename; serialization
writes the current field name.

[`realm::lifecycle`](../../crates/verse-world/src/service/realm/lifecycle.rs) saves
logout state before retiring its actor, grant, connection, book, and resident
placement under one sealed head. Inventory, progression, equipment, appearance,
resources, and remaining cooldowns stay with the permanent character ID.
Dormant characters receive no combat rewards, regeneration, or cooldown progress.
Defeated characters resume defeated and use the existing respawn contract.
Abrupt disconnection retains a parked resident; explicit host retirement chooses
when it becomes dormant. Logout stops temporary effects and casts at the retired
life. These are defined slice rules; they do not establish a broader MMORPG
combat-logout penalty or recovery entitlement policy.

Primary characters can transfer or log out. Their authored scene templates remain
available for later entry, while the vacant anchor has no replicated pose,
collision body, damage target, player admission, or reward ownership. Retired secondary
actor slots recycle with increasing generations and bounded body records. Host
transactions clone live authority rather than restoring an entire instance:
unrelated player and spectator sessions remain live, and replication revisions
increase through a full baseline resynchronization.

Wire version 25 adds authenticated owned-account metadata, character selection,
and life/epoch-fenced logout. A dormant account first authenticates as an observer; selection uses a
bounded collision-checked authored entry region rather than client coordinates.
The SDK validates those outcomes. Operator recovery requires current realm leases,
a fresh key, and the expected account epoch; it retains all character IDs and
books, retires the old credential, and fences its sessions. No wire body grants
recovery authority. Stale recovery requests fail; account readback resolves an
uncertain acknowledgment after recovery of a poisoned coordinator. Retired keys cannot create another account implicitly. Public guest admission
registers verified accounts under the same sealed registry; logout releases guest
capacity, and saved version 11 retains the guest policy across restart.

Authored reward policies select enrolled residents (the retained cooperative
default), connected players, or connected players within a bounded radius. The
latter modes exclude disconnected and distant characters as declared. Loot is a
private per-character grant with exact retry receipts. Shared scarce drops,
contribution ranking, trading, and auctions remain V25.

**Acceptance evidence:** The [lifecycle receipt](../../bench/verse/2026-10-04/character-lifecycle/run.json)
retains source identities, scratch TLS outcomes, and check logs. The final broad
world run passes 485 checks and exposes one legacy-fixture conversion failure;
after removing fields that did not exist in v8, all nine migration checks pass.
Earlier broad stages and both initial failure logs remain retained. Tests cover
1,100 transient slot turnovers, 80 distinct retired accounts, 4,096 synthetic
account-index records, the 64-resident and eight-character bounds, version-one
head upgrade, unrelated live sessions, authored participation, defeated resume,
and 15 forced-termination boundaries across logout, resume, and recovery. Exact
item receipts, equipment, progression, resources, and remaining inventory survive
lifecycle boundaries. Public guests receive registered ownership only after
verification; logout frees guest capacity, restart preserves policy, and retired
keys remain refused. On the final main integration, 110 engine tests, four native
hosted tests, three CLI chamber tests, and the real CLI REACH grant test pass.
Host/migration examples compile, targeted formatting passes, and local document
links resolve. These functional timings do not establish hardware performance.

**Limits:** Instances still cap concurrent residents at 64 and realm instance
slots at 32. Archive checkpoints retain their original content and catalog
identities; incompatible catalogs require migration. Each dormant record pins a
bounded whole-world checkpoint rather than a compact character-only blob, and
immutable storage grows with history. Serialized copies, validation, storage,
retention, and backup operating budgets remain V18/V19. The operator API supplies
key replacement, not proof of human recovery entitlement. Account recovery does
not grant Studio, host execution, or other NIP-HOST rights. Mobile/browser
selection screens remain V24. No hardware or production population acceptance is
claimed by these functional contracts.

## Rendering, assets, and simulation scale

### V10: Profiling cannot yet attribute the frame budget

[`FrameTimings`](../../crates/verse/src/imported/mod.rs) measures preparation,
encoding, submission, CPU waiting, and readback. The render passes do not request
GPU timestamp writes. `gpu_wait_ms` measures a CPU wait, and live draw timings
stop after submission; neither is isolated GPU execution. The remote recorder
also introduces readback, encoding, duplication, and capture drops.

**Improve:** Add optional GPU timestamp queries with delayed readback and
capability fallback. The [wgpu feature documentation](https://wgpu.rs/doc/wgpu/struct.Features.html#associatedconstant.TIMESTAMP_QUERY)
describes pass timestamp writes and conversion through the queue timestamp
period. Check the repository's pinned API when implementing it. Separately
instrument presentation/acquire waits, simulation stages, encoding, queues,
network age, and input-to-display latency. Exclude warm-up from steady-state
percentiles and retain it as a separate startup metric.

**Acceptance:** An isolated client and a multi-client workload report CPU, GPU,
presentation, capture, and network metrics independently. Each regression names
its device, resolution, quality settings, build, and active workload.

### V11: Rendering paths need common budgets and graceful degradation

[`pbr::gpu`](../../crates/verse/src/pbr/gpu.rs) consumes engine quality tiers.
The original chamber's [`imported::Renderer`](../../crates/verse/src/imported/mod.rs)
uses fixed four-sample pipelines and its own lighting/shadow setup instead.
[`ResolvedInstances`](../../crates/verse-engine/src/presentation.rs) caps one
extracted frame at 1,024 instances. The latest change raises that limit from 256
and indexes attachment parents instead of scanning the full frame for each
mount. Renderable instances include equipment and effects as well as actors,
so player capacity alone does not determine fit.

The chamber already has conservative frustum bounds, reusable world/shadow
bundles, and exact frozen-caster caches. It evaluates actor palettes and bounds
before visibility rejection, retains per-instance palette buffers, and submits
actor/model surfaces independently. There is no authored mesh or animation LOD
contract in the reviewed pack/frame path.

**Improve:** Share capability admission and measurable quality budgets across
both renderers. Count actors, mounts, effects, surfaces, shadow views, upload
bytes, and target memory separately. Add animation/mesh LOD and batching where
profiling supports them. Define prioritization for excess visual effects;
valid gameplay should not fail because optional visuals exhaust a frame cap.

**Acceptance:** A crowded battle stays within declared CPU/GPU/memory budgets,
with a tested low-quality fallback and observable degradation. Device loss
recreates admitted resources and presentation state; successful hot reload does
not substitute for device-loss recovery evidence.

### V12: Catalog handles do not implement streaming residency

[`residency::Catalog`](../../crates/verse-engine/src/residency.rs) provides
generation-safe lookup, not an eviction, streaming, or memory manager.
[`loading::Prepared`](../../crates/verse-engine/src/loading.rs) prepares a whole
pack, including decoded textures, before renderer construction. The renderer
then uploads all textures and geometry. The loader's budgets do not account for
all GPU mip levels, multisample targets, shadow maps, caches, and temporary
copies. Zone entry is a useful coarse loading boundary, not a large-world
streaming system.

**Improve:** Add cooked chunks, dependency-aware residency, prioritized async
read/decode/upload work, upload time budgets, eviction, and device-loss rebuild
inputs. Track CPU and GPU high-water memory independently. Compile static
lighting into content where practical; keep runtime bake caching explicit.

**Acceptance:** Traverse content larger than the memory budget with bounded
frame stalls and cache growth. Cancel a load, change zones, exhaust memory, and
lose the device without admitting stale results or dropping authority state.

### V13: Mipmap generation has a concrete color-space defect

[`Renderer::build`](../../crates/verse/src/imported/mod.rs) averages texture RGBA
bytes into each mip level, then offers sRGB and linear views of the same image.
For base color and emissive RGB, averaging encoded sRGB values does not compute
the correct linear-light average. The same universal filter does not normalize
normal maps or preserve alpha-test coverage. The material contract already
distinguishes these channel roles.

**Improve:** Cook role-specific mip chains: decode/filter/re-encode sRGB color,
retain linear scalar maps, renormalize normals, and preserve cutout coverage.
Declare when one source image requires distinct cooked variants. The
[glTF material specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#materials)
defines the different color and data-texture semantics. This is a correctness
fix before higher-resolution art, not a request for a new rendering dependency.

**Acceptance:** Black/white color averaging matches linear-light reference
values, normal mips remain normalized, and distant cutout silhouettes retain
coverage. Tests exercise actual cooked/uploaded mip bytes and material roles.

### V14: Physics acceleration is incomplete at the scene level

[`physics::queries::Mesh`](../../crates/physics/src/queries.rs) has a triangle
BVH, but scene ray, overlap, and sweep queries iterate admitted collider maps
and dynamic capsule shapes. [`World::detect`](../../crates/physics/src/collision.rs)
enumerates collider pairs before filters and bounding tests. Small scenes benefit
from the existing code; large collider populations still incur broad enumeration.

The shared rigid-body path implements capsule-versus-oriented-box contact and
warm starting. The retained Ruins
[`collision_static`](../../crates/verse-ruins/vendor/crates/collision_static/src/lib.rs)
still has its separate unimplemented capsule/OBB function; the documented
retained limitation does not describe the shared solver. Character sweeps and
relative-motion sphere/capsule CCD exist; they do not establish general rotating
rigid-body CCD.

**Improve:** Measure scene candidate counts, then add a deterministic top-level
broadphase and dynamic updates. Keep static per-mesh BVHs. Define generic rigid
CCD, islands, and parallel solve only for required workloads, preserving stable
ordering and replay/conservation checks.

**Acceptance:** Increasing distant colliders has bounded query cost. Large
crowds, fast props, rotated contacts, stacks, supports, and removal/reuse pass
correctness fixtures. Publish candidates, narrow-phase work, solve time, and
momentum/energy residuals rather than claiming scale from body count alone.

### V15: Navigation needs a world-content lifecycle

[`walkable::Navigation`](../../crates/physics/src/walkable.rs) compiles
multilayer cells, supports bounded routing, and fences dynamic blockers. It caps
cells and nodes at 65,536, allocates per-query search arrays, and compiles links
through collision queries. [`room`](../../crates/verse-world/src/room.rs) caches
navigation for named built-in profiles. The older horizontal box router is not
the complete current chamber navigation implementation.

**Improve:** Cook tiled navigation with content identities, reuse search
scratch, schedule route work, and add hierarchical paths, off-mesh links, and
crowd avoidance. Specify local invalidation when doors, construction, or spell
geometry changes. Path goals must continue through collision admission.

**Acceptance:** A multilevel zone routes crowds through stairs and doors,
replans around construction, and reports no-path separately from exhausted work.
Measure route p99 and total per-tick navigation cost during synchronized pursuit.

### V16: The primary adventurer remains a special implementation path

[`play::Game`](../../crates/verse-world/src/play.rs) and
[`play::multiplayer`](../../crates/verse-world/src/play/multiplayer.rs) separate
the primary player's fields from additional players. Additional-player admission
explicitly refuses `Ability::Spell` and `Ability::SpellCommand`, while the
[`spell catalog`](../../crates/verse-world/src/spells/mod.rs) callbacks mutate
`Game` and derive facing from primary state. The original ten abilities have
shared-caster implementations; the newer physics spells do not have equivalent
multiplayer coverage.

Named model/profile checks, built-in rooms, ability enums, hardcoded resource
defaults, and encounter adapters also mix chamber content with reusable rules.

**Improve:** Make every player an actor-scoped record and pass an explicit caster
context to every ability. Preserve the original ten-ability behavior while
moving definitions and encounter parameters into validated content. Keep AI,
player, and agent controllers on the same command boundary.

**Acceptance:** Every shipped ability works for two independent casters with
separate resources, concentration, cooldowns, collisions, events, and saves.
Adding a second encounter or character class does not require a new player branch
or a renderer-specific combat rule.

### V17: Engine extraction needs actual consumers

The broad [`verse` dependency graph](../../crates/verse/Cargo.toml) contains
GPU, glTF, fonts, Nostr, zones, retained compatibility, and optional agent/UI
systems. Generic renderer and original content compilation remain under
`imported`. The headless host example calls `verse::imported` for content and
collision admission, so its executable does not have the same minimal boundary
as the default `verse-world` crate.

[`core::Entities`](../../crates/verse-engine/src/core.rs) provides generational
slots, but the inspected world/runtime paths do not use that container.
`FixedSchedule` is also not the production world's common scheduling owner.
An extracted API with only tests does not establish migration of real consumers.

**Improve:** Move portable content identity, collision cooking, and validation
out of the renderer. Extract the renderer behind admitted frame contracts, then
move original compilation into Rust tools. Adopt a common schedule/entity
contract where it removes duplication; avoid a speculative ECS rewrite or crate
proliferation without a real consumer.

**Acceptance:** Build and run a dedicated world host without GPU/window/font,
private-reader, or agent dependencies. A second original world uses the same
engine contracts without copying the chamber application.

### V18: Crowd recovery improves without passing scale acceptance

The new [`verse_load`](../../crates/verse/examples/verse_load.rs) and
[battle harness](../../scripts/bench/verse-battle-capture.py) exercise 20
authenticated players, 40 hostile NPCs, delayed connections, one native client,
and 19 headless clients. The
[retained failure](../../bench/verse/2026-10-04/battle-scale-recovery-failure/run.json)
records host exit after 2,743 ticks and 55,371 requests, with
[`host.log`](../../bench/verse/2026-10-04/battle-scale-recovery-failure/host.log)
ending in `Character spawn recovery did not converge`. Moving the initial NPC
positions did not eliminate the failure. The compiled revision is `e87066b8c5`;
the fixture revision is recorded separately. The receipt reports the earlier
native presentation capacity failure resolved for this run.

[`Character::step`](../../crates/physics/src/character.rs) calls overlap recovery
during movement, not only at spawn. The original implementation returned an
error after 12 unsuccessful displacement iterations. Upstream fixes now allow 64 corrections,
resolve opposing contact planes together, verify the final correction, and
retain actor/contact diagnostics. The network loop exits when
[`Gateway::tick`](../../crates/verse-world/src/service/net.rs) returns an error.
This establishes the failure propagation; the original receipt does not identify
the specific actor, contact configuration, or reason recovery fails to converge.

The [stable battle receipt](../../bench/verse/2026-10-04/battle-scale-stable/run.json)
completes all processes, but its constructor overrides authored cultist health
with 15; live hostile occupancy averages 8.7 and falls to one. A subsequent
operator-selected `authored_combat_health` setting produces the
[sustained battle receipt](../../bench/verse/2026-10-04/battle-scale-sustained/run.json),
with forty live hostile NPCs in every measured headless occupancy sample. All
processes complete, but acceptance still fails: native frame p95 is 107.393 ms,
CPU command-buffer finalization p95 is 87.594 ms, and prediction correction p95
is 0.456 m with a 1.961 m maximum. Two proxy errors need classification.

A newer [four-contact failure](../../bench/verse/2026-10-04/battle-scale-four-contacts/run.json)
again stops the host after 1,273 ticks and 18,694 requests. Its
[host log](../../bench/verse/2026-10-04/battle-scale-four-contacts/host.log)
identifies actor nine and retains the four overlapping capsule geometries and
starting position needed for reproduction. A subsequent
[bounded capsule-exit fix](https://github.com/OpenAgentsInc/openagents/commit/148d2b6bcd)
reproduces those four contacts and searches a limited horizontal exit through
already embedding capsules, while walls and newly encountered obstacles still
block it. Its regression also refuses a wall enclosure without changing the
character. Later runs retain all forty NPCs: the
[anchored battle](../../bench/verse/2026-10-04/battle-scale-anchored/run.json)
records frame p95 of 82.048 ms; the
[instanced-shadow battle](../../bench/verse/2026-10-04/battle-scale-instanced-shadows/run.json)
records 74.605 ms, correction p95 of 2.210 m, and a 4.385 m maximum. Different
trajectories prevent isolated attribution. The newest
[three-contact failure](../../bench/verse/2026-10-04/battle-scale-three-contacts/run.json),
compiled at `533d3b0cde`, stops the host at 1,352 ticks for actor 216, with 8.094
dropped seconds and a simulation p99 histogram upper bound of 250 ms. This
truncated run cannot establish prediction or sustained-scale acceptance.
Per-character failure containment remains open. A subsequent
[typed recovery outcome](https://github.com/OpenAgentsInc/openagents/commit/9051f1f282)
adds an atomic `Step::BlockedRecovery` result that preserves the character while
keeping invalid inputs and query failures as errors. Host integration remains
required at this revision. These runs disable persistent
storage; external host CPU sampling does not establish an isolated per-tick CPU
or GPU budget.

The harness now retains partial player measurements after failures, including
all nineteen headless rows and their observation stages in the three-contact run.
Its optional scratch state directory permits future durable benchmarks but has
no retained runtime acceptance yet. Historical nondurable failures and missing
profiles remain evidence of their original revisions. Zero dropped server
seconds alone is not a passing timing distribution.

**Improve:** Preserve the upstream crowd-recovery regressions and historical
contact sequences. Measure combined scale with the corrected authored-health
configuration so the stated NPC workload remains active. Define bounded
per-character recovery or containment without hiding invariant or storage
corruption; a recoverable blocked character should not stop unrelated
players. Emit partial profiles, actor/contact diagnostics, and omission counts
even when a participant fails. Extend the harness in stages: authority-only,
network-only, one isolated renderer, and durable combined acceptance. Mix movement,
targeting, casts, AoE, NPC pursuit, equipment, quests, disconnects, and respawns.
Retain full workload parameters and source/content revisions. A capacity test
that only spawns players is insufficient.

**Acceptance:** A regression fixture covers the recorded recovery failure and
crowded movement without host shutdown. The declared 20/40 battle passes its
agreed budgets repeatedly, then a longer soak exposes ledger, event, memory,
and content-cache lifetime. Larger realm targets follow measured bottlenecks and operating cost, not an
extrapolation from a three-client video.

### V19: Operators need visibility while the world is running

[`net::Stats`](../../crates/verse-world/src/service/net.rs) retains aggregate
connections, requests, ticks, dropped time, and checkpoint totals. The example
prints these on exit. Socket worker errors are discarded. There are no live
per-client bandwidth/age counters or world-specific backup/restore tools in the
original host path. V02 adds lifetime simulation/capture/commit histograms,
writer queue watermarks, explicit storage pauses/refusals, and an ordered shutdown
drain. These metrics still need a live operator surface and verified backups.

**Improve:** Add structured live diagnostics, health/readiness distinctions,
bounded audit records, backup verification, safe draining, version reporting,
and a restore tool. Record refusals by stage and cause without keys or private
chat. Define storage failure behavior and recovery objectives before public use.

**Acceptance:** An operator can identify a slow client, expensive encounter,
stalled writer, exhausted budget, and incompatible build while the service is
running. A backup restores into a scratch host with verified receipt and
character state; rollback never creates a second active writer.

## Production content and player experience

### V20: Artists need tools over the runtime's own contracts

[`original`](../../crates/verse/src/imported/original.rs),
[`characters`](../../crates/verse/src/imported/characters.rs), and the
[`Everglade compiler`](../../crates/verse/src/zones/everglade_pack/compile.rs)
are useful Rust content pipelines. Scenes and catalogs can be authored as data,
and validated reload exists. Geometry, collision profiles, layout, clip mapping,
icons, and many gameplay definitions still require Rust changes. No integrated
scene/property/timeline editor with transactions and undo/redo exists in the
reviewed path.

**Improve:** Build a Rust pack inspector and content CLI first, then scene
placement, collision/nav visualization, timeline editing, ability/quest
validation, and undoable transactions. Use runtime validators and stable IDs.
Add incremental builds and diagnostics that point to source assets/fields.

**Acceptance:** An author creates a second playable zone, adds a quest giver,
changes an encounter, and previews the result without editing renderer code.
Bad content reports actionable errors and cannot replace the running generation.

### V21: Animation foundations need a production character workflow

[`animation_graph`](../../crates/verse-engine/src/animation_graph.rs) implements
clips, one-dimensional blends, masks, additive layers, and transitions.
[`sockets`](../../crates/verse-engine/src/sockets.rs) and native mounts share
final palettes. These capabilities should remain the basis of character work.
The reviewed contract does not supply a complete retargeting/editor workflow,
foot IK, terrain-aware locomotion, general root-motion admission, facial
performance, or crowd animation budgeting. Mesh-wide grounding of death poses
does not establish foot placement on stairs or slopes.

**Improve:** Prioritize locomotion blend parameters, turns, stop/start transitions,
aim layers, foot placement, and author-visible graph debugging. Define how any
root motion enters authority and prediction. Add broader skeleton retargeting
only with fixtures for admitted rigs; do not assume one successful kit covers
arbitrary skeletons.

**Acceptance:** Different outfits and rigs move, cast, equip, die, and respawn
on slopes and stairs without sliding, socket drift, or stale markers. Animation
quality tiers reduce crowd cost without altering damage timing.

### V22: Lighting needs one tested art and device contract

The chamber has validated local metallic/roughness materials, fog, HDR output,
32 lights, and up to four cube-shadow sources.
[`pbr`](../../crates/verse/src/pbr/mod.rs) has a different environment pipeline
with quality tiers, cascades, screen-space effects, and baked irradiance.
[`textured_bake`](../../crates/verse/src/pbr/textured_bake.rs) now bakes Everglade
vertex ambient and probes. The Everglade compiler intentionally retains only
base-color maps from environment models. These are distinct supported profiles,
not one common high-fidelity material path.

**Improve:** Define common material semantics, light units/exposure, shadow
priority, color grading, transparency, and sky/ambient behavior. Add clustered
light selection, more shadow work, temporal effects, or improved reflections
only where art requirements and GPU measurements justify them. Diagnose the
current foreground/readability problems before adding a general GI system.

**Acceptance:** Indoor torch/spell scenes and outdoor forest/station scenes
retain readable characters, calibrated materials, and bounded shadow cost at
every supported tier. Visual comparisons name exposure and content revisions.

### V23: Audio needs a content and lifecycle layer

[`audio::Mixer`](../../crates/verse-engine/src/audio.rs) provides bounded PCM
voices, spatial gain/pan, pitch, looping, and life-scoped release. The
[`native adapter`](../../crates/verse/src/audio_native.rs) supplies device output,
bounded command work, and counters. It is a sound foundation, but does not supply
streaming music/dialogue, mix buses, priorities/virtual voices, environmental
occlusion, localization, or complete browser/mobile mounting.

**Improve:** Add authored sound banks/cues, voice priorities, buses, music and
dialogue streaming, listener/zone transitions, and platform focus/device recovery.
Profile callback work and PCM destruction as well as allocation; bounded queues
alone do not prove audio deadline safety.

**Acceptance:** A crowded fight preserves critical cues without underruns,
handles device/focus changes, and restores music correctly after zone changes.
Captions/subtitles and volume controls remain usable when audio is unavailable.

### V24: Shared rendering is narrower than multiplayer platform parity

[`everglade-web`](../../crates/everglade-web/README.md) mounts an offline glade:
no plaza, relay, or studio host. Native mobile uses shared Rust state and secure
identity injection, but the inspected mobile surface mounts `WorldRuntime` and
presence rather than the desktop remote chamber worker/view/prediction path.
Raw TLS/TCP chamber connections also cannot be used directly by browser code.

**Improve:** Adapt the authoritative client to the planned reachable channel and
shared platform input/session lifecycle. Test touch/controller remapping, combat
HUD, readable text, accessibility, network/focus interruptions, and device
resource recovery. Preserve thin Swift/Kotlin glue and Rust-owned application
state. Desktop feature success does not imply a phone has the same capabilities.

**Acceptance:** A desktop, physical phone, and supported browser share one
authoritative instance, see the same outcomes, reconnect, and handle suspend/
resume within their device budgets. Publish an explicit supported-platform and
feature matrix; simulator or offline rendering checks do not establish parity.

### V25: MMO features require domains beyond the chamber

The service has XP thresholds, prerequisite quests, giver enrollment/dialogue,
consumables, outfits, and two equipment slots. Gear currently contributes health
and mana. These should not be described as absent. They do not implement a full
class/stat/progression model, inventory item instances, trading, crafting,
auction/mail, parties, guilds, matchmaking, or persistent faction/reputation
systems. NIP-XP work achievements and world character progression also have
different authority and identity contracts.

**Improve:** Define a coherent playable loop and then implement the required
domains behind typed authority operations. Separate social presence/chat from
party, guild, loot, and economy membership. Add item instance identity and
atomic transfer before any trade. Build abandon/repeat/reset rules for quests,
and deterministic stat derivation before expanding equipment catalogs.

**Acceptance:** A party completes a progression loop across sessions and zones;
loot, inventory, and quest changes survive retries and restart. Unauthorized
membership changes and duplicate trades cannot create items or rewards. Defer
commerce breadth until those guarantees are established.

### V26: Creator content and studio data need explicit admission

[`inventory`](../../crates/verse-engine/src/inventory.rs) distinguishes original,
research, owner-supplied, capture, and redistribution provenance. It explicitly
treats declarations as metadata rather than legal attestation. Some retained
Ruins asset notices still identify unknown original authors/licenses. Their
presence in an archive is not evidence of a completed shipping review.

The zone API accepts closed supported rules, not arbitrary downloaded executable
code. That is a useful boundary. A future creator world also needs publisher
identity, distribution approval, content limits, compatibility, moderation, and
revocation. Public world access must not disclose private Agent Studio panels
or cause tool execution.

**Improve:** Build a release artifact inventory that validates the dependency
closure and excludes research-only assets. Define creator publication and
revocation separately from loading a valid pack. Preserve archives and source
notices. Keep studio projections scoped to the viewer's grant even in a shared
world; add account block/report and bounded abuse handling to public social
surfaces.

**Acceptance:** A release builds with private game directories absent and proves
every shipped dependency's admission. A valid but unapproved creator pack cannot
publish itself. A world-only viewer cannot read private studio content or invoke
host commands through scene interaction.

### V27: Replay guarantees need a declared execution profile

[`Game` checkpoint tests](../../crates/verse-world/src/play.rs) compare restored
simulation across future ticks. Physics uses double precision and seeded dice;
these are valuable local guarantees. The checkpoint pins rules, and saves pin
content, but they are not a complete cross-build replay package with executable,
toolchain, target, RNG algorithm, and ordered external input history. Wall-time
catch-up drops are observable but not a general operational replay stream.

**Improve:** Declare supported determinism profiles and retain ordered admitted
commands, commit/tick boundaries, executable/rules/content identity, RNG state,
and divergence hashes. Keep input replays distinct from presentation trajectories.
Do not promise cross-architecture bit equality without tests.

**Acceptance:** Replay retained multi-player commands through save/restore,
shutdown, and supported builds. Report the first divergent tick and field.
Rejected commands, controller handoffs, and dropped elapsed time have explicit
recorded semantics.

### V28: Documentation can misdirect implementation priorities

[`verse-world/README.md`](../../crates/verse-world/README.md) retains earlier
paragraphs saying persistence, native service mounting, duplex behavior, or
prediction remain, alongside later implementation updates.
[`networking.md`](../verse/networking.md) names wire version 14; current
[`wire::VERSION`](../../crates/verse-world/src/service/wire.rs) is 21. Historical
milestone entries are useful evidence, but readers need a current status distinct
from that history.

**Improve:** Add a compact capability/status table owned by current runtime
guides. Link historical implementation receipts instead of accumulating
contradictory status paragraphs. Generate version and feature references where
practical; keep proposed networking convergence clearly labeled.

**Acceptance:** Every advertised capability identifies the implemented path,
supported platform, measured acceptance, and remaining limitation. An engineer
can distinguish a current gap from a dated receipt without reconstructing issue
history.

## Delivery order and proposed acceptance

The following budgets are proposed engineering targets for the next slice, not
universal AAA standards or measured Verse results. Confirm hardware, resolution,
quality, and network profiles when implementing the harness.

| Stage | Work | Evidence required to advance |
| --- | --- | --- |
| 1. Persistent correctness | V01–V03, V09, V18: transaction lifetime, ordered durability, migration, character identity, and crowded movement recovery. | Long-lived rewarded play; forced commit-boundary termination; exact retries after recovery; populated save upgrade; recorded battle failure reproduced and fixed. |
| 2. Playable movement and replication | V04–V08, V24: effective input timing, relevance/deltas, reachable sessions, and social authority. | Repeated impaired-network movement/lifecycle matrix; ordinary correction p95 initially below 0.10 m for the declared flat-ground profile, with separate collision/teleport results. |
| 3. Measured battle | V10–V19: timing, quality, physics/query/nav scaling, caster parity, and operations. | About 20 authenticated players and 40 active NPCs, durable state enabled; server tick p99 below its 33.3 ms interval; separate storage backlog, bandwidth, correction, and isolated-client metrics. |
| 4. Device and content production | V12–V14, V20–V24, V26: streaming, cooking, authoring, locomotion, lighting, audio, and platform clients. | One isolated reference desktop at a declared 60 FPS profile, initially targeting frame-time p95 ≤16.7 ms and no steady-state stalls over 50 ms; declared phone/browser targets; content authored through tools. |
| 5. MMO persistence and population | V06, V09, V25–V27: transfers, social/economy domains, publication, and replay/operations. | Multiple instances and growing population with exclusive character ownership, atomic item transfer, bounded operating cost, and tested recovery. |

Run short smoke workloads during individual implementation slices, then dedicated
scale and soak jobs on contributor machines or non-GitHub infrastructure.
Suggested progression is 2 players → 20 players/40 NPCs → longer persistent
soak → measured multi-instance population. A soak should cross transaction,
event-retention, respawn, reconnect, and cache lifetime boundaries; merely waiting
without mutations does not exercise them.

Each implementation has its own claimed issue and relevant targeted checks.
Complete one issue, update this audit, and push to `main` before starting the
next. Remediation status does not claim completion of the engine roadmap. Avoid a wholesale renderer/ECS rewrite,
new product languages, new engine dependencies, or broad release gates as a
prerequisite for the first correctness fixes.

## Verification of this audit

The original audit is documentation-only: verification checks local Markdown
targets, cited source paths, finding identifiers, retained JSON receipts, and
`git diff --check`. V01 remediation runs `cargo fmt -p verse-world`,
`cargo test -p verse-world` (274 tests), and
`cargo test -p verse-world --features service-net` (375 tests) on the pinned
toolchain. All pass. After the subsequent authored-health integration, the five
affected `service::host::` tests and formatting also pass. Clippy, release gates,
and live owner-host probes are not run. V02 runs formatting for `verse-world` and
`verse`, `cargo test -p verse-world` (274 tests),
`cargo test -p verse-world --features service-net -- --test-threads=2`
(383 passing tests; one helper is launched as a subprocess by its parent test),
and `cargo check -p verse --example verse_host --features remote-chamber`.
The isolated cadence and growing-state measurements are retained in the V02
receipt. All checks pass. After integrating the bounded capsule-exit change,
the affected network tests and host example check also pass. Existing transcripts, research artifacts, and
measurement receipts remain in place.

V06 runs formatting for `verse-world` and `physics`, the full service suite
(432 passing tests, two ignored helpers), final realm acceptance (10 passing
tests, including five subprocess crash boundaries and two real TLS listeners),
portable world tests (289), physics tests (115, one ignored), and native remote
consumer tests (14). The final realm checks cover changes added after the full
suite: trusted reward grants, startup lease validation, and graceful lease release.
All pass. The native check uses the installed ALSA development package and the
imported-desktop/remote-chamber feature set. Source hashes, parent identity, timing,
and scope are retained in the V06 receipt. No release gate, Clippy, or owner-host
live smoke runs.

After integrating `d073728f79` and its shared REACH transport, V06 reruns the
full `service-reach` world suite (440 passing tests, two ignored helpers) and
the 14 native consumer tests. Both pass. The upstream grant and device-identity
checks remain intact; realm TLS continues to require explicit enrollment.
The receipt retains the original measurement identity separately from the
integrated source hashes and final TLS observation.
