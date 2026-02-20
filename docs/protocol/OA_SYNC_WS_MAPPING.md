# Khala WS Mapping (`openagents.sync.v1`)

Date: 2026-02-20
Status: Proposed (v1)

This document maps `proto/openagents/sync/v1/*.proto` messages to the Phoenix Channel wire format used by Khala.

## Transport

- Live sync transport: WebSocket only.
- Runtime endpoint: Phoenix socket.
- Channel topic for v1: `sync:v1`.
- Existing SSE endpoints are not part of this protocol.

## Event Names

Client -> server:

- `sync:subscribe` (`Subscribe`)
- `sync:unsubscribe` (topic list subset; no proto message in v1)
- `sync:heartbeat` (keepalive; returns current topic watermarks)

Server -> client:

- `sync:subscribed` (`Subscribed`)
- `sync:update_batch` (`UpdateBatch`)
- `sync:heartbeat` (`Heartbeat`)
- `sync:error` (`Error`)

## Encoding Rules

1. Envelope payloads use proto-JSON field names (snake_case).
2. Enum values are encoded as enum names (for example `SYNC_TOPIC_RUNTIME_RUN_SUMMARIES`).
3. `payload` in `Update` is base64-encoded bytes in JSON transport.
4. `payload_hash` in `Update` is base64-encoded bytes in JSON transport.
5. Watermarks and versions are represented as JSON numbers where safe; string fallback is allowed if a client platform cannot safely represent `uint64`.

## Message Mapping

| Channel event | Proto message | Required fields |
|---|---|---|
| `sync:subscribe` | `Subscribe` | `topics[]`, optional `resume_after[]`, optional `request_id` |
| `sync:subscribed` | `Subscribed` | `subscription_id`, `current_watermarks[]` |
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

## References

- `proto/openagents/sync/v1/sync.proto`
- `proto/openagents/sync/v1/topics.proto`
- `proto/openagents/sync/v1/errors.proto`
- `docs/sync/thoughts.md`
