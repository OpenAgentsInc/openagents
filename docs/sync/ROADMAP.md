# Khala Sync Roadmap (WS-Only)

Date: 2026-02-20  
Status: Active program  
Owner lanes: Runtime, Web, iOS, Desktop, Infra, Protocol
Authority ADRs: `docs/adr/ADR-0003-khala-ws-only-replay-transport.md`, `docs/adr/ADR-0002-proto-first-contract-governance.md`

Khala is the OpenAgents runtime-owned sync engine.

## Program Goal

Deliver a production-grade, Postgres-backed, WebSocket sync lane for runtime read models with deterministic replay and clear ownership boundaries.

## Program Constraints

1. Runtime/Postgres remain authority for execution state.
2. Khala is projection + replay delivery only.
3. Live transport is WS only for Khala (no new SSE lane).
4. Proto contracts are schema authority.
5. Topic watermarks are DB-native per-topic sequences.
6. Stream journal supports pointer-mode payload delivery.

## Current Baseline (2026-02-20)

1. Runtime projector/checkpoint/reprojection stack exists.
2. Runtime default sink is noop in some env configs.
3. Laravel token endpoints exist at `/api/khala/token` and `/api/sync/token`.
4. Web/iOS/desktop have Khala lanes with staged rollout controls.
5. Lightning control-plane lane uses API/mock transport and does not require Khala as authority transport.

## Status Legend

- `Completed`: merged and verified in this repo.
- `In Progress`: partially landed, still missing done criteria.
- `Planned`: not merged yet.

## Epic 1: Contract and Doctrine Lock

### KHALA-001: ADR lock for runtime-owned Khala

Status: Completed  
Depends on: none

Done when:

- ADR documents WS-only, runtime-owned placement, and authority boundaries.

Verification:

- ADR present and linked from sync docs.

### KHALA-002: Proto package `openagents.sync.v1`

Status: Planned  
Depends on: KHALA-001

Scope:

- Define subscribe/subscribed/update/heartbeat/error wire contract.
- Define topic enum and error enum (`stale_cursor`, `payload_too_large`, etc.).

Done when:

- proto package compiles and passes Buf compatibility checks.

Verification:

- `buf lint`
- `buf breaking --against '.git#branch=main,subdir=proto'`
- `./scripts/verify-proto-generate.sh`

### KHALA-003: WS mapping doc for proto-to-channel wire

Status: Planned  
Depends on: KHALA-002

Scope:

- Add `docs/protocol/OA_SYNC_WS_MAPPING.md` with channel/event mapping.
- Define replay batch framing, heartbeat semantics, and error handling contract.

Done when:

- mapping doc merged and cross-linked from sync docs.

Verification:

- docs cross-link check.

### KHALA-004: Canonical payload hashing doctrine

Status: Planned  
Depends on: KHALA-002

Scope:

- Define canonical hashing rules for proto payloads and JSON payloads.
- Add deterministic fixtures for Elixir + TypeScript.

Done when:

- fixtures and tests prove cross-language hash parity.

Verification:

- runtime + TS fixture tests.

### KHALA-005: Surface contract document

Status: Completed  
Depends on: KHALA-001

Done when:

- `docs/sync/SURFACES.md` enumerates topic/subscription/bootstrap model per app.

Verification:

- docs cross-link check.

## Epic 2: Runtime Data Model and Sink

### KHALA-006: Sync stream/read-model schema

Status: In Progress  
Depends on: KHALA-001

Scope:

- Add migrations for stream events and runtime summary read models.
- Add replay and retention indexes.

Done when:

- migrations run cleanly in runtime dev/test and schema is documented.

Verification:

- `cd apps/runtime && mix ecto.migrate`
- `cd apps/runtime && mix test`

### KHALA-007: Per-topic sequence table

Status: Planned  
Depends on: KHALA-006

Scope:

- Add `runtime.sync_topic_sequences` migration.
- Seed v1 topics.

Done when:

- concurrency-safe allocator test fixtures exist.

Verification:

- `cd apps/runtime && mix test --only sync_watermarks`

### KHALA-008: DB-native watermark allocator

Status: Planned  
Depends on: KHALA-007

Scope:

- Implement transaction-safe `UPDATE ... RETURNING` allocator.

Done when:

- monotonic/no-duplicate guarantees verified under concurrency tests.

Verification:

- `cd apps/runtime && mix test --only sync`

### KHALA-009: `OpenAgentsRuntime.Sync.ProjectorSink`

Status: In Progress  
Depends on: KHALA-006, KHALA-008

Scope:

- Upsert read models.
- Allocate watermark and append stream journal row.
- Emit telemetry.
- Support pointer mode payload delivery.

Done when:

- sink is runtime-configurable and durable stream rows are produced in tests.

Verification:

- runtime integration tests.
- `mix runtime.contract.check`

### KHALA-010: Retention job + stale boundary metrics

Status: Planned  
Depends on: KHALA-006

Scope:

- Add retention worker for stream journal horizon.
- Track oldest retained watermark per topic.

Done when:

- stale boundary is observable and tested.

Verification:

- runtime integration tests for retention/stale-cursor conditions.

## Epic 3: WS Delivery Service

### KHALA-011: `sync:v1` channel with auth join

Status: Planned  
Depends on: KHALA-002, KHALA-009

Scope:

- Add authenticated channel join and topic entitlement checks.

Done when:

- unauthorized topic subscriptions fail deterministically.

Verification:

- runtime channel auth tests.

