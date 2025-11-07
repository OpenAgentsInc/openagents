# Client API — Swift (iOS/macOS)

Goals

- Mirror Convex‑style ergonomics with a Swift‑native implementation.
- Combine publishers for subscriptions; async/await for mutations/actions.

API Surface (proposed)

- TinyvexClient
  - init(endpoint: URL)
  - func subscribe<T: Decodable>(_ name: String, params: Encodable, as: T.Type) -> AnyPublisher<T, TinyvexError>
  - func mutation<T: Decodable>(_ name: String, args: Encodable, as: T.Type) async throws -> T
  - func action<T: Decodable>(_ name: String, args: Encodable, as: T.Type) async throws -> T
  - func setAuthToken(_ token: String?)

Behavior

- subscribe
  - Opens (or reuses) a WebSocket and sends `tinyvex/subscribe` with a client‑chosen `subId`.
  - Decodes `tinyvex/data` notifications into `T` and publishes updates with increasing `seq`.
  - Cancellation unsubscribes upstream and closes the sub when no more subscribers are present.

- mutation/action
  - Sends `tinyvex/mutation` / `tinyvex/action`; awaits JSON‑RPC result and decodes to `T`.
  - Errors are surfaced as `TinyvexError.server(code:message:data:)` or `TinyvexError.decoding`.

Decoding/Encoding

- Use JSONEncoder/Decoder with explicit models; avoid ad‑hoc JSON string building.
- Avoid force‑unwrap/try!; convert `DecodingError` into `TinyvexError.decoding`.

Auth

- Optional `setAuthToken` mirrors Convex’s `setAuth(token:)` shape for future remote/cloud use; no storage policy baked in.

Resilience

- Auto‑reconnect with exponential backoff; resubscribe on reconnect with last seen `seq` per `subId`.
- Heartbeat handling (pong within 120s) and in‑flight request retries where safe (idempotent actions only).

