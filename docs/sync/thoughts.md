# OA Sync Thoughts (WS-First, Proto-First)

Date: 2026-02-20

## TL;DR

Build OpenAgents Sync (OA Sync) as a Postgres-backed, WebSocket-delivered sync plane owned by runtime.

Non-negotiables:

1. Runtime + Postgres remain authority.
2. OA Sync is projection + delivery only.
3. New sync transport is WS only (Phoenix Channels), not SSE.
4. Contracts are proto-first; no hand-authored TypeScript schema authority.
5. Do not clone Convex protocol compatibility.

## Why this exists

Convex currently covers multiple concerns. OA Sync should split them cleanly:

1. Authority: runtime event log + policy/spend state in Postgres.
2. Projection: deterministic read models.
3. Delivery: resumable push updates to clients.
4. Query: explicit read-model fetch/list endpoints.

This matches OpenAgents doctrine from `docs/ARCHITECTURE.md`, `docs/adr/ADR-0029-convex-sync-layer-and-codex-agent-mode.md`, and `apps/openagents-runtime/docs/CONVEX_SYNC.md`.

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

- Default projection sink remains `NoopSink` in `apps/openagents-runtime/config/config.exs`.

### Web/mobile/desktop lanes

- Laravel mints Convex token bridge via `/api/convex/token`:
  - `apps/openagents.com/app/Support/Convex/ConvexTokenIssuer.php`
- Mobile still boots Convex client/provider:
  - `apps/mobile/app/app.tsx`
- Mobile Codex admin data plane is already runtime API-driven (list/snapshot/stream/request/stop), not Convex queries:
  - `apps/mobile/app/screens/CodexWorkersScreen.tsx`
- Desktop runtime task flow is already Laravel API-driven:
  - `apps/desktop/src/effect/taskProvider.ts`

### Lightning lane

- `apps/lightning-ops` remains directly Convex-dependent for control-plane reads/writes:
  - `apps/lightning-ops/src/controlPlane/convexTransport.ts`
  - `apps/lightning-ops/src/controlPlane/convex.ts`

Conclusion: runtime/Codex is ready for OA Sync first; Lightning migrates second.

## WS-only transport decision

Decision:

- OA Sync uses WebSockets only for live subscriptions (Phoenix Channels).
- No new SSE endpoints are added for OA Sync.
- Existing SSE endpoints remain for existing runtime APIs until clients migrate.

Reason:

- multi-topic multiplexing,
- cleaner resume protocol over a stateful socket,
- native fit for BEAM/Phoenix,
- avoids carrying two live transports for the new sync plane.

## Target architecture

## Authority plane (unchanged)

- Runtime writes canonical events and state in Postgres.

## Projection plane (owned by runtime)

- Runtime projector derives read models from authority events.
- Read models are persisted in Postgres sync tables.

## Delivery plane (new OA Sync)

- Runtime emits projection events with topic watermarks.
- OA Sync WS service pushes updates to subscribers.
- Resume uses durable watermark replay from Postgres.

## Protocol shape (proto-first)

Define new proto package (suggestion):

- `proto/openagents/sync/v1/*.proto`

Core messages (minimal v1):

1. `Subscribe`:
- `topics[]`
- `resume_after` map (topic -> watermark)

2. `Subscribed`:
- `subscription_id`
- `current_watermarks`

3. `Update`:
- `topic`
- `doc_key`
- `doc_version`
- `payload` (proto bytes or proto-compatible JSON bytes)
- `watermark`

4. `Heartbeat`:
- `watermarks`

5. `Error`:
- `code`
- `message`
- `retry_after_ms`

Topic strategy:

- `runtime.run_summaries`
- `runtime.codex_worker_summaries`
- optional later: `runtime.notifications`

Do not expose arbitrary query execution in the sync protocol.

## Data model for resume correctness

Add durable sync event journal in Postgres:

Table: `runtime.sync_stream_events`

Columns:

- `topic` text
- `watermark` bigint
- `doc_key` text
- `doc_version` bigint
- `payload` jsonb (or bytea)
- `inserted_at` timestamptz

Indexes:

- unique `(topic, watermark)`
- index `(topic, doc_key, watermark desc)`
- index `(inserted_at)` for retention jobs

Read-model tables (runtime-owned):

- `runtime.sync_run_summaries`
- `runtime.sync_codex_worker_summaries`

Each row keeps at least:

- `doc_key`
- `doc_version`
- `payload`
- `updated_at`

Retention rule:

- Keep bounded history in `sync_stream_events` (for example 24h to 7d by env).
- If client resume watermark is older than retained window, return `stale_cursor` and require full resync fetch.

