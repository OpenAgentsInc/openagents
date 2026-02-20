# Khala Sync Roadmap (WS-Only)

Date: 2026-02-20
Status: Proposed
Owner lanes: Runtime, Web, Mobile, Desktop, Lightning, Infra

Khala is the codename for the OpenAgents sync engine.

## Goal

Replace Convex for runtime/Codex sync with a runtime-owned sync engine on Postgres + WebSockets, while avoiding a big-bang Lightning cutover.

## Constraints

1. Runtime/Postgres remain authority.
2. Khala is projection + delivery only.
3. New sync transport is WS only (Phoenix Channels), not SSE.
4. Proto definitions are schema authority.
5. Runtime/Codex migrates first; Lightning migrates second.
6. Watermarks are DB-backed per-topic sequences.
7. Stream journal supports pointer mode payload delivery.

## Current baseline (factored into this plan)

1. Runtime projector/checkpoint/reprojection stack already exists.
2. Runtime default projection sink is still `NoopSink` in config.
3. Laravel Convex token bridge exists at `/api/convex/token`.
4. Mobile still initializes Convex client, but Codex data path is already runtime API-driven.
5. Desktop task flow already uses Laravel APIs for Lightning tasks.
6. `apps/lightning-ops` still depends on Convex transport for control-plane data.

## Epic 1: Contracts and architecture lock

## KHALA-001: Add Khala ADR

Depends on: none

Scope:

- Add ADR locking Khala boundaries, WS-only decision, migration lanes.
- Lock DB-backed per-topic watermark allocator and pointer-mode requirement.
- Reference ADR-0028 and ADR-0029.

Done when:

- ADR merged under `docs/adr/` and linked from sync docs.

Verification:

- docs link checks.

## KHALA-002: Create proto package `openagents.sync.v1`

Depends on: KHALA-001

Scope:

- Add proto files for:
  - subscribe/subscribed
  - update/heartbeat/error
  - topic and error enums
  - watermark/doc-version fields
- Include `stale_cursor` and `payload_too_large` error codes.

Done when:

- Proto package compiles and passes Buf lint/breaking checks.

Verification:

- `buf lint`
- `buf breaking --against '.git#branch=main,subdir=proto'`
- `./scripts/verify-proto-generate.sh`

## KHALA-003: Add WS mapping doc for proto-to-wire format

Depends on: KHALA-002

Scope:

- Add `docs/protocol/OA_SYNC_WS_MAPPING.md` with:
  - channel names,
  - event names,
  - payload mapping,
  - replay batch limits,
  - pointer-mode behavior,
  - error taxonomy + retry guidance.

Done when:

- Mapping doc merged and cross-linked from sync docs.

Verification:

- docs link checks.

## KHALA-004: Define canonical payload hashing rules

Depends on: KHALA-002

Scope:

- Extend ADR-0006 or add a focused ADR for canonical JSON hashing rules.
- Add fixtures proving stable hashes across TS + Elixir.

Done when:

- Hash fixtures are committed and language-level tests pass.

Verification:

- runtime + TS fixture tests green.

## KHALA-005: Add `docs/sync/SURFACES.md`

Depends on: KHALA-001

Scope:

- Document surface-to-topic mapping and required initial hydration endpoints.

Done when:

- Surfaces mapping merged and linked by roadmap.

Verification:

- docs link checks.

## Epic 2: Runtime data model and sink

## KHALA-006: Add sync stream/read-model tables and indexes

Depends on: KHALA-001

Scope:

- Add migrations for:
  - `runtime.sync_stream_events`
  - `runtime.sync_run_summaries`
  - `runtime.sync_codex_worker_summaries`
- Add replay and retention indexes.

Done when:

- Migrations apply cleanly in dev/test.
- Schema is documented.

Verification:

- `cd apps/openagents-runtime && mix ecto.migrate`
- `cd apps/openagents-runtime && mix test`

## KHALA-007: Add per-topic sequence table

Depends on: KHALA-006

Scope:

- Add migration for `runtime.sync_topic_sequences`.
- Seed known topics for v1.

Done when:

- Table is live and concurrency test fixtures exist.

Verification:

- `cd apps/openagents-runtime && mix test --only sync_watermarks`

## KHALA-008: Implement DB-native watermark allocator

