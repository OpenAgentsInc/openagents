# SpacetimeDB Architecture and OpenAgents/Autopilot Adoption Audit (Deep Follow-Up)

Status: deep-audit follow-up snapshot  
Date: 2026-02-25  
Owner: repo audit (Codex)

## Follow-up question this answers

How far can we push this model for OpenAgents/Autopilot:

1. Multi-agent systems should run inside a database (MMORPG-style shared world).
2. Agents should edit shared files in realtime and coordinate conflicts in one world state.
3. Worktrees/branches/merges/commits should be minimized or removed from the inner loop.
4. State changes should be ACID and replayable at line-level granularity.

## Method and evidence base

This follow-up is docs-first and code-first across both repos, with emphasis on transaction/replay internals.

### OpenAgents sources re-reviewed

1. `docs/core/ARCHITECTURE.md`
2. `docs/execution/REPLAY.md`
3. `apps/runtime/docs/KHALA_ORDERING_DELIVERY_CONTRACT.md`
4. `apps/runtime/docs/KHALA_RETENTION_COMPACTION_SNAPSHOT_POLICY.md`
5. `apps/runtime/docs/KHALA_SYNC.md`
6. `apps/runtime/src/event_log.rs`
7. `apps/runtime/src/authority.rs`
8. `apps/runtime/src/fanout.rs`
9. `apps/runtime/src/server.rs`
10. `crates/openagents-client-core/src/khala_protocol.rs`
11. `crates/openagents-app-state/src/reducer.rs`
12. `crates/openagents-app-state/tests/reducer_replay.rs`
13. `crates/openagents-app-state/tests/stream_watermarks.rs`
14. `crates/autopilot-core/src/workflow.rs`
15. `crates/autopilot-core/src/preflight.rs`
16. `crates/autopilot-core/src/startup.rs`
17. `crates/autopilot/src/app/git.rs`
18. `crates/autopilot/src/app_entry/state_actions.rs`

### SpacetimeDB sources re-reviewed

1. `/Users/christopherdavid/code/SpacetimeDB/README.md`
2. `/Users/christopherdavid/code/SpacetimeDB/docs/docs/00200-core-concepts/00100-databases/00100-transactions-atomicity.md`
3. `/Users/christopherdavid/code/SpacetimeDB/docs/docs/00200-core-concepts/00200-functions/00300-reducers/00300-reducers.md`
4. `/Users/christopherdavid/code/SpacetimeDB/docs/docs/00200-core-concepts/00200-functions/00400-procedures.md`
5. `/Users/christopherdavid/code/SpacetimeDB/docs/docs/00200-core-concepts/00300-tables/00550-event-tables.md`
6. `/Users/christopherdavid/code/SpacetimeDB/docs/docs/00200-core-concepts/00300-tables/00500-schedule-tables.md`
7. `/Users/christopherdavid/code/SpacetimeDB/docs/docs/00200-core-concepts/00400-subscriptions/00200-subscription-semantics.md`
8. `/Users/christopherdavid/code/SpacetimeDB/docs/docs/00300-resources/00100-how-to/00400-row-level-security.md`
9. `/Users/christopherdavid/code/SpacetimeDB/crates/core/src/host/module_host.rs`
10. `/Users/christopherdavid/code/SpacetimeDB/crates/core/src/subscription/module_subscription_actor.rs`
11. `/Users/christopherdavid/code/SpacetimeDB/crates/core/src/db/relational_db.rs`
12. `/Users/christopherdavid/code/SpacetimeDB/crates/core/src/db/durability.rs`
13. `/Users/christopherdavid/code/SpacetimeDB/crates/core/src/host/instance_env.rs`
14. `/Users/christopherdavid/code/SpacetimeDB/crates/core/src/host/scheduler.rs`
15. `/Users/christopherdavid/code/SpacetimeDB/crates/subscription/src/lib.rs`
16. `/Users/christopherdavid/code/SpacetimeDB/crates/durability/src/lib.rs`
17. `/Users/christopherdavid/code/SpacetimeDB/crates/commitlog/src/lib.rs`

## Executive conclusion

The thesis is directionally correct for the collaborative inner loop.

1. SpacetimeDB already provides the core MMORPG-like primitives: one shared state world, ACID reducers, realtime subscriptions, and durable replay history.
2. OpenAgents already enforces deterministic replay/idempotency on its authority side and would benefit from a shared collaboration world.
3. The hard blocker is not database capability, it is product/contract reality: current Autopilot and OpenAgents flows are explicitly git branch/PR centric and bound to current authority invariants.
4. Best path is phased: Spacetime as collaborative world authority for editor/session state first, while git remains an export/provenance boundary during migration.

## What SpacetimeDB is, deeper than the first audit

SpacetimeDB is not only storage. It is a transaction engine plus application runtime plus push replication.

### Transaction and execution model

