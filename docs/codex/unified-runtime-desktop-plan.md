# Unified Codex Runtime + Desktop Plan

Status: Active (canonical)  
Date: 2026-02-19  
Owner: Runtime + Desktop + Web teams

## Purpose

Define one canonical Codex integration path that:

1. Ships desktop-first Codex execution.
2. Makes runtime the durable admin and event authority.
3. Lets web now (and mobile next) observe and administer desktop Codex sessions through the same contract.

As of 2026-02-19, `docs/plans/` has no Codex-specific plan entry. Runtime and desktop Codex behavior exists in code and contracts, but planning/docs are fragmented. This document is now the single plan of record.

## Product Decision

Desktop-first is the default execution mode for Codex.

- The Rust desktop app (`apps/autopilot-desktop/`) is the first-class Codex executor.
- The Elixir runtime (`apps/openagents-runtime/`) is the authoritative durable control/event ledger.
- Laravel (`apps/openagents.com/`) is the authenticated control-plane facade for web/mobile clients.
- Web/mobile do not own Codex execution; they own admin, visibility, and orchestration surfaces over runtime-backed state.

Cloud-hosted Codex is not deleted, but it is a later adapter mode behind the same runtime worker contract.

## Current State (Implemented)

### Runtime internal Codex worker contract exists

- Routes: `apps/openagents-runtime/lib/openagents_runtime_web/router.ex`
- Controller: `apps/openagents-runtime/lib/openagents_runtime_web/controllers/codex_worker_controller.ex`
- Domain/processes:
  - `apps/openagents-runtime/lib/openagents_runtime/codex/workers.ex`
  - `apps/openagents-runtime/lib/openagents_runtime/codex/worker_process.ex`
  - `apps/openagents-runtime/lib/openagents_runtime/codex/worker_stream_tailer.ex`
  - `apps/openagents-runtime/lib/openagents_runtime/codex/worker.ex`
  - `apps/openagents-runtime/lib/openagents_runtime/codex/worker_event.ex`
  - `apps/openagents-runtime/lib/openagents_runtime/codex/worker_supervisor.ex`
- Contract docs:
  - `apps/openagents-runtime/docs/RUNTIME_CONTRACT.md`
  - `apps/openagents-runtime/docs/openapi-internal-v1.yaml`
- DB tables:
  - `runtime.codex_workers`
  - `runtime.codex_worker_events`
  - Migration: `apps/openagents-runtime/priv/repo/migrations/20260219038000_create_runtime_codex_worker_tables.exs`

Current adapter behavior is development-only `in_memory`.

- Adapter behavior: `apps/openagents-runtime/lib/openagents_runtime/codex/adapter.ex`
- In-memory adapter: `apps/openagents-runtime/lib/openagents_runtime/codex/adapters/in_memory.ex`

### Laravel proxy API exists

- Controller: `apps/openagents.com/app/Http/Controllers/Api/RuntimeCodexWorkersController.php`
- Routes:
  - `GET /api/runtime/codex/workers`
  - `POST /api/runtime/codex/workers`
  - `GET /api/runtime/codex/workers/{workerId}`
  - `GET /api/runtime/codex/workers/{workerId}/stream`
  - `POST /api/runtime/codex/workers/{workerId}/events`
  - `POST /api/runtime/codex/workers/{workerId}/requests`
  - `POST /api/runtime/codex/workers/{workerId}/stop`
  - Defined in `apps/openagents.com/routes/api.php`
- Contract tests: `apps/openagents.com/tests/Feature/Api/RuntimeCodexWorkersApiTest.php`
- Admin UI: `apps/openagents.com/resources/js/pages/admin/index.tsx`

### Rust desktop Codex executor with runtime sync baseline exists

- App: `apps/autopilot-desktop/`
- Main orchestration: `apps/autopilot-desktop/src/main.rs`
- Desktop still spawns/uses `AppServerClient` directly for local Codex execution (`thread/start`, `thread/resume`, `turn/start`, `turn/interrupt`).
- Desktop now optionally mirrors worker lifecycle/events into runtime when runtime sync env vars are set (`OPENAGENTS_RUNTIME_SYNC_*`):
  - creates/reattaches worker on thread bootstrap/session start/resume,
  - posts async events to `POST /api/runtime/codex/workers/{workerId}/events`,
  - runs periodic runtime heartbeat event sync (`worker.heartbeat`) for attached workers,
  - uses stable per-thread worker IDs for idempotent reattach.

### Local bridge protocol exists

- Bridge implementation: `crates/pylon/src/local_bridge.rs`
- ADR: `docs/adr/ADR-0020-pylon-local-ui-bridge.md`
- Codex channels/events:
  - `client-codex.connect`
  - `client-codex.request`
  - `client-codex.respond`
  - `client-codex.disconnect`
  - `pylon.codex.event`
  - `pylon.codex.response`
  - `pylon.codex.status`
- `pylon.codex.event` payload now includes normalized `runtime_event` metadata for downstream runtime taxonomy consumers.

## Canonical Target Architecture