Depends on: KHALA-007

Scope:

- Implement allocator using `UPDATE ... RETURNING` on `sync_topic_sequences` in transaction.

Done when:

- Tests prove monotonicity and no duplicates under concurrency.

Verification:

- `cd apps/openagents-runtime && mix test --only sync`

## KHALA-009: Implement `OpenAgentsRuntime.Sync.ProjectorSink`

Depends on: KHALA-006, KHALA-008

Scope:

- Upsert read-model rows.
- Append stream event with watermark.
- Emit telemetry.
- Keep Convex sink available during dual publish.
- Support pointer mode (payload optional in stream rows).

Done when:

- Sink is configurable and durable stream rows are produced.

Verification:

- runtime integration tests with deterministic fixtures.
- `mix runtime.contract.check` remains green.

## KHALA-010: Add stream retention job + stale boundary metrics

Depends on: KHALA-006

Scope:

- Add retention horizon deletion job.
- Track oldest retained watermark per topic.

Done when:

- Retention behavior is tested and observable.

Verification:

- runtime stale-cursor boundary tests.

## Epic 3: WebSocket delivery service (Phoenix)

## KHALA-011: Add `sync:v1` channel with authenticated join

Depends on: KHALA-002, KHALA-009

Scope:

- Add channel with socket auth, entitlement checks, subscribe command.

Done when:

- Authenticated clients can subscribe only to allowed topics.

Verification:

- channel authorization tests.

## KHALA-012: Implement replay-on-subscribe with watermark resume

Depends on: KHALA-011

Scope:

- Read events after supplied watermark.
- Page replay in bounded batches.
- Enter live mode after catch-up.

Done when:

- reconnect/resume tests show no gaps under disconnect/reconnect.

Verification:

- integration tests with forced socket drops.

## KHALA-013: Implement stale cursor handling

Depends on: KHALA-010, KHALA-012

Scope:

- Detect resume watermark older than retention horizon.
- Return deterministic `stale_cursor` response.

Done when:

- stale cursor tests pass.

Verification:

- integration tests with retention purge simulation.

## KHALA-014: Add heartbeat + connection health telemetry

Depends on: KHALA-011

Scope:

- Add heartbeat events + timeout handling.
- Add metrics for connections, reconnects, lag, catch-up duration.

Done when:

- dashboards/alerts can track sync health.

Verification:

- telemetry tests + metrics docs update.

## Epic 4: Auth bridge (Laravel -> runtime/Khala)

## KHALA-015: Add `/api/sync/token` token issuer

Depends on: KHALA-001

Scope:

- Add Laravel endpoint minting sync JWT claims with `kid` header.
- Keep `/api/convex/token` for migration period.

Done when:

- endpoint is feature-tested for success/failure/expiry.

Verification:

- `cd apps/openagents.com && php artisan test --filter=SyncToken`

## KHALA-016: Add runtime socket JWT verification + key rotation path

Depends on: KHALA-011, KHALA-015

Scope:

- Verify token signature and required claims.
- Enforce topic scopes and doc ownership.
- Support key-id (`kid`) based rotation path (HS256 now, RS256/JWKS-ready).

Done when:

- unauthorized scopes fail deterministically and rotation path is tested.

Verification:

- runtime socket auth tests.

## Epic 5: Client SDK and surface integrations

## KHALA-017: Create TS Khala client package (web/mobile/desktop)

Depends on: KHALA-002, KHALA-012

Scope:

- Implement client with connect/auth/subscribe.
- Persist watermarks and auto-resume.
- Expose stale-cursor callback.
- Maintain doc cache keyed by `doc_key` with monotonic `doc_version` application.

Done when:

- package is used by one harness and one product surface.

Verification:

- package tests + integration harness.

## KHALA-018: Integrate web Codex admin behind feature flag

Depends on: KHALA-017

Scope:

- Replace Convex summary subscription with Khala on selected screens.
- Keep runtime action APIs unchanged.

Done when:

- web flag switches between legacy and Khala subscription lanes.

Verification:

- web integration tests + staging smoke.

## KHALA-019: Integrate mobile Codex worker screen behind feature flag

Depends on: KHALA-017

Scope:

- Use Khala for summary updates.
- Remove Convex provider boot for flagged users once needed reactive data is covered.

Done when:

