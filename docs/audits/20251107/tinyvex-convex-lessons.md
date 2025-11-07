# Tinyvex Persistence & Sync — Lessons from Convex (Swift + Rust)

Date: 2025-11-07
Author: OpenAgents Audit

Purpose: Summarize the key patterns in Convex’s Swift client and Rust backend that we should adopt (or avoid) when building Tinyvex — a local, desktop‑friendly WebSocket server backed by SQLite with multi‑client live sync.

## Executive Summary

- Convex’s Rust backend separates concerns cleanly: a timestamped persistence log over SQLite; a single‑threaded sync worker that maintains query subscriptions and emits minimal transitions; and a WebSocket layer that bridges clients to the worker with flow control and heartbeat.
- The Swift client is a thin facade over a Rust core via UniFFI, exposing a Combine‑based API for subscriptions, mutations, and actions, with custom JSON encoding for 64‑bit ints and special floats.
- For Tinyvex (desktop‑embedded), we should copy the core ideas (document log + indexes, query‑set versioning, transition messages, single‑flight backpressure, heartbeat) but simplify packaging (no UniFFI) and improve Swift‑side error handling (avoid try!).

## Rust Backend (Convex) — What To Learn

**SQLite as an Append‑Only Document Log**
- Schema uses immutable, timestamped rows and late materialization:
  - `documents(ts, table_id, id, json_value, deleted, prev_ts)`
  - `indexes(index_id, key, ts, deleted, table_id, document_id)`
  - `persistence_globals(key, json_value)`
- See: convex-backend/crates/sqlite/src/lib.rs:75, convex-backend/crates/sqlite/src/lib.rs:520
- Writes are grouped and transactionally inserted; conflict policy can be Error or Overwrite. See: convex-backend/crates/sqlite/src/lib.rs:258
- Readers compute latest state by grouping on `(index_id, key)` and joining into documents at the max `ts` ≤ read timestamp. See: convex-backend/crates/sqlite/src/lib.rs:120
- Prior revisions are queryable for pagination/backfills and validation. See: convex-backend/crates/sqlite/src/lib.rs:462

Why this matters for Tinyvex:
- A log‑structured model gives deterministic snapshot reads and cheap invalidation. We can implement a minimal version with WAL mode and one writer thread, matching desktop needs.

**Retention + Snapshot Validation Hooks**
- Readers chain a “validate snapshot” step to enforce retention windows asynchronously while streaming results. See: convex-backend/crates/sqlite/src/lib.rs:102, convex-backend/crates/sqlite/src/lib.rs:426
- This pattern keeps retention concerns out of query code and prevents serving too‑old snapshots.

Tinyvex take:
- Keep a hook to validate/cap snapshots (e.g., GC logs past horizon) without coupling it to read paths.

**Single‑Threaded Sync Worker**
- Core ideas: maintain a query set, track subscriptions, and emit state transitions summarizing deltas.
- Worker queues client operations and executes at most one mutation at a time for simplicity and determinism. See: convex-backend/crates/sync/src/worker.rs:255
- Uses “invalidations” to rerun queries only when overlapping writes occur. Each query holds a subscription handle and an abortable invalidation future. See: convex-backend/crates/sync/src/state.rs:24
- Produces `ServerMessage::Transition` with `StateModification::{QueryUpdated, QueryFailed, QueryRemoved}` between `start_version` and `end_version`. See: convex-backend/crates/convex/sync_types/src/types.rs:288

Tinyvex take:
- One sync loop per connection is enough for local; serialize mutations, recompute invalidated queries, and push only the diffs.

**Versioned Sync Protocol**
- Client → Server messages include `Connect`, `ModifyQuerySet`, `Mutation`, `Action`, `Authenticate`. See: convex-backend/crates/convex/sync_types/src/types.rs:160
- Server → Client messages include `Transition`, `TransitionChunk`, `MutationResponse`, `ActionResponse`, `AuthError`, `FatalError`, `Ping`. See: convex-backend/crates/convex/sync_types/src/types.rs:288
- Messages carry a `StateVersion { query_set, identity, ts }` and optional `client_clock_skew` and `server_ts` for observability.

Tinyvex take:
- Adopt a minimal subset: `Connect`, `ModifyQuerySet`, `Mutation`, and `Transition` (+ `TransitionChunk` for large payloads). Consider mapping these onto JSON‑RPC method names to align with ADR‑0004.

