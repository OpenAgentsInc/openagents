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

## Current status baseline (factored into this plan)

1. Runtime projector/checkpoint/reprojection stack already exists.
2. Runtime default sink is still `NoopSink` in repo config.
3. Laravel Convex token bridge exists at `/api/convex/token`.
4. Mobile still initializes Convex client, but Codex data path already uses runtime APIs.
5. Desktop task flow already uses Laravel APIs for lightning tasks.
6. Lightning ops remains directly dependent on Convex function transport.

## Epic 1: Contracts and Architecture Lock

## Issue OA-SYNC-001: Add OA Sync ADR

Depends on: none

Scope:

- Add ADR defining OA Sync boundaries, WS-only decision, and migration lanes.
- Reference existing ADR-0028 and ADR-0029.

Done when:

- ADR merged under `docs/adr/` and linked from sync docs.

Verification:

- docs lint/check links.

## Issue OA-SYNC-002: Create proto package `openagents.sync.v1`

Depends on: OA-SYNC-001

Scope:

- Add proto files for:
  - subscribe/subscribed
  - update/heartbeat/error
  - topic and error enums
- Define watermark and doc-version fields explicitly.

Done when:

- Proto package compiles and passes Buf lint/breaking checks.

Verification:

- `buf lint`
- `buf breaking --against '.git#branch=main,subdir=proto'`
- `./scripts/verify-proto-generate.sh`

## Issue OA-SYNC-003: Add protocol mapping doc for WS wire format

Depends on: OA-SYNC-002

Scope:

- Add `docs/protocol/OA_SYNC_WS_MAPPING.md` with proto-to-JSON (or binary) mapping.
- Define channel event names and payload shape.

Done when:

- Mapping doc merged and cross-linked from `docs/sync/thoughts.md`.

Verification:

- docs link checks.

## Epic 2: Runtime Data Model and Sink

## Issue OA-SYNC-004: Add sync stream event tables and indexes

Depends on: OA-SYNC-001

Scope:

- Add migration(s) for:
  - `runtime.sync_stream_events`
  - `runtime.sync_run_summaries`
  - `runtime.sync_codex_worker_summaries`
- Add indexes for `(topic, watermark)` and replay scans.

Done when:

- Migrations apply cleanly in dev/test.
- Schema documented in `apps/openagents-runtime/docs/DB_SCHEMA.md`.

Verification:

- `cd apps/openagents-runtime && mix ecto.migrate`
- `cd apps/openagents-runtime && mix test`

## Issue OA-SYNC-005: Implement watermark allocator

Depends on: OA-SYNC-004

Scope:

- Add monotonic per-topic watermark allocator with transaction safety.

Done when:

- Unit tests prove monotonicity and no duplicates under concurrency.

Verification:

- `cd apps/openagents-runtime && mix test --only sync`

## Issue OA-SYNC-006: Implement `OA.Sync.ProjectorSink`

Depends on: OA-SYNC-004, OA-SYNC-005

Scope:

- Add runtime sink that:
  - upserts sync read-model rows,
  - appends stream event with watermark,
  - emits telemetry.
- Keep existing convex sink path available during dual publish.

Done when:

- Sink can be configured in runtime env and produces durable stream rows.

Verification:

- runtime integration tests with deterministic fixture events.
- `mix runtime.contract.check` remains green.

## Issue OA-SYNC-007: Add retention policy job for stream events

Depends on: OA-SYNC-004

Scope:

- Periodic deletion by retention horizon.
- Config knobs by env.

Done when:

- Retention job tested and observable via telemetry/logs.

Verification:

- runtime tests for stale cursor boundary.

## Epic 3: WebSocket Service (Phoenix Channels)

## Issue OA-SYNC-008: Add `sync:v1` channel + authenticated join

Depends on: OA-SYNC-002, OA-SYNC-006

Scope:

- Add Channel module with:
  - socket auth
  - topic entitlement checks
  - subscribe command

Done when:

- Authenticated client can subscribe to allowed topics only.

Verification:

- channel authorization tests.

## Issue OA-SYNC-009: Implement replay-on-subscribe with watermark resume

Depends on: OA-SYNC-008

Scope:

- On subscribe:
  - read stream rows after supplied watermark,
  - send catch-up updates,
  - begin live push mode.

Done when:

- reconnect/resume tests show no gaps under disconnect/reconnect.

Verification:

- integration tests with forced socket drop.

## Issue OA-SYNC-010: Implement stale cursor handling

Depends on: OA-SYNC-009, OA-SYNC-007

Scope:

- Detect resume watermark older than retained window.
- Return explicit error code requiring full resync.

Done when:

- stale cursor test passes.

Verification:

- integration tests with retention purge simulation.

## Issue OA-SYNC-011: Add heartbeat and connection health telemetry

Depends on: OA-SYNC-008

Scope:

- heartbeat events and server-side timeout handling.
- metrics for connection count, reconnect count, lag.

Done when:

- dashboards/alerts can track sync health.

Verification:

- runtime telemetry tests + metrics docs update.

## Epic 4: Auth Bridge (Laravel -> OA Sync)

## Issue OA-SYNC-012: Add `/api/sync/token` token issuer

Depends on: OA-SYNC-001

Scope:

- Add Laravel endpoint to mint OA Sync JWT claims.
- Keep `/api/convex/token` unchanged during migration.

Done when:

- endpoint covered by feature tests for success/failure/expiry.

Verification:

- `cd apps/openagents.com && php artisan test --filter=SyncToken`

## Issue OA-SYNC-013: Add runtime socket JWT verification