- mobile receives live summaries without Convex client for flagged users.

Verification:

- `cd apps/mobile && bun run compile && bun run test`

## KHALA-020: Integrate desktop status surfaces behind feature flag

Depends on: KHALA-017

Scope:

- Remove Convex reachability dependency in desktop sync surfaces where applicable.
- Keep Lightning task APIs unchanged.

Done when:

- desktop sync lane runs without Convex URL/token requirement.

Verification:

- `cd apps/desktop && npm run typecheck && npm test`

## Epic 6: Dual publish, parity, and runtime/Codex cutover

## KHALA-021: Implement dual-publish parity auditor

Depends on: KHALA-004, KHALA-009, KHALA-018

Scope:

- Compare Convex and Khala payload hashes for sampled entities.
- Emit mismatch metrics/logs and lag drift metrics.

Done when:

- parity dashboard exists with mismatch rate tracking.
- dashboard definition lives in `docs/sync/PARITY_DASHBOARD.md`.

Verification:

- runtime load/chaos tests include parity checks.

## KHALA-022: Create runtime/Codex cutover runbook

Depends on: KHALA-021

Scope:

- Document staged rollout (internal, cohort, full).
- Include rollback switch, SLOs, and drill checklist.

Done when:

- runbook merged and exercised in staging.
- runbook path: `docs/sync/RUNTIME_CODEX_CUTOVER_RUNBOOK.md`.

Verification:

- staging drill report committed.
- drill artifact path: `docs/sync/status/2026-02-20-khala-runtime-codex-staging-drill.md`.

## KHALA-023: Remove Convex client dependency from migrated surfaces

Depends on: KHALA-022

Scope:

- Remove Convex providers/config from migrated web/mobile/desktop paths.
- Keep compatibility only for non-migrated lanes.

Done when:

- migrated surfaces no longer require Convex URL/token.

Verification:

- surface-specific CI checks all green.

## Epic 7: Lightning control-plane migration (second wave)

## KHALA-024: Define proto contracts for Lightning control-plane models

Depends on: KHALA-002

Scope:

- Move lightning control-plane schema authority from TS-only to proto.
- Add adapters in `apps/lightning-ops`.

Done when:

- proto contracts and adapters merged with behavior parity.

Verification:

- proto checks + lightning typecheck/tests.

## KHALA-025: Build runtime/Laravel APIs for Lightning control-plane state

Depends on: KHALA-024

Scope:

- Add Postgres-backed authority tables/APIs for paywall policy, security controls, settlements.

Done when:

- API parity with current Convex-backed flows is validated.

Verification:

- Lightning smoke suites pass in API-backed mode.

## KHALA-026: Replace Convex transport in `apps/lightning-ops`

Depends on: KHALA-025

Scope:

- Replace `ConvexHttpClient` transport with OA API transport adapter.
- Keep rollback flag during bake-in.

Done when:

- lightning-ops reconcile/smoke flows pass in API mode.

Verification:

- `cd apps/lightning-ops && npm test`
- `cd apps/lightning-ops && npm run smoke:compile -- --json`
- `cd apps/lightning-ops && npm run smoke:security -- --json`
- `cd apps/lightning-ops && npm run smoke:settlement -- --json`
- `cd apps/lightning-ops && npm run smoke:full-flow -- --json`

## KHALA-027: Lightning cutover and Convex decommission

Depends on: KHALA-026

Scope:

- Execute staged Lightning cutover.
- Remove remaining Convex dependencies/env/runbooks after rollback window.

Done when:

- production cutover complete and rollback drill evidence captured.

Verification:

- staging/production drill artifacts committed.

## Suggested GitHub project columns

1. Backlog
2. Ready
3. In Progress
4. In Review
5. Verified in Staging
6. Done

## Suggested labels

- `khala-sync`
- `khala-runtime`
- `khala-web`
- `khala-mobile`
- `khala-desktop`
- `khala-lightning`
- `khala-proto`
- `khala-infra`
- `migration-risk-high`
- `migration-risk-medium`
- `migration-risk-low`

## Immediate next 5 issues to open

1. KHALA-001 (ADR lock)
2. KHALA-002 (proto package)
3. KHALA-006 (DB schema)
4. KHALA-007 (topic sequence table)
5. KHALA-009 (projector sink)