**WebSocket Server Integration**
- Axum WebSocket handler runs three loops: receive client messages, run sync worker, send server messages. See: convex-backend/crates/local_backend/src/subs/mod.rs:53
- Heartbeat ping every 5s and client timeout after 120s; graceful close with best‑effort final error message. See: convex-backend/crates/local_backend/src/subs/mod.rs:108, convex-backend/crates/local_backend/src/subs/mod.rs:240
- “Single‑flight” backpressure: a counting channel skips redundant transitions if the client is backlogged. See: convex-backend/crates/sync/src/worker.rs:189
- Large transitions are chunked into `TransitionChunk` parts. See: convex-backend/crates/local_backend/src/subs/mod.rs:292

Tinyvex take:
- Mirror this structure using Network.framework or SwiftNIO for macOS: separate recv/worker/send tasks, ping/pong, chunking, and a simple counter for transition backlog.

## Swift Client (Convex) — What To Learn

**Layering and Responsibilities**
- Public Swift API is a thin facade; all networking and protocol logic run in Rust via UniFFI.
- See overview: /Users/christopherdavid/code/convex-swift/docs/Architecture.md:1

**Combine‑First Subscription API**
- `subscribe<T: Decodable>` returns `AnyPublisher<T, ClientError>`, wiring cancellation to a Rust `SubscriptionHandle`. See: /Users/christopherdavid/code/convex-swift/docs/Swift-API.md:6
- Mutations/actions are async functions returning decoded `Decodable` payloads.

Tinyvex take:
- Keep the Combine shape for Swift apps: `subscribe` publisher, async mutations/actions. No UniFFI needed; talk directly to our WS server with URLSessionWebSocketTask.

**Data Encoding Conventions**
- Uses extended JSON wrappers for 64‑bit ints and special floats (`{"$integer": base64}`, `{"$float": base64}`) and string‑built JSON for arrays/dicts. See: /Users/christopherdavid/code/convex-swift/docs/Data-Encoding.md:1

Tinyvex take:
- Unless interoperability requires it, prefer standard JSON encoding to avoid string‑concat pitfalls and custom wrappers. If we need full‑range ints, add explicit wrappers but escape keys and avoid manual string building.

**FFI and Async Bridging**
- UniFFI futures are polled/completed via callbacks; Swift awaits them through helper shims. See: /Users/christopherdavid/code/convex-swift/docs/FFI-and-Interop.md:1

Tinyvex take:
- Not needed for a local server in our app. Use pure Swift for the client; keep the server in Swift or Rust behind a local process. If we ever embed Rust, adopt UniFFI patterns deliberately.

**Error Handling Gotchas**
- Swift decoding uses `try!` in subscription and RPC result paths, causing process crashes on malformed payloads. See: /Users/christopherdavid/code/convex-swift/docs/Error-Handling.md:1

Tinyvex take:
- Never `try!` decode server payloads. Surface `DecodingError` as a structured client error and keep the connection alive.

## Tinyvex Architecture Proposal (Desktop‑Local)

**Goals**
- WebSocket server on top of SQLite, multi‑client live updates, desktop‑local latency, integrates with our existing bridge (ADR‑0004) and Swift UI.

**Storage Model**
- Adopt a minimal Convex‑style log over SQLite:
  - Tables: `documents`, `indexes`, `globals` (same fields as above).
  - Writes: single writer task in WAL mode; append immutable rows with `ts` and optional `prev_ts`.
  - Reads: compute latest per key by `MAX(ts)` join; expose previous revisions for pagination/history.
  - Indices: maintain synthetic secondary indexes (e.g., by session/thread/user) to accelerate query scans.

**Sync Worker**
- Per‑connection worker state with:
  - Query set (map of `QueryId → Query`), `StateVersion` with `query_set` and `ts`.
  - For each query: last result hash, invalidation handle, and subscription cursor into the commit log.
  - Single operation queue: serialize mutations; actions in parallel if needed.
  - Emit `Transition` messages containing `StateModification::{QueryUpdated|QueryFailed|QueryRemoved}` and bump `StateVersion`.

**Protocol Shape**
- Map Convex messages onto JSON‑RPC to align with ADR‑0004:
  - `tinyvex/connect` (params: session info, client clock)
  - `tinyvex/modifyQuerySet` (params: baseVersion, newVersion, modifications)
  - `tinyvex/mutation` and `tinyvex/action` (params: udfPath, args)
  - Notifications: `tinyvex/transition`, `tinyvex/transitionChunk`, `tinyvex/ping`
