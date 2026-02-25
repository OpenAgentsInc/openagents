# SpacetimeDB Full Replacement Plan (Spacetime Retirement)

Status: active implementation plan
Date: 2026-02-25
Owner lanes: `owner:autopilot`, `owner:runtime`, `owner:protocol`, `owner:docs`

## 1) Objective

Replace Spacetime sync/replay delivery with SpacetimeDB as the canonical sync substrate for OpenAgents.

End state:

1. All live sync/replay channels currently served by Spacetime are served by SpacetimeDB.
2. Spacetime topic fanout/replay endpoints and protocol docs are retired.
3. Autopilot-to-Autopilot collaboration and runtime sync share one SpacetimeDB transport and state model.
4. Hydra/Aegis authority domains remain intact and continue using authenticated command boundaries.

## 2) Non-goals

1. Replacing runtime/control authority for money/policy/trust decisions with unaudited ad hoc lanes.
2. Removing Nostr interoperability and NIP-90 marketplace participation.
3. Keeping long-term dual-primary sync planes.

## 3) Preflight Constraints and Required Governance Changes

Checked authorities:

1. `docs/adr/ADR-0001-rust-only-architecture-baseline.md`
2. `docs/adr/ADR-0002-proto-first-contract-governance.md`
3. `docs/adr/ADR-0009-spacetime-sync-canonical-transport.md`
4. `docs/plans/rust-migration-invariant-gates.md`

Implication:

1. Current ADR/invariant language still names Spacetime as the WS sync lane.
2. Full replacement requires a superseding ADR and invariant updates before production cutover.
3. Until supersession lands, Spacetime remains implemented reality and Spacetime work is replacement program execution.

## 4) SpacetimeDB Mechanics We Must Design Around

These are direct implementation-relevant findings from `/Users/christopherdavid/code/SpacetimeDB`:

1. Session topology is client-to-shared-database, not peer-to-peer.
2. Clients connect over outbound websocket to `GET /v1/database/:name_or_identity/subscribe`.
3. Auth tokens are accepted via `Authorization: Bearer` or query token (`?token=...`).
4. SDK path commonly mints a short-lived websocket token first (`/v1/identity/websocket-token`).
5. Server negotiates websocket subprotocol versions (`v2.bsatn.spacetimedb` / `v1.bsatn.spacetimedb`).
6. On connect, server sends initial connection identity/context.
7. Subscription lifecycle is `Subscribe` -> `SubscribeApplied` (atomic snapshot) -> `TransactionUpdate` deltas.
8. Delivery semantics are explicitly ordered/atomic at subscription init and transaction boundaries.
9. Fanout is aggregated per transaction per client, not arbitrary per-row pushes.
10. Confirmed reads can gate message release on durable offset.
11. Event tables are first-class for transient realtime signaling and are emitted on commit.

## 5) Replacement Architecture

### 5.1 Sync ownership

1. SpacetimeDB becomes the single live sync/replay transport for retained OpenAgents surfaces.
2. Spacetime is demoted to deprecation lane, then removed.
3. Nostr remains interoperability and marketplace surface, not in-domain primary sync.

### 5.2 Authority boundaries

1. Auth/session and policy decisions remain under control/runtime authenticated command APIs.
2. Spacetime reducers mutate sync/collaboration state under scoped session claims.
3. Hydra/Aegis authoritative economics/verification flows remain under existing command authority, with mirrored projections to sync where needed.

### 5.3 State model

Minimum canonical tables:

1. `sync_stream` (logical stream metadata)
2. `sync_event` (append-only sequenced events, idempotency key, payload hash)
3. `sync_checkpoint` (per client/per stream resume watermark)
4. `session_presence` (active peers and liveness)
5. `provider_capability` (NIP-90/provider capability signaling)
6. `compute_assignment` (request/assignment visibility for collaboration clients)
7. `bridge_outbox` (policy-gated Nostr mirror queue)
8. Event tables for transient presence/conflict/typing or progress signals

### 5.4 Reducers/procedures

Required reducer set:

1. `append_sync_event`
2. `ack_checkpoint`
3. `upsert_presence`
4. `publish_provider_capability`
5. `open_compute_assignment`
6. `update_compute_assignment`
7. `enqueue_bridge_event`
8. `mark_bridge_event_sent`

Reducer rules:

1. Idempotent by explicit key.
2. Deterministic serialization and hashing.
3. Monotonic sequence assignment per stream.
4. Explicit conflict errors (no silent overwrite).

### 5.5 Delivery and replay contract

1. Client subscribes with query set + resume watermark context.
2. Server emits `SubscribeApplied` snapshot.
3. Server emits ordered `TransactionUpdate` batches.
4. Client apply path stays idempotent and monotonic.
5. Confirmed-read mode is enabled for authority-sensitive streams that require durability-gated delivery.

## 6) Mapping From Spacetime Semantics

