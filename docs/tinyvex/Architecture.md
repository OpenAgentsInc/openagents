# Architecture — Tinyvex Server (Swift‑only)

Components

- WebSocket Server (SwiftNIO)
  - Accepts a single persistent WS per client (iOS/macOS).
  - Runs three loops per connection: receive → worker channel, worker → send channel, send (with heartbeat/backpressure).
- ConnectionManager (actor)
  - Tracks connected clients, heartbeat timestamps, and per‑connection queues.
  - Enforces backpressure policy and message chunking.
- SubscriptionManager (actor)
  - Deduplicates identical reactive queries across clients (query name + normalized params → key).
  - For each key, holds one GRDB ValueObservation and broadcasts updates to all subscribers.
  - Maintains sequence numbers per stream to support resume on reconnect.
  - Normalizes parameters using canonical JSON (sorted keys, no whitespace) so equivalent requests share a computation.
  - Optionally maintains a small in-memory journal (recent values by seq) to support fast resume when `lastSeq` is recent.
- ACP Session Fanout
  - ACP `session/update` frames are produced by the agent runner (after translation) and first persisted to the Tinyvex log.
  - A lightweight dispatcher then emits ACP‑compliant `session/update` notifications to all connected clients.
  - This path is distinct from Tinyvex query subscriptions (which use `tinyvex/*`).
- DbLayer (actor)
  - GRDB/SQLite access; WAL mode; single writer; transactional mutations.
  - Exposes read APIs that align with SubscriptionManager’s observation queries.

Event Ingestion Path

- Source: The existing Swift agent runner (CLI bridge) emits provider JSON‑L which is translated to ACP `SessionUpdate`.
- Write‑through: These updates are written to Tinyvex via DbLayer first (append to the log and update any secondary indexes).
- Propagation: ACP dispatcher sends `session/update` to clients; SubscriptionManager observes DB projections for non‑ACP list/history queries.
- Replay: On reconnect, clients resubscribe and either receive missed deltas (by seq/journal) or a fresh snapshot.
  - Applies a retention policy for log rows (compaction/GC) and exposes a snapshot validation helper for resume.

Concurrency & Backpressure

- Actors isolate shared state: connections, subscriptions, and DB.
- Each connection has an outbound queue (bounded). When backlog exceeds a threshold, coalesce transition updates (single‑flight) and optionally drop intermediate deltas.
- Heartbeat: ping every 15s; consider timeout at 120s without pong/data.
- Large payloads are split into chunks under a size limit (e.g., 64 KiB) and reassembled client‑side.
  - Single‑flight transitions: if multiple invalidations occur while a client is backpressured, coalesce to the latest state and drop intermediate transitions.

Discovery & Transport

- Bonjour (`_openagents._tcp`) for LAN discovery per ADR‑0004.
- JSON‑RPC 2.0 method names prefixed `tinyvex/*` muxed over the same bridge connection.
  - Handshake advertises server features and max chunk size.

Error Handling

- No `try!` or process‑fatal errors for decode/encode paths.
- Convert server errors into JSON‑RPC errors and/or `tinyvex/error` notifications with `code`, `message`, and optional `data`.
  - Standardize error codes (see Errors.md) and include `requestId`/`subId` when applicable.

Security (MVP)

- Local dev: plaintext WS on loopback. LAN use: start with plaintext; plan TLS (wss) or token pairing in follow‑ups.
- Optional token via `tinyvex/auth.setToken` to accommodate future cloud.

Operational settings (recommended)

- SQLite PRAGMA: `journal_mode=WAL`, `synchronous=NORMAL`, `busy_timeout=5000`, `wal_autocheckpoint=0`; run explicit checkpoints during idle.
- Message size thresholds: 64 KiB default chunk size; 1–4 MiB max per transition after chunking.
- Outbound queue bounds: start 100 messages; apply drop/coalesce policy beyond that.
