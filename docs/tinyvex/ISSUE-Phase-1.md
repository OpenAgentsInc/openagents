# Tinyvex Phase 1 — ACP‑First Local Sync Server (SwiftNIO + GRDB)

Labels: area:tinyvex, type:feature, priority:P0
Assignees: TBD

## Summary

Build the first working slice of Tinyvex: a Swift‑only, desktop‑embedded WebSocket server backed by SQLite (GRDB) that:
- Accepts ACP session lifecycle calls and emits ACP `session/update` notifications in real time.
- Persists every ACP `SessionUpdate` to a durable `acp_events` log.
- Exposes minimal Tinyvex utility methods for queries/history.
- Provides a Swift client with Combine/async APIs mirroring Convex‑style ergonomics.

## Background

- ADR‑0002 establishes ACP as the canonical runtime contract for our app. All on‑wire session traffic must be ACP‑compliant.
- ADR‑0004 defines our WS bridge (JSON‑RPC 2.0). Tinyvex coexists on this bridge, using ACP method names for sessions and `tinyvex/*` only for persistence utilities.
- The Tinyvex docs outline architecture, protocol, schema, client API, and testing.

References
- docs/adr/0002-agent-client-protocol.md:1
- docs/tinyvex/Architecture.md:1
- docs/tinyvex/Protocol.md:1
- docs/tinyvex/SQLite-Schema.md:1
- docs/tinyvex/Client-API.md:1
- docs/tinyvex/Backpressure-and-Chunking.md:1
- docs/tinyvex/Errors.md:1
- docs/tinyvex/Implementation-Plan.md:1

## Goals (Phase 1)

- ACP‑first transport: implement `initialize`, `session/new`, `session/prompt`, `session/cancel`, and `session/update` notification end‑to‑end.
- Store‑first delivery: persist `SessionUpdate` to SQLite before fan‑out to clients.
- Minimal Tinyvex utilities: a single streaming `tinyvex/subscribe` for a demo query, and a `tinyvex/history.sessionUpdates` backfill endpoint.
- Swift client SDK: ACP lifecycle methods, `subscribeSessionUpdates(...)`, and demo tinyvex subscribe/mutation.
- Tests: unit + integration validating live fan‑out and offline backfill.

## Non‑Goals (Phase 1)

- TLS/mTLS or pairing; discovery (Bonjour) — tracked for later phases.
- Complex query catalog or search; large set of projections.
- Advanced backpressure policies beyond coalescing and chunking defaults.

## Deliverables

- New SwiftPM targets (in OpenAgents workspace):
  - TinyvexCore (models, JSON‑RPC envelopes, error types)
  - TinyvexServer (SwiftNIO server, ConnectionManager, SubscriptionManager, DbLayer)
  - TinyvexClient (Swift client SDK: Combine + async/await)
- GRDB migrations for `acp_events` + minimal projections
- WebSocket server handling JSON‑RPC 2.0 envelopes
- ACP session lifecycle handlers and `session/update` dispatcher
- Tinyvex utilities: `tinyvex/connect`, `tinyvex/subscribe` (demo query), `tinyvex/history.sessionUpdates`
- Tests and basic metrics/logging

## Tasks

1) Package and Targets
- [ ] Add SwiftPM targets TinyvexCore, TinyvexServer, TinyvexClient to the `OpenAgents.xcworkspace` (under `ios/OpenAgentsCore` or as sibling SPM packages).
- [ ] Wire targets into the macOS app (server) and iOS/macOS apps (client).

2) Protocol & Server Skeleton
- [ ] Implement JSON‑RPC codec and router (SwiftNIO) with validation (no `try!`).
- [ ] Implement heartbeat (ping/pong) and idle timeout (120s).
- [ ] Define error mapping per docs/tinyvex/Errors.md:1.

