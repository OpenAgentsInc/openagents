Below is a **drop-in rewrite** of your doc with the extra decisions + clarifications fully spelled out, plus a tightened roadmap addendum. I kept your structure and tone, but “locked” the parts that would otherwise turn into ambiguity later.

---

# OA Sync Thoughts (WS-First, Proto-First)

Date: 2026-02-20
Status: Proposed (ready to ADR)
Scope: Replace Convex for runtime/Codex projection delivery first; migrate Lightning second.

## TL;DR

Build OpenAgents Sync (OA Sync) as a Postgres-backed, WebSocket-delivered sync plane **owned by runtime**.

Non-negotiables:

1. Runtime + Postgres remain authority.
2. OA Sync is projection + delivery only (no second authority store).
3. New sync transport is **WS only** (Phoenix Channels), not SSE.
4. Contracts are **proto-first**; no hand-authored TypeScript schema authority.
5. Do not clone Convex protocol compatibility.

Additionally locked (v1):

6. OA Sync v1 ships **inside** `apps/openagents-runtime` and shares its Postgres for transactional correctness. It may be extracted later after semantics are proven.
7. OA Sync uses **durable DB-backed resume** (watermarks + replay) so WS node restarts and transient push failures never lose updates.
8. The stream journal is primarily an **ordering log**; payload authority lives in read models. Stream rows may inline payload in v1 but must support “pointer mode” without protocol change.

---

## Why this exists

Convex currently covers multiple concerns. OA Sync must split them cleanly:

1. **Authority**: runtime event log + policy/spend state in Postgres.
2. **Projection**: deterministic read models derived from authority.
3. **Delivery**: resumable push updates to clients with reconnect/resume correctness.
4. **Query**: explicit read-model fetch/list endpoints (no arbitrary server-side functions).

This matches OpenAgents doctrine from `docs/ARCHITECTURE.md`, `docs/adr/ADR-0029-convex-sync-layer-and-codex-agent-mode.md`, and `apps/openagents-runtime/docs/CONVEX_SYNC.md`.

---

## Current status snapshot (codebase as of 2026-02-20)

### Runtime lane

Implemented and strong:

* Projector/checkpoint/replay stack exists:

  * `apps/openagents-runtime/lib/openagents_runtime/convex/projector.ex`
  * `apps/openagents-runtime/lib/openagents_runtime/convex/reprojection.ex`
  * `apps/openagents-runtime/lib/openagents_runtime/convex/projection_checkpoint.ex`
  * `apps/openagents-runtime/priv/repo/migrations/20260219101000_create_runtime_convex_projection_checkpoints.exs`
* Projection triggers are integrated into run and worker event writes:

  * `apps/openagents-runtime/lib/openagents_runtime/runs/run_events.ex`
  * `apps/openagents-runtime/lib/openagents_runtime/codex/workers.ex`
* Runtime/Convex boundary is documented as projection-only.

Gap:

* Default projection sink remains `NoopSink` in `apps/openagents-runtime/config/config.exs`.

### Web/mobile/desktop lanes

* Laravel mints Convex token bridge via `/api/convex/token`:

  * `apps/openagents.com/app/Support/Convex/ConvexTokenIssuer.php`
* Mobile still boots Convex client/provider:

  * `apps/mobile/app/app.tsx`
* Mobile Codex admin data plane is already runtime API-driven (list/snapshot/stream/request/stop), not Convex queries:

  * `apps/mobile/app/screens/CodexWorkersScreen.tsx`
* Desktop runtime task flow is already Laravel API-driven:

  * `apps/desktop/src/effect/taskProvider.ts`

### Lightning lane

* `apps/lightning-ops` remains directly Convex-dependent for control-plane reads/writes:

  * `apps/lightning-ops/src/controlPlane/convexTransport.ts`
  * `apps/lightning-ops/src/controlPlane/convex.ts`

Conclusion: runtime/Codex is ready for OA Sync first; Lightning migrates second.

---

## WS-only transport decision

Decision:

* OA Sync uses **WebSockets only** for live subscriptions (Phoenix Channels).
* No new SSE endpoints are added for OA Sync.
* Existing SSE endpoints remain for existing runtime APIs until clients migrate.

Reason:

* multi-topic multiplexing,
* cleaner resume protocol over a stateful socket,
* native fit for BEAM/Phoenix,
* avoids carrying two “new” live transports for the sync plane.

