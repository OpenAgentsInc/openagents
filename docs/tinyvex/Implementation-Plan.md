# Implementation Plan — MVP

Milestone 0: Skeleton

- Create Swift package/module `TinyvexServer` with targets:
  - `TinyvexServer` (SwiftNIO server, ConnectionManager, SubscriptionManager, DbLayer)
  - `TinyvexClient` (WebSocket client wrapper with Combine/async APIs)
  - `TinyvexCore` (shared models: protocol messages, errors, JSON codable types)
- Wire a basic WebSocket echo to validate transport.

Milestone 1: SQLite + Mutation

- Integrate GRDB; set WAL and PRAGMAs; migrations to create `documents`, `indexes`, `globals`.
- Implement `DbLayer` actor with transactional `applyMutation(name,args)` and simple demo mutation (insert/update one table).
- Add `tinyvex/connect`, `tinyvex/mutation` methods; return values and map errors to JSON‑RPC.

Milestone 2: Subscription Pipeline

- Implement `SubscriptionManager` actor with normalized (name,params) → key mapping.
- Add GRDB `ValueObservation` for a demo query; broadcast `tinyvex/data` with `seq` and optional `journal`.
- Add client‑side `subscribe` API returning a publisher/stream; handle cancel/unsubscribe.

Milestone 3: Resilience & Backpressure

- Add heartbeat (ping/pong), idle timeout, exponential reconnect on client.
- Implement bounded outbound queue per connection; coalesce updates; metrics counters.
- Add chunking of large values and client reassembly.

Milestone 4: Resume & Journal Horizon

- Maintain per‑sub in‑memory journal (N latest values) and accept `lastSeq`/`journal` on subscribe.
- Validate snapshot horizon; if expired, send fresh value with new `seq`.

Milestone 5: Discovery & Polish

- Bonjour advertising on server; discovery on client.
- Error taxonomy integration (Errors.md) with consistent codes.
- Documentation and examples.

Out of Scope For MVP

- TLS/mTLS, pairing (tracked under Security)
- Multi‑tenant, cross‑platform server binaries