Spacetime semantics to preserve in Spacetime replacement:

1. `(topic, seq)` monotonic apply discipline -> `(stream_id, seq)` monotonic apply discipline.
2. Replay-first then live-tail semantics.
3. Duplicate-safe at-least-once delivery handling.
4. Explicit stale cursor handling and client rebootstrap path.
5. Deterministic payload hash vectors for replay verification.

## 7) OpenAgents Workstreams

### A) Governance and contracts

1. Add superseding ADR: SpacetimeDB sync authority replacing Spacetime transport ADR scope.
2. Update `docs/plans/rust-migration-invariant-gates.md` to replace Spacetime-specific transport gates with Spacetime sync gates.
3. Add/extend proto contracts for sync envelopes, checkpoints, errors, and compatibility negotiation.

### B) Runtime/control integration

1. Control service issues short-lived Spacetime websocket claims bound to user/device/session scope.
2. Runtime service publishes authoritative projection events into Spacetime-compatible reducer APIs.
3. Runtime removes dependency on Spacetime-specific fanout after cutover.

### C) Desktop and shared crates

1. Add `crates/autopilot-spacetime` typed client.
2. Desktop adopts Spacetime connection manager, subscription manager, checkpoint persistence.
3. Desktop retains local Codex-first command execution and uses Spacetime for shared sync/discovery.

### D) NIP-90/Nostr interoperability

1. Keep provider operations and marketplace signaling mirrored to Nostr where policy requires.
2. Ensure mirror failure cannot block core Spacetime state progression.

### E) Deletion and retirement

1. Remove Spacetime protocol mapping docs and runbooks after cutover acceptance.
2. Remove runtime Spacetime endpoints and tests once Spacetime parity gates are green.
3. Remove control-issued Spacetime token paths.

## 8) Execution Phases

### Phase 0: Governance unblocking

1. Land ADR supersession for sync transport ownership.
2. Update invariant gates for Spacetime sync semantics.
3. Approve cutover acceptance criteria and rollback runbook (`docs/sync/SPACETIME_CUTOVER_ACCEPTANCE_AND_ROLLBACK.md`).

Gate:

1. ADR/invariant docs merged.

### Phase 1: Contract and client foundation

1. Land proto envelope/checkpoint/error schemas.
2. Build `crates/autopilot-spacetime` client with deterministic codec and replay helpers.
3. Add dual-stack test harness (Spacetime baseline vs Spacetime candidate outputs).

Gate:

1. Replay/idempotency parity tests green.

### Phase 2: Spacetime world and reducers

1. Implement schema/reducers/subscription query sets.
2. Add confirmed-read mode on streams requiring durable visibility guarantees.
3. Add metrics for sequence lag, replay gap, fanout latency.

Gate:

1. Stale/resume/duplicate chaos tests pass.

### Phase 3: Desktop cut-in

1. Switch desktop sync transport to Spacetime by default (feature flag first).
2. Keep local Codex orchestration first-class.
3. Keep Nostr interoperability lanes supplemental.

Gate:

1. Multi-device sync acceptance tests pass with Spacetime-only mode.

### Phase 4: Runtime/control cutover

1. Move runtime projection feed and worker/event sync lanes to Spacetime.
2. Disable Spacetime token minting and WS subscription paths in staged cohorts.
3. Validate compatibility negotiation and client upgrade UX.

Gate:

1. No unresolved replay correctness regressions in staging soak.

### Phase 5: Spacetime retirement

1. Delete Spacetime runtime fanout/replay endpoints.
2. Remove Spacetime protocol docs and fixtures from active authority set.
3. Archive retired materials to backroom.

Gate:

1. Two release cycles with zero Spacetime dependency in retained surfaces.

## 9) Verification Matrix

1. Deterministic replay parity across restart/snapshot boundaries.
2. Idempotent duplicate-frame apply tests.
3. Resume/stale-cursor recovery tests.
4. Durability-gated delivery tests for confirmed-read streams.
5. Cross-surface compatibility negotiation tests.
6. Nostr mirror failure isolation tests.

## 10) Success Criteria

1. `apps/autopilot-desktop` and retained clients rely on Spacetime for live sync/replay.
2. Spacetime endpoints are removed from runtime/control production paths.
3. Sync SLOs meet or exceed pre-cutover baseline.
4. Hydra/Aegis correctness and receipt integrity are unchanged.

## 11) Docs That Must Stay Aligned During This Program

1. `docs/core/ARCHITECTURE.md`
2. `docs/core/ROADMAP.md`
3. `docs/core/PROJECT_OVERVIEW.md`
4. `docs/plans/rust-migration-invariant-gates.md`
5. `docs/sync/README.md`
6. `docs/sync/ROADMAP.md`
7. `docs/protocol/SPACETIME_SYNC_TRANSPORT_MAPPING.md`