---

## Target architecture

### Authority plane (unchanged)

* Runtime writes canonical events and state in Postgres.

### Projection plane (runtime-owned)

* Runtime projector derives read models from authority events.
* Read models are persisted in runtime-owned Postgres tables.

### Delivery plane (OA Sync, runtime-owned)

* Runtime emits projection update journal entries with topic watermarks.
* OA Sync WS service pushes updates to subscribed clients.
* Resume uses durable watermark replay from Postgres (not in-memory PubSub).

---

## Protocol shape (proto-first)

Define new proto package:

* `proto/openagents/sync/v1/*.proto`

Core messages (minimal v1):

1. `Subscribe`

   * `topics[]`
   * `resume_after` (map: topic -> watermark)

2. `Subscribed`

   * `subscription_id`
   * `current_watermarks` (map: topic -> watermark)

3. `Update`

   * `topic`
   * `doc_key`
   * `doc_version`
   * `payload` (bytes: proto bytes OR proto-compatible JSON bytes)
   * `watermark` (monotonic per topic)

4. `Heartbeat`

   * `watermarks` (map: topic -> watermark)

5. `Error`

   * `code` (enum)
   * `message`
   * `retry_after_ms`

### Topic strategy (locked for v1)

Topics are **coarse, per model class**:

* `runtime.run_summaries`
* `runtime.codex_worker_summaries`
* optional later: `runtime.notifications`

We do **not** create per-tenant topics in v1.

Tenant scoping is enforced by:

* JWT topic scope claims + org membership
* doc-level access filtering (doc_key ownership constraints)

### Query strategy (locked for v1)

OA Sync does not expose arbitrary query execution in the sync protocol.

Client fetch patterns are explicit:

* `GET /sync/v1/doc/:doc_key` for initial hydration
* optional `GET /sync/v1/list/:collection?...` for bounded lists (only where required)

---

## Data model for resume correctness

### Durable sync event journal (Postgres)

Table: `runtime.sync_stream_events`

Purpose:

* An ordering journal for resumable replay.
* Enables correctness under WS failures and node restarts.

Columns:

* `topic` text
* `watermark` bigint
* `doc_key` text
* `doc_version` bigint
* `payload` jsonb (or bytea) **optional** (see “pointer mode” below)
* `payload_hash` bytea (optional but recommended)
* `inserted_at` timestamptz

Indexes:

* unique `(topic, watermark)`
* index `(topic, watermark)` for replay scans
* index `(topic, doc_key, watermark desc)` for debugging and per-doc traces
* index `(inserted_at)` for retention jobs

### Read-model tables (runtime-owned)

* `runtime.sync_run_summaries`
* `runtime.sync_codex_worker_summaries`

Each row keeps at least:

* `doc_key` (primary key)
* `doc_version` bigint
* `payload` jsonb (or bytea)
* `payload_hash` bytea (optional but recommended)
* `updated_at` timestamptz

### Pointer mode (locked requirement)

Stream events are primarily an ordering journal. Payload authority lives in read models.

* v1 may inline payload in `sync_stream_events.payload` for simplicity.
* OA Sync must also support pointer mode where stream events do **not** store full payload, and the server joins/fetches payload from the read-model tables when delivering an `Update`.

The WS protocol does not change between inline vs pointer mode.

### Retention rule (locked semantics)

* Keep bounded history in `sync_stream_events` (e.g., 24h–7d depending on env).
* If client resume watermark is older than retained window:

  * return deterministic `stale_cursor` error code
  * client must perform full resync fetch and reset watermark.

---

## Watermark allocation (locked, DB-backed)

OA Sync uses monotonic per-topic sequences allocated in Postgres with a single-row update.

Table: `runtime.sync_topic_sequences`

Columns:

* `topic` text primary key
* `next_watermark` bigint (starts at 0 or 1)

Allocation algorithm (transactional):

* `UPDATE runtime.sync_topic_sequences
   SET next_watermark = next_watermark + 1
   WHERE topic = $topic
   RETURNING next_watermark;`

Properties:

* monotonic per topic
* no duplicates under concurrency
* no dependence on node-local state
* safe across restarts and multi-node deployments

---

## Server design (BEAM, runtime-owned)

