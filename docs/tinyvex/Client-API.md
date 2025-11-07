# Client API — Swift (iOS/macOS)

Goals

- Mirror Convex‑style ergonomics with a Swift‑native implementation.
- Combine publishers for subscriptions; async/await for mutations/actions.

API Surface (proposed)

- TinyvexClient
  - init(endpoint: URL)
  - // ACP session lifecycle (passthrough, ACP‑compliant)
  - func initialize(_ req: ACP.Agent.InitializeRequest) async throws -> ACP.Agent.InitializeResponse
  - func sessionNew(_ req: ACP.Agent.SessionNewRequest) async throws -> ACP.Agent.SessionNewResponse
  - func sessionPrompt(_ req: ACP.Agent.SessionPromptRequest) async throws
  - func sessionCancel(sessionId: ACPSessionId) async
  - func subscribe<T: Decodable>(_ name: String, params: Encodable, as: T.Type) -> AnyPublisher<T, TinyvexError>
  - func mutation<T: Decodable>(_ name: String, args: Encodable, as: T.Type) async throws -> T
  - func action<T: Decodable>(_ name: String, args: Encodable, as: T.Type) async throws -> T
  - func setAuthToken(_ token: String?)
  - var connectionState: AnyPublisher<ConnectionState, Never> { get }
  - func subscribe<T: Decodable>(_ name: String, params: Encodable, as: T.Type) -> AsyncThrowingStream<T, Error>
  - // ACP streaming updates
  - func subscribeSessionUpdates(sessionId: ACPSessionId) -> AnyPublisher<ACP.Client.SessionNotificationWire, TinyvexError>

Behavior

- subscribe
  - Opens (or reuses) a WebSocket and sends `tinyvex/subscribe` with a client‑chosen `subId`.
  - Decodes `tinyvex/data` notifications into `T` and publishes updates with increasing `seq`.
  - Cancellation unsubscribes upstream and closes the sub when no more subscribers are present.
  - Resubscribes after reconnect with `lastSeq` to resume if possible.
  - For ACP session streams, use `subscribeSessionUpdates`: the server delivers ACP‑compliant `session/update` notifications (no tinyvex wrapper).

- mutation/action
  - Sends `tinyvex/mutation` / `tinyvex/action`; awaits JSON‑RPC result and decodes to `T`.
  - Errors are surfaced as `TinyvexError.server(code:message:data:)` or `TinyvexError.decoding`.

Decoding/Encoding

- Use JSONEncoder/Decoder with explicit models; avoid ad‑hoc JSON string building.
- Avoid force‑unwrap/try!; convert `DecodingError` into `TinyvexError.decoding`.
 - Canonicalize params with `JSONEncoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]` for dedupe.

Auth

- Optional `setAuthToken` mirrors Convex’s `setAuth(token:)` shape for future remote/cloud use; no storage policy baked in.

Resilience

- Auto‑reconnect with exponential backoff; resubscribe on reconnect with last seen `seq` per `subId`.
- Heartbeat handling (pong within 120s) and in‑flight request retries where safe (idempotent actions only).
 - Backpressure-aware send queue with per-connection bounds; coalesce subscription updates when queue is over capacity.
 - ACP `session/update` streams are not re-shaped; the server may coalesce only by dropping intermediate thought/message chunks per policy while ensuring order and delivering the latest state.
