# Codex Thread Read-Model Persistence (OA-WEBPARITY-009)

Rust now owns Codex thread/message projection storage for runtime thread read paths.

## Storage model

- Backing store: JSON snapshot on local filesystem.
- Config:
  - `OA_CODEX_THREAD_STORE_PATH=/absolute/or/relative/path.json`
- Behavior:
  - if `OA_CODEX_THREAD_STORE_PATH` is unset, projections remain in-memory.
  - if set, projections are loaded at boot and persisted after message appends.

## Repository

- `apps/openagents.com/src/codex_threads.rs`
- Store type: `CodexThreadStore`

Models provided:

- `ThreadProjection`
- `ThreadMessageProjection`
- `AppendThreadMessageResult`

## API wiring

- `GET /api/runtime/threads` — list thread projections for current user
- `GET /api/runtime/threads/:thread_id/messages` — list projected messages for current user-owned thread
- `POST /api/runtime/threads/:thread_id/messages` — append user message command + update projection

Ownership boundaries are enforced per user id.

## Verification

- `cargo test --manifest-path apps/openagents.com/Cargo.toml`
- Added tests for:
  - append/list thread round-trip
  - store reload from disk snapshot
  - HTTP read-path projection responses
  - cross-user ownership boundary rejection
