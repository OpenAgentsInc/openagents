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
V18 remediation in [#10637](https://github.com/OpenAgentsInc/openagents/issues/10637)
contains recoverable crowd movement and passes a declared durable 20-player/
40-hostile profile, including three combined repeats and a ten-minute soak.
Historical failed battles remain retained. The accepted profile uses one native
renderer and nineteen headless clients on one machine. V20 remediation in
[#10736](https://github.com/OpenAgentsInc/openagents/issues/10736) adds a validated
content workbench, undoable edits, authority previews, and sealed playable
generations. V21 remediation in
[#10737](https://github.com/OpenAgentsInc/openagents/issues/10737) adds admitted
locomotion, terrain contacts, aim, crowd sampling, and character diagnostics.
V22 remediation in [#10738](https://github.com/OpenAgentsInc/openagents/issues/10738)
adds shared shading semantics, contribution-ranked physical lamps, and tiered
lighting references. V23 remediation in [#10739](https://github.com/OpenAgentsInc/openagents/issues/10739)
adds admitted audio banks, streaming, priorities, captions, and native lifecycle
handling with controlled callback evidence. Broader art, coordinated operations, device, and population
readiness remains open in V24–V28.

The [engine roadmap](../verse/engine/roadmap.md) already names a battle with
about 20 authenticated players and 40 active NPCs. That milestone now has a bounded accepted profile. Neither a 64-player admission limit nor a video with two
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
| [`verse-ruins`](https://github.com/OpenAgentsInc/openagents/blob/e3d774841b39bca2a7a916ebe115e442bc7dffe2/crates/verse-ruins/README.md) | 3 / 1,354 | Adapter boundary and retained source/provenance; selected vendored collision, replication, and server schedule interfaces. |
| [`verse-wow` at the original baseline](https://github.com/OpenAgentsInc/openagents/blob/e3d774841b39bca2a7a916ebe115e442bc7dffe2/crates/verse-wow/src/lib.rs) | Compatibility adapter | Imported snapshots, numeric motion bindings, and separation from original content. |
| [`everglade-web`](../../crates/everglade-web/README.md) | 3 / 809 | Pinned pack fetching, local world mounting, input, and WebGPU/WebGL2 rendering. |
| [Mobile surface](../../crates/coder-mobile/src/verse_app.rs) and [OpenAgents wrapper](../../crates/openagents-mobile/src/verse.rs) | Integration review | Rust-owned state, injected identity, native surface lifecycle, and feature boundaries. |
| [Host example](../../crates/verse/examples/verse_host.rs), [remote client](../../crates/verse-imported/src/imported/remote_window.rs), and [battle harness](../../scripts/bench/verse-battle-capture.py) | Execution-path review | Startup, content identity, configured rights, persistence selection, network workers, authenticated load, capture, and profiling. |
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
| V10 | P1 | CPU, GPU, capture, and transport costs have separate measurement contracts. | Code, recorded | Profiling and acceptance | Complete ([#10619](https://github.com/OpenAgentsInc/openagents/issues/10619)) |
| V11 | P1 | Renderer budgets and quality behavior differ by path. | Code, risk | Renderer and device capabilities | Complete ([#10623](https://github.com/OpenAgentsInc/openagents/issues/10623)) |
| V12 | P1 | Cooked static chunks have bounded native streaming residency. | Code, recorded | Content loading and residency | Complete ([#10625](https://github.com/OpenAgentsInc/openagents/issues/10625)), static-content profile |
| V13 | P1 | Runtime mip generation ignores texture semantics. | Code | Content compiler and texture upload | Complete ([#10629](https://github.com/OpenAgentsInc/openagents/issues/10629)) |
| V14 | P1 | Spatial queries and rigid-body detection need scene-level scaling. | Code, risk | Shared physics | Complete ([#10630](https://github.com/OpenAgentsInc/openagents/issues/10630)) |
| V15 | P1 | Content-bound navigation tiles have scheduled routes and local invalidation. | Code, recorded | Navigation and AI | Complete ([#10634](https://github.com/OpenAgentsInc/openagents/issues/10634)), grounded profile |
| V16 | P1 | Game rules and primary-player special cases limit reuse. | Code | World rules and ability adapters | Complete (chamber profile; [#10635](https://github.com/OpenAgentsInc/openagents/issues/10635)) |
| V17 | P1 | Shared rendering, compiled content, and dedicated TLS hosting have working consumers. | Code, recorded | Engine extraction and host packaging | Complete ([#10636](https://github.com/OpenAgentsInc/openagents/issues/10636)) |
| V18 | P0 | Contained crowd recovery and a durable 20/40 battle pass the declared profile. | Recorded, code | Movement failure handling and scale acceptance | Complete ([#10637](https://github.com/OpenAgentsInc/openagents/issues/10637)), one native renderer |
| V19 | P1 | Persistent operations lack complete live diagnostics and recovery tooling. | Code, gap | World operations | Complete ([#10735](https://github.com/OpenAgentsInc/openagents/issues/10735)) |
| V20 | P2 | Content production still requires Rust implementation work. | Code, gap | Rust authoring tools | Complete ([#10736](https://github.com/OpenAgentsInc/openagents/issues/10736)) |
| V21 | P2 | Admitted characters share locomotion, terrain contacts, and author diagnostics. | Code, recorded | Animation and character content | Complete, named-rig profile ([#10737](https://github.com/OpenAgentsInc/openagents/issues/10737)) |
| V22 | P2 | Declared lighting profiles share shading semantics and tier references. | Code, recorded | Rendering and art direction | Complete, controlled profiles ([#10738](https://github.com/OpenAgentsInc/openagents/issues/10738)) |
| V23 | P2 | Audio is a bounded mixer, not a complete game audio system. | Code, gap | Audio and platform adapters | Complete for the controlled native profile ([#10739](https://github.com/OpenAgentsInc/openagents/issues/10739)) |
| V24 | P1 | Mobile/browser rendering does not establish authoritative game parity. | Code, gap | Platform world clients | Complete for the portable combat profile ([#10742](https://github.com/OpenAgentsInc/openagents/issues/10742)) |
| V25 | P2 | MMO social and progression systems need dedicated domains. | Code, gap | Verse game services | Addressed [#10743](https://github.com/OpenAgentsInc/openagents/issues/10743) |
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
and retention remain V19. V17 moves portable content admission into `verse-content` and provides a
dedicated TLS host that does not link the renderer.

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
[`solids`](../../crates/verse-zone-everglade/src/zones/everglade/solids.rs), while the chamber
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

### V10: Frame costs have separate measurement contracts

**Implemented in [#10619](https://github.com/OpenAgentsInc/openagents/issues/10619).**
The original finding conflated CPU submission and GPU execution and included
startup in frame percentiles. Subsequent upstream work already added the imported
renderer’s optional three-slot GPU timestamp sampler and invalid-sample health
counters. This remediation preserves that sampler and attributes its delayed
results to their original submitted frame. The pinned
[wgpu 29.0.4 queue contract](https://docs.rs/wgpu/29.0.4/wgpu/struct.Queue.html#method.get_timestamp_period)
supplies the timestamp conversion period. `VERSE_GPU_TIMING=1` requests the
feature when supported; ordinary rendering remains available without it.

[`FrameProfile`](../../crates/verse-gfx/src/profiling.rs) retains the first 120
submitted frames separately from steady work. Each phase admits at most 48
series of 8,192 observations and reports invalid values and omissions. Renderer
construction uses frame zero; delayed GPU and capture results keep their source
frame. The recorder’s version-eight profile retains legacy lifetime fields for
compatibility and labels their inclusion of startup.

CPU measurements distinguish client projection, preparation, timestamp polling,
shadow/world/overlay encoding, command completion, and queue submission. Native
presentation separates configuration, surface acquisition, encoding, submission,
and the present call. Capture separately reports queue residence, copy submission,
fence/map waiting, row copying, duplicate writes, and encoder pipe writes. These
are CPU elapsed spans: a driver or pipe wait can dominate them, and parent and
child spans must not be added together. GPU pass spans exclude presentation and
capture copies. A successful present call does not prove scanout.

The SDK’s optional bounded `worker::Observer` supplies request turnaround,
verified-to-consumer delivery, pending requests, channel depths, and local
snapshot freshness without blocking authority updates or retaining credentials.
Turnaround includes queues, TLS, server work, and validation. Snapshot age starts
at local verification or application, not the server clock. Native input records
oldest handled input to the next CPU submission as a proxy; input-to-display and
one-way network age remain explicitly unavailable. Standalone host simulation,
save capture, and commit distributions retain their first 120 stage observations
separately, with histogram percentile upper bounds.

The durable scratch workload exposed a worker that terminated on a temporary
`storage_busy` read refusal. Snapshot, event, and inventory reads now back off
independently for 100 ms, stop after ten seconds of continuous refusal, and
recover after successful reads. Commands are never replayed. A real authenticated
duplex regression refuses each read class, resumes replication, and proves a
single command executes once.

**Acceptance evidence:** The
[retained receipt](../../bench/verse/2026-10-04/frame-attribution/run.json)
contains isolated and three-client TLS workloads with durable scratch state,
480 submitted frames per client, 1280 × 720 targets, four-sample antialiasing,
and NVIDIA GeForce RTX 4080/Vulkan device metadata. Each client has 120 startup
and 360 steady CPU frames and 359 steady GPU samples; the last asynchronous
sample is not drained by a blocking shutdown fence. The isolated scene GPU p95
is 4.163 ms; three clients range from 2.910 to 4.207 ms. These are separate runs
with evolving original chamber content, not a controlled claim of multiplayer
speedup. Sampling raw capture every 30 frames retains separate copy/wait/row
costs. A 180-frame run with timestamps disabled renders and replicates without
GPU latency series. A real native GPU/FFmpeg regression verifies two captures,
two duplicate frames, a four-frame decodable video, and separate startup/steady
capture queue and pipe measurements. After integration with main, 17 worker,
16 client, 15 transport/histogram, and 76 native imported checks pass; six native
evidence tests remain opt-in, including the separately executed GPU timer and
recorder cases. The host, native remote, and profiling examples compile, and
local documentation links exist. A subsequent rebase onto main’s replication
optimizations passes 12 replication and 17 worker checks, and the three examples
still compile.

**Remaining operating limits:** These debug workloads share one process and
adapter and do not establish 20-player/40-NPC acceptance, hardware exclusivity,
mobile/browser GPU parity, or a production frame budget. The offscreen fixture
reports display and presentation measurements as unavailable. The native surface
instrumentation compiles, but this receipt does not claim a display run or
physical input-to-display measurement. Encoder pipe blocking is measured; encoder
process CPU is unavailable. The PBR renderer and realm host do not yet expose
all of this chamber-specific attribution. V11, V18, V19, and V24 own the common
quality, population, operational, and platform acceptance work.

### V11: Shared renderer budgets and bounded device recovery are implemented

Resolved for the measured native rendering profile in
[#10623](https://github.com/OpenAgentsInc/openagents/issues/10623).
[`quality::Budget`](../../crates/verse-engine/src/quality.rs) declares common
instance, optional-effect, surface, target, geometry, texture, and buffer limits.
The chamber and physical adapters probe actual renderable/filterable/blendable
formats and supported color/depth multisampling. `VERSE_QUALITY` can lower the
supported tier. Low quality uses one sample, 128 optional effects, and six
256-pixel chamber shadow faces; high uses four samples, 768 optional effects,
and 24 512-pixel faces. Physical lighting retains its tier-specific cascades.
Managed payload reservations precede initial GPU allocations and target resize;
a rejected resize preserves the active extent. Timings remain measured targets,
not frame cancellation deadlines.

[`VisualSelection`](../../crates/verse-engine/src/presentation.rs) admits up to
8,192 source instances and selects optional visuals before the 1,024-instance
extraction cap. Smaller priorities and then source order determine retention.
Every source value is validated, including omitted effects. Actor roots and
mounts cannot be classified as optional. The chamber adapter prioritizes known
stock effect and particle models; callers must use the portable selector for
other optional content. Counts distinguish actor roots, mounts, retained and
dropped effects, surface batches, shadow views, upload payload, and reservations.
Physical glow uses a bounded triangle prefix; it does not invent actor counts
for geometry without life identities.

Opaque and shadow instancing already existed at implementation time. V11 adds
compatible additive triangle instancing after measuring excess effect draws;
alpha-blended effects retain their existing ordering. A GPU comparison reduces
32 additive draws to one with identical captured RGBA pixels. No authored mesh
LOD is claimed, and actor animation remains fully evaluated. The retained crowd
measurements identify enough headroom for this fixture without introducing an
untested animation LOD policy.

[`gpu_lifecycle`](../../crates/verse-gfx/src/gpu_lifecycle.rs) records the device's
loss callback and polling errors. Native recovery creates a new admitted device,
pipelines, targets, and presentation bindings from retained verified source
values, without reopening deleted asset files. It preserves compatible actor
playback and monotonically increasing frame IDs, rejects old catalogs and
presenters, and stops after three recoveries. Grid, chamber, remote-window, and
physical surface adapters invoke recovery. Browser callers receive an async
recovery API. Capture fence and callback waits are bounded to five seconds.
A zero-shadow frame resolves only written timestamp queries; resolving the
unwritten shadow query previously stalled the hardware queue.
Reload performs fallible resize before transferring active playback.

**Acceptance evidence:** The
[renderer budget receipt](../../bench/verse/2026-10-04/renderer-budgets/run.json)
retains 300-frame, 1280×720 debug workloads with 120 startup frames on an RTX 4080
and Vulkan. Each has 20 synthetic player roots, 40 synthetic NPC roots, 20 bow
mounts, and 2,000 optional effects. High-quality sparks retain 768 effects and
record CPU p95 6.35 ms and GPU p95 2.56 ms; low retains 128 and records 4.89 ms
and 0.97 ms. Both fit the declared 16.67 ms frame target and each managed resource
category. All roots and mounts remain present. A separate mist workload records
high CPU p95 27.75 ms, above target, and low CPU p95 7.70 ms, below target;
its fallback, rather than high-quality acceptance, is the supported result.

Explicit software-Vulkan tests on a scratch X11 display destroy drained devices
and verify new presentation submissions for both paths, physical world/overlay
preservation at normal and low quality, source-file-independent actor playback,
and stale-handle refusal. These exercise actual wgpu device destruction and
resource recreation rather than hot reload. An RTX 4080 offscreen test also destroys and recreates
the device after the source files are removed, preserves actor graph playback
and a bow mount, and completes a subsequent frame. A hardware zero-shadow
timestamp test verifies the queue-stall fix. The receipt records targeted unit,
shader, consumer-compilation, formatting, and GPU checks, including failures.

**Limits:** Resource figures are logical payloads or conservative reservations,
not process RSS or driver memory. Driver padding, swapchain images, fixed
shader-private resources, transient replacement peaks, and asynchronous capture
copies are excluded. Reserved ordinary dynamic buffer capacity is reported
separately. Frame counts do not establish server population capacity; V18 owns
networked 20/40-player acceptance. The scratch display and llvmpipe recovery
checks do not establish physical scanout. Hardware destruction/recreation is
verified, while a spontaneous driver reset remains outside this fixture. The chamber
still requires cube-array textures and explicitly refuses adapters below that
contract. Platform recovery integration, externally owned `Layer` devices,
capture-worker interruption, browser lifecycle acceptance, and physical display
latency remain V18/V24 acceptance work. Authored mesh/animation LOD remains a
content capability gap for V20/V21 and larger future profiles.

### V12: Cooked static chunks stream under independent residency budgets

**Status:** Complete in [#10625](https://github.com/OpenAgentsInc/openagents/issues/10625)
for the first native static-content profile.

[`streaming`](../../crates/verse-engine/src/streaming/mod.rs) separates desired
content, verified CPU data, and complete GPU uploads. SHA-256-bound chunks use a
versioned binary header and an admitted dependency graph. Root order establishes
priority; dependencies upload first and stay pinned with the current view.
Independent CPU/GPU payload budgets and a fixed pending-job count bound source
work, LRU eviction, and allocation admission. A rejected view preserves the
previous one. Failures require an explicit retry. Cancelled workers retain their
reservations until their results arrive; zone generations and exact upload
tickets prevent stale work from replacing active content.

[`streaming::store`](../../crates/verse-engine/src/streaming/store.rs) performs
regular-file reads and zero-copy payload validation on fixed background workers
under an explicit source root. Native Unix reads anchor the directory descriptor
and refuse symlinks and nonregular files. The cooker publishes a verified,
synced file without replacing an existing digest. Manifest metadata has separate
chunk, edge, and JSON bounds.

[`Source`](../../crates/verse-pbr/src/streaming.rs) integrates with both native
`render::Renderer` and `render::Layer`. Per-frame bytes limit incremental buffer
and image writes; a soft CPU deadline stops before the next driver call.
Evicted GPU handles are released before replacement allocation. Geometry becomes
visible only after its own upload and dependencies commit. Device recreation
retains verified CPU data and fences prior uploads. Automatic native renderer
recovery and the host-owned layer's explicit `rebuild` use that state. The first
profile stores static triangles or lines, single-level sRGB RGBA images, and an
optional lighting-recipe digest; vertex colors hold the offline result. It
performs no runtime bake or hidden bake-cache allocation.

**Recorded acceptance:** The original 48-tile
[`streaming_traversal`](../../crates/verse/examples/streaming_traversal.rs)
fixture draws a 42,485,280-byte dataset under 4 MiB CPU and 3 MiB GPU payload
budgets at 1280×720, paced at 60 Hz. Retained
[evidence](../../bench/verse/2026-10-05/streaming-residency/README.md) records
1,200 frames on an RTX 4080/Vulkan, separate high-water counts, bounded uploads,
eviction, two stale results after zone cancellation, an oversized-view refusal,
and actual device recreation with world state preserved. A pixel-checked PNG
proves textured terrain reached the target. The explicit GPU regression uses
four-byte uploads across partial image rows, verifies both depth conventions,
deletes the source files, and obtains identical pixels on a replacement device.

**Limits:** These budgets cover managed payloads, not total process RSS or driver
VRAM. Metadata, render targets, and fixed shader resources have separate bounds. Queue
staging and in-flight driver resource retirement are excluded; V18 retains GPU
backlog admission. A driver call or host filesystem syscall already in progress cannot be
preempted. Device reconstruction is a separate measured interruption. New views
may show a loading gap until their dependencies commit. Streamed geometry uses
baked or unlit colors with cutout alpha; it does not enter cascade-shadow or
screen-space prepasses. Animated packs and full physical materials retain their
existing loading paths. Authoring, LOD and animation, physical lighting, broader
fault containment, and phone/browser streaming acceptance remain V20–V22, V18,
and V24. This fixture does not establish networked population or physical
scanout performance.

### V13: Material-role mip cooking and uploads

**Completed:** [#10629](https://github.com/OpenAgentsInc/openagents/issues/10629).
The imported renderer previously averaged encoded color and data bytes with one
universal filter and reused an sRGB image through a linear view. The physical
textured renderer already filtered color in linear light, but selected one
coverage recipe per source image and omitted material opacity from that recipe.

[`verse_engine::mips`](../../crates/verse-engine/src/mips.rs) now provides a shared
versioned RGBA8 recipe. Color and emissive RGB decode to linear light before area
filtering and return to sRGB; scalar maps retain linear values. Normal texels and
each normal reduction normalize vectors, with a forward fallback for cancellation.
Fractional area weights retain the edges of odd-sized images. These channel roles
follow the [glTF material specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#materials).

A mask variant includes the material's effective cutoff (`cutoff / opacity`). Each
reduction targets the original image's covered texel fraction, selects the nearest
representable count, and calibrates alpha contrast against a fixed bilinear-repeat
sampling grid. Corrections do not feed later reductions. An opaque material,
different cutoffs, and normal or scalar users of the same source receive distinct
chains. Both native paths reserve those variants before upload. The imported path
uses explicit sRGB color and linear data textures without view reinterpretation;
its shadow bindings use the same mask chain as the color pass.

**Evidence:** [Retained mip checks](../../bench/verse/2026-10-05/mip-semantics/README.md)
include real RTX 4080 Vulkan readbacks and material-binding probes. Black/white
color reduces to RGB 188, scalar data to 128, and opposed normals to the forward
fallback. Five channel bindings return their expected linear samples. The masked
checker retains 50% rendered coverage at mip level 1. Portable tests also cover
odd image sizes, thin stems, effective-cutoff variants, malformed inputs, and
variant resource accounting. Native consumer and browser compilation checks pass.

**Limits:** This is a CPU cooker used during upload, with a portable API for
content tools. V20 adds persisted, source-bound authored RGBA8 mip chains and direct upload.
Compressed GPU formats remain unsupported. Coverage
is approximate: one coarse texel cannot represent a fraction, so a nonempty mask
retains one visible texel; contrast calibration cannot guarantee every cutoff,
view angle, anisotropic filter, trilinear transition, or vertex-alpha multiplier.
Ranking ties can change coarse silhouette shape. The physical glTF loader also
uses the material recipe when fitting oversized sources to its 2048-texel limit.
The hardware check uses a scratch generated fixture and does not establish phone or browser image
quality, frame latency, or production art acceptance.

### V14: Deterministic scene and rigid broadphase

**Completed:** [#10630](https://github.com/OpenAgentsInc/openagents/issues/10630).
[`physics::broadphase`](../../crates/physics/src/broadphase.rs) adds a balanced
dynamic AABB tree with stable leaf identities, reusable node slots, small retained
bounds, and explicit pose/removal updates. Scene queries use separate instance
roots and retain per-mesh triangle BVHs. Sorted candidates preserve hit ordering,
life/layer/usage exclusions, and nearest-hit truncation. Triangle admission uses
a maintained count. Main's immutable shared mesh buffers are preserved.

The rigid step indexes conservative shape-radius and per-collider motion bounds.
A second tree limits static/sleeping queries to bodies that can respond. Kinematic
wake-up also uses spatial candidates. The solver receives the original pair order
and margin arithmetic. Derived indexes stay outside serialized authoritative state.
The arbitrary pair-margin callback remains an exhaustive reference because its
return values have no spatial bound; `detect_bounded` and
`detect_motion_profiled` expose explicit indexed contracts. Optional query/step
measurements report scene visits, candidate/filter/bound/narrow-phase work, updates,
and wake-up work separately.

**Evidence:** [Retained broadphase evidence](../../bench/verse/2026-10-05/scene-broadphase/README.md)
includes an exact reconstructed baseline binary, a CPU-core-pinned comparison,
source/executable receipts, and chronological samples. With 4,096 colliders and
a sparse static background, candidates fall from 8,386,560 pairs to zero with
4,320 scene visits. Total-step p95 falls from 12.635 ms to 0.408 ms; initial tree
construction remains visible in a 2.082 ms first step. Local query candidates stay
at one, with 7–27 top-level visits as distant geometry increases. A 1,024-capsule
moving fixture returns 59,520 hits from 61,440 sweeps; pose and sweep phase p95
is 0.303 and 1.014 ms. A separate 256-sphere contact run measures 30,600 narrow
contacts, solve p95 0.235 ms, linear momentum residual about `7e-16`, zero angular
residual, and expected energy dissipation.

123 physics, 586 Verse, and 315 world tests pass. New fixtures compare indexed
hits/manifolds and exact serialized state with exhaustive paths through rotated
shapes, scopes, filters, truncation, fast motion, wake-up, removal/reuse, and
restoration. Existing supports, stacks, sweeps, CCD, and conservation tests pass.
Native consumers and the browser build compile. Timings are observations on a
shared machine, not a whole-engine performance gate.

**Limits:** Dense overlaps, loose radius bounds, large absolute-speed margins,
and selective filters can retain large candidate sets. Synchronization remains
linear in colliders, and small scenes pay tree maintenance overhead. Rigid sensor
rays, plume scans, and arbitrary-margin detection retain enumeration. Existing
sleep islands and sequential solve cover the measured workload; general rotating
rigid-body time of impact and parallel solving need a declared workload before
implementation. Supported character/capsule sweeps and linear sphere/capsule CCD,
plus speculative rigid margins, do not establish general rotating-debris CCD.
The retained Ruins capsule/OBB stub stays separate from the shared solver's working
capsule/OBB path. V18 retains whole-game overload and tick-budget acceptance.

### V15: Content-bound tiles and scheduled navigation

**Completed:** [#10634](https://github.com/OpenAgentsInc/openagents/issues/10634).

[`walkable`](../../crates/physics/src/walkable.rs) now cooks aligned tiles with
local collision source and character-setting identities. Directed seams connect
multilayer spans; a coarse tile corridor guides fine search, which can leave that
corridor when obstructions or height layers require it. Explicit `Grounded`
transitions must pass capsule motor admission in each declared direction.
[`VNT1`](../../crates/physics/src/walkable/tiled.rs) encodes a graph pinned by a
trusted content manifest, with structural checks before loading. Limits are
4,096 tiles, 1,048,576 cells/spans, and 64 MiB of cooked bytes. The source snapshot
retains its existing 4,096-collider and 16,384-triangle limits.

Epoch-stamped search storage reuses distance, parent, and heap allocations.
[`Game`](../../crates/verse-world/src/play.rs) uses a 256-request FIFO with actor
life fencing and expiring request heartbeats. Each authority tick reserves at
most four searches, 65,536 expansions, and 2,000,000 collision work units,
including unsuccessful searches. Deferral holds position. Direct probes have a
2,048-unit soft quota; optional smoothing has a 4,096-unit total quota. Ending
those quotas retains compiled waypoints. Hard work exhaustion remains distinct
from no path. Occupied starts and stalled motor probes stop early.

Doors, construction props, corpses, and spell walls invalidate routes in affected
tiles. Distant routes keep their obstruction revision. Transient spell-wall
bounds fence graph edges without changing the durable blocker life book. Seven
sampled velocities avoid up to 16 selected nearby actors, using bounded spatial
buckets and stable actor priority. Every final movement continues through the
capsule motor. Checkpoint revision v23 preserves queued route work and rejects
stale actor lives or future scheduling ticks; v22 checkpoints remain readable.

The [retained crowd fixture](../../bench/verse/2026-10-05/navigation-crowd/README.md)
uses 40 live capsules, stairs, a closing/reopening door, and construction over
360 ticks at 30 Hz. All 40 receive admitted routes; construction invalidates 31
routes. It records 264 no-path outcomes, zero ordinary hard-budget exhaustions,
and an independent exhaustion probe. Complete trajectories, outcomes, and
content identities replay identically. On a shared i7-14700K pinned to CPU 0,
route p99 is **3.42 ms** and total navigation tick p99 is **5.60 ms**. The archive
pins sources, executable, compiler, and the 119,680-byte cooked graph.

**Limits:** Seven actors reach their goals within the 12-second congested run.
Local avoidance and conservative replanning do not guarantee crowd liveness.
The graph remains resident; tile unloading and new walkable-surface edits need
content orchestration. Explicit transitions cover continuous grounded movement;
jumps and ladders need their own admitted controller actions. The component
measurement does not establish a whole-game or production MMO frame gate; V18
owns those budgets. Physics, Verse, world, native consumer, and browser checks
cover the shared implementation; no owner host or phone run is claimed.

<a id="v16-the-primary-adventurer-remains-a-special-implementation-path"></a>

### V16: Shared caster records and validated chamber tuning

**Status:** Complete for the audited chamber ability profile
([#10635](https://github.com/OpenAgentsInc/openagents/issues/10635)).

[`Game`](../../crates/verse-world/src/play.rs) stores primary and additional
players in the same `ActorState` record. The original ten abilities use the same
actor dispatcher. Each of the nine
[`catalog constructors`](../../crates/verse-world/src/spells/mod.rs) receives a
captured `Caster` and checks its life, source, pose, selection, save difficulty,
and authority tick. Player, AI, and agent inputs use the same admitted command
boundary. The primary-player field projection remains for existing local
presentation and the flat checkpoint layout.

[`Caster dispatch`](../../crates/verse-world/src/play/caster.rs) scopes catalog
cooldowns, mana charges, commands, concentration, and seeded dice to the caster.
Thunderwave also uses the admitted caster's dice and save difficulty. Catalog
casts emit authority events with the caster's life. Ongoing Gust of Wind follows
each owner, concurrent Wind Walls remain in projectile collision, and Wind Wall
retains the support state its next physics step needs after restoration. Failed
casts restore gameplay changes while keeping an admitted command sequence
consumed, as required by the transport contract. Generation changes discard
pending navigation work with obsolete actor lives.

[`Character content`](../../crates/verse-world/src/content.rs) validates class
keys, health, mana, save difficulty, and catalog costs/cooldowns. Bounds are
200–600 health, 20–60 mana, DC 5–30, costs 0–20, and cooldowns 0–600 seconds.
[`Encounter content`](../../crates/verse-world/src/combat.rs) validates hostile
health, duration, warmup, and stagger. Defaults preserve the chamber profile.
A test defines a storm warden with different resources, DC, and Gust tuning
without a player branch or renderer rule. Another changes encounter health and
timing through the existing rules. Character tuning and remaining catalog
cooldowns survive world transfer; restarts retain the selected definitions.
Checkpoint v24 reads the supported older profiles, including v23's additional
player aliases and primary catalog cooldowns.

**Evidence:** The paired-caster tests admit all 19 abilities with independent
records and resources, retain events and cooldowns, and compare 26 subsequent
50-millisecond updates byte for byte with restored worlds. Separate checks cover
scoped saves, hand/release and altitude commands, concentration, moving remote
Gust ownership, character transfer, legacy layout migration, and refusal fences.
The service-enabled world suite passed 510 tests with two intentional subprocess
helpers ignored; the six final caster tests passed separately. Verse passed 507 tests with 13 intentional GPU/fixture checks ignored after
regenerating and repinning stale Grid and Everglade artifacts; its 22 content-pack
checks also passed separately. Native desktop,
mobile, and CLI checks, the browser build, and affected-package formatting
passed. Retained evidence is under
[`actor-casters`](../../bench/verse/2026-10-05/actor-casters/README.md).

**Remaining limits:** This covers the chamber's ten original and nine catalog
abilities. Class data tunes these implementations; new effect mechanics and
hostile archetypes still require Rust adapters. Named room/model choices and
other application zone demos remain application content. Remote catalog UI and
those demos' authority parity are covered by V24; authoring is covered by V20.
Cast rollback currently clones the bounded game state; its cost needs V18 scale
acceptance. These two-caster, same-platform replay checks establish neither MMO
raid throughput nor cross-platform replay equivalence, which remain V18 and V27.

### V17: Engine extraction needs actual consumers

**Status:** Complete in [#10636](https://github.com/OpenAgentsInc/openagents/issues/10636).

The broad [`verse` dependency graph](../../crates/verse/Cargo.toml) remains the
application composition. [Issue #10636](https://github.com/OpenAgentsInc/openagents/issues/10636)
extracts working engine consumers from its imported path:

- [`verse-content`](../../crates/verse-content/README.md) owns portable content
  identity, scene/model admission, outfit/equipment admission, and original
  furnishing collision. Its optional Rust compiler owns procedural geometry,
  glTF character/prop import, retargeting, icons, effects, and inventory
  provenance. Native clients and the dedicated host use the same admission.
- [`verse-pbr::imported`](../../crates/verse-pbr/src/imported/mod.rs) owns the
  shared skeletal renderer, materials, shadows, culling, instancing, semantic
  mip upload, and GPU timing. Its frame contract depends on engine presentation
  values. Engine `MountPose` describes socket, planar grip, and offset frames;
  the renderer no longer selects bow behavior by an application model name.
  The shared UI pipeline lives in `verse-gfx`.
- [`verse-host`](../../crates/verse-host/README.md) is a dedicated authenticated
  TLS executable. Its normal dependency graph excludes GPU, windows, fonts,
  glTF import, private readers, Coder, and agents. The previous Verse example
  delegates to this executable's library entry point. REACH hosting remains
  available through the OpenAgents CLI.
- [`FixedSchedule`](../../crates/verse-engine/src/core.rs) now drives the world
  network loop and host check mode at 30 Hz, with at most three catch-up steps
  and explicit dropped time. Storage pauses retain their separate metric.
  `core::Entities` remains an unused generational container; this change does
  not claim migration to a new entity system.

The Rust compiler produces the original ritual combat world and an independent
observatory social world. The observatory authors a different floor, spawn,
seat, and switch through the same asset, scene, collision, content identity,
render extraction, social authority, and checkpoint contracts. It does not copy
the chamber application. Host `--check` admits a configured world, advances its
schedule, and checks checkpoint restoration without opening a listener or
writing a checkpoint.

Combined consumer tests exposed journal replay depending on JSON map feature
unification. The world crate now pins ordered maps and exact float round trips;
journal changes replace objects whose key layout changes. Recovery accepts an
older sorted-map representation only when its complete digest matches the
sealed parent or state. Regressions cover insertion, deletion, reordered keys,
legacy sorted-map records, and tampering. This preserves journal version and
digest validation.

**Verification:** The final library suite passes 509 Verse, 11 content, 136
engine, 25 graphics, two host, 72 renderer, and 514 world tests, with 18
intentional GPU, artifact-writing, or subprocess helper tests ignored. The
standalone integration, both compiler commands, separate minimal host build,
300-tick checks through that binary, native desktop/mobile/CLI compilation,
browser compilation, formatting, and local documentation links pass. The stale
Everglade pack is regenerated and repinned; its previous reviewed platform pack
is retained. Evidence and source checksums are under
[`engine-boundaries`](../../bench/verse/2026-10-05/engine-boundaries/README.md).
The standalone integration compiles both worlds, validates their admitted render
frames, runs 300 scheduled check ticks, starts each actual TLS host process,
connects with enrolled scratch credentials and its content digest, observes live
ticks, toggles the observatory switch, and requests clean shutdown. It also
refuses absent actor models and modified runtime textures.

**Remaining limits:** These two offline scratch worlds establish working engine
boundaries, not production population or raid throughput. Original furnishing
collision retains its explicit model whitelist. New mechanics and arbitrary
world authoring remain V20; platform and zone authority parity remain V24;
scale and cross-platform replay remain V18 and V27. The application still owns
its UI, zones, authored frame composition, and optional agent integrations.

### V18: Contained crowd recovery and durable battle acceptance

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

Remediation in [#10637](https://github.com/OpenAgentsInc/openagents/issues/10637)
contains recoverable character movement and establishes a measured durable
20-player/40-hostile profile. The [capacity evidence notes](../../bench/verse/2026-10-05/battle-scale/README.md)
retain every historical failure, budget, command, source patch, executable hash,
and measurement limitation. The [final execution manifest](../../bench/verse/2026-10-05/battle-scale/execution-manifest-final-main-repeat.json)
records seven passing stages on source baseline
`c0611aab1f880ff9184a091eb6c62ccb36658a62` plus its pinned implementation patch.

The production changes preserve the authority and durability boundaries:

- Recorded three-capsule recovery has an atomic blocked outcome. The world
  contains that character while unrelated players and ticks continue. Invalid
  inputs, query truncation, and storage failures still propagate. Actor/contact
  diagnostics, partial profiles, and omission counters remain bounded.
- Durable reads capture their immutable admission prefix before later requests
  enter the stream; delivery waits for commit. Queues and held reply bytes bound
  backpressure. Wire version 29 carries actual completed movement within the
  original accepted sequence, separately from body time and permission credit.
  Sixteen runtime confirmations per actor are omitted from saves.
- Prediction retains at most 257 motor states and 256 deferred steps. It compares
  a confirmation at its own physics step and preserves already processed travel
  against later actor poses. Unchanged walls constrain historical horizontal
  corrections; changed motor state, support, fixed geometry, forces, or policy
  still requires reconciliation. Fresh interval input affects future integration.
  A projected capsule overlap permits straight motion only if it separates every
  original contact, sweeps other geometry, preserves penetration bounds, and
  retains the same floor. Past deferred time stays deferred. Corrections are
  neither capped nor smoothed.
- Native movement sends complete four-step histories at 30 Hz, retains bounded
  twelve-step catch-up, and integrates main's verified clock recovery and
  epoch-checked movement during teleport reply waits. Inventory and quest
  mutations remain ordered. Live presentation keeps authority time beyond the
  cinematic endpoint; explicit render animation phase ownership prevents marker
  catch-up across clock sources while preserving marker limits and atomic errors.
  Canonical hostile-flight endpoints retain collision order and checkpoint
  validation at long lifetimes.

The new driver uses scratch TLS hosts, Schnorr-authenticated participants,
ordered durable storage, one native session, and nineteen headless workers on
one machine. It sustains thirty-nine 20,000-HP cultists and one boss, uses a
bounded 40 ms plus 0–20 ms jitter pipelined route, and draws the actual low-quality
scene, mounts, lighting, and HUD offscreen at 1280 × 720. Movement, casts/AoE,
pursuit, equipment, seeded quest claims, items, midpoint reconnect, and actual
defeat/respawn use the production authority paths.

| Final profile | Result |
| --- | --- |
| Three consecutive 60-second combined repeats | All gates pass; 1,793, 1,792, and 1,792 workload ticks; ordinary correction p95 below 0.214 m and maximum below 0.641 m. |
| Isolated renderer | Required roots/mounts retained; draw CPU p95 6.344 ms and GPU p95 1.131 ms against 16.667 ms. |
| Authority and delayed durable network | 600 simulated authority seconds and 60 actual network seconds pass. |
| 600-second combined soak | All clients complete both segments; 17,910 workload ticks; simulation p99 upper bound 28.667 ms against 33.333 ms. |
| Soak presentation and correction | Steady CPU frame p95 5.248 ms, GPU p95 5.772 ms, and snapshot age p95 223.896 ms; native ordinary correction maxima 0.6401 m and 0.4801 m. All late windows pass. |
| Soak lifetime and recovery | 114 post-thirty-second RSS samples; peak growth 44,429,312 bytes against 64 MiB; twenty inventories verified, forty live hostiles, sixty actors, 127 active receipts, ledger revision 787, 512 events, and a 446,777-byte checkpoint; two actual respawns. |

Request queue peak is twenty against 128, writer queue peak is two against two,
and held replies peak at 911,226 bytes against 32 MiB. Completed measurement
segments use integrity-checked scratch files; compact typed producer histories
retain all declared fields and explicit omission counts. Whole-process RSS and
late windows remain gated. Acceptance thresholds stay unchanged. Earlier
campaigns retain failed motor recovery, cinematic/animation clocks, correction,
RSS, and disk-throughput results; passing receipts do not relabel them.

Targeted verification passes 546 world tests, six native session tests, 78
renderer tests, 125 engine tests, 131 physics tests, the actual TLS host tests,
and scratch TCP proxy tests; each log records its source stage and existing
ignored fixtures. Formatting, local documentation links, receipt hashes, and
source-patch integrity pass. Disk-full build failures and narrowly scoped
obsolete-executable cleanup are retained separately.

This establishes the declared single-machine profile. It does not establish
twenty renderers, minimum-device performance, physical input-to-display latency,
default authored combat balance, quest-giver/loot progression, browser/phone
parity, larger realms, or multi-host operations. Matching wire-29 host/client
deployment remains an owner step in `NEEDS_OWNER.md`. V19, V24, V25, and V27
address the remaining operational, platform, MMO, and acceptance scope.

A later [blocked-write soak](../../bench/verse/2026-10-05/battle-scale-blocked-write/soak-run.json)
against an updated transport fails journal append/sync after 3,700 workload
ticks and records eighteen movement expiries. The shared disk was observed at
1.7 MiB free; exhaustion is a likely cause, not a proven errno attribution.
Late flat memory after authority shutdown does not establish healthy sustained
operation. The earlier accepted revision remains historical evidence; V28 must
retain a healthy latest-revision rerun and resolve its movement expiries before
claiming uninterrupted operation for that profile.

### V19: Live chamber diagnostics and verified recovery

**Status:** Complete in [#10735](https://github.com/OpenAgentsInc/openagents/issues/10735).

The original finding was exit-only visibility and missing chamber recovery tools.
V02 added ordered persistence and shutdown draining; later transport admission
added bounded stage counters. This remediation exposes those contracts while a
dedicated TLS world is running.

[`operator`](../../crates/verse-world/src/service/operator.rs) retains one latest
snapshot, 128 volatile connection observations, and 128 phase/reason records.
Observers receive owned copies, so retaining an old sample cannot hold the
publisher's channel lock. Snapshots report build/wire/content/instance identity,
health and readiness, timing histograms, queue occupancy and budgets, durable
revision and pending age, encounter population, and motor/navigation refusals.
Per-connection fields report payload totals, delivered tick lag, activity age,
authentication state, and budget refusals. There are no keys, addresses, names,
chat, command bodies, or raw errors in this surface.

The [dedicated host](../../crates/verse-host/README.md#local-operations) provides
owner-local status and drain commands. Its eight IPC workers have two-second
limits, responses have a 256 KiB limit, and the caller has a three-second total
timeout. Samples older than three seconds cannot report readiness. Retained
post-exit status is explicitly unavailable. Drain stops admission, commits final
admitted state, and releases writer ownership after persistence completes.
A failed write produces failed health and prevents its acknowledgment. A blocked
writer keeps its lock and produces stalled or stale diagnostics.

[`backup`](../../crates/verse-world/src/service/persistence/backup.rs) acquires
exclusive offline ownership, exports the last durable revision and reachable
receipt nodes, and verifies all manifest/checkpoint/history digests, characters,
and retained migration records. Restore requires a new directory, holds its
writer lock through verification and synchronization, and marks interrupted
restores so host admission refuses them. It preserves reviewed rollback and its
refusal after later progress. Offline pruning plans a bounded scan before
removing only history unreachable from current state and every migration backup.
The default operation limits are 65,536 files, 1 GiB, and a 16 MiB manifest.
These bounds limit maintenance operations without exhausting the reward ledger.

The [retained evidence](../../bench/verse/2026-10-05/operator-recovery/README.md)
binds source, compressed patch, executables, tests, and structured receipts. The
accepted scratch run observed a 1,864 ms oldest pending commit, two writer copies,
a 47.81 ms injected expensive tick, an idle authenticated client, and one pending
handshake. Readiness was false during the writer stall; drain recovered authority
tick 4 and durable revision 6. A 104-file, 132,075-byte backup restored all 300
reward transactions and the original retry receipt. Five process-death boundaries
refused partial restoration and retained a verified source; each retry recovered
all 140 transactions. Rollback remained exclusive and refused later progress.
The real host commands also passed live status, active-writer export refusal,
drain, offline readiness refusal, export, verify, restore, and pruning for scratch
original content. Targeted checks passed 572 world tests and four host unit tests,
plus the integration covering two original worlds; three ignored helpers run
through their parent crash tests. Formatting passed. Development test-fixture
failures and a disk-exhausted combined build remain retained with their corrected
successful checks.

Recovery targets every effect and exact retry receipt in the selected verified
backup revision. Backup frequency determines the recovery point; no production
recovery-time target is inferred from scratch storage. The implementation covers
one chamber store. Coordinated realm transfer/registry recovery, off-machine
backup custody, and deployment/storage-specific drills need their own operating
profile. Native reference rendering and battle receipts remain scoped to V18;
this issue does not establish production MMORPG availability.

### V20: Validated content authoring workbench

**Implemented:** [#10736](https://github.com/OpenAgentsInc/openagents/issues/10736)
adds the Rust [`authoring` workbench](../../crates/verse-content/src/authoring/mod.rs)
and [`command workflow`](../../crates/verse-content/README.md). An inspector
reports source identities, model keys, materials, clips, states, sockets, texture
slots, and journal history. Transactions edit scene properties, actors,
placements, stable-ID timeline cues, parametric box geometry, material and clip
mappings, existing ability tuning, collision, and gameplay catalogs. All edits
pass the runtime validators before journal admission. Source/field diagnostics
identify invalid quest givers, animation references, ability costs, and malformed
records. Canonical numeric keys reject aliases and duplicates.

Undo and redo persist across restarts, use expected revisions, and retain up to
32 documents in a bounded journal. An OS lock excludes concurrent writers.
Previews own an isolated authority and validate the actual render projection;
SVG output shows placements, collision, compiled capsule-clearance navigation,
and cue timing. A rejected edit or candidate preview preserves the active world.
Authored combat retains its own duration, hostile IDs, and cue sequence.
Collision, regional navigation, and character tuning survive reset and durable
recovery through the existing host.

Builds reuse snapshotted assets and publish immutable, sealed generations.
Content identity binds scene, geometry, textures, gameplay catalogs, and retained
mip bytes. Only an admitted, synced generation updates the atomic current
pointer. Reuse checks all sealed files; corruption and unsealed files refuse
admission. Interrupted build directories remain unadmitted for inspection.
The generated host template uses the same runtime admission and durable
content fence as the preview.

Persisted RGBA8 mip archives complete the authored-chain gap assigned here by
V14. Source hashes, material roles, variant coverage, contiguous full-resolution
chains, and payload hashes are checked before loading or GPU upload. Matching
authored variants survive material edits; new variants use the shared runtime
recipes. Portable byte loading and device recreation retain the same chains.
Source pixels and archive storage share the loader's memory budget.

**Acceptance and evidence:** The retained
[content authoring evidence](../../bench/verse/2026-10-05/content-authoring/README.md)
runs the actual CLI against the original licensed ritual assets. The
[sample transaction](../../assets/verse/authoring/chamber-outpost.transaction.json)
creates a second outpost zone, adds quest giver 100, changes guardian 2's health
to 175, adds matching barricade geometry and collision, and edits dialogue.
The authority preview reports the giver as interactable and produces a navigation
visualization. Undo/redo reproduces the same generation; an invalid giver reports
its quest field and leaves the current pointer unchanged. A dedicated-host test
accepts the quest, advances the authority, commits, recovers the same checkpoint after the expected controller fence,
and refuses changed quest rules against the retained state. Targeted engine,
world, content, host, and renderer checks and native archive upload readbacks are
recorded with source, executable, and artifact hashes. The final authoring suite
passes ten tests, the CLI workflow passes, the engine passes 141 tests, and the
renderer passes 78 library tests plus the native archive readback. Both host
integrations pass after rebasing onto current main. The full world run passes
572 tests with one storage-resume timing failure and three ignored tests; the
timing test passes alone. Both results remain retained.

**Remaining limits:** This is a command editor with standalone SVG diagnostics.
It is not a windowed 3D editor. Geometry tools create static boxes and reuse
existing imported models; clip mappings reuse existing clips. New ability
algorithms, rig importers, and shaders still need engine work. Archives use
RGBA8; compressed GPU cook targets are not implemented. The fixture proves
content admission and recovery, not art quality, author productivity, device
frame time, or production-scale asset builds. V21 owns locomotion and character
authoring; V22–V28 retain their respective readiness scope.

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

**Status:** Complete, named-rig profile, in [#10737](https://github.com/OpenAgentsInc/openagents/issues/10737).

**Remediation:** The shared Rust locomotion controller admits named root, spine,
and two-bone leg chains against the retained skin and inverse binds. The
character compiler uses it for the six standard Universal outfits. A speed
blend shares normalized idle/walk/run clip phases; presented travel drives gait
phase, with bounded turn anticipation and upper-body aim. Start/stop behavior
uses the authored semantic transitions. Root translation and yaw remain in
place, and no animation displacement enters authority or prediction.

Foot placement queries the authority or prediction's admitted geometry without
advancing either simulation. It retains stance contacts within reach and slope
bounds, solves the two-bone chain, aligns the foot to support, and reports
clamps and residuals. Missing or uncertain support leaves ordinary animation
in control. Death, prone, airborne motion, seeks, teleports, new lives, changed
phase owners, and graph reloads have explicit anchor and marker reset behavior.
Equipment sockets read the final adjusted body palette.

The command editor admits graph and rig edits atomically with model edits;
preview diagnostics expose speed, turn rate, phase, source and selection epochs,
aim, tier, sampled bones, contacts, clamps, and residuals. Failed admission
preserves the journal and preview. The preview and renderer share the same
controller. Crowd tiers sample clip/hierarchy poses every frame, every second
frame, or every fourth frame, with forced samples at relevant state/source
changes; marker cursors and contact adjustments advance every frame. GPU
skinning still runs every frame.

**Recorded verification:** The [named-rig fixtures](../../bench/verse/2026-10-05/character-locomotion/README.md)
exercise male peasant and female ranger models through movement, casting,
equipment sockets, death, prone, and a new life over slope and stair query
geometry. The greatest planted-foot drift is 4.17 mm against a 5 mm bound, with
zero socket matrix difference. Pose samples fall from 300 to 152 and 79;
marker sequences match across tiers. The same independent combat script retains
identical ordered authority checkpoint hashes, cast outcomes, and damage events
across tiers. Each visual update leaves its authority checkpoint unchanged.
The merged engine passes 152 tests. The optimized world suite passes 576 tests and
three ignored helpers before the spectator geometry addition; the final
focused terrain-query check passes against that addition. Content passes 24
tests and its CLI workflow before the final pitch-sign correction; the final
named-rig fixture passes against the corrected controller. Renderer library
checks pass 80 tests with six ignored graphics checks against current main.
The final geometry and control integration also passes all 361 default world tests, including
prediction and contact isolation. The merged native client passes 54 tests with five ignored graphics checks;
the final offscreen Vulkan check separately passes on an RTX 4080, with 29
planted contacts and zero equipment-socket matrix error over 18 frames. Its
retained frame shows both bodies and their socket-mounted wands. Query terrain
is not drawn in that diagnostic image. The capture predates the final
selection-geometry reconciliation change; that change preserves the direct
terrain-ray and pose paths, and the final world and native checks cover its
integration. Source phases, exact build profiles,
executable and artifact hashes, observed fixture failures, and disk workarounds
remain retained.

**Remaining limits:** The fixture scripts body placement rather than proving a
complete motor traversal across terrain. The default graph supplies forward
gaits; dedicated backward/strafe clips still need authored admission and
fixtures. Named mappings and proportion/reorder
admission do not establish arbitrary skeleton retargeting or facial animation.
Root motion is explicitly in place; gameplay root motion requires a future
authority contract. The retained behavior profiles and sampling counters do
not establish physical-device frame time, cross-build floating-point identity,
or production character art quality. V22–V28 retain their respective scope.

### V22: Lighting needs one tested art and device contract

**Complete for the declared controlled profiles in
[#10738](https://github.com/OpenAgentsInc/openagents/issues/10738).** The chamber,
physical daylight stage, and space sky retain distinct light and ambient
profiles with common material and output semantics. This establishes an engine
contract and reproducible references, not final production art approval.

[`PointProfile`](../../crates/verse-engine/src/lighting.rs) declares the retained
chamber attenuation separately from physical candela attenuation. Chamber
intensity remains authored relative intensity; it is not relabeled as a
photometric measurement. Both native shaders expand the same
[`shading.wgsl`](../../crates/verse-pbr/src/shading.wgsl) before backend
translation. The physical camera uses the common EV100 conversion; base exposure
is applied once before the existing output grade's additional offset in stops.
Recent split toning and dusk settings remain supported.

Linear base-color factors, perceptual roughness, metallic weighting, sRGB color
images, linear data channels, and alpha coverage retain declared semantics.
Surviving masked chamber fragments now write full coverage, and blended coverage
is clamped. Straight and premultiplied blend states remain explicit. Physical
textured scenes still have a narrower map set than chamber materials; the
Everglade compiler intentionally retains base-color maps. This work does not
claim a common full-fidelity normal-map or layered-material pipeline.

Physical stage lamps use the chamber shadow selector's contribution estimate
rather than source-list order. A bright visible source in slot 31 survives every
8/16/32-lamp budget; offscreen sources cannot displace it. Chamber shadows retain
cache hysteresis and their 6/12/24-view limits. Physical stage cascades remain
2/2/3; the physical space sky uses one sun map. There are no added lighting,
reflection, GI, or temporal passes.

The renderers expose prepared per-frame lighting settings: linear exposure,
grade stops, selected points and shadowed points, ambient profile, and active
shadow size/count. The ambient profile distinguishes authored diffuse fill,
hemispheric probes, daylight sky illumination, and the space profile's Sun,
Earth, and optional irradiance probes. Active map counts include cached maps;
they are not GPU timing or full-scene frame budgets.

[Retained acceptance](../../bench/verse/2026-10-05/lighting-contract/README.md)
contains all twelve 640 × 360 Vulkan captures on an NVIDIA GeForce RTX 4080:
torch and spell profiles, a daylight forest-light profile, and the physical
station-light/space-sky profile, each at low, medium, and high. The same admitted
original procedural character appears with gray and metallic reference cards.
Each comparison names the generated content digest, source revision, exposure,
quality tier, and shadow limits. Character-versus-background comparisons pass
at every tier, with RGB-difference p95 above 12 display code values and more
than 100 changed character-region pixels. This metric and the images establish
visibility in the declared references; they do not measure perceptual quality.

Verification passes 155 engine tests, 80 renderer tests with seven explicit GPU
helpers ignored, all three selected GPU runs, and ten focused GLES/Metal shader
translation tests in the Verse consumer. The source and commands are retained.
The existing warm target remains in use; private RAM retains compiler outputs
when the shared disk fills. Checks use scratch assets and headless GPU targets without owner host services or chats.

The references use procedural geometry under the named lighting profiles,
rather than complete forest or station content. Full-zone art review,
traversal/adaptation comparisons, phone/browser measurements, and production
frame-time targets remain broader acceptance work. No physical-device FPS or
full-zone shadow-cost claim follows from these captures. V24 and V28 retain
platform and readiness-reporting scope.

### V23: Audio needs a content and lifecycle layer

**Original finding:** The owned mixer had bounded PCM, spatial gain/pan, pitch,
looping, and life-scoped release, but lacked authored banks, streaming, buses,
priorities, and output-independent presentation. Native queue bounds alone did
not establish callback allocation, destruction, or deadline behavior.

**Remediation:** [#10739](https://github.com/OpenAgentsInc/openagents/issues/10739)
adds [`audio_bank`](../../crates/verse-engine/src/audio_bank.rs) admission for
versioned cues, priorities, four buses, localized captions, original procedural
sources, and digest-bound PCM. Long music/dialogue providers supply immutable
stereo PCM readers; admission verifies length, samples, and digest before a
bounded feeder publishes frames. No bank selects a file or network path.

[`Mixer`](../../crates/verse-engine/src/audio.rs) holds 128 logical voices,
mixes 32 audible voices, and bounds streaming to eight sources. It preserves
music against effect displacement, prioritizes critical effects over footsteps,
advances virtual clip clocks analytically, and ducks music during dialogue.
Admission preflights every limit before replacing a voice. Each release uses its
own fade duration; a regression check prevents music transition gain spikes.
Completed or refused sources remain owned until off-callback reclamation.

The [`native adapter`](../../crates/verse/src/audio_native.rs) uses preallocated
SPSC command, capture, and retirement queues, with separate critical admission.
Control threads prepare voices, decode and feed streams, and reclaim resources;
callback counters expose timing, gaps, pressure, and separate device/capture/
stream errors. The [`chamber`](../../crates/verse/examples/chamber_app/audio.rs)
retains captions and master/music controls without output, pauses on focus loss,
keeps music source position through suspension, and retries failed output.
Caption state does not grant gameplay authority. Zone music binding supports
restoration within a zone and a new clock when the zone changes.

**Verification:** The [retained audio receipt](../../bench/verse/2026-10-05/audio-contract/run.json)
records the pinned toolchain, source digests, commands, and logs: 166 engine
checks; three native callback checks; the explicit deadline fixture; native
chamber compilation; and formatting. The fixture uses an i7-14700K, the optimized
Cargo test profile, 48 kHz stereo, 512 frames, 64 commands per callback, a
128-voice target (minimum 127 after retirement), 32 audible voices,
and one music stream. Across 1,000 callbacks, p99 was
0.193 ms and maximum was 0.278 ms against a
10.667 ms buffer budget, with 0 overruns, zero callback allocations and
deallocations, zero stream gaps, and no blocked retirement. Control-side draining
reclaimed 1,968 sources; reclamation p99 was 211 ns and maximum was
1,078 ns. Allocator guards also cover queue saturation and deferred PCM destruction.

The [current-main integration receipt](../../bench/verse/2026-10-05/audio-contract/integration-current-main.json)
retains the same callback checks and chamber compilation after upstream prediction,
workbench, and zone changes. Its source patch reconstructs the earlier source phase.
Storage-related refusals remain alongside the accepted checks; no owner cache was
removed or owner device opened.

**Remaining limits:** This is a controlled CPU callback profile with explicit
PCM production and control-side queue draining. It proves no driver scheduling,
physical output quality, worker scheduling latency, or general real-time deadline.
[`NEEDS_OWNER.md`](../../NEEDS_OWNER.md) records device/focus/output verification.
V24 mounts browser and phone captions; device output audio remains unimplemented. Providers must supply immutable admitted
readers; compressed codecs, environmental occlusion, production scores, and
recorded dialogue localization remain content/platform work. No complete AAA
audio-production claim follows from these original cues.

### V24: Authoritative platform clients and interruption lifecycle

**Addressed:** [#10742](https://github.com/OpenAgentsInc/openagents/issues/10742)
mounts the same authenticated client, worker, prediction view, and Rust-owned
session over TLS or NIP-REACH TCP/WebSocket on native, and REACH WebSocket in
`everglade-web`'s explicit `?zone=chamber` mode. The earlier audit paragraph
understated mobile: the existing RITUAL visit already mounted the authoritative
session. V24 extends that path and adds the browser mount; Grid relay presence
and the offline glade remain separate modes.

Channel proof and chamber signing bind the same world key. Host grants do not
confer a world role. Shared bounded framing, identity/content verification, and
request validation run on both executors; browser scheduling uses the local
executor and monotonic clock. The browser admits same-origin bounded content
before joining, verifies texture/mip identity, and refreshes enrollment on
reconnect without replaying uncertain commands. DOM buffering limits retained
Rust data; they cannot bound allocation before a browser delivers a message.

Validated mappings combine independent held controls, fence repeats and held
inputs across focus/rebinding, and preserve the mobile visit's bindings across
suspend/reconnect. Mobile and browser combat overlays scale logical text to
physical pixels and show priority captions. Browser controls retain DOM Tab
focus, have labels and minimum 44-pixel targets, and support keyboard, touch,
pointer camera, and gamepad. Focus/visibility loss releases session and GPU
resources; a fresh connection retains character authority through the existing
host enrollment rather than trusting a stale local controller.

**Evidence:** [platform client receipts](../../bench/verse/2026-10-05/platform-clients/README.md)
retain the source patch, checks, and Wasm artifact digest. Native loopbacks cover
TCP and WebSocket, foreign-signing-key refusal, same-character reconnect, and
revocation. Mobile tests cover connection cancellation, suspend/resume, host
loss, respawn, remapping, and viewport scaling. Shared session and content tests
exercise the original authoritative path; Wasm linking verifies browser target
compatibility. A fresh headless Chromium profile and scratch REACH host verify
authenticated keyboard movement and stopped input against the server position
and sequence,
visible 44-pixel controls, narrow layout, focus release/rejoin, browser shortcuts,
and refused/restored grants. A stale-config failure led to cache bypass on
reconnect. Full-resolution software rendering initially starved movement at about four
frames per second. Adaptive graphics resolution now passes the original
1000-by-800 CSS viewport with a 237-by-190 backing canvas, stable control,
and zero drift after input release. DOM controls and captions keep their
logical size. Verified control handoffs also retire stale status messages.

**Limits:** The [supported feature matrix](../verse/platform-clients.md)
separates implemented paths from device acceptance. Physical-phone frame,
thermal, network, and shared-instance outcomes, plus hardware browser GPU,
gamepad, and screen-reader checks remain explicit owner steps in
`NEEDS_OWNER.md`. Mobile and browser mount captions but no output audio adapter;
inventory/quest panels remain desktop UI. Browser graphics and session work
share one thread; adaptive resolution reduces graphics work within that budget. This delivery establishes a portable
combat client. Complete platform and AAA MMORPG parity remain unverified.

### V25: Persistent party progression and atomic gear trades

**Addressed:** [#10743](https://github.com/OpenAgentsInc/openagents/issues/10743)
adds typed realm game services over stable character identities. Authenticated
requests check the current account, resident character, and connection; mutations
also bind the realm, a nonzero operation ID, and the exact action. The existing
immutable registry selects membership, item ownership, offers, and original retry
receipts without imposing a lifetime transaction cap.

A character can belong to one eight-member party and one 64-member guild.
Leaders invite and remove members; recipients join or decline, and departure
transfers leadership. Membership grants no chat, Studio, or world-control right.
Private reads expose only the admitted character's memberships and pending work.

Individually identified gear pins its authored definition digest, stable owner,
and version. Materialization allocates an already-owned unit. An offer locks only
its sender's items; the named recipient consents at acceptance. Both inventories,
item owners, versions, cleared locks, and the receipt publish through one realm
head. A changed destination definition, stale version, missing unit, or equipped
last unit refuses the complete exchange. Either participant can cancel, including
after expiry or a requested item's transfer. Each character retains at most 64
gear identities, 16 invitations, and 16 pending offers; an offer carries up to
eight items per side and lasts at most five minutes.

The separate trusted host party-loot operation uses the reward ledger and freezes
resident recipients and authored amounts under the first event ID. Membership
changes, logout, transfer, and retry cannot enlarge an earlier grant. Authored
quests now have durable cycles, abandonment, and repeatable reset windows; old
claims return their original outcome without claiming a later cycle. Ever-completed
quests still satisfy prerequisites. Class health and mana combine the authored
character definition, 10 health and one mana per level after the first, and
owned equipped gear, capped at 600 and 60. Derivation preserves current resources
and wounds. Legacy saves upgrade without healing. Save version 12, character
schema 4, and wire version 30 carry this profile.

The [retained evidence](../../bench/verse/2026-10-06/game-services/README.md)
binds the final source revision and test executable to 599 world tests, 25
content compiler tests, the battle consumer check, and formatting. The actual
two-listener TLS fixture authenticates two party members across two instances,
claims and repeats a quest, exchanges gear by consent, verifies class/level/gear
limits, transfers, logs out, reconnects, resumes, and restarts. Exact retries
leave experience at 200. Other checks cover guild membership, foreign mutation,
conflicting operation reuse, changed definitions, expiry, recipient-item changes,
quest abandonment, invalid recovery, and five process-death publication boundaries.
Recovery selects both old inventories or both new inventories, with matching
item ownership and an exact retry receipt.

This is a bounded native TLS engine and SDK profile. Existing UI actions remain
cycle zero; typed SDK and worker operations expose later cycles. Standalone REACH
chambers need a realm adapter to offer these operations. Automatic party combat
contribution attribution, crafting, auctions, mail, matchmaking, faction/reputation,
and a complete commerce UI remain outside this loop. The correctness fixture is
not raid throughput, distributed failover, phone/browser UI acceptance, or a
complete AAA class and economy system. NIP-XP work achievements retain separate
authority from world character progression.

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
