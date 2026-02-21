# Rust Runtime Service Foundation

Status: Introduced by OA-RUST-033.

This document defines the initial Rust runtime service footprint inside `apps/runtime`.

## Goals

1. Provide a buildable Rust runtime service entrypoint in `apps/runtime`.
2. Establish module boundaries for authority writes, orchestration, and projectors.
3. Expose baseline health/readiness and runtime contract smoke routes.

## Current shape

- Cargo package: `openagents-runtime-service` (`apps/runtime/Cargo.toml`)
- Entrypoint: `apps/runtime/src/main.rs`
- Service wiring: `apps/runtime/src/lib.rs`
- HTTP handlers: `apps/runtime/src/server.rs`
- Boundaries:
  - `apps/runtime/src/authority.rs`
  - `apps/runtime/src/orchestration.rs`
  - `apps/runtime/src/projectors.rs`
  - `apps/runtime/src/types.rs`

## Endpoint contract (foundation scope)

- `GET /healthz` returns service/build metadata.
- `GET /readyz` returns authority/projector readiness state.
- `POST /internal/v1/runs` creates a run and appends `run.started`.
- `POST /internal/v1/runs/:run_id/events` appends runtime events.
- `GET /internal/v1/runs/:run_id` reads current run state.
- `GET /internal/v1/projectors/checkpoints/:run_id` reads latest projector checkpoint.

## Operational notes

1. Storage is currently in-memory and intentionally ephemeral for bootstrap validation.
2. Runtime authority persistence and full projector parity are delivered in follow-on OA-RUST issues.
3. Existing Elixir runtime remains present as the migration source until cutover milestones are complete.
