# Khala Sync Thoughts (WS-First, Proto-First)

Date: 2026-02-20
Status: Proposed (ready to ADR)
Scope: Replace Convex for runtime/Codex projection delivery first; migrate Lightning second.

Khala is the codename for the OpenAgents sync engine: shared consciousness / collective signal.

## TL;DR

Build Khala as a Postgres-backed, WebSocket-delivered sync plane owned by runtime.

Non-negotiables:

1. Runtime + Postgres remain authority.
2. Khala is projection + delivery only (no second authority store).
3. New sync transport is WS only (Phoenix Channels), not SSE.
4. Contracts are proto-first; no hand-authored TypeScript schema authority.
5. Do not clone Convex protocol compatibility.
6. Khala v1 ships inside `apps/openagents-runtime` and shares its Postgres.
7. Khala uses durable DB-backed resume (watermarks + replay).
8. Stream journal is ordering-first; payload authority lives in read models (pointer mode supported).

## Why this exists

Convex currently covers multiple concerns. Khala splits them cleanly:

1. Authority: runtime event log + policy/spend state in Postgres.
2. Projection: deterministic read models derived from authority.
3. Delivery: resumable push updates to clients.
4. Query: explicit read-model fetch/list endpoints (no arbitrary server-side functions).

This aligns with `docs/ARCHITECTURE.md`, `docs/adr/ADR-0029-convex-sync-layer-and-codex-agent-mode.md`, and `apps/openagents-runtime/docs/CONVEX_SYNC.md`.

## Current status snapshot (codebase as of 2026-02-20)

### Runtime lane

Implemented and strong:

- Projector/checkpoint/replay stack exists:
  - `apps/openagents-runtime/lib/openagents_runtime/convex/projector.ex`
  - `apps/openagents-runtime/lib/openagents_runtime/convex/reprojection.ex`
  - `apps/openagents-runtime/lib/openagents_runtime/convex/projection_checkpoint.ex`
  - `apps/openagents-runtime/priv/repo/migrations/20260219101000_create_runtime_convex_projection_checkpoints.exs`
- Projection triggers are integrated into run and worker event writes:
  - `apps/openagents-runtime/lib/openagents_runtime/runs/run_events.ex`
  - `apps/openagents-runtime/lib/openagents_runtime/codex/workers.ex`
- Runtime/Convex boundary is documented as projection-only.

Gap:

- Default projection sink is still `OpenAgentsRuntime.Convex.NoopSink` in `apps/openagents-runtime/config/config.exs`.

### Web/mobile/desktop lanes

- Laravel mints Convex token bridge via `/api/convex/token`:
  - `apps/openagents.com/app/Support/Convex/ConvexTokenIssuer.php`
- Mobile Khala lane is implemented behind `EXPO_PUBLIC_KHALA_SYNC_ENABLED` and Convex provider boot was removed:
  - `apps/mobile/app/app.tsx`
- Mobile Codex admin data plane is already runtime API-driven:
  - `apps/mobile/app/screens/CodexWorkersScreen.tsx`
- Desktop runtime task flow is already Laravel API-driven:
  - `apps/desktop/src/effect/taskProvider.ts`
- Desktop Khala lane can run without Convex URL/token:
  - `apps/desktop/src/lib/khalaConfig.ts`

### Lightning lane

- Lightning control-plane schema authority is now proto-first:
  - `proto/openagents/lightning/v1/control_plane.proto`
- Postgres-backed authority APIs now exist in Laravel:
  - `apps/openagents.com/app/Http/Controllers/Api/Internal/LightningOpsControlPlaneController.php`
  - `apps/openagents.com/app/Services/L402/L402OpsControlPlaneService.php`
- `apps/lightning-ops` supports API-backed transport in addition to Convex rollback mode:
  - `apps/lightning-ops/src/controlPlane/apiTransport.ts`
  - `apps/lightning-ops/src/controlPlane/convex.ts`

Conclusion: runtime/Codex Khala lane is live behind flags; Lightning second-wave migration is underway with API parity scaffolding merged.

## WS-only transport decision

Decision:

- Khala uses WebSockets only for live subscriptions (Phoenix Channels).
- No new SSE endpoints are added for Khala.
- Existing SSE endpoints remain for existing runtime APIs until clients migrate.

Reason:

- multi-topic multiplexing,
- cleaner resume protocol over stateful sockets,
- native fit for BEAM/Phoenix,
- avoids carrying two new live transports.

## Target architecture

### Authority plane (unchanged)

- Runtime writes canonical events/state in Postgres.

### Projection plane (runtime-owned)

- Runtime projector derives read models from authority events.
- Read models persist in runtime-owned Postgres tables.