OA Sync v1 ships inside `apps/openagents-runtime` for transactional correctness.

### Components

1. `OA.Sync.ProjectorSink` (runtime integration point)

   * Called on projection updates.
   * Upserts read-model table row.
   * Appends `sync_stream_events` row with newly allocated watermark.
   * Emits telemetry.
   * Broadcasts lightweight wakeup via PubSub (optimization only).

2. `OA.Sync.WatermarkAllocator`

   * DB-backed allocator using `runtime.sync_topic_sequences`.

3. `OA.Sync.Channel` (Phoenix)

   * Authenticated socket join.
   * Handles subscribe/unsubscribe messages.
   * Delivers historical catch-up from `sync_stream_events` then live updates.

4. `OA.Sync.Replay`

   * Reads events `> watermark` per topic.
   * Enforces bounded batching to avoid unbounded memory/latency.
   * Supports pointer mode payload retrieval.

5. `OA.Sync.RetentionJob`

   * Deletes expired stream rows by retention policy.
   * Produces metrics for “oldest retained watermark” per topic.

### Replay batching limits (locked for v1)

To prevent unbounded memory growth and to preserve interactivity:

* Maximum updates per batch: **200**
* Maximum payload bytes per update: **256KB** (configurable)
* Maximum total batch payload: **2MB** (configurable)

Server behavior:

* On replay, server sends batches until caught up to head watermark.
* Then switches to live mode.
* If payload exceeds limits, server may:

  * truncate payload and require doc fetch
  * or send a `payload_too_large` error code with guidance

(Exact behavior should be deterministic and documented in mapping doc.)

---

## Delivery flow

1. Runtime writes authority event.
2. Runtime projector computes summary/doc update.
3. OA Sync sink:

   * upserts read-model row,
   * allocates watermark,
   * appends stream event,
   * emits telemetry.
4. OA Sync broadcasts topic wakeup (optimization).
5. Channel:

   * replays from subscriber watermark to head,
   * then streams live updates.

---

## Failure model (locked semantics)

* DB write succeeds, WS push fails: safe; client catches up by watermark replay.
* WS node restart: safe; subscriptions recover by resume watermark.
* PubSub drop/loss: safe; DB-sourced replay is authoritative for delivery correctness.
* Duplicate delivery: possible across reconnects; clients must handle idempotently by `(topic, watermark)` or `(doc_key, doc_version)` monotonic rules.

---

## Auth model (Laravel -> OA Sync)

Laravel remains auth/session authority. It mints OA Sync tokens (not Convex tokens).

JWT claims (v1):

* `sub` (user principal)
* `oa_org_id` (tenant/workspace scope)
* `oa_sync_scopes` (allowed topics)
* `exp` (short TTL)
* `jti`
* `oa_claims_version`

JWT header (required):

* `kid` (key id) for rotation

Verification:

* MVP can be HS256 for fast path.
* Target RS256/JWKS for key rotation and multi-service trust.
* Even in HS256 MVP, `kid` should be present so rotation path is non-breaking later.

Channel enforcement:

* validate signature + claims
* enforce topic scopes at subscribe time
* enforce doc-level ownership constraints (e.g. doc_key belongs to org)

---

## Canonical hashing and parity (locked approach)

OA Sync will attach deterministic hashes for:

* read-model payloads (`payload_hash`)
* stream event payloads (`payload_hash` when inlined)
* parity auditor comparisons (Convex vs OA Sync)

Hashing must be canonical and stable:

* If payload is proto bytes: hash proto bytes directly.
* If payload is JSON: hash canonical JSON encoding (field ordering + normalization), defined once and reused everywhere (preferably via ADR-0006 extension for OA Sync).

Parity auditing uses these canonical rules; otherwise it becomes noise.

---

## Client model

### Client responsibilities (locked)

1. Open WS and authenticate.
2. Subscribe to explicit topics.
3. Maintain per-topic high watermark.
4. Persist watermark locally for reconnect (per surface choice).
5. Maintain an in-memory doc cache keyed by `doc_key`.
6. Apply updates idempotently:

   * ignore out-of-order doc_version regressions
   * accept monotonic doc_version increases
7. On `stale_cursor`, perform full fetch of required docs/lists and reset watermark.

### Watermark persistence per surface (suggested)

