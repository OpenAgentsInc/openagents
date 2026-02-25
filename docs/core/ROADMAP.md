# OpenAgents Roadmap (Rust-Only Program)

This document is the high-level roadmap summary.

Detailed execution backlog and OA-RUST issue ordering live in:
- `docs/core/ARCHITECTURE.md`

## Program Objective

Complete migration to a Rust-only production architecture with:
- Rust control service + landing-first web distribution page
- Rust runtime authority service
- SpacetimeDB as canonical sync/replay transport (replacing Spacetime)
- Shared Rust client/UI/state crates across retained surfaces
- Proto-first contract governance

## Current Priority Phases

1. Rust service and surface convergence
- Keep `apps/runtime/` and `apps/openagents.com/service/` as the only authority services.
- Continue eliminating legacy non-Rust runtime behavior from production paths.

2. Sync transport replacement
- Execute full Spacetime -> Spacetime replacement plan in `docs/plans/spacetimedb-full-integration.md`.
- Preserve replay/idempotency/resume guarantees while switching transport ownership.

3. Contract and compatibility correctness
- Maintain proto-first wire contracts under `proto/`.
- Keep compatibility negotiation deterministic for retained clients during cutover.

4. Retained-surface parity
- Align desktop and runtime control contracts through shared Rust crates.
- Keep cross-surface contract harness green for retained consumer surfaces.

5. Operations and reliability
- Enforce deploy+migration coupling for runtime.
- Keep local CI Rust-first and compatibility lanes opt-in only.
- Maintain incident runbooks for sync/auth/reconnect/stale-cursor behaviors.

6. Documentation integrity
- Keep canonical docs aligned to active Rust-era surfaces.
- Move legacy material to archived/historical locations.

## Canonical References

- `docs/core/ARCHITECTURE.md`
- `docs/core/DEPLOYMENT_RUST_SERVICES.md`
- `docs/core/PROJECT_OVERVIEW.md`
- `docs/core/README.md`
- `docs/plans/spacetimedb-full-integration.md`