### Delivery plane (Khala, runtime-owned)

- Runtime emits projection update journal entries with topic watermarks.
- Khala WS service pushes updates to subscribers.
- Resume uses durable watermark replay from Postgres, not in-memory-only signaling.

## Protocol shape (proto-first)

Define proto package:

- `proto/openagents/sync/v1/*.proto`

Core messages (minimal v1):

1. `Subscribe`
- `topics[]`
- `resume_after` map (topic -> watermark)

2. `Subscribed`
- `subscription_id`
- `current_watermarks` map

3. `Update`
- `topic`
- `doc_key`
- `doc_version`
- `payload` (proto bytes or canonical JSON bytes)
- `watermark`

4. `Heartbeat`
- `watermarks` map

5. `Error`
- `code`
- `message`
- `retry_after_ms`

### Topic strategy (locked for v1)

Topics are coarse, per model class:

- `runtime.run_summaries`
- `runtime.codex_worker_summaries`
- optional later: `runtime.notifications`

No per-tenant topics in v1.

Tenant scoping is enforced via:

- JWT topic scopes + org membership,
- doc-level access filtering (`doc_key` ownership constraints).

### Query strategy (locked for v1)

Khala does not expose arbitrary query execution.

Client fetch patterns are explicit:

- `GET /sync/v1/doc/:doc_key` for initial hydration.
- optional `GET /sync/v1/list/:collection?...` for bounded lists.

## Data model for resume correctness

### Durable sync stream journal

Table: `runtime.sync_stream_events`

Purpose:

- ordering journal for replay,
- durable resume across WS disconnects/restarts.

Columns:

- `topic` text
- `watermark` bigint
- `doc_key` text
- `doc_version` bigint
- `payload` jsonb or bytea (optional inline mode)
- `payload_hash` bytea (recommended)
- `inserted_at` timestamptz

Indexes:

- unique `(topic, watermark)`
- index `(topic, watermark)` for replay scans
- index `(topic, doc_key, watermark desc)`
- index `(inserted_at)` for retention

### Read-model tables

- `runtime.sync_run_summaries`
- `runtime.sync_codex_worker_summaries`

Each row keeps at least:

- `doc_key` primary key
- `doc_version` bigint
- `payload` jsonb or bytea
- `payload_hash` bytea (recommended)
- `updated_at` timestamptz

### Pointer mode (locked requirement)

Stream events are ordering-first. Payload authority lives in read-model tables.

- v1 may inline payload in `sync_stream_events.payload` for simplicity.
- Khala must support pointer mode where stream rows do not store full payload and delivery resolves from read-model tables.
- WS protocol does not change between inline and pointer modes.

### Retention rule

- Keep bounded history in `sync_stream_events` (24h-7d by env).
- If resume watermark is older than retained window:
  - return deterministic `stale_cursor`,
  - require full resync fetch + watermark reset.

## Watermark allocation (locked, DB-native)

Khala uses monotonic per-topic sequences in Postgres with single-row update.

Table: `runtime.sync_topic_sequences`

Columns:

- `topic` text primary key
- `next_watermark` bigint

Allocation algorithm (transactional):

```sql
UPDATE runtime.sync_topic_sequences
SET next_watermark = next_watermark + 1
WHERE topic = $1
RETURNING next_watermark;
```

Properties:

- monotonic per topic,
- no duplicates under concurrency,
- no node-local allocator state.

## Server design (runtime-owned)

Khala v1 ships inside `apps/openagents-runtime` for transactional correctness.

Components:

1. `OpenAgentsRuntime.Sync.ProjectorSink`
- upserts read-model rows,
- allocates watermark,
- appends stream event,
- emits telemetry,
- publishes wakeup via PubSub (optimization only).

2. `OpenAgentsRuntime.Sync.WatermarkAllocator`
- DB-backed allocator using `runtime.sync_topic_sequences`.

3. `OpenAgentsRuntime.Sync.Channel`
- authenticated socket join,
- subscribe/unsubscribe handling,
- replay catch-up then live updates.

4. `OpenAgentsRuntime.Sync.Replay`
- reads rows `> watermark` per topic,
- paginates bounded batches,
- supports pointer-mode payload resolution.

5. `OpenAgentsRuntime.Sync.RetentionJob`
- deletes expired stream rows,
- tracks oldest retained watermark per topic.

### Replay batching limits (locked for v1)

- max updates per batch: `200`
- max payload bytes per update: `256KB` (configurable)
- max total batch payload: `2MB` (configurable)

Behavior:

- replay in bounded batches until head watermark, then live mode,
- deterministic handling for oversize payloads (`payload_too_large` or fetch-required fallback).

## Delivery flow