* Web: localStorage/IndexedDB
* Mobile: AsyncStorage/SQLite
* Desktop: SQLite

---

## Initial surfaces

1. Web admin Codex views (summary badges and lists).
2. Mobile Codex workers (replace residual Convex usage).
3. Desktop status surfaces that rely on reactive summaries.

Note: existing runtime control/action APIs remain HTTP endpoints; OA Sync is read-sync transport.

---

## Migration strategy (hybrid)

### Phase A: Runtime/Codex dual publish

* Keep current Convex projection path (if enabled in deployed envs).
* Add OA Sync sink alongside existing sink path.
* Run parity checks for doc payload/version and lag drift.
* Gate by feature flags in clients.

### Phase B: Client cutover to OA Sync WS

* Web/mobile/desktop switch subscription lane to OA Sync.
* Keep runtime action/control APIs unchanged.
* Remove Convex client deps after stability window.

Fast win:

* Mobile currently boots Convex client mostly for posture; once OA Sync is in place, remove Convex provider initialization for flagged users early.

### Phase C: Lightning control plane migration

* Move lightning control-plane authority data from Convex to Postgres/runtime-owned APIs.
* Replace `ConvexHttpClient` in lightning-ops with OA API transport.
* Decommission Convex usage by lane after rollback window.

---

## What not to build (locked)

1. No arbitrary server-side function execution in sync plane.
2. No generic reactive query language.
3. No attempt to emulate Convex sync protocol byte-for-byte.
4. No second authority store.
5. No per-tenant topic explosion in v1.

---

## Operational requirements

### Metrics (minimum)

* per-topic lag (head watermark - client watermark; server-side estimates)
* replay batch duration (p50/p95)
* WS reconnect rate
* stale cursor rate
* dropped/retried pushes
* **catch-up duration p95** (time from reconnect to caught up)
* stream events table size per topic (and oldest retained watermark)

### Alerts (minimum)

* lag SLO breaches
* replay error spikes
* watermark allocation failures
* retention job failures / oldest watermark stalling

### Runbooks

* full-resync response procedure
* retention tuning
* reprojection + OA Sync re-emit procedures
* failover behavior expectations for Postgres

---

## Verification matrix (minimum)

### Unit

* watermark monotonic allocation
* stale cursor behavior
* authorization join checks
* canonical hashing determinism

### Integration

* disconnect/reconnect with no gaps
* dual-publish parity (Convex vs OA Sync payload hash)
* retention purge + stale cursor recovery
* duplicate delivery idempotency (client behavior)

### Load/chaos

* burst update throughput
* WS node restart mid-stream
* Postgres failover/retry behavior
* replay under backpressure (bounded memory)

---

## Open questions to settle early (narrowed)

1. Topic partitioning granularity:

   * Locked for v1: per model class topics only. (No per-tenant topics.)
2. Watermark source:

   * Locked for v1: per-topic sequences in Postgres (`sync_topic_sequences`).
3. Payload storage:

   * Locked requirement: pointer mode supported; v1 may inline but must be swappable without protocol changes.
4. Token endpoint shape:

   * Recommended: add `/api/sync/token` and keep `/api/convex/token` during migration.
   * Do not overload Convex naming with OA Sync semantics.

---

## Recommended next artifacts

1. `docs/adr/ADR-00XX-oa-sync.md` (locks boundaries + WS-only decision)
2. `proto/openagents/sync/v1/*.proto`
3. `docs/protocol/OA_SYNC_WS_MAPPING.md` (proto-to-wire mapping + batch limits + error taxonomy)
4. `docs/sync/SURFACES.md` (which UI surfaces subscribe to which topics and doc_keys)
5. `docs/sync/ROADMAP.md` (issue-ready plan)

---

---

# OA Sync Roadmap (WS-Only)

Date: 2026-02-20
Status: Proposed
Owner lanes: Runtime, Web, Mobile, Desktop, Lightning, Infra

## Goal

Replace Convex for runtime/Codex sync with an OpenAgents-owned sync engine on Postgres + WebSockets, while avoiding a big-bang cutover for Lightning.

## Constraints

1. Runtime/Postgres remain authority.
2. OA Sync is projection + delivery only.
3. New sync transport is WebSocket only (Phoenix Channels), not SSE.
4. Proto definitions are schema authority.
5. Runtime/Codex migrates first; Lightning migrates second.
6. Watermarks are DB-backed per-topic sequences.
7. Stream journal supports pointer mode payload delivery.

