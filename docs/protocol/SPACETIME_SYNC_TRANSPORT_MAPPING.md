# Spacetime Sync Transport Mapping (`spacetime.sync.v1`)

Date: 2026-02-25
Status: Active canonical mapping

This document defines the canonical retained-client sync transport contract for OpenAgents.

Archived historical mapping:
- `docs/protocol/archived/oa-sync-ws-legacy-mapping-v1.md`

## Transport Contract

- Canonical protocol version: `spacetime.sync.v1`
- Canonical transport: Spacetime subscription stream
- Commands/mutations remain HTTP authority operations (`INV-02`)
- Sync transport is delivery/projection only (`INV-06`)

## Authentication

Clients obtain scoped sync claims from control service:

- canonical endpoint: `POST /api/sync/token`
- retired aliases: `/api/spacetime/token`, `/api/v1/spacetime/token`, `/api/v1/sync/token`

Claims must bind:

1. audience and issuer
2. scope set (for stream access)
3. expiry and refresh window

## Compatibility Negotiation

Compatibility window and deterministic rejection payloads are governed by:

- `docs/protocol/COMPATIBILITY_NEGOTIATION_POLICY.md`

Required handshake fields:

- `client_build_id`
- `protocol_version`
- `schema_version`

`protocol_version` must match `spacetime.sync.v1` for retained lanes.

## Delivery and Ordering

- Ordering key: `(stream_id, seq)`
- Delivery mode: at-least-once
- Client apply must be idempotent (discard `seq <= last_applied(stream_id)`)
- Replay/bootstrap then live tail is required behavior

## Replay/Resume Rules

1. Client subscribes with optional `resume_after` checkpoint per stream.
2. Server returns deterministic replay batch before live updates.
3. If checkpoint is below retention floor, server returns stale-cursor error.
4. Client must rebootstrap and continue with refreshed checkpoint.

## Error Taxonomy

Representative deterministic classes:

- `unauthorized`
- `forbidden_topic`
- `bad_subscription`
- `stale_cursor`
- `slow_consumer`
- `rate_limited`
- `internal`

Exact wire envelopes are governed by proto contracts and runtime implementations.

## References

- `docs/adr/ADR-0007-spacetime-only-sync-transport-hard-mandate.md`
- `docs/plans/rust-migration-invariant-gates.md`
- `docs/sync/SPACETIME_TOKEN_SCOPE_AND_ROTATION.md`
- `docs/sync/SPACETIME_TOPIC_STREAM_CURSOR_CONTINUITY.md`