1. Desktop executes Codex requests locally.
2. Runtime stores durable worker state and event history.
3. Laravel exposes authenticated worker admin/stream APIs.
4. Web/mobile consume Laravel APIs for admin and observability.

This gives one worker contract for all surfaces while keeping desktop-first execution.

## Khala Sync Lane (Optional, Recommended for Multi-Client Reactivity)

Khala is allowed as a reactive sync/read-model layer for Codex UI surfaces, but
not as Codex lifecycle authority.

Rules:

1. Runtime/Postgres remains source-of-truth for Codex worker lifecycle/events.
2. Khala stores derived worker summaries/notification state for subscriptions.
3. Runtime is the single writer into Khala projection docs.
4. Laravel remains auth/session authority and mints Khala auth tokens.

Active implementation plan:

- `docs/plans/active/khala-self-hosting-runtime-sync-plan.md`

## Required Contract Shape

### Keep (already implemented)

- `POST /internal/v1/codex/workers`
- `GET /internal/v1/codex/workers/{worker_id}/snapshot`
- `POST /internal/v1/codex/workers/{worker_id}/events`
- `POST /internal/v1/codex/workers/{worker_id}/requests`
- `GET /internal/v1/codex/workers/{worker_id}/stream`
- `POST /internal/v1/codex/workers/{worker_id}/stop`

### Remaining Hardening

1. Add explicit end-to-end desktop restart simulation coverage in CI (runtime + desktop integration lane).
2. Evaluate whether a dedicated heartbeat endpoint is needed beyond event-ingest semantics.

Runtime event ingest is now live, so web/mobile can observe desktop-originated activity through runtime stream/snapshot APIs.

## Layer-0 Schema Authority

Cross-surface contract authority for Codex worker lifecycle/events is `proto/`, not language-local DTOs.

- Canonical schema root: `proto/`
- Canonical package namespace: `proto/openagents/protocol/v1/*`
- Governance and generation policy: `proto/README.md`
- Compatibility enforcement: `buf lint` + `buf breaking` + `scripts/verify-proto-generate.sh`

Wire format can remain JSON/SSE; mappings must stay proto-compatible (see `docs/protocol/LAYER0_PROTOBUF_MAPPING.md`).

## Planned Proto Definitions (Codex)

Add these Layer-0 protocol files to support desktop + hosted Codex parity:

1. `proto/openagents/protocol/v1/codex_workers.proto`
   - `CodexWorker`
   - `CodexWorkerCreateRequest`
   - `CodexWorkerCreateResponse`
   - `CodexWorkerSnapshot`
   - `CodexWorkerRequestEnvelope`
   - `CodexWorkerRequestResponse`
   - `CodexWorkerStopRequest`
   - `CodexWorkerStopResponse`
2. `proto/openagents/protocol/v1/codex_events.proto`
   - `CodexWorkerEvent` (`oneof` event payload)
   - payloads for started/request/response/error/heartbeat/stopped
   - explicit `event_version`
3. `proto/openagents/protocol/v1/codex_sandbox.proto`
   - `CodexExecutionMode` (`DESKTOP`, `SANDBOX`)
   - `CodexSandboxBackend` (`CLOUDFLARE_SANDBOX`, `DAYTONA`, `OPENAGENTS_GCP`)
   - worker sandbox binding metadata (`workspace_ref`, `codex_home_ref`, region/placement hints)
4. `proto/openagents/protocol/v1/codex_auth.proto`
   - backend-neutral auth state envelopes (device flow status, token material references, hydration status; no secret value fields)

## Integration Plan

### Phase 0: Canonical doc alignment (this change)

- Make this file canonical.
- Mark older Codex planning docs as reference/historical.
- Update docs indexes/maps to point here.

### Phase 1: Runtime adapter and event model hardening

Goal: runtime can represent a desktop-backed worker, not only `in_memory`.

Work:

- Add `desktop_bridge` adapter mode in runtime worker supervision/resolution.
- Add event ingest path for async notifications (`thread/*`, `turn/*`, errors, status).
- Define durable event taxonomy and minimum payload schema for Codex worker events.
- Keep stream cursor semantics aligned with run stream semantics.

Primary files:

- `apps/openagents-runtime/lib/openagents_runtime/codex/worker_supervisor.ex`
- `apps/openagents-runtime/lib/openagents_runtime/codex/workers.ex`
- `apps/openagents-runtime/lib/openagents_runtime_web/controllers/codex_worker_controller.ex`
- `apps/openagents-runtime/docs/RUNTIME_CONTRACT.md`
- `apps/openagents-runtime/docs/openapi-internal-v1.yaml`

Verification:

- `cd apps/openagents-runtime && mix test test/openagents_runtime/codex/workers_test.exs`
- `cd apps/openagents-runtime && mix test test/openagents_runtime_web/controllers/codex_worker_controller_test.exs`

### Phase 2: Desktop runtime sync client

Goal: each desktop Codex session is mirrored into runtime worker state/events.

Work:

