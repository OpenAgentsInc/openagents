# Onyx Integration Contract v1 (OA-RUST-096)

Status: Active  
Source issue: OA-RUST-096 (`#1931`)

## Purpose

Define the allowed OpenAgents integration surface for `apps/onyx`, lock the identity model, and document non-goals so Onyx remains decoupled from Codex control/runtime authority lanes.

## Identity Model

1. WorkOS remains the identity/authentication provider for user sign-in.
2. OpenAgents control-plane remains authoritative for session, device binding, authorization, and revocation.
3. Onyx sync access is always user-scoped and device-bound:
   - `oa_user_id` must be present.
   - `oa_device_id` must be present.
   - `oa_client_surface` must be `onyx`.
4. Delegated service identities and anonymous guest sync tokens are out of contract for Onyx.

## Allowed Surface (v1)

### Control-plane

Allowed classes of integration:

1. Standard auth/session flows (challenge/verify/refresh/revoke) through control APIs.
2. Sync token minting through control APIs with explicit `onyx` surface attribution and scoped grants.

### Runtime/Khala Topics

Onyx is limited to run-event replay lanes in v1:

1. Allowed: `run:{run_id}:events` with `runtime.run_events` scope.
2. Not allowed:
   - `runtime.codex_worker_events`
   - `worker:{worker_id}:lifecycle`
   - any other Khala topic outside run-event lanes.

## Enforcement

Runtime sync authorization enforces this contract:

1. `apps/runtime/src/sync_auth.rs` applies an Onyx surface policy gate.
2. Violations return deterministic deny code: `surface_policy_denied`.
3. Policy is covered by sync auth unit tests and runtime server integration tests.

## Offline and Sync Non-Goals

1. Onyx is not a privileged control surface for Codex worker administration.
2. Onyx does not gain mutation authority through Khala transport.
3. Onyx local-first note editing behavior remains independent of Khala availability.
4. Onyx is not a replacement for autopilot-desktop or autopilot-ios Codex administration flows.