1. Reducers are automatic ACID transactions (`transactions-atomicity.md`).
2. Nested reducer calls execute in the same transaction, not nested transactions.
3. Procedures are more flexible but dangerous for determinism:
   - `with_tx` blocks may be re-invoked and must be side-effect deterministic (`procedures.md`).
4. Runtime implementation uses serializable mutable transactions broadly:
   - `begin_mut_tx(IsolationLevel::Serializable, ...)` appears across reducer/scheduler/subscription paths.
5. Current host behavior documents that only one reducer runs at a time in practice (`module_host.rs` comments), with procedure concurrency as a separate lane.
6. Procedure HTTP requests are rejected if a transaction is open (`instance_env.rs`, `WouldBlockTransaction`), which is an explicit safety boundary against long blocking calls inside transactional critical sections.

### Commit, durability, and replay path

1. `RelationalDB::commit_tx` commits state first, then requests async durability (`relational_db.rs` + `db/durability.rs`).
2. Durability is non-blocking append API with monotonic durable offsets (`durability/src/lib.rs`).
3. Startup restore logic is explicit:
   - choose snapshot not newer than durable offset,
   - then replay history via `apply_history` fold,
   - then rebuild state (`relational_db.rs`).
4. Commitlog and durability are first-class crates, not incidental logging.

This matches the requirement for deterministic rebuild and replayable history.

### Subscription semantics and delivery behavior

1. Docs state atomic subscription initialization and atomic transaction update semantics.
2. Runtime code is careful about lock ordering:
   - DB lock before subscription lock to avoid deadlocks.
   - Commit path takes a subscription lock before commit/broadcast to avoid duplicate delivery (`module_subscription_actor.rs`).
3. Subscription compiler enforces constraints:
   - indexed joins required,
   - event table lookup-side join restrictions (`crates/subscription/src/lib.rs`).

### Event and schedule primitives for agent worlds

1. Event tables are ephemeral per transaction but recorded in commit history.
2. Event table inserts can be used as transient fanout channel with RLS-aware filtering.
3. Schedule tables provide periodic or delayed reducer/procedure execution for maintenance, compaction, snapshots, and background reconciliation.

### Security and access posture caveat

RLS exists but is explicitly marked experimental, and docs recommend views for most access-control needs.

## OpenAgents reality that matters to this thesis

### Authority and replay invariants are already strict

1. `docs/core/ARCHITECTURE.md` keeps HTTP mutation authority, Khala WS transport, and `(topic, seq)` replay/idempotency as non-negotiable.
2. `docs/execution/REPLAY.md` requires deterministic replay logs with hashable tool call/result structure.
3. Runtime run-event authority path is already deterministic and idempotent:
   - `event_log.rs` enforces per-run monotonic seq, idempotency key replay, and expected seq conflicts.
4. Khala fanout explicitly sorts by logical sequence and emits deterministic stale-cursor reasons:
   - `retention_floor_breach`, `replay_budget_exceeded`,
   - recovery metadata returned by API (`fanout.rs`, `server.rs`).
5. Client and app state code are already watermark monotonic/idempotent:
   - `khala_protocol.rs`,
   - app reducer watermarks + deterministic replay tests.

### Autopilot is still deeply git-centered today

Evidence from code:

1. Workflow explicitly orchestrates issue -> branch -> PR (`autopilot-core/src/workflow.rs`).
2. Preflight collects branch status, unpushed commits, and dirty tree (`autopilot-core/src/preflight.rs`).
3. Startup execution/review lanes embed `git status` and `git diff --stat` (`autopilot-core/src/startup.rs`).
4. UI runtime has dedicated git status/diff/log system (`autopilot/src/app/git.rs`).
5. Session UI includes explicit fork semantics (`autopilot/src/app_entry/state_actions.rs`).

This is the current product contract. A total immediate "no commits/no branches" switch would be a breaking change, not a refactor.

## Thesis analysis: what is true now, what is not

### 1) "Multi-agent systems should run inside a database"

Mostly true for collaboration state.

1. SpacetimeDB is built for shared mutable worlds and low-latency subscriptions.
2. OpenAgents already needs deterministic replay and conflict-safe coordination.
3. A shared transactional world removes many out-of-band race conditions between agents.

### 2) "All agents share one world and edit in realtime"

Feasible and strongly aligned.

1. Reducers can make collaborative edits atomic.
2. Subscriptions can push state deltas immediately.
3. Event tables can carry transient conflict/presence signals.

### 3) "No worktrees, no branches, no merges, no commits"

Not fully feasible immediately in OpenAgents as currently designed.

1. Current Autopilot workflow and customer expectations are PR/git based.
2. External ecosystem integration (GitHub, CI, code review) still depends on commits.
3. Removing git from the outer loop today would fight the product and architecture, not help it.

Practical interpretation:

1. Yes: no branch/worktree overhead inside the live collaborative session world.
2. Not yet: remove commits from outer provenance/publishing boundary.

### 4) "ACID and replayable, line-by-line"

