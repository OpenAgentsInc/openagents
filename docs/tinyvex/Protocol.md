# Protocol — JSON‑RPC over WebSocket (tinyvex/*)

Transport

- JSON‑RPC 2.0 messages over a single persistent WebSocket.
- Namespaced methods: `tinyvex/*` per ADR‑0004 to coexist with existing bridge methods.
- Server may send notifications at any time (data, error, pong, transitionChunk).

Correlation & Sequence

- Client supplies `id` for requests; server replies with matching `id`.
- Subscriptions are identified by `subId` (client‑chosen) and stream a monotonically increasing `seq` per subscription.
- Clients can resume by resending `subscribe` with `lastSeq` after reconnect.

Methods (Client → Server)

1) tinyvex/connect
- params: { client: { name, version }, clockSkewHint?: number }
- result: { serverVersion, nowTs, features: { chunks: boolean } }

2) tinyvex/subscribe
- params: { subId: string, name: string, params: any, lastSeq?: number }
- result: { accepted: true }
- semantics: Starts or resumes a stream keyed by (name, normalized(params)). Duplicate subscribers share a single computation.

3) tinyvex/unsubscribe
- params: { subId: string }
- result: { ok: true }

4) tinyvex/mutation
- params: { requestId: string, name: string, args: any }
- result: { requestId: string, value: any }
- semantics: Executes serially per server; triggers downstream subscription updates if results change.

5) tinyvex/action
- params: { requestId: string, name: string, args: any }
- result: { requestId: string, value: any }
- semantics: Like mutation but side‑effect semantics are looser (e.g., external calls), does not guarantee local state change.

6) tinyvex/ping
- params: { t?: number }
- result: { t?: number }

Notifications (Server → Client)

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

Errors

- Request errors use JSON‑RPC error response with { code, message, data? }.
- Stream errors use `tinyvex/error` with optional `subId`.

Resubscribe on Reconnect

- Client caches active `{ subId, name, params, lastSeq }` and resends `tinyvex/subscribe` for each.
- Server MAY fast‑path resume if journal still valid; otherwise emits a fresh value with a new `seq`.

Examples

- Subscribe request
  { "jsonrpc": "2.0", "id": 1, "method": "tinyvex/subscribe", "params": { "subId": "inbox", "name": "messagesByThread", "params": { "threadId": "t1" } } }

- Data notification
  { "jsonrpc": "2.0", "method": "tinyvex/data", "params": { "subId": "inbox", "seq": 3, "value": [ { "id": "m1", "text": "hi" } ] } }