### KHALA-012: Replay-on-subscribe and resume

Status: Planned  
Depends on: KHALA-011

Scope:

- Replay stream events after client watermark in bounded batches.
- Switch subscriber to live mode after catch-up.

Done when:

- reconnect tests show gap-free resume.

Verification:

- integration tests with forced disconnect/reconnect.

### KHALA-013: Stale cursor handling

Status: Planned  
Depends on: KHALA-010, KHALA-012

Scope:

- Detect stale watermark relative to retention horizon.
- Return deterministic stale cursor error contract.

Done when:

- stale-cursor path is fully tested and documented.

Verification:

- integration tests with retention purge simulation.

### KHALA-014: Heartbeat + connection health telemetry

Status: Planned  
Depends on: KHALA-011

Scope:

- Add heartbeat/timeout path.
- Emit telemetry for reconnects, lag, catch-up duration.

Done when:

- dashboard and alerting can track production health.

Verification:

- telemetry tests + docs update.

## Epic 4: Laravel Auth Bridge

### KHALA-015: Sync token issuer endpoint

Status: Completed  
Depends on: KHALA-001

Delivered:

- `POST /api/sync/token`
- legacy-compatible `POST /api/khala/token`

Verification:

- `cd apps/openagents.com && php artisan test --filter=SyncTokenApiTest`
- `cd apps/openagents.com && php artisan test --filter=KhalaTokenApiTest`

### KHALA-016: Runtime socket JWT verification + rotation path

Status: Planned  
Depends on: KHALA-011, KHALA-015

Scope:

- Verify signature and required claims.
- Enforce scopes + doc ownership.
- Support `kid` rotation posture.

Done when:

- invalid claims/scopes are denied predictably and tested.

Verification:

- runtime socket auth tests.

## Epic 5: Client SDK and Surface Integration

### KHALA-017: Shared TS Khala client package

Status: In Progress  
Depends on: KHALA-002, KHALA-012

Scope:

- Connection lifecycle, auth handshake, topic subscribe.
- Watermark persistence and auto-resume.
- Local doc cache with doc-version monotonic merge rules.

Done when:

- package is used by at least one production surface and one integration harness.

Verification:

- package tests + integration harness.

### KHALA-018: Web Codex integration behind flag

Status: In Progress  
Depends on: KHALA-017

Scope:

- Wire Codex summary UI updates to Khala lane.
- Keep action APIs unchanged.

Done when:

- flag toggles between legacy reactive lane and Khala lane.

Verification:

- web integration tests + staging smoke.

### KHALA-019: iOS Codex integration behind flag

Status: In Progress  
Depends on: KHALA-017

Scope:

- Use Khala for worker summary updates.
- Remove old provider boot requirement for flagged users.

Done when:

- flagged users receive live summaries without legacy provider boot.

Verification:

- `xcodebuild -project apps/autopilot-ios/Autopilot/Autopilot.xcodeproj -scheme Autopilot -destination 'platform=iOS Simulator,name=iPhone 17 Pro' test -only-testing:AutopilotTests`

### KHALA-020: Desktop status integration behind flag

Status: In Progress  
Depends on: KHALA-017

Scope:

- Use Khala for status summary lanes where applicable.
- Keep Lightning task APIs unchanged.

Done when:

- desktop sync lane runs without legacy sync URL/token dependence.

Verification:

- `cargo check -p autopilot-desktop && cargo test -p autopilot_ui`

## Epic 6: Parity, Cutover, and Cleanup

### KHALA-021: Dual-path parity auditor

Status: In Progress  
Depends on: KHALA-004, KHALA-009, KHALA-018

Scope:

- Compare legacy reactive lane outputs against Khala output hashes/version.
- Emit mismatch metrics and lag-drift telemetry.

Done when:

- parity dashboard gate is in place and actively used for rollout decisions.

Verification:

- runtime load/chaos parity checks.

### KHALA-022: Runtime/Codex cutover runbook

Status: Completed  
Depends on: KHALA-021

Delivered:

- `docs/sync/RUNTIME_CODEX_CUTOVER_RUNBOOK.md`
- staging drill artifact under `docs/sync/status/`

Verification:

- runbook evidence artifacts committed.

### KHALA-023: Remove legacy sync dependency from migrated surfaces

Status: Planned  
Depends on: KHALA-022

Scope:

- Remove legacy client providers/config from migrated web/iOS/desktop paths.
- Keep compatibility only for non-migrated surfaces during rollback window.

Done when:

- migrated surfaces no longer require legacy sync-specific configuration.

Verification:

- surface CI checks + manual smoke.

## Epic 7: Lightning Lane (Completed in Current Baseline)

### KHALA-024: Proto contracts for Lightning control-plane models

Status: Completed  
Depends on: KHALA-002

### KHALA-025: Laravel/runtime APIs for Lightning control-plane state

Status: Completed  
Depends on: KHALA-024

### KHALA-026: API transport replacement in `apps/lightning-ops`

Status: Completed  
Depends on: KHALA-025

### KHALA-027: Lightning cutover and legacy sync decommission

Status: Completed  
Depends on: KHALA-026

## Labels for Tracking

- `khala-sync`
- `khala-runtime`
- `khala-web`
- `khala-ios`
- `khala-desktop`
- `khala-lightning`
- `khala-proto`
- `khala-infra`

## Immediate Next Issues

1. KHALA-002
2. KHALA-006
3. KHALA-007
4. KHALA-008
5. KHALA-011
