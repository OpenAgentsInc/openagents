# Khala Sync Thoughts (Rust-Era, WS-Only, Proto-First)

Date: 2026-02-21  
Status: Doctrine lock (active)

Authority ADRs:
- `docs/adr/ADR-0003-khala-ws-only-replay-transport.md`
- `docs/adr/ADR-0002-proto-first-contract-governance.md`

Khala is the runtime-owned sync/replay delivery subsystem.

## Locked Decisions

1. Runtime + `runtime.*` Postgres remain authority for execution state.
2. Khala is delivery/replay infrastructure only; it never performs authority writes.
3. Live sync transport is WebSocket-only.
4. Commands/mutations are HTTP-only.
5. Contracts are proto-first under `proto/openagents/sync/v1/`.
6. Replay ordering is defined by monotonic per-topic sequence (`seq`).
7. Clients persist watermarks and apply updates idempotently by `(topic, seq)`.
8. `stale_cursor` requires HTTP snapshot/bootstrap before live tail resume.

## Why Khala Exists

Khala separates concerns cleanly:

1. Authority writes: runtime events and canonical state updates.
2. Projection: deterministic read-model materialization.
3. Delivery: resumable replay + live fanout over WS.
4. Bootstrap: explicit HTTP read endpoints for initial hydration and stale-cursor recovery.

This keeps sync transport from becoming an implicit authority API.

## Rust-Era Consumer Set

Active sync consumers:
- `apps/openagents.com/web-shell`
- `apps/autopilot-desktop`
- `apps/autopilot-ios`
- `apps/onyx` (limited scope)

Removed legacy surfaces (historical only):
- `apps/mobile/` (removed)
- `apps/desktop/` (removed)
- `apps/inbox-autopilot/` (removed)

## Runtime Placement

Khala runs inside `apps/runtime` in v1.

Reasoning:
- projector write and stream append stay in one transactional boundary,
- replay correctness remains operationally simple,
- delivery drift is easier to detect and remediate.

## Data Plane Model

Core runtime tables:
- `runtime.sync_topic_sequences`
- `runtime.sync_stream_events`
- runtime read-model tables (for summaries, worker state, notifications)

Required behaviors:
- transaction-safe sequence allocation,
- replay scan by topic + sequence,
- retention horizon with deterministic stale-cursor boundary,
- integrity checks via payload hash and sequence monotonicity.

## Auth Model

- Control service mints scoped sync tokens via `POST /api/sync/token`.
- Runtime validates signature/claims/scopes before topic subscribe.
- Topic ownership and scope mismatches are denied deterministically.

## Delivery Semantics

1. Client bootstrap via HTTP read model.
2. Client subscribes with resume watermark.
3. Runtime replays gap (if available) then switches to live tail.
4. Duplicate frames are dropped client-side when `seq <= last_applied`.
5. On stale cursor, client resets watermark and rehydrates via HTTP.

## Operational Focus

Critical operating signals:
- auth denial rate,
- stale-cursor rate,
- replay bootstrap latency,
- reconnect storm metrics,
- slow-consumer evictions.

Runbook authority:
- `docs/sync/RUNTIME_CODEX_CUTOVER_RUNBOOK.md`

## Canonical References

- `docs/sync/SURFACES.md`
- `docs/sync/ROADMAP.md`
- `docs/ARCHITECTURE-RUST.md`
- `apps/runtime/docs/RUNTIME_CONTRACT.md`