3) Persistence (GRDB)
- [ ] Set PRAGMA: WAL, synchronous=NORMAL, busy_timeout=5000; explicit checkpointing.
- [ ] Create `acp_events` table + indexes (docs/tinyvex/SQLite-Schema.md:ACP Event Log).
- [ ] Add optional minimal projections: `sessions`, `messages`, `tool_calls`.
- [ ] Implement DbLayer actor: append SessionUpdate, read ranges by (session_id, seq/ts).

4) ACP Lifecycle + Dispatcher
- [ ] `initialize` (ACP): accept `ACP.Agent.InitializeRequest`, return `InitializeResponse`.
- [ ] `session/new` (ACP): create session row, return id.
- [ ] `session/prompt` (ACP): accept prompt; invoke agent runner stub; return ok.
- [ ] Ingestion path: from agent runner stub → translate to ACP `SessionUpdate` → DbLayer.append → dispatcher fan‑out `session/update`.
- [ ] `session/cancel` (ACP): cancel any in‑flight work for the session.

5) Tinyvex Utilities
- [ ] `tinyvex/connect`: return { serverVersion, nowTs, features: { chunks, maxChunkBytes }, sessionId? }.
- [ ] `tinyvex/history.sessionUpdates`: params { session_id, since_seq? | since_ts? , limit? } → array of ACP `SessionNotificationWire`.
- [ ] `tinyvex/subscribe` (demo query): one ValueObservation (e.g., per‑session message count) with coalescing + seq.

6) Backpressure & Chunking
- [ ] Per‑connection bounded outbound queue (default 100), coalesce subscription updates.
- [ ] Chunk payloads > 64 KiB into `tinyvex/transitionChunk` and reassemble client‑side.
- [ ] Ensure ACP ordering is preserved; drop only superseded intermediate chunks of the same discriminator.

7) Client SDK (Swift)
- [ ] TinyvexClient: ACP lifecycle APIs (initialize/sessionNew/sessionPrompt/sessionCancel).
- [ ] `subscribeSessionUpdates(sessionId:) -> AnyPublisher<ACP.Client.SessionNotificationWire, TinyvexError>`
- [ ] tinyvex subscribe/mutation stubs for demo query.
- [ ] Connection state publisher; reconnection + resubscribe for tinyvex streams; ACP fan‑out is push‑only.

8) Tests & Observability
- [ ] Unit: DbLayer CRUD, JSON‑RPC codec, dispatcher wiring, param canonicalization.
- [ ] Integration: two clients receive live `session/update`; offline client reconnects and backfills via history endpoint.
- [ ] Large payload chunking: server chunk → client reassemble.
- [ ] Metrics/logging: queue depth, dropped/coalesced, chunk counts; os.Logger categories.

9) Docs
- [ ] Update docs/tinyvex to reflect any protocol tweaks discovered during implementation.
- [ ] Add example code snippets for client usage.

## Acceptance Criteria

- A simulated agent produces ACP `SessionUpdate`s that are appended to `acp_events` and immediately delivered over WS to connected clients.
- Killing the client connection and reconnecting allows fetching missed events via `tinyvex/history.sessionUpdates`.
- Demo `tinyvex/subscribe` stream publishes updates to multiple clients and coalesces under backpressure.
- No force‑unwrap/`try!` in server/client message handling paths; all request validation returns JSON‑RPC errors.
- Basic metrics are logged; chunking is exercised in tests.

## Risks & Mitigations

- ACP ordering under backpressure — preserve order, only drop superseded intermediate chunks; add tests.
- SQLite contention — single writer actor, WAL, busy_timeout, and explicit checkpoints.
- JSON drift — use shared ACP Swift types (`ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/`) for all ACP payloads.
- Resume semantics — limit Phase 1 to history backfill endpoint; defer streaming resume until Phase 2.

## Tracking

- Epic: Tinyvex (Phase 1)
- Follow‑ups: TLS/pairing, discovery, richer projections, streaming resume, metrics dashboard.

