# Khala Sync Thoughts (WS-Only, Proto-First)

Date: 2026-02-20  
Status: Proposed (doctrine lock)  
Scope: Runtime/Codex sync first, then remaining reactive surfaces.

Khala is the codename for the OpenAgents sync engine: shared consciousness / collective signal.

## TL;DR

Khala is a runtime-owned sync subsystem that runs on runtime Postgres and delivers read-model updates over WebSockets.

Locked v1 decisions:

1. Runtime + runtime Postgres remain authority for execution state.
2. Khala is delivery/replay infrastructure only, not an authority write path.
3. Khala transport is WS-only for live updates (no new SSE lane).
4. Contracts are proto-first under `proto/openagents/sync/v1/`.
5. Khala v1 ships inside `apps/openagents-runtime` (same deployable, same DB plane).
6. Watermarks are DB-native, monotonic per topic, allocated transactionally.
7. Stream journal is ordering-first; payload authority lives in read-model tables.
8. Pointer mode is mandatory: stream rows may omit full payload without protocol change.
9. Topics are coarse (model-class topics), not per-tenant topics.
10. Clients must persist per-topic watermark and apply doc updates idempotently.

## Why Khala Exists

We explicitly separate four concerns:

1. Authority writes: runtime event/state writes in runtime Postgres.
2. Projection: deterministic read models generated from authority events.
3. Delivery: resumable subscription transport for UI reactivity.
4. Query bootstrap: explicit HTTP endpoints for initial hydration and stale-cursor recovery.

This avoids coupling UI freshness to authority writes and avoids turning sync transport into an ad-hoc database API.

## Current Codebase Status (2026-02-20)

### Runtime lane

Implemented:

- Runtime projector/checkpoint/reprojection modules exist:
  - `apps/openagents-runtime/lib/openagents_runtime/khala/projector.ex`
  - `apps/openagents-runtime/lib/openagents_runtime/khala/reprojection.ex`
  - `apps/openagents-runtime/lib/openagents_runtime/khala/projection_checkpoint.ex`
- Projection checkpoints migration exists:
  - `apps/openagents-runtime/priv/repo/migrations/20260219101000_create_runtime_khala_projection_checkpoints.exs`
- Runtime event writes already trigger projection integration:
  - `apps/openagents-runtime/lib/openagents_runtime/runs/run_events.ex`
  - `apps/openagents-runtime/lib/openagents_runtime/codex/workers.ex`

Gap:

- Default sink in config remains noop for some environments:
  - `apps/openagents-runtime/config/config.exs`

### Web/mobile/desktop lanes

Implemented:

- Sync token endpoints exist in Laravel:
  - `POST /api/khala/token` (legacy-compatible)
  - `POST /api/sync/token` (primary)
- Token issuers/controllers:
  - `apps/openagents.com/app/Http/Controllers/Api/KhalaTokenController.php`
  - `apps/openagents.com/app/Http/Controllers/Api/SyncTokenController.php`
- Mobile/desktop/web have feature-gated Khala lanes.

### Lightning control-plane lane

Current posture:

- Lightning control-plane authority is API + Postgres in Laravel.
- `apps/lightning-ops` control-plane transport is API/mock oriented.
- Khala is not the primary transport for Lightning control-plane operations.

## Runtime-Owned Khala Architecture

### Subsystem placement

Khala v1 runs inside `apps/openagents-runtime`.

Reason:

- keeps projector write + stream append in one transactional boundary,
- avoids cross-service compensators for projection-vs-delivery divergence,
- preserves deterministic replay and simpler incident handling.

### Core runtime components

1. `OpenAgentsRuntime.Sync.ProjectorSink`
2. `OpenAgentsRuntime.Sync.WatermarkAllocator`
3. `OpenAgentsRuntime.Sync.Channel`
4. `OpenAgentsRuntime.Sync.Replay`
5. `OpenAgentsRuntime.Sync.RetentionJob`

## Data Model (Runtime Postgres)

### Topic sequence table

Table: `runtime.sync_topic_sequences`

- `topic text primary key`
- `next_watermark bigint not null`

Allocation:

```sql
UPDATE runtime.sync_topic_sequences
SET next_watermark = next_watermark + 1
WHERE topic = $1
RETURNING next_watermark;
```

### Stream journal table

Table: `runtime.sync_stream_events`

- `topic text`
- `watermark bigint`
- `doc_key text`
- `doc_version bigint`
- `payload jsonb/bytea` (optional inline)
- `payload_hash bytea` (recommended)
- `inserted_at timestamptz`

Required indexes:

- unique `(topic, watermark)`
- replay scan index `(topic, watermark)`
- per-doc trace index `(topic, doc_key, watermark desc)`
- retention index `(inserted_at)`

### Read-model tables