- Desktop creates/reattaches worker via Laravel runtime API at session start.
- Desktop mirrors local Codex notifications into runtime worker events.
- Desktop forwards request results and explicit stop state.
- Add reconnect/resume behavior after desktop restart.
- Map desktop thread/session IDs to stable worker IDs with idempotent reattach semantics.

Primary files:

- `apps/autopilot-desktop/src/main.rs`
- `crates/pylon/src/local_bridge.rs` (if bridge events are reused for runtime sync path)
- `apps/openagents.com/routes/api.php` and runtime proxy controller (if extra routes are needed)

Verification:

- `cargo check -p autopilot-desktop`
- `cargo check -p pylon`
- `cd apps/openagents.com && php artisan test --filter=RuntimeCodexWorkersApiTest`

### Phase 3: Web admin + observability

Goal: web can administer and observe desktop Codex workers through runtime-backed APIs.

Work:

- Add Laravel stream proxy route for worker SSE.
- Add web UI for worker list/detail/status/stop/request and live stream events.
- Correlate worker events with user/thread context in UI.

Primary files:

- `apps/openagents.com/routes/api.php`
- `apps/openagents.com/app/Http/Controllers/Api/RuntimeCodexWorkersController.php`
- web UI pages/components under `apps/openagents.com/resources/`

Verification:

- `cd apps/openagents.com && php artisan test --filter=RuntimeCodexWorkersApiTest`
- Web E2E smoke for worker create -> request -> stream -> stop

### Phase 4: Mobile read/admin parity

Goal: mobile consumes same Laravel Codex APIs for status and controls.

Work:

- Start read-only parity (snapshot + stream).
- Add safe admin controls (stop, limited request actions) behind role/policy guards.
- Reuse the same runtime worker IDs and event schema.
- Reuse Laravel khala token minting for mobile Khala auth (`POST /api/khala/token`) with refresh-aware caching.

Implementation status (2026-02-19):

- Mobile now exposes a Codex worker tab backed by Laravel runtime APIs for list/snapshot/stream/admin actions.
- Stream parity uses runtime cursor/tail semantics with reconnect-safe long polling.
- Khala auth path on mobile now mints short-lived tokens via Laravel instead of reusing raw session token directly.

## Event Taxonomy Baseline

Runtime Codex worker events should standardize on:

- `worker.started`
- `worker.request.received`
- `worker.response`
- `worker.error`
- `worker.event` (desktop notification passthrough with normalized `method`)
- `worker.heartbeat`
- `worker.stopped`

Desktop notification methods currently seen in `apps/autopilot-desktop/src/main.rs` include:

- `thread/started`
- `turn/started`
- `turn/completed`
- error projections like `codex/error`

These should map deterministically into runtime event payloads so web/mobile can render a stable activity model.

Current deterministic mapping (desktop + local bridge):

| Source signal | Runtime `event_type` |
|---|---|
| `thread/started` | `worker.started` |
| `thread/stopped` or `thread/completed` | `worker.stopped` |
| `*/error` or `codex/error` | `worker.error` |
| `*/heartbeat` | `worker.heartbeat` |
| bridge request envelope | `worker.request.received` |
| all other `thread/*`, `turn/*`, `codex/*` notifications | `worker.event` |

Baseline payload shape sent with each mapped event:

- `source` (`autopilot-desktop` or `pylon-local-bridge`)
- `method` (original notification/request method)
- `params` (original params object; `{}` when missing)
- `occurred_at` (desktop path) or `request_id` (bridge request path when present)

Runtime snapshot/list heartbeat policy fields:

- `heartbeat_state` (`fresh|stale|missing|stopped|failed`)
- `heartbeat_age_ms`
- `heartbeat_stale_after_ms`

## Identity and Ownership Rules

1. Worker ownership stays principal-bound (`x-oa-user-id` or `x-oa-guest-scope`) at runtime.
2. Desktop never bypasses control-plane auth for user-scoped operations.
3. Runtime remains authoritative for ownership checks; request payload claims are not trusted.

## Out of Scope (for this plan stage)

- Reintroducing Cloudflare worker sandbox as the primary Codex path.
- Replacing Pylon bridge protocol.
- Multi-tenant remote desktop fleet orchestration.

## Historical Docs Status

- `docs/codex/webapp-sandbox-and-codex-auth-plan.md` is the active hosted sandbox backend architecture companion (Cloudflare/Daytona/OpenAgents GCP).
- `docs/plans/archived/codex/opencode-codex-auth.md` is archived historical auth deep-dive context.

## ADR Alignment

- `docs/adr/ADR-0029-khala-sync-layer-and-codex-agent-mode.md` defines the
  Khala sync-layer boundary and Codex operational posture for Khala CLI/MCP.

## Change Control

Any future Codex architecture change must update:

1. This plan (`docs/codex/unified-runtime-desktop-plan.md`)
2. Runtime contract docs (`apps/openagents-runtime/docs/RUNTIME_CONTRACT.md`, OpenAPI)
3. Laravel runtime Codex API docs/routes
4. Relevant desktop integration docs/code comments