## Current status baseline (factored into this plan)

1. Runtime projector/checkpoint/reprojection stack already exists.
2. Runtime default sink is still `NoopSink` in repo config.
3. Laravel Convex token bridge exists at `/api/convex/token`.
4. Mobile still initializes Convex client, but Codex data path already uses runtime APIs.
5. Desktop task flow already uses Laravel APIs for lightning tasks.
6. Lightning ops remains directly dependent on Convex function transport.

---

## Epic 1: Contracts and Architecture Lock

### OA-SYNC-001: Add OA Sync ADR

Depends on: none

Scope:

* Add ADR defining OA Sync boundaries, WS-only decision, and migration lanes.
* Lock DB-backed per-topic watermark allocator and pointer-mode requirement.
* Reference ADR-0028 and ADR-0029.

Done when:

* ADR merged under `docs/adr/` and linked from sync docs.

Verification:

* docs lint/check links.

### OA-SYNC-002: Create proto package `openagents.sync.v1`

Depends on: OA-SYNC-001

Scope:

* Add proto files for:

  * subscribe/subscribed
  * update/heartbeat/error
  * topic and error enums
  * watermark and doc-version fields
* Include `stale_cursor` and `payload_too_large` error codes.

Done when:

* Proto package compiles and passes Buf lint/breaking checks.

Verification:

* `buf lint`
* `buf breaking --against '.git#branch=main,subdir=proto'`
* `./scripts/verify-proto-generate.sh`

### OA-SYNC-003: Add protocol mapping doc for WS wire format

Depends on: OA-SYNC-002

Scope:

* Add `docs/protocol/OA_SYNC_WS_MAPPING.md`:

  * channel name(s)
  * event names
  * JSON mapping (or binary mapping)
  * replay batching limits
  * pointer-mode behavior (payload inline vs fetched)
  * error taxonomy and retry guidance

Done when:

* Mapping doc merged and cross-linked.

Verification:

* docs link checks.

### OA-SYNC-003A: Define canonical payload hashing rules

Depends on: OA-SYNC-002

Scope:

* Extend ADR-0006 (or add a small ADR) defining canonical JSON encoding for OA Sync payload hashing.
* Add fixtures proving stable hashes across languages (TS + Elixir at minimum).

Done when:

* Hash fixtures are in repo and tests pass.

Verification:

* `mix test` + `pnpm test` (or equivalent) for fixtures.

### OA-SYNC-003B: Add `docs/sync/SURFACES.md`

Depends on: OA-SYNC-001

Scope:

* Document which surfaces subscribe to which topics and doc_keys.
* Document required initial fetch endpoints per surface.

Done when:

* Surfaces mapping merged and referenced by roadmap.

Verification:

* docs link checks.

---

## Epic 2: Runtime Data Model and Sink

### OA-SYNC-004: Add sync stream event tables and indexes

Depends on: OA-SYNC-001

Scope:

* Add migrations for:

  * `runtime.sync_stream_events`
  * `runtime.sync_run_summaries`
  * `runtime.sync_codex_worker_summaries`
* Add indexes for `(topic, watermark)` replay scans.
* Add retention-friendly index on `inserted_at`.

Done when:

* Migrations apply cleanly in dev/test.
* Schema documented.

Verification:

* `cd apps/openagents-runtime && mix ecto.migrate`
* `cd apps/openagents-runtime && mix test`

### OA-SYNC-004A: Add per-topic sequence table

Depends on: OA-SYNC-004

Scope:

* Add migration for `runtime.sync_topic_sequences`.
* Initialize known topics.

Done when:

* Allocator uses single-row update and passes concurrency tests.

Verification:

* `mix test --only sync_watermarks`

### OA-SYNC-005: Implement watermark allocator

Depends on: OA-SYNC-004A

Scope:

* Implement allocator that uses `sync_topic_sequences` update+returning within transaction.

Done when:

* Unit tests prove monotonicity and no duplicates under concurrency.

Verification:

* `cd apps/openagents-runtime && mix test --only sync`

### OA-SYNC-006: Implement `OA.Sync.ProjectorSink`

Depends on: OA-SYNC-004, OA-SYNC-005

Scope:

