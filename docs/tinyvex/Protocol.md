# Protocol — JSON‑RPC over WebSocket (tinyvex/*)

Transport

- JSON‑RPC 2.0 messages over a single persistent WebSocket.
- ACP‑first: Use ACP method names and payloads for session lifecycle and streaming (`session/new`, `session/prompt`, `session/cancel`, `session/update`).
- Tinyvex‑namespaced methods: Reserved for persistence/query utilities (e.g., list/history snapshots) that are not ACP‑defined.
- Server may send notifications at any time (ACP `session/update`, tinyvex/data, error, pong, transitionChunk).
- All requests MUST include `"jsonrpc": "2.0"`, `method`, and either `id` (request) or omit `id` (notification).

Correlation & Sequence

- Client supplies `id` for requests; server replies with matching `id`.
- Subscriptions are identified by `subId` (client‑chosen) and stream a monotonically increasing `seq` per subscription.
- Clients can resume by resending `subscribe` with `lastSeq` after reconnect.
- The server MAY return a `journal` identifier to enable fast resume within a retention window.

Methods (Client → Server)

1) initialize (ACP)
- params: ACP.Agent.InitializeRequest
- result: ACP.Agent.InitializeResponse

2) session/new (ACP)
- params: ACP.Agent.SessionNewRequest
- result: ACP.Agent.SessionNewResponse

3) session/prompt (ACP)
- params: ACP.Agent.SessionPromptRequest (prompt is array of ACP.Client.ContentBlock)
- result: { ok: true }

4) session/cancel (ACP, notification)
- params: { session_id }
- result: —

5) tinyvex/connect
- params: { client: { name, version }, clockSkewHint?: number }
- result: { serverVersion, nowTs, features: { chunks: boolean, maxChunkBytes: number }, sessionId: string }

6) tinyvex/subscribe (non‑ACP streams)
- params: { subId: string, name: string, params: any, lastSeq?: number, journal?: string }
- result: { accepted: true, resumed: boolean, startSeq: number }
- semantics: Starts or resumes a stream keyed by (name, normalized(params)). Duplicate subscribers share a single computation.

7) tinyvex/unsubscribe
- params: { subId: string }
- result: { ok: true }

8) tinyvex/mutation
- params: { requestId: string, name: string, args: any }
- result: { requestId: string, value: any }
- semantics: Executes serially per server; triggers downstream subscription updates if results change.

9) tinyvex/action
- params: { requestId: string, name: string, args: any }
- result: { requestId: string, value: any }
- semantics: Like mutation but side‑effect semantics are looser (e.g., external calls), does not guarantee local state change.

10) tinyvex/auth.setToken (optional)
- params: { token: string | null }
- result: { ok: true }
- semantics: Applies/clears a bearer token for the connection; unused in pure local mode but future‑proofs the wire protocol.

11) tinyvex/ping
- params: { t?: number }
- result: { t?: number }

Notifications (Server → Client)

- session/update (ACP)
  - params: ACP.Client.SessionNotificationWire { session_id, update: SessionUpdate, _meta? }
- tinyvex/data
  - params: { subId: string, seq: number, value: any, journal?: string }
- tinyvex/error
  - params: { subId?: string, code: string, message: string, data?: any }
- tinyvex/mutate_result
  - params: { requestId: string, value?: any, error?: { code, message, data? } }
- tinyvex/pong
  - params: { t?: number }
- tinyvex/transitionChunk (optional)
  - params: { subId: string, seq: number, chunk: string, part: number, total: number, transitionId: string }

Payload Guidelines

- JSON encoding: Standard JSON (no special $integer/$float wrappers). Use explicit string/number mapping in schemas.
- Size limits: Servers MAY chunk `value` if serialized size exceeds a configured threshold; clients MUST reassemble.
- Ordering: For a given `subId`, messages with higher `seq` supersede lower ones; clients SHOULD drop older in‑flight chunks once a newer complete value arrives.
- Canonical params: Clients SHOULD send params as canonical JSON (sorted keys). Servers MUST canonicalize before keying subscriptions.

Errors

- Request errors use JSON‑RPC error response with { code, message, data? }. See Errors.md for code taxonomy.
- Stream errors use `tinyvex/error` with optional `subId`.
 - ACP method errors follow the same JSON‑RPC envelope; do not wrap ACP payloads.

Resubscribe on Reconnect

- Client caches active `{ subId, name, params, lastSeq }` and resends `tinyvex/subscribe` for each.
- Server MAY fast‑path resume if journal still valid; otherwise emits a fresh value with a new `seq`.

Durable Delivery Clarification

- The server persists all updates from the agent/CLI into SQLite before notifying clients.
- If a client is offline, upon reconnection it will receive the latest snapshot (and, where supported, replay deltas from `lastSeq`). This avoids missed events and ensures consistency with the desktop source‑of‑truth.

Examples

- Subscribe request
  { "jsonrpc": "2.0", "id": 1, "method": "tinyvex/subscribe", "params": { "subId": "inbox", "name": "messagesByThread", "params": { "threadId": "t1" } } }

- Data notification
  { "jsonrpc": "2.0", "method": "tinyvex/data", "params": { "subId": "inbox", "seq": 3, "value": [ { "id": "m1", "text": "hi" } ] } }

- Error response (invalid params)
  { "jsonrpc": "2.0", "id": 42, "error": { "code": -32602, "message": "Invalid params", "data": { "path": ["params","threadId"] } } }
