# Khala WS Mapping (`openagents.sync.v1`)

Date: 2026-02-21
Status: Active (v1)

This document maps `proto/openagents/sync/v1/*.proto` messages to the Rust Khala WebSocket wire contract.

## Transport

- Live sync transport: WebSocket only.
- Channel topic: `sync:v1`.
- No SSE/poll live sync lanes for Khala.

## Auth

- Clients present control-service-minted sync JWT via socket `token` parameter.
- Runtime validates signature, issuer/audience, expiry, and topic scopes.
- Required client metadata in join params:
  - `client`
  - `client_build_id`
  - `protocol_version`
  - `schema_version`

## Compatibility Negotiation

Join/subscribe uses:

- `docs/adr/ADR-0005-compatibility-negotiation-and-support-window-policy.md`
- `docs/protocol/COMPATIBILITY_NEGOTIATION_POLICY.md`

Deterministic reject codes include:

- `invalid_client_build`
- `unsupported_protocol_version`
- `unsupported_schema_version`
- `upgrade_required`

## Canonical Events

Client -> server:

- `sync:subscribe`
- `sync:unsubscribe`
- `sync:heartbeat`

Server -> client:

- `sync:subscribed`
- `sync:frame` (canonical envelope)
- `sync:update_batch` (compatibility payload)
- `sync:heartbeat`
- `sync:error`

## Envelope and Ordering

- `sync:frame` carries `KhalaFrame`.
- `seq` is monotonic per topic.
- Logical ordering key is `(topic, seq)`.
- Delivery is at-least-once.
- Clients must discard duplicates where `seq <= last_applied(topic)`.

## Replay and Resume

1. Client subscribes with optional per-topic `resume_after` watermark.
2. Server replays missing events first.
3. Server transitions to live tail after replay completion.
4. If cursor is below retention floor, server returns `stale_cursor` and client must rebootstrap.

## Error Taxonomy

| SyncErrorCode | Meaning | Client action |
| --- | --- | --- |
| `SYNC_ERROR_CODE_UNAUTHORIZED` | token invalid/expired | refresh token + reconnect |
| `SYNC_ERROR_CODE_FORBIDDEN_TOPIC` | topic not allowed | remove topic + surface auth error |
| `SYNC_ERROR_CODE_BAD_SUBSCRIPTION` | malformed subscribe payload | fix payload + retry |
| `SYNC_ERROR_CODE_STALE_CURSOR` | cursor older than retention floor | reset watermark + snapshot bootstrap |
| `SYNC_ERROR_CODE_SLOW_CONSUMER` | outbound queue overflow | backoff + reconnect |
| `SYNC_ERROR_CODE_PAYLOAD_TOO_LARGE` | frame exceeds size policy | hydrate via HTTP path if provided |
| `SYNC_ERROR_CODE_RATE_LIMITED` | server throttled request | retry after `retry_after_ms` |
| `SYNC_ERROR_CODE_INTERNAL` | transient server fault | reconnect with jittered backoff |
