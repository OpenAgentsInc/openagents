# Rust Runtime Service Foundation

Status: Introduced by OA-RUST-033 and expanded by OA-RUST-034.

This document defines the initial Rust runtime service footprint inside `apps/runtime`.

## Goals

1. Provide a buildable Rust runtime service entrypoint in `apps/runtime`.
2. Establish module boundaries for authority writes, orchestration, and projectors.
3. Expose baseline health/readiness and runtime contract smoke routes.
4. Port worker lifecycle authority basics (registration, heartbeat, status transitions).
5. Enforce deterministic run state transitions for replay-safe runtime events.
6. Introduce durable append-only runtime event log with idempotency/ordering safeguards.

## Current shape

- Cargo package: `openagents-runtime-service` (`apps/runtime/Cargo.toml`)
- Entrypoint: `apps/runtime/src/main.rs`
- Service wiring: `apps/runtime/src/lib.rs`
- HTTP handlers: `apps/runtime/src/server.rs`
- Boundaries:
  - `apps/runtime/src/authority.rs`
  - `apps/runtime/src/orchestration.rs`
  - `apps/runtime/src/projectors.rs`
  - `apps/runtime/src/workers.rs`
  - `apps/runtime/src/types.rs`

## Endpoint contract (foundation scope)

- `GET /healthz` returns service/build metadata.
- `GET /readyz` returns authority/projector readiness state.
- `POST /internal/v1/runs` creates a run and appends `run.started`.
- `POST /internal/v1/runs/:run_id/events` appends runtime events.
- `GET /internal/v1/runs/:run_id` reads current run state.
- `GET /internal/v1/runs/:run_id/receipt` returns deterministic runtime receipt artifact.
- `GET /internal/v1/runs/:run_id/replay` returns deterministic replay JSONL artifact.
- `GET /internal/v1/projectors/checkpoints/:run_id` reads latest projector checkpoint.
- `GET /internal/v1/projectors/run-summary/:run_id` returns projected run read model.
- `GET /internal/v1/projectors/drift?topic=<topic>` returns drift detection metadata.
- `POST /internal/v1/workers` registers worker ownership/lifecycle state.
- `GET /internal/v1/workers/:worker_id` reads owner-scoped worker state.
- `POST /internal/v1/workers/:worker_id/heartbeat` updates worker liveness.
- `POST /internal/v1/workers/:worker_id/status` applies deterministic status transitions.
- `GET /internal/v1/workers/:worker_id/checkpoint` reads worker lifecycle projection checkpoint.

## Operational notes

1. Runtime event authority now uses a durable JSONL append log; run/projector read models remain in-memory during bootstrap.
2. Run events are durably appended to `RUNTIME_EVENT_LOG_PATH` (JSONL) before in-memory run projection updates.
3. Event append requests support idempotency (`idempotency_key`) and optimistic ordering checks (`expected_previous_seq`).
4. Runtime can emit deterministic receipt (`openagents.receipt.v1`) and replay (`REPLAY.jsonl`) artifacts from authoritative run events.
5. Projection checkpoints/read-model summaries are persisted (`RUNTIME_CHECKPOINT_PATH`) and recovered on restart.
6. Projection apply is idempotent (`seq <= checkpoint.last_seq` no-op) and drift hooks record sequence gaps.
7. Run transitions are validated against a deterministic state machine (`created -> running -> terminal/canceling` lanes) before events are accepted.
8. Runtime authority persistence and full projector parity are delivered in follow-on OA-RUST issues.
9. Shadow-mode parity harness (`runtime-shadow-harness`) compares legacy vs Rust artifacts and enforces cutover gate thresholds.
10. Authority cutover is controlled by `RUNTIME_AUTHORITY_WRITE_MODE`; legacy write freeze is controlled by `LEGACY_RUNTIME_WRITE_FREEZE`.
11. Khala live delivery path is wired through an internal fanout seam (`FanoutDriver`) with bounded in-memory adapter and external-driver hooks.
12. Khala topic polling enforces strict `stale_cursor` semantics (`410` with deterministic replay-floor metadata) and successful poll responses expose replay bootstrap metadata (`oldest_available_cursor`, `head_cursor`, `next_cursor`, `replay_complete`).
13. Khala topic polling enforces sync token auth, topic scope ACL matrix, worker ownership checks, and deterministic denied-path reason codes.
14. Existing Elixir runtime remains present as the migration source until cutover milestones are complete.
