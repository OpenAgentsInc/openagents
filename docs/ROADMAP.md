# OpenAgents Roadmap (Rust-Only Program)

This document is the high-level roadmap summary.

Detailed execution backlog and OA-RUST issue ordering live in:
- `docs/ARCHITECTURE-RUST-ROADMAP.md`

## Program Objective

Complete migration to a Rust-only production architecture with:
- Rust control service + Rust/WGPUI web shell
- Rust runtime authority service
- WS-only Khala replay/delivery for live subscriptions
- Shared Rust client/UI/state crates across web/desktop/iOS
- Proto-first contract governance

## Current Priority Phases

1. Rust service and surface convergence
- Keep `apps/runtime/` and `apps/openagents.com/service/` as the only authority services.
- Continue eliminating legacy non-Rust runtime behavior from production paths.

2. Contract and sync correctness
- Maintain proto-first wire contracts under `proto/`.
- Keep Khala delivery semantics deterministic (resume, replay, stale cursor, idempotent apply).

3. Cross-surface parity
- Align web-shell, desktop, and iOS behavior through shared Rust crates.
- Keep contract harness and replay fixtures green across surfaces.

4. Operations and reliability
- Enforce deploy+migration coupling for runtime.
- Keep local CI Rust-first and compatibility lanes opt-in only.
- Maintain runbooks for WS/auth/reconnect/stale-cursor incidents.

5. Documentation integrity
- Keep canonical docs aligned to active Rust-era surfaces.
- Move legacy material to archived/historical locations.

## Canonical References

- `docs/ARCHITECTURE-RUST.md`
- `docs/ARCHITECTURE-RUST-ROADMAP.md`
- `docs/PROJECT_OVERVIEW.md`
- `docs/README.md`