* Add runtime sink that:

  * upserts sync read-model rows,
  * appends stream event with watermark,
  * emits telemetry.
* Keep existing convex sink path available during dual publish.

Done when:

* Sink configurable in runtime env and produces durable stream rows.
* Pointer mode supported (can run with payload omitted in stream events).

Verification:

* runtime integration tests with deterministic fixtures.

### OA-SYNC-007: Add retention policy job for stream events

Depends on: OA-SYNC-004

Scope:

* Periodic deletion by retention horizon.
* Track oldest retained watermark per topic.

Done when:

* Retention job tested and observable via telemetry/logs.

Verification:

* runtime tests for stale cursor boundary.

---

## Epic 3: WebSocket Service (Phoenix Channels)

### OA-SYNC-008: Add `sync:v1` channel + authenticated join

Depends on: OA-SYNC-002, OA-SYNC-006

Scope:

* Add Channel module with:

  * socket auth
  * topic entitlement checks
  * subscribe command

Done when:

* Authenticated client can subscribe to allowed topics only.

Verification:

* channel authorization tests.

### OA-SYNC-009: Implement replay-on-subscribe with watermark resume

Depends on: OA-SYNC-008

Scope:

* On subscribe:

  * read stream rows after supplied watermark,
  * paginate in bounded batches,
  * send catch-up updates,
  * begin live push mode.

Done when:

* reconnect/resume tests show no gaps under disconnect/reconnect.

Verification:

* integration tests with forced socket drop.

### OA-SYNC-010: Implement stale cursor handling

Depends on: OA-SYNC-009, OA-SYNC-007

Scope:

* Detect resume watermark older than retained window.
* Return explicit error code requiring full resync.

Done when:

* stale cursor tests pass.

Verification:

* integration tests with retention purge simulation.

### OA-SYNC-011: Add heartbeat and connection health telemetry

Depends on: OA-SYNC-008

Scope:

* heartbeat events and server-side timeout handling.
* metrics for connection count, reconnect count, lag, and catch-up duration.

Done when:

* dashboards/alerts can track sync health.

Verification:

* telemetry tests + metrics docs update.

---

## Epic 4: Auth Bridge (Laravel -> OA Sync)

### OA-SYNC-012: Add `/api/sync/token` token issuer

Depends on: OA-SYNC-001

Scope:

* Add Laravel endpoint to mint OA Sync JWT claims with `kid`.
* Keep `/api/convex/token` unchanged during migration.

Done when:

* endpoint covered by feature tests for success/failure/expiry.

Verification:

* `cd apps/openagents.com && php artisan test --filter=SyncToken`

### OA-SYNC-013: Add runtime socket JWT verification

Depends on: OA-SYNC-012, OA-SYNC-008

Scope:

* Verify token signature and required claims.
* Enforce topic scopes at join/subscription time.

Done when:

* unauthorized scopes are denied with deterministic errors.

Verification:

* runtime socket auth tests.

---

## Epic 5: Client SDK and Surface Integrations

### OA-SYNC-014: Create TS OA Sync client package (web/mobile/desktop)

Depends on: OA-SYNC-002, OA-SYNC-009

Scope:

* Implement minimal client:

  * connect/auth
  * subscribe
  * watermark persistence
  * auto-resume
  * stale-cursor callback for full fetch
  * doc cache keyed by `doc_key`

Done when:

* package consumed by one internal harness and one product surface.

Verification:

* package unit tests + runtime integration harness.

### OA-SYNC-015: Integrate web Codex admin with OA Sync behind flag

Depends on: OA-SYNC-014

Scope:

* Replace Convex summary badges in web admin page with OA Sync topic updates.
* Keep runtime API actions unchanged.

Done when:

* feature flag can switch web admin summary lane between old/new.

Verification:

* web integration tests + staging smoke.

### OA-SYNC-016: Integrate mobile Codex worker screen with OA Sync behind flag

Depends on: OA-SYNC-014

Scope:

* Use OA Sync for summary updates.
* Remove Convex provider boot for flagged users once OA Sync is active for needed data.

Done when:

* mobile screen receives live summary updates without Convex client for flagged users.

Verification:

* `cd apps/mobile && bun run compile && bun run test`

### OA-SYNC-017: Integrate desktop status surfaces with OA Sync behind flag

Depends on: OA-SYNC-014

