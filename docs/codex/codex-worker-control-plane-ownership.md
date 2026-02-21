# Codex Worker Control-Plane Ownership

Status: Active decision  
Date: 2026-02-21

## Decision

For `/api/runtime/codex/workers*`, Laravel proxy remains the single authoritative public control-plane path.

Authoritative lane:
1. iOS/Desktop/Web client -> `apps/openagents.com` Laravel API
2. Laravel `RuntimeCodexWorkersController` -> runtime internal codex endpoints
3. Runtime persists and fans out worker state/events

Rust service (`apps/openagents.com/service`) is explicitly non-authoritative for these endpoints in this phase.

## Why

1. Existing production clients already use Laravel session/auth for codex worker APIs.
2. Ownership and policy enforcement for user-scoped worker calls is already validated in Laravel API tests.
3. Avoids dual-control ambiguity while the Rust service route surface is still in migration.

## Enforcement

`apps/openagents.com/service/src/route_split.rs` enforces this invariant:
1. Any `/api/runtime/codex/workers*` path is forced to `Legacy` target.
2. This guard applies even when route split mode is `rust`, root-routed, or manually overridden to rust.

## Rollout Notes

1. Keep clients pointed at `/api/runtime/codex/workers*` (no endpoint migration).
2. Keep route split enabled as needed for other rust-routed paths.
3. Monitor codex worker API traffic for any unexpected rust-shell handling attempts.

## Rollback Notes

1. If codex worker control regressions occur, keep current ownership (Laravel-only) and disable new control verbs client-side.
2. If broader route split issues occur, set route split to legacy mode globally while preserving Laravel codex worker APIs.

## Exit Criteria For Ownership Migration

To move ownership to Rust service in a future phase:
1. Rust service implements full `/api/runtime/codex/workers*` parity.
2. Auth/session and policy semantics are proven equivalent.
3. Replay/stream behavior is contract-compatible.
4. Dual-write or staged cutover runbook exists with rollback.
