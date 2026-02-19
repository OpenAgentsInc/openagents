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

### Laravel proxy API exists (partial)

- Controller: `apps/openagents.com/app/Http/Controllers/Api/RuntimeCodexWorkersController.php`
- Routes:
  - `POST /api/runtime/codex/workers`
  - `GET /api/runtime/codex/workers/{workerId}`
  - `POST /api/runtime/codex/workers/{workerId}/requests`
  - `POST /api/runtime/codex/workers/{workerId}/stop`
  - Defined in `apps/openagents.com/routes/api.php`
- Contract tests: `apps/openagents.com/tests/Feature/Api/RuntimeCodexWorkersApiTest.php`

Gap: Laravel does not yet expose a Codex worker stream proxy endpoint.

### Rust desktop Codex executor exists (not runtime-synced yet)

- App: `apps/autopilot-desktop/`
- Main orchestration: `apps/autopilot-desktop/src/main.rs`
- Desktop currently spawns/uses `AppServerClient` directly and handles local Codex lifecycle (`thread/start`, `thread/resume`, `turn/start`, `turn/interrupt`) plus local notifications/events.
- Desktop is not yet wiring those sessions/events into runtime Codex worker APIs.

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

## Canonical Target Architecture

1. Desktop executes Codex requests locally.
2. Runtime stores durable worker state and event history.
3. Laravel exposes authenticated worker admin/stream APIs.
4. Web/mobile consume Laravel APIs for admin and observability.

This gives one worker contract for all surfaces while keeping desktop-first execution.

## Required Contract Shape

### Keep (already implemented)

- `POST /internal/v1/codex/workers`
- `GET /internal/v1/codex/workers/{worker_id}/snapshot`
- `POST /internal/v1/codex/workers/{worker_id}/requests`
- `GET /internal/v1/codex/workers/{worker_id}/stream`
- `POST /internal/v1/codex/workers/{worker_id}/stop`

### Add (required for desktop sync)

1. Runtime event ingest endpoint for async desktop notifications:
   - `POST /internal/v1/codex/workers/{worker_id}/events`
2. Runtime heartbeat semantics for desktop-attached workers (via explicit event type or dedicated endpoint).
3. Laravel SSE proxy for Codex stream:
   - `GET /api/runtime/codex/workers/{workerId}/stream`

Without event ingest, web/mobile cannot see full local desktop Codex activity (only request/response envelopes).

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

## Identity and Ownership Rules

1. Worker ownership stays principal-bound (`x-oa-user-id` or `x-oa-guest-scope`) at runtime.
2. Desktop never bypasses control-plane auth for user-scoped operations.
3. Runtime remains authoritative for ownership checks; request payload claims are not trusted.

## Out of Scope (for this plan stage)

- Reintroducing Cloudflare worker sandbox as the primary Codex path.
- Replacing Pylon bridge protocol.
- Multi-tenant remote desktop fleet orchestration.

## Historical Docs Status

- `docs/codex/webapp-sandbox-and-codex-auth-plan.md` is historical and no longer canonical for current desktop-first direction.
- `docs/codex/opencode-codex-auth.md` remains a low-level auth flow reference.

## Change Control

Any future Codex architecture change must update:

1. This plan (`docs/codex/unified-runtime-desktop-plan.md`)
2. Runtime contract docs (`apps/openagents-runtime/docs/RUNTIME_CONTRACT.md`, OpenAPI)
3. Laravel runtime Codex API docs/routes
4. Relevant desktop integration docs/code comments