Scope:

* Remove Convex reachability/dependency in desktop sync surfaces where applicable.
* Keep lightning task APIs unchanged.

Done when:

* desktop runs without requiring Convex URL/token for sync lane.

Verification:

* `cd apps/desktop && npm run typecheck && npm test`

---

## Epic 6: Dual Publish, Parity, and Cutover

### OA-SYNC-018: Implement dual-publish parity auditor

Depends on: OA-SYNC-006, OA-SYNC-003A, OA-SYNC-015

Scope:

* Compare Convex summary payload hash vs OA Sync payload hash for sampled entities.
* Emit mismatch metrics and logs.

Done when:

* parity dashboard exists with mismatch rate tracking.

Verification:

* runtime load/chaos tests include parity checks.

### OA-SYNC-019: Runtime/Codex cutover runbook

Depends on: OA-SYNC-018

Scope:

* Document staged rollout:

  * internal users
  * limited cohort
  * full cohort
* Include rollback switch and expected SLOs.

Done when:

* runbook merged and exercised in staging.

Verification:

* staging drill report artifact committed.

### OA-SYNC-020: Remove Convex client dependency from migrated surfaces

Depends on: OA-SYNC-019

Scope:

* Remove Convex providers/config from migrated web/mobile/desktop sync paths.
* Keep backward compatibility only where still required by non-migrated lanes.

Done when:

* migrated surfaces no longer require Convex URL/token.

Verification:

* surface-specific CI checks all green.

---

## Epic 7: Lightning Control Plane Migration (Second Wave)

### OA-SYNC-021: Define proto contracts for lightning control-plane read/write models

Depends on: OA-SYNC-002

Scope:

* Move lightning control-plane schema authority from TS-only contracts to proto.
* Add mappings/adapters in `apps/lightning-ops`.

Done when:

* proto contracts and adapters merged, backward-compatible with current behavior.

Verification:

* proto checks + lightning typecheck/tests.

### OA-SYNC-022: Build runtime/Laravel APIs backing lightning control-plane state in Postgres

Depends on: OA-SYNC-021

Scope:

* Add authoritative tables/APIs for paywall policy, security controls, settlements.

Done when:

* API endpoints provide parity with current Convex-backed flows.

Verification:

* lightning smoke suites pass in API-backed mode.

### OA-SYNC-023: Replace `ConvexHttpClient` transport in lightning-ops

Depends on: OA-SYNC-022

Scope:

* Implement transport adapter from lightning-ops to OA APIs.
* Keep mode flag for rollback to Convex path during bake-in.

Done when:

* `apps/lightning-ops` runs all reconcile/smoke flows in API mode.

Verification:

* `npm test`
* `npm run smoke:compile -- --json`
* `npm run smoke:security -- --json`
* `npm run smoke:settlement -- --json`
* `npm run smoke:full-flow -- --json`

### OA-SYNC-024: Lightning migration cutover + Convex decommission checklist

Depends on: OA-SYNC-023

Scope:

* Execute staged cutover of lightning control plane.
* Remove remaining Convex runtime dependencies/envs/runbooks after rollback window.

Done when:

* production cutover complete, rollback drill evidence captured, decommission checklist signed.

Verification:

* production/staging drill reports committed.

---

## Suggested GitHub project columns

1. Backlog
2. Ready
3. In Progress
4. In Review
5. Verified in Staging
6. Done

## Suggested labels

* `oa-sync`
* `oa-sync-runtime`
* `oa-sync-web`
* `oa-sync-mobile`
* `oa-sync-desktop`
* `oa-sync-lightning`
* `oa-sync-proto`
* `oa-sync-infra`
* `migration-risk-high`
* `migration-risk-medium`
* `migration-risk-low`

## Immediate next 5 issues to open

1. OA-SYNC-001 (ADR lock)
2. OA-SYNC-002 (proto package)
3. OA-SYNC-004 (DB schema)
4. OA-SYNC-004A (topic sequences table)
5. OA-SYNC-006 (projector sink)

---

If you want, next I can also generate the actual **proto file stubs** (`openagents/sync/v1/sync.proto`, `topics.proto`, `errors.proto`) and the **OA_SYNC_WS_MAPPING.md** content so OA-SYNC-002 and OA-SYNC-003 are basically ready-to-merge text.