Feasible with careful data model.

1. ACID comes from reducer transactions.
2. Replay comes from append-only operation/event history plus snapshots.
3. Line-level behavior requires explicit operation schema and deterministic conflict protocol.

## Proposed OpenAgents/Autopilot "Agent World" design on SpacetimeDB

This is the concrete architecture that matches the thesis without violating current invariants.

### Data model (minimum viable)

1. `workspace`
   - workspace identity, policy mode, creation metadata.
2. `workspace_member`
   - agent/human identity, role, capabilities.
3. `file_head`
   - `workspace_id`, `file_id`, path, current version, current content hash, current content blob or chunk pointer.
4. `file_op` (append-only canonical history)
   - `op_seq`, `op_id`, actor, timestamp, `expected_base_version`, operation payload, before/after hash.
5. `file_snapshot`
   - periodic checkpoint of fully materialized content at version N.
6. `presence_event` (event table)
   - cursor, typing, active span, soft locks.
7. `conflict_event` (event table + optional persistent `conflict_ticket`)
   - conflict reason and remediation metadata.

### Reducers

1. `apply_file_ops(...)`
   - idempotent by `op_id`,
   - validates `expected_base_version`,
   - applies operation batch atomically,
   - updates `file_head`,
   - appends `file_op`,
   - emits presence/conflict events.
2. `resolve_conflict(...)`
   - deterministic resolution path (accept head, rebase, or explicit patch).
3. `checkpoint_file(...)`
   - emits periodic `file_snapshot` for replay efficiency.
4. `rebuild_file_from_snapshot(...)`
   - deterministic fold for audit/recovery testing.

### Conflict protocol

1. Every write carries `expected_base_version` and optional range hash.
2. Mismatch returns explicit conflict payload with current head metadata.
3. No silent last-write-wins for overlapping edits unless policy explicitly enables it.
4. Conflict intent and resolution become first-class replay events.

### Replay and determinism

1. Materialized file state is derived from `file_snapshot + file_op fold`.
2. `file_op` is append-only and idempotent by `op_id`.
3. Hash chaining of operations gives tamper-evident provenance.
4. Replay test should prove equal final content and equal hash for identical op stream.

## How this integrates with OpenAgents today

### Boundary-preserving integration model

1. Keep OpenAgents control/runtime authority lanes unchanged for current canonical domains.
2. Add a new collaboration world lane backed by SpacetimeDB for shared session/file state.
3. Bridge adapters translate between proto/HTTP authority boundaries and Spacetime reducer contracts where needed.
4. Keep Khala as authority replay transport for existing runtime authority topics.

### Git posture during migration

1. Inner loop:
   - agents collaborate in one shared DB world (branchless session state).
2. Outer loop:
   - export deterministic result to git commit/PR when crossing ecosystem boundary.
3. This removes most merge/worktree overhead without breaking current product rails.

## What we should do vs should not do

### Should do

1. Use SpacetimeDB as a shared collaboration authority for multi-agent editing/session coordination.
2. Make line-level operations first-class, idempotent, and replayable.
3. Preserve OpenAgents authority invariants by isolating domains and using explicit adapters.
4. Keep deterministic replay tests as release gates for the new world model.

### Should not do right now

1. Replace control/runtime canonical economic/auth authority with SpacetimeDB.
2. Remove git as final provenance/publish boundary immediately.
3. Introduce dual competing authority paths for the same domain without ADR changes.

## Adoption sequence (pragmatic)

1. Phase A: pilot collaboration world
   - one bounded domain, for example Autopilot live collaborative file editing/presence.
2. Phase B: branchless inner loop
   - default multi-agent editing session in shared world with deterministic export-to-git step.
3. Phase C: selective authority expansion
   - only after ADR and invariant changes, and only where replay and ownership semantics remain clear.

## Risks and mitigations

1. Risk: accidental second authority plane.
   - Mitigation: explicit domain ownership map and adapter-only crossings.
2. Risk: deterministic mismatch in edit replay.
   - Mitigation: hash-chain op log, snapshot+fold replay tests, idempotency keys.
3. Risk: subscription query cost explosion.
   - Mitigation: schema/index design aligned with subscription compiler constraints.
4. Risk: procedure nondeterminism and side effects.
   - Mitigation: keep critical write logic in reducers; isolate external I/O.
5. Risk: user workflow mismatch with existing git-centric tooling.
   - Mitigation: keep git export boundary until product contract intentionally changes.

## Concrete recommendation

OpenAgents should adopt SpacetimeDB as the shared multi-agent collaboration world for Autopilot editing/session state, with ACID operation application and deterministic replay by design.

OpenAgents should not currently replace existing control/runtime canonical authority domains or remove git as the external provenance boundary.

The right near-term target is:

1. branchless, realtime, shared DB world in the inner loop;
2. deterministic export to current git/PR rails at the outer boundary;
3. future deeper changes only through ADR and invariant gate updates.