## Server design (BEAM)

## Components

1. `OA.Sync.ProjectorSink` (runtime integration point)
- Called on projection updates.
- Upserts read-model table.
- Appends `sync_stream_events` row with next watermark.
- Broadcasts lightweight wakeup via PubSub.

2. `OA.Sync.WatermarkAllocator`
- Monotonic per-topic sequence allocation.
- Backed by Postgres transaction.

3. `OA.Sync.Channel` (Phoenix)
- Authenticated socket join.
- Handles subscribe/unsubscribe.
- Delivers historical catch-up from `sync_stream_events` then live updates.

4. `OA.Sync.Replay`
- Reads events `> watermark` per topic.
- Paginates and pushes bounded batches.

5. `OA.Sync.RetentionJob`
- Deletes expired sync stream rows by retention policy.

## Delivery flow

1. Runtime writes authority event.
2. Runtime projector computes summary/doc update.
3. OA Sync sink writes read-model + stream event + watermark.
4. OA Sync broadcasts topic wakeup.
5. Channel processes replay from subscriber watermark and pushes updates.

## Failure model

- DB write succeeds, WS push fails: safe; client catches up by watermark replay.
- WS node restart: safe; subscriptions recover by resume watermark.
- PubSub drop/loss: safe; replay loop is DB-sourced.

## Auth model

Use Laravel as auth/session authority, but mint OA Sync tokens, not Convex tokens.

JWT claims (v1):

- `sub` (user principal)
- `oa_org_id` (or equivalent tenant/workspace scope)
- `oa_sync_scopes` (allowed topics)
- `exp` (short TTL)
- `jti`
- `oa_claims_version`

Verification:

- MVP can be HS256 for fast path.
- Target RS256/JWKS for key rotation and multi-service trust.

Channel join checks:

- validate topic entitlement per token scope and ownership constraints.

## Client model

## Client responsibilities

1. Open WS and authenticate.
2. Subscribe to explicit topics.
3. Maintain per-topic high watermark.
4. Persist watermark locally for reconnect.
5. On `stale_cursor`, perform full fetch and reset watermark.

## Initial surfaces

1. Web admin Codex views.
2. Mobile Codex workers.
3. Desktop status surfaces that still rely on reactive summaries.

Note: existing runtime control/action APIs remain HTTP endpoints; OA Sync is read-sync transport.

## Migration strategy (hybrid)

## Phase A: Runtime/Codex dual publish

- Keep current Convex projection path.
- Add OA Sync sink alongside existing sink path.
- Run parity checks for doc payload/version/watermark lag.
- Gate by feature flags in clients.

## Phase B: Client cutover to OA Sync WS

- Web/mobile/desktop switch subscription lane to OA Sync.
- Keep runtime action/control APIs unchanged.
- Remove Convex client deps after stability window.

## Phase C: Lightning control plane migration

- Move lightning control-plane authority data from Convex to Postgres/runtime-owned APIs.
- Replace `ConvexHttpClient` in lightning-ops with OA API transport.
- Decommission Convex usage by lane after rollback window.

## What not to build

1. No arbitrary server-side function execution in sync plane.
2. No generic reactive query language.
3. No attempt to emulate Convex sync protocol byte-for-byte.
4. No second authority store.

## Operational requirements

1. Metrics:
- per-topic lag
- replay batch duration
- ws reconnect rate
- stale cursor rate
- dropped/retried pushes

2. Alerts:
- lag SLO breaches
- replay error spikes
- watermark allocation failures

3. Runbooks:
- full-resync response procedure
- retention tuning
- backfill/reprojection with OA Sync enabled

## Verification matrix (minimum)

1. Unit:
- watermark monotonic allocation
- stale cursor behavior
- authorization join checks

2. Integration:
- disconnect/reconnect with no gaps
- dual-publish parity (Convex vs OA Sync payload hash)
- retention purge + stale cursor recovery

3. Load/chaos:
- burst update throughput
- ws node restart mid-stream
- Postgres failover/retry behavior

## Open questions to settle early

1. Topic partitioning granularity:
- one topic per model class, or per tenant + model.

2. Watermark source:
- per-topic sequences only, or global sequence + topic filters.

3. Payload storage:
- store full payload in stream events vs compact delta references.

4. Token endpoint shape:
- new `/api/sync/token` vs evolving `/api/convex/token` into generic sync token.

## Recommended next artifact

- `docs/sync/ROADMAP.md` as issue-ready implementation plan, sequenced by dependency and current repo state.