- `runtime.sync_run_summaries`
- `runtime.sync_codex_worker_summaries`

Minimum row fields:

- `doc_key` primary key
- `doc_version`
- `payload`
- `payload_hash`
- `updated_at`

### Retention and stale cursor

- Retain bounded journal window (env-specific horizon).
- If resume watermark is older than retained window, return `stale_cursor`.
- Client then performs full HTTP rehydrate and resets watermark.

## Protocol Shape (Proto-First)

Package:

- `proto/openagents/sync/v1/*.proto`

Core messages:

1. `Subscribe(topics[], resume_after)`
2. `Subscribed(subscription_id, current_watermarks)`
3. `Update(topic, doc_key, doc_version, payload, watermark)`
4. `Heartbeat(watermarks)`
5. `Error(code, message, retry_after_ms)`

## Topic Strategy (v1)

Coarse topics only:

- `runtime.run_summaries`
- `runtime.codex_worker_summaries`

Tenant scoping is enforced by token scopes and doc ownership checks at delivery time.

## Replay and Delivery Limits

Locked defaults (configurable):

- max updates per replay batch: `200`
- max payload per update: `256KB`
- max total payload per batch: `2MB`

Behavior:

- replay from watermark to head, then switch to live mode,
- use deterministic error/fallback for oversize payloads.

## Auth Model (Laravel -> Khala)

Laravel remains auth/session authority and mints short-lived sync JWTs.

Required claims/header shape:

- claims: `sub`, `exp`, `jti`, `oa_claims_version`, scoped org/topic claims
- header: `kid` for key rotation

Crypto posture:

- HS256 acceptable for MVP where operationally constrained,
- retain non-breaking path to RS256 + JWKS.

## Canonical Hashing

Khala parity and integrity checks rely on deterministic hashing.

Rules:

1. Proto payloads: hash canonical proto bytes.
2. JSON payloads: hash canonical JSON representation (single cross-language rule).

Hashes are used for:

- stream row integrity,
- read-model parity checks,
- cutover drift detection.

## Client Responsibilities

Every Khala client must:

1. Authenticate and subscribe to explicit topics.
2. Persist per-topic watermark.
3. Keep a local cache keyed by `doc_key`.
4. Apply updates idempotently using `doc_version` monotonicity.
5. Trigger full fetch + watermark reset on `stale_cursor`.

Persistence by surface:

- Web: localStorage or IndexedDB
- Mobile: AsyncStorage/SQLite/MMKV-backed persistence
- Desktop: SQLite

## Surfaces and Consumption

Detailed surface contract lives in `docs/sync/SURFACES.md`.

High-level:

- Web/mobile/desktop consume Khala for reactive runtime summaries behind flags.
- Autopilot iOS uses Khala WS as the primary live Codex event lane (`runtime.codex_worker_events`).
- Lightning control-plane does not depend on Khala for authority operations.

## Laravel Postgres vs Runtime Postgres in Khala Context

Khala only reads/writes runtime-owned sync/read-model tables in runtime Postgres.

- Laravel Postgres remains identity/control-plane authority.
- Runtime Postgres remains execution/sync authority.
- Cross-plane behavior happens through HTTP/internal API contracts, not direct cross-writes.

## Migration Strategy

### Phase A: Runtime dual path (stabilization)

- keep legacy reactive lane available,
- enable runtime Khala sink path,
- run parity and lag comparison metrics.

### Phase B: Client cutover

- flip web/mobile/desktop sync flags by staged cohorts,
- keep control/action APIs unchanged,
- hold rollback switches during bake window.

### Phase C: Cleanup and hardening

- remove legacy client-side sync wiring from migrated surfaces,
- tighten alert thresholds and retention policy,
- archive migration-specific runbooks once steady state is proven.

## Operational Requirements

Key metrics:

- per-topic lag,
- replay duration p50/p95,
- reconnect rate,
- stale cursor rate,
- catch-up duration p95,
- stream table size and oldest retained watermark per topic.

Key alerts:

- lag SLO breach,
- replay error spikes,
- watermark allocation failures,
- retention job failures.

## Verification Matrix

Unit tests:

- watermark monotonicity,
- auth/entitlement checks,
- stale-cursor handling,
- hashing determinism.

Integration tests:

- reconnect without event gaps,
- replay batching correctness,
- duplicate delivery idempotency,
- pointer-mode payload resolution.

Load/chaos tests:

- socket restart during catch-up,
- burst update throughput,
- Postgres failover/retry behavior,
- bounded-memory replay under backpressure.

## References

- `docs/ARCHITECTURE.md`
- `docs/sync/ROADMAP.md`
- `docs/sync/SURFACES.md`
- `apps/openagents-runtime/docs/KHALA_SYNC.md`
- `docs/adr/ADR-0030-khala-sync-runtime-owned-ws-proto-first.md`