1. Runtime writes authority event.
2. Runtime projector computes summary/doc update.
3. Khala sink upserts read-model row + allocates watermark + appends stream event + emits telemetry.
4. Khala broadcasts wakeup (optimization).
5. Channel replays from subscriber watermark to head, then streams live updates.

## Failure model

- DB write succeeds, WS push fails: safe; replay catches up.
- WS node restart: safe; resume by watermark.
- PubSub loss/drop: safe; DB replay remains delivery authority.
- Duplicate delivery can happen across reconnects; client must apply idempotently by `(topic, watermark)` or `(doc_key, doc_version)`.

## Auth model (Laravel -> Khala)

Laravel remains auth/session authority and mints Khala tokens.

JWT claims (v1):

- `sub`
- `oa_org_id`
- `oa_sync_scopes`
- `exp` (short TTL)
- `jti`
- `oa_claims_version`

JWT header (required):

- `kid`

Verification:

- MVP may use HS256,
- path to RS256 + JWKS must remain non-breaking,
- enforce signature + claims + topic scope + doc ownership.

## Canonical hashing and parity

Khala should attach deterministic `payload_hash` for:

- read-model payloads,
- stream payloads when inlined,
- parity checks against Convex during dual publish.

Hashing rules:

- proto payloads: hash proto bytes,
- JSON payloads: hash canonical JSON encoding (single shared rule across languages, via ADR extension).

## Client model

### Client responsibilities

1. Open WS and authenticate.
2. Subscribe to explicit topics.
3. Maintain per-topic high watermark.
4. Persist watermark locally.
5. Maintain in-memory doc cache keyed by `doc_key`.
6. Apply updates idempotently:
- ignore out-of-order `doc_version` regressions,
- accept monotonic `doc_version` increases.
7. On `stale_cursor`, full-fetch and reset watermark.

### Watermark persistence per surface

- Web: localStorage/IndexedDB
- Mobile: AsyncStorage/SQLite
- Desktop: SQLite

## Initial surfaces

1. Web Codex admin summary views.
2. Mobile Codex workers.
3. Desktop status surfaces still needing reactive summaries.

Note: control/action APIs remain HTTP; Khala is read-sync transport.

## Migration strategy

### Phase A: dual publish (runtime/Codex)

- keep Convex projection path where enabled,
- add Khala sink in parallel,
- run parity checks (payload hash/version/lag),
- gate by feature flags.

### Phase B: client cutover to Khala WS

- web/mobile/desktop switch subscription lane,
- keep action/control APIs unchanged,
- remove Convex client deps after stability window.

Fast win:

- remove mobile Convex provider initialization for flagged users as soon as Khala covers needed reactive data.

### Phase C: Lightning migration

- move lightning control-plane authority from Convex to Postgres/runtime APIs,
- replace `ConvexHttpClient` in lightning-ops,
- decommission Convex by lane after rollback window.

## What not to build

1. No arbitrary function execution in sync plane.
2. No generic reactive query language.
3. No Convex wire-compat emulation.
4. No second authority store.
5. No per-tenant topic explosion in v1.

## Operational requirements

Metrics:

- per-topic lag,
- replay batch duration p50/p95,
- reconnect rate,
- stale cursor rate,
- dropped/retried pushes,
- catch-up duration p95,
- stream event table size per topic + oldest retained watermark.

Alerts:

- lag SLO breach,
- replay error spikes,
- watermark allocation failures,
- retention job failures/retention stall.

Runbooks:

- full resync procedure,
- retention tuning,
- reprojection + re-emit,
- Postgres failover behavior.

## Verification matrix

Unit:

- watermark monotonic allocation,
- stale cursor behavior,
- auth checks,
- hashing determinism.

Integration:

- disconnect/reconnect without gaps,
- dual publish parity (Convex vs Khala),
- retention purge + stale cursor recovery,
- duplicate delivery idempotency.

Load/chaos:

- burst throughput,
- WS node restart mid-stream,
- Postgres failover/retry,
- replay under backpressure bounded memory.

## Open questions to settle early

1. Topic partitioning is locked for v1: per model class only.
2. Watermark source is locked for v1: per-topic DB sequence rows.
3. Payload storage is locked requirement: pointer mode supported, inline optional.
4. Token endpoint path recommendation: add `/api/sync/token`; keep `/api/convex/token` during migration.

## Recommended next artifacts

1. `docs/adr/ADR-0030-khala-sync-runtime-owned-ws-proto-first.md`
2. `proto/openagents/sync/v1/*.proto`
3. `docs/protocol/OA_SYNC_WS_MAPPING.md`
4. `docs/sync/SURFACES.md`
5. `docs/sync/ROADMAP.md`