Depends on: OA-SYNC-012, OA-SYNC-008

Scope:

- Verify token signature and required claims.
- Enforce topic scopes at join/subscription time.

Done when:

- unauthorized scopes are denied with deterministic errors.

Verification:

- runtime socket auth tests.

## Epic 5: Client SDK and Surface Integrations

## Issue OA-SYNC-014: Create TS OA Sync client package (web/mobile/desktop)

Depends on: OA-SYNC-002, OA-SYNC-009

Scope:

- Implement minimal client:
  - connect/auth
  - subscribe
  - watermark persistence
  - auto-resume
  - stale-cursor callback for full fetch

Done when:

- package consumed by one internal test harness and one product surface.

Verification:

- package unit tests + runtime integration harness.

## Issue OA-SYNC-015: Integrate web Codex admin with OA Sync behind flag

Depends on: OA-SYNC-014

Scope:

- Replace Convex summary badges in web admin page with OA Sync topic updates.
- Keep existing runtime API actions unchanged.

Done when:

- feature flag can switch web admin summary lane between old/new.

Verification:

- web integration tests + manual staging smoke.

## Issue OA-SYNC-016: Integrate mobile Codex worker screen with OA Sync behind flag

Depends on: OA-SYNC-014

Scope:

- Use OA Sync for summary updates.
- Keep runtime API control paths unchanged.

Done when:

- mobile screen receives live summary updates without Convex client for flagged users.

Verification:

- `cd apps/mobile && bun run compile && bun run test`

## Issue OA-SYNC-017: Integrate desktop status surfaces with OA Sync behind flag

Depends on: OA-SYNC-014

Scope:

- Replace Convex reachability/dependency in desktop sync surfaces where applicable.
- Keep lightning task APIs unchanged.

Done when:

- desktop runs without requiring Convex URL for sync lane.

Verification:

- `cd apps/desktop && npm run typecheck && npm test`

## Epic 6: Dual Publish, Parity, and Cutover

## Issue OA-SYNC-018: Implement dual-publish parity auditor

Depends on: OA-SYNC-006, OA-SYNC-015

Scope:

- Compare Convex summary payload hash vs OA Sync payload hash for sampled entities.
- Emit mismatch metrics and logs.

Done when:

- parity dashboard exists with mismatch rate tracking.

Verification:

- runtime load/chaos tests include parity checks.

## Issue OA-SYNC-019: Runtime/Codex cutover runbook

Depends on: OA-SYNC-018

Scope:

- Document staged rollout:
  - internal users
  - limited cohort
  - full cohort
- Include rollback switch and expected SLOs.

Done when:

- runbook merged and exercised in staging.

Verification:

- staging drill report artifact committed.

## Issue OA-SYNC-020: Remove Convex client dependency from migrated surfaces

Depends on: OA-SYNC-019

Scope:

- Remove Convex providers/config from migrated web/mobile/desktop sync paths.
- Keep backward compatibility only where still required by non-migrated lanes.

Done when:

- migrated surfaces no longer require Convex URL/token.

Verification:

- surface-specific CI checks all green.

## Epic 7: Lightning Control Plane Migration (Second Wave)

## Issue OA-SYNC-021: Define proto contracts for lightning control-plane read/write models

Depends on: OA-SYNC-002

Scope:

- Move lightning control-plane schema authority from TS-only contracts to proto.
- Add mappings/adapters in `apps/lightning-ops`.

Done when:

- proto contracts and adapters merged, backward-compatible with current behavior.

Verification:

- proto checks + lightning typecheck/tests.

## Issue OA-SYNC-022: Build runtime/Laravel APIs backing lightning control-plane state in Postgres

Depends on: OA-SYNC-021

Scope:

- Add authoritative tables/APIs for paywall policy, security controls, settlements.

Done when:

- API endpoints provide parity with current Convex-backed flows.

Verification:

- lightning smoke suites pass in API-backed mode.

## Issue OA-SYNC-023: Replace `ConvexHttpClient` transport in lightning-ops

Depends on: OA-SYNC-022

Scope:

- Implement transport adapter from lightning-ops to OA APIs.
- Keep mode flag for rollback to Convex path during bake-in.

Done when:

- `apps/lightning-ops` runs all reconcile/smoke flows in API mode.

Verification:

- `npm test`
- `npm run smoke:compile -- --json`
- `npm run smoke:security -- --json`
- `npm run smoke:settlement -- --json`
- `npm run smoke:full-flow -- --json`

## Issue OA-SYNC-024: Lightning migration cutover + Convex decommission checklist

Depends on: OA-SYNC-023

Scope:

- Execute staged cutover of lightning control plane.
- Remove remaining Convex runtime dependencies/envs/runbooks after rollback window.

Done when:

- production cutover complete, rollback drill evidence captured, decommission checklist signed.

Verification:

- production/staging drill reports committed.

## Suggested GitHub project columns

1. Backlog
2. Ready
3. In Progress
4. In Review
5. Verified in Staging
6. Done

## Suggested labels

- `oa-sync`
- `oa-sync-runtime`
- `oa-sync-web`
- `oa-sync-mobile`
- `oa-sync-desktop`
- `oa-sync-lightning`
- `oa-sync-proto`
- `oa-sync-infra`
- `migration-risk-high`
- `migration-risk-medium`
- `migration-risk-low`

## Immediate next 5 issues to open

1. OA-SYNC-001 (ADR lock)
2. OA-SYNC-002 (proto package)
3. OA-SYNC-004 (DB schema)
4. OA-SYNC-006 (projector sink)
5. OA-SYNC-008 (WS channel)
