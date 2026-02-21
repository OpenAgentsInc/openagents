# ADR-0007: Onyx Integration Surface and Non-Goals

- Status: Accepted
- Date: 2026-02-21
- Owners: Runtime + Contracts
- Source issue: OA-RUST-096 (`#1931`)

## Context

Onyx is a Rust local-first app, but its integration with OpenAgents control/runtime surfaces was not explicitly constrained. Without a hard contract, Onyx could drift into Codex administration paths and uncontrolled Khala topic coupling.

## Decision

1. Onyx uses WorkOS-backed authentication through OpenAgents control APIs.
2. OpenAgents control-plane remains authoritative for Onyx authorization/session/device state.
3. Onyx sync tokens must be user-bound + device-bound and carry `oa_client_surface=onyx`.
4. Onyx is limited to `run:{run_id}:events` Khala topics in v1.
5. Onyx is explicitly denied from:
   - `runtime.codex_worker_events`
   - `worker:{worker_id}:lifecycle`
   - any RPC-style or mutation semantics over Khala transport.
6. Runtime enforces this boundary in sync authorization with deterministic deny code `surface_policy_denied`.

## Consequences

1. Onyx remains a constrained integration client rather than a Codex control surface.
2. Cross-surface auth policy becomes explicit and testable.
3. Future Onyx scope expansion requires an ADR update and policy/test changes before rollout.

## Enforcement Artifacts

1. `docs/protocol/onyx-integration-contract-v1.md`
2. `apps/runtime/src/sync_auth.rs`
3. `apps/runtime/src/server.rs` (integration test coverage for denied Onyx topic access)
