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
- DbLayer (actor)
  - GRDB/SQLite access; WAL mode; single writer; transactional mutations.
  - Exposes read APIs that align with SubscriptionManager’s observation queries.

Concurrency & Backpressure

- Actors isolate shared state: connections, subscriptions, and DB.
- Each connection has an outbound queue (bounded). When backlog exceeds a threshold, coalesce transition updates (single‑flight) and optionally drop intermediate deltas.
- Heartbeat: ping every 15s; consider timeout at 120s without pong/data.
- Large payloads are split into chunks under a size limit (e.g., 64 KiB) and reassembled client‑side.

Discovery & Transport

- Bonjour (`_openagents._tcp`) for LAN discovery per ADR‑0004.
- JSON‑RPC 2.0 method names prefixed `tinyvex/*` muxed over the same bridge connection.

Error Handling

- No `try!` or process‑fatal errors for decode/encode paths.
- Convert server errors into JSON‑RPC errors and/or `tinyvex/error` notifications with `code`, `message`, and optional `data`.

Security (MVP)

- Local dev: plaintext WS on loopback. LAN use: start with plaintext; plan TLS (wss) or token pairing in follow‑ups.
- Optional token via `tinyvex/auth.setToken` to accommodate future cloud.

