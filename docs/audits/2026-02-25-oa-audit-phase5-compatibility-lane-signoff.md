# OA-AUDIT Phase 5 Compatibility Lane Signoff

Date: 2026-02-25  
Issue: `#2223`  
Status: completed

## Scope

Phase 5 closes the compatibility-lane cleanup gate by removing stale legacy CI references, retiring runtime-driver compatibility aliases, and formally sunset-marking retained compatibility endpoints with an explicit retirement date.

## Implemented Changes

1. Local CI trigger matrix cleanup:
- Removed stale `runtime-codex-workers-php` trigger/lane from `scripts/local-ci.sh`.
- Changed-mode local CI no longer references archived PHP paths.

2. Runtime-driver compatibility alias retirement:
- `apps/openagents.com/service/src/runtime_routing.rs` now accepts only Rust-era labels:
  - `control_service` / `control`
  - `runtime_service` / `runtime`
- Removed legacy alias parsing (`legacy`, `laravel`, `openagents.com`, `elixir`).

3. Formal sunset metadata for retained compatibility lanes:
- Control service now attaches sunset metadata headers on compatibility lanes:
  - `sunset: Tue, 30 Jun 2026 00:00:00 GMT`
  - `x-oa-compat-sunset-date: 2026-06-30`
  - `x-oa-compat-migration-doc: docs/audits/2026-02-25-oa-audit-phase5-compatibility-lane-signoff.md`
- Applied to:
  - `/api/v1/control/*`
  - `/api/v1/auth/*`
  - `/api/v1/sync/token`
  - legacy chat aliases (`/api/chat/*`, `/api/chats/*`)

4. OpenAPI sunset/deprecation alignment:
- Added OpenAPI compatibility sunset metadata extensions for compatibility lanes.
- Marked route-split/runtime-routing compatibility control operations as deprecated in OpenAPI.

## Sunset Policy (Concrete Date)

- Compatibility/admin lanes above are formally sunset and targeted for full removal by **2026-06-30**.
- No new feature work should be added to sunset-marked endpoints.

## Documentation Consistency Updates

Updated to match code + sunset posture:

- `docs/core/ARCHITECTURE.md`
- `docs/core/PROJECT_OVERVIEW.md`
- `apps/openagents.com/service/docs/RUNTIME_ROUTE_OWNERSHIP.md`

## Acceptance Criteria Mapping

1. Deprecated compatibility lanes removed or formally sunset with date: **met** (`2026-06-30`).
2. CI trigger matrix contains no stale legacy patterns: **met** (`runtime-codex-workers-php` removed).
3. Architecture/project overview/route inventory consistency: **met** (docs aligned to code changes).
