# Codex/Spacetime -> Vercel SSE Compatibility Mapping (v1)

Date: 2026-02-25
Status: Active compatibility contract
Owner: `owner:openagents-web`

This document defines the adapter-only mapping from Codex authority events delivered through Spacetime sync into Vercel-compatible SSE events.

Archived historical mapping:
- `docs/protocol/archived/codex-spacetime-vercel-sse-mapping-v1.md`

Authority boundaries:

1. Codex worker control remains mutation authority.
2. Spacetime sync is delivery/replay transport.
3. SSE is serialization compatibility only (ADR-0008), not an authority lane.

## Scope

Mapped endpoints:

1. `POST /api/chat/stream`
2. `POST /api/chats/{conversationId}/stream`

## Required Deterministic Output Order

For successful turns:

1. `start`
2. `start-step`
3. `text-start` (optional)
4. `text-delta` (zero or more)
5. `tool-input` / `tool-output` (zero or more)
6. `finish-step`
7. `finish`
8. `data: [DONE]`

Rules:

1. Input events are consumed in stable `(seq ASC)` order within stream.
2. Adapter must not emit duplicate terminal markers.
3. `[DONE]` is always final output frame.

## Error Semantics

1. Pre-stream failure: JSON error envelope (`4xx/5xx`).
2. In-stream failure: deterministic terminal `error`, terminal `finish` (`status=error`) when needed, then `[DONE]`.

## Replay and Resume

1. Adapter preserves replay ordering from source stream.
2. Duplicate source events are ignored by idempotency key/sequence guard.
3. Replay metadata may be attached to adapter payload metadata, but must not alter ordering.

## Headers

Success responses include:

1. `Content-Type: text/event-stream; charset=utf-8`
2. `Cache-Control: no-cache, no-transform`
3. `Connection: keep-alive`
4. `X-Accel-Buffering: no`

Compatibility headers follow control compatibility policy.

## References

- `docs/adr/ADR-0008-bounded-vercel-sse-compatibility-lane.md`
- `docs/protocol/SPACETIME_SYNC_TRANSPORT_MAPPING.md`
- `docs/protocol/codex-worker-events-v1.md`
- `docs/protocol/COMPATIBILITY_NEGOTIATION_POLICY.md`
