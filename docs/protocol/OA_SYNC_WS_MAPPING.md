# Khala WS Mapping (`openagents.sync.v1`)

Date: 2026-02-20
Status: Proposed (v1)

This document maps `proto/openagents/sync/v1/*.proto` messages to the Phoenix Channel wire format used by Khala.

## Transport

- Live sync transport: WebSocket only.
- Runtime endpoint: Phoenix socket.
- Channel topic for v1: `sync:v1`.
- Existing SSE endpoints are not part of this protocol.

## Auth (v1)

- Clients present Laravel-minted JWT via socket `token` param.
- Runtime validates `alg`, `kid`, signature, and required claims.
- HS256 is active with `kid` keyring rotation (current + previous keys).
- Claim checks include issuer/audience/claims_version and topic scopes (`oa_sync_scopes`).

## Compatibility Negotiation (v1)

Khala join/subscribe paths follow the shared compatibility policy:

- `docs/adr/ADR-0005-compatibility-negotiation-and-support-window-policy.md`
- `docs/protocol/COMPATIBILITY_NEGOTIATION_POLICY.md`

Required client metadata for negotiation:

1. `client_build_id`
2. `protocol_version`
3. `schema_version`

Deterministic compatibility failure codes:

- `invalid_client_build`
- `unsupported_protocol_version`
- `unsupported_schema_version`
- `upgrade_required`
- `unsupported_client_build`

Failure payloads must include active support-window metadata (`min_client_build_id`, `max_client_build_id`, schema min/max, protocol version) so clients can deterministically block reconnect loops and prompt upgrade.

## Event Names

Client -> server:

- `sync:subscribe` (`Subscribe`)
- `sync:unsubscribe` (topic list subset; no proto message in v1)
- `sync:heartbeat` (keepalive; returns current topic watermarks)

Server -> client:

- `sync:subscribed` (`Subscribed`)
- `sync:frame` (`KhalaFrame`) **canonical envelope**
- `sync:update_batch` (`UpdateBatch`)
- `sync:heartbeat` (`Heartbeat`)
- `sync:error` (`Error`)

Compatibility note:

- `sync:frame` is the canonical server->client envelope for replay/live/error delivery.
- Legacy event payloads (`sync:update_batch`, `sync:heartbeat`, `sync:error`) remain for transition compatibility and carry equivalent content.

## Encoding Rules

1. Envelope payloads use proto-JSON field names (snake_case).
2. Enum values are encoded as enum names (for example `SYNC_TOPIC_RUNTIME_RUN_SUMMARIES`).
3. `payload` in `Update` is base64-encoded bytes in JSON transport.
4. `payload_hash` in `Update` is base64-encoded bytes in JSON transport.
5. Watermarks and versions are represented as JSON numbers where safe; string fallback is allowed if a client platform cannot safely represent `uint64`.
6. `KhalaFrame.payload_bytes` is base64-encoded protobuf-bytes field; v1 transport payload is JSON-encoded payload bytes for the frame kind.
7. `KhalaFrame.schema_version` starts at `1` and must be validated by clients before decoding payload bytes.

## Message Mapping

| Channel event | Proto message | Required fields |
|---|---|---|
| `sync:subscribe` | `Subscribe` | `topics[]`, optional `resume_after[]`, optional `request_id` |
| `sync:subscribed` | `Subscribed` | `subscription_id`, `current_watermarks[]` |
| `sync:frame` | `KhalaFrame` | `topic`, `seq`, `kind`, `payload_bytes`, `schema_version` |
| `sync:update_batch` | `UpdateBatch` | `updates[]`, `replay_complete`, `head_watermarks[]` |
| `sync:heartbeat` | `Heartbeat` | `watermarks[]` |
| `sync:error` | `Error` | `code`, `message`, optional `retry_after_ms`, optional `full_resync_required` |

Server runtime behavior:

1. Runtime emits periodic `sync:heartbeat` messages on subscribed channels.
2. Client may send `sync:heartbeat` to refresh liveness and fetch current watermarks.
3. Runtime can close idle channels after heartbeat timeout.

## Replay and Batch Limits (v1)

Server-side limits (default):

- max updates per batch: `200`
- max payload per update: `256KB`
- max payload per batch: `2MB`

Replay behavior:

1. On `sync:subscribe`, server replays `watermark > resume_after` for each topic.
2. Server emits one or more `sync:update_batch` events.
3. When caught up to head, `replay_complete=true` is emitted and channel transitions to live updates.

Frame behavior (v1):

1. Replay/live payloads are emitted as `sync:frame` with `kind=KHALA_FRAME_KIND_UPDATE_BATCH`.
2. Error payloads (including stale cursor) are emitted as `sync:frame` with `kind=KHALA_FRAME_KIND_ERROR`.
3. `seq` is monotonic by topic and represents the highest watermark included in the frame payload.
4. Clients must discard frames with `seq <= last_applied(topic)`.

## Pointer Mode Behavior

Khala stream rows may omit inlined payloads.

When payload is omitted or truncated:

- `Update.hydration_required=true`
- `Update.payload` may be empty
- client fetches authoritative doc via HTTP hydration endpoint and updates local cache/doc version