- Preserve `StateVersion` fields and client/server timestamp hints.

**WebSocket Server**
- Implement as part of our macOS app process:
  - Separate tasks: receive loop → worker channel, worker task → send channel, send loop → socket.
  - Heartbeat ping/pong; 120s idle timeout; graceful close on error.
  - Single‑flight transition queue to avoid flooding slow clients; chunk large transitions.

**Swift Client API**
- Swift‑native client (no FFI) with the same ergonomics Convex uses:
  - `subscribe<T: Decodable>(query: ...) -> AnyPublisher<T, TinyvexError>`
  - `mutation<T: Decodable>(_: String, args: Encodable) async throws -> T`
  - `action<T: Decodable>(_: String, args: Encodable) async throws -> T`
- Use `JSONEncoder/Decoder` with explicit schemas; no `try!`.

**Auth & Security (Local Scope)**
- Start unauthenticated on LAN‑loopback only; reuse ADR‑0004 discovery and introduce pairing if/when needed. Keep identity versioning in the protocol for future expansion.

## Practical Differences vs. Convex

- Packaging: No embedded Rust for the client; server can be Swift (Network.framework) or a Rust sidecar. For desktop builds, Swift server reduces FFI complexity.
- Encoding: Prefer standard JSON unless compatibility demands Convex’s wrappers.
- Concurrency: One writer, one per‑connection worker is sufficient. We don’t need Convex’s multi‑tenant scale.
- Error policy: Non‑fatal decoding errors; explicit `TinyvexError` mapping.

## Implementation Checklist

- Schema: Create `documents`, `indexes`, `globals` tables mirroring Convex’s.
- Storage: Single writer task, WAL mode, conflict strategy = overwrite by default.
- Worker: Implement per‑connection SyncWorker with invalidation futures and transition emission.
- WS Server: Recv/Send loops, ping/pong, single‑flight queue, chunking.
- Swift Client: Combine publisher for subscriptions, async mutations/actions; robust decoding.
- Tests: Unit tests for diffing and invalidation; integration tests with two concurrent clients observing the same query set.

## Key References

- SQLite persistence: convex-backend/crates/sqlite/src/lib.rs:75, convex-backend/crates/sqlite/src/lib.rs:258, convex-backend/crates/sqlite/src/lib.rs:520
- Sync worker: convex-backend/crates/sync/src/worker.rs:229, convex-backend/crates/sync/src/state.rs:24
- WS layer: convex-backend/crates/local_backend/src/subs/mod.rs:53, convex-backend/crates/local_backend/src/subs/mod.rs:240, convex-backend/crates/local_backend/src/subs/mod.rs:292
- Message types: convex-backend/crates/convex/sync_types/src/types.rs:160, convex-backend/crates/convex/sync_types/src/types.rs:288
- Swift client overview: /Users/christopherdavid/code/convex-swift/docs/Architecture.md:1
- Swift API surface: /Users/christopherdavid/code/convex-swift/docs/Swift-API.md:6
- Encoding details: /Users/christopherdavid/code/convex-swift/docs/Data-Encoding.md:1
- FFI layout: /Users/christopherdavid/code/convex-swift/docs/FFI-and-Interop.md:1
- Error handling notes: /Users/christopherdavid/code/convex-swift/docs/Error-Handling.md:1

## Risks & Mitigations

- Backlog growth on slow clients → Use single‑flight transitions and chunking; drop intermediate deltas when safe.
- SQLite contention → Single writer, WAL mode, and careful long‑read avoidance; consider read replicas if needed later.
- Query invalidation correctness → Hash results and rerun only on overlapping writes; add tests for edge cases (empty results, error → success flips).
- Protocol drift vs. ADR‑0004 → Define `tinyvex/*` JSON‑RPC methods and version them; keep message schemas in `OpenAgentsCore`.

## Next Steps

1) Decide server implementation language (Swift vs. Rust sidecar) for macOS target.
2) Add `OpenAgentsCore/Tinyvex` module with message types and a Swift client.
3) Prototype a minimal server with a single query and mutation over a local SQLite DB; validate two‑client live transitions.
4) Extend to multi‑query sets and chunked transitions, then integrate discovery/pairing as needed.

