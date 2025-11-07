# Tinyvex — Local‑First Sync Server for OpenAgents

Tinyvex is a Swift‑only, local‑first sync server that runs inside the macOS OpenAgents app and serves iOS/macOS clients over WebSockets. It persistently stores data in SQLite and streams live updates to all connected clients.

Why Tinyvex?

- Swift‑first: Smooth Xcode tooling, great debugging/profiling, minimal packaging.
- Local‑first: Desktop‑embedded server; low latency; works offline; syncs across multiple local clients.
- Proven ideas: Adopts core patterns from Convex (document log, versioned transitions, backpressure) while optimizing for a desktop app runtime.

Scope

- macOS host server (SwiftNIO + GRDB + SQLite WAL)
- iOS/macOS Swift client (Combine + async/await)
- JSON‑RPC 2.0 over a persistent WebSocket
- ACP‑first transport: session lifecycle and streamed updates use ACP method names/payloads (`session/new`, `session/prompt`, `session/update`, `session/cancel`).
- Tinyvex‑namespaced methods (`tinyvex/*`) are only for persistence/query utilities (e.g., list/history snapshots), never for ACP session streams.
 - Canonical parameter encoding and per-subscription journaling for fast resume.

Non‑goals (for MVP)

- Cross‑platform server binaries (Linux/Windows)
- Cloud multi‑tenant scale and auth
- CRDTs and conflict resolution beyond single‑writer semantics
 - Full Convex parity (e.g., actions vs. mutations distinctions can be minimal initially)

Read next

- Architecture: Server components and concurrency model (actors/NIO)
- Protocol: JSON‑RPC methods and notifications
- SQLite Schema: Tables, indexes, and queries
- Client API: Swift subscribe/mutation interface
- Roadmap: Milestones and test plan

ACP Compliance

- On‑wire: All session lifecycle and streaming updates are 100% ACP compliant using the Swift ACP module in `ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/`.
- In storage: We persist ACP `SessionUpdate` events (JSON) as the canonical log and project the minimum additional tables needed for UI queries. Extra OpenAgents metadata is stored alongside via `_meta` fields.
 - Security: Local threat model and hardening plan

Delivery Semantics (Store‑first, Push‑second)

- Ingestion: All agent/CLI JSON‑L events first persist into Tinyvex (SQLite) on the desktop host. Tinyvex is the source of truth.
- Fan‑out: The WebSocket server then pushes updates to any connected clients via `tinyvex/data` notifications.
- Offline/Backfill: If a client is disconnected (app closed, OS suspended, network drop), no events are lost — the client resubscribes on reconnect and replays from the last seen sequence (or receives the latest snapshot), driven by the durable log.
- Benefit: Prevents missed events and keeps device state consistent without requiring continuous connectivity.