Protocol remains unchanged between inline and pointer modes.

## Error Taxonomy (v1)

| `SyncErrorCode` | Meaning | Client action |
|---|---|---|
| `SYNC_ERROR_CODE_UNAUTHORIZED` | token invalid/expired | refresh token, reconnect |
| `SYNC_ERROR_CODE_FORBIDDEN_TOPIC` | subscription not allowed | remove forbidden topic, surface auth error |
| `SYNC_ERROR_CODE_BAD_SUBSCRIPTION` | malformed payload/request | fix client request, retry |
| `SYNC_ERROR_CODE_STALE_CURSOR` | resume watermark too old | full hydration + reset watermark |
| `SYNC_ERROR_CODE_PAYLOAD_TOO_LARGE` | payload exceeds configured limit | hydrate via HTTP doc endpoint |
| `SYNC_ERROR_CODE_RATE_LIMITED` | server throttled request | retry after `retry_after_ms` |
| `SYNC_ERROR_CODE_INTERNAL` | transient server fault | reconnect with backoff |

`SYNC_ERROR_CODE_STALE_CURSOR` should set `full_resync_required=true`.

Current runtime payload shape for stale cursor (`sync:error` event and subscribe error reply):

```json
{
  "code": "stale_cursor",
  "message": "cursor is older than retention floor",
  "full_resync_required": true,
  "stale_topics": [
    {
      "topic": "runtime.run_summaries",
      "resume_after": 10,
      "retention_floor": 42
    }
  ]
}
```

## Example: Subscribe

```json
{
  "event": "sync:subscribe",
  "payload": {
    "topics": [
      "SYNC_TOPIC_RUNTIME_CODEX_WORKER_SUMMARIES"
    ],
    "resume_after": [
      {
        "topic": "SYNC_TOPIC_RUNTIME_CODEX_WORKER_SUMMARIES",
        "watermark": 1042
      }
    ]
  }
}
```

## Example: Update Batch

```json
{
  "event": "sync:update_batch",
  "payload": {
    "updates": [
      {
        "topic": "SYNC_TOPIC_RUNTIME_CODEX_WORKER_SUMMARIES",
        "doc_key": "org_123:worker_456",
        "doc_version": 22,
        "payload": "eyJzdGF0dXMiOiJydW5uaW5nIn0=",
        "watermark": 1043,
        "payload_hash": "3qm7mQ==",
        "hydration_required": false
      }
    ],
    "replay_complete": false,
    "head_watermarks": [
      {
        "topic": "SYNC_TOPIC_RUNTIME_CODEX_WORKER_SUMMARIES",
        "watermark": 1049
      }
    ]
  }
}
```

## Example: Khala Frame (Replay/Live)

```json
{
  "event": "sync:frame",
  "payload": {
    "topic": "runtime.codex_worker_summaries",
    "seq": 1043,
    "kind": "KHALA_FRAME_KIND_UPDATE_BATCH",
    "payload_bytes": "eyJ1cGRhdGVzIjpbeyJ0b3BpYyI6InJ1bnRpbWUuY29kZXhfd29ya2VyX3N1bW1hcmllcyIsImRvY19rZXkiOiJvcmdfMTIzOndvcmtlcl80NTYiLCJkb2NfdmVyc2lvbiI6MjIsInBheWxvYWQiOnsic3RhdHVzIjoicnVubmluZyJ9LCJ3YXRlcm1hcmsiOjEwNDMsInBheWxvYWRfaGFzaCI6IjNxbTdtUT09IiwiaHlkcmF0aW9uX3JlcXVpcmVkIjpmYWxzZX1dLCJyZXBsYXlfY29tcGxldGUiOmZhbHNlLCJoZWFkX3dhdGVybWFya3MiOlt7InRvcGljIjoicnVudGltZS5jb2RleF93b3JrZXJfc3VtbWFyaWVzIiwid2F0ZXJtYXJrIjoxMDQ5fV19",
    "schema_version": 1
  }
}
```

## Example: Khala Frame (Stale Cursor Error)

```json
{
  "event": "sync:frame",
  "payload": {
    "topic": "runtime.run_summaries",
    "seq": 0,
    "kind": "KHALA_FRAME_KIND_ERROR",
    "payload_bytes": "eyJjb2RlIjoic3RhbGVfY3Vyc29yIiwibWVzc2FnZSI6ImN1cnNvciBpcyBvbGRlciB0aGFuIHJldGVudGlvbiBmbG9vciIsImZ1bGxfcmVzeW5jX3JlcXVpcmVkIjp0cnVlLCJzdGFsZV90b3BpY3MiOlt7InRvcGljIjoicnVudGltZS5ydW5fc3VtbWFyaWVzIiwicmVzdW1lX2FmdGVyIjoxMCwicmV0ZW50aW9uX2Zsb29yIjo0Mn1dfQ==",
    "schema_version": 1
  }
}
```

## References

- `proto/openagents/sync/v1/sync.proto`
- `proto/openagents/sync/v1/topics.proto`
- `proto/openagents/sync/v1/errors.proto`
- `docs/protocol/fixtures/khala-frame-v1.json`
- `docs/sync/thoughts.md`
