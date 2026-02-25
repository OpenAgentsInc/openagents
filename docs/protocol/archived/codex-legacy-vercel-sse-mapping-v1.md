# Codex/Spacetime -> Vercel SSE Compatibility Mapping (v1)

Date: 2026-02-22
Status: Active compatibility contract
Owner: `owner:openagents-web`

This document defines the adapter-only mapping from codex/Spacetime authority events to Vercel-compatible SSE stream events.

Authority boundaries:

1. Codex worker control remains mutation authority.
2. Spacetime WS remains live/replay authority transport.
3. SSE is serialization compatibility only (ADR-0008), not an authority lane.

## Scope

Mapped endpoints:

1. `POST /api/chat/stream`
2. `POST /api/chats/{conversationId}/stream`

These endpoints expose Vercel-compatible stream envelopes over existing codex authority outputs.

## Required Deterministic Output Order

For successful turns, adapter emits exactly this terminal order:

1. `start` (exactly once)
2. `start-step` (exactly once per turn)
3. `text-start` (optional; at most once per assistant text item)
4. `text-delta` (zero or more)
5. `tool-input` / `tool-output` (zero or more, in observed source order)
6. `finish-step` (exactly once)
7. `finish` (exactly once)
8. `data: [DONE]` sentinel (exactly once)

Rules:

1. Input events are consumed in stable `(seq ASC)` order.
2. Adapter must never emit duplicate `start`, `start-step`, `finish-step`, or `finish`.
3. `[DONE]` is always final output frame.
4. Replay frames preserve source ordering; adapter must not reorder by timestamp.

## Event Mapping Table

| Codex source event | Mapping output | Notes |
|---|---|---|
| `thread/started` | `start` | Emits stream/thread metadata only once. |
| `turn/started` | `start-step` | Creates active step context. |
| `item/started` (`item_kind=agent_message`) | `text-start` | Emitted before first text delta for that item if not already emitted. |
| `item/agentMessage/delta` | `text-delta` | Assistant text chunk. |
| `item/reasoning/summaryTextDelta` | `text-delta` | Tagged as reasoning channel metadata. |
| `item/started` (`item_kind=mcp_tool_call`) | `tool-input` | Adapter emits normalized tool name/arguments envelope. |
| `item/toolOutput/delta` | `tool-output` | Tool output chunk in source order. |
| `item/completed` (`item_kind=mcp_tool_call`) | `tool-output` | Final tool completion marker if needed for pairing closure. |
| `turn/completed` | `finish-step` then `finish` | Includes normalized token usage/status metadata. |

## Tool Pairing Semantics

1. `tool-input` is emitted once per tool call item.
2. `tool-output` frames for that item are emitted in source order.
3. If a tool item completes without deltas, adapter emits one terminal `tool-output` with `status=completed`.
4. Tool frames must include stable `toolCallId`/item identity so UI can pair call/result.

## Error Semantics

Two error classes are allowed:

1. Pre-stream failures (no SSE bytes written):
   - Return normal JSON error envelope with HTTP status (`4xx/5xx`).
   - Examples: auth failure, compatibility rejection, malformed payload, unknown thread.
2. In-stream failures (after SSE begins):
   - Emit terminal `error` event with deterministic `{code,message,retryable}` payload.
   - Emit terminal `finish` with `status=error` (unless stream already finished).
   - Emit final `data: [DONE]`.

Codex terminal error mapping:

1. `turn/failed`, `turn/aborted`, `turn/interrupted`, `codex_error` -> in-stream terminal `error`.
2. `stale_cursor` / `reauth_required` from replay layer map to deterministic error code with `retryable`/`reauthRequired` flags.

## Replay and Resume Semantics

1. Adapter accepts replay-derived frames and preserves their source order.
2. Replay metadata may be included in event payload metadata but must not alter event ordering.
3. Duplicate source events (`seq <= last_seen`) are ignored to preserve idempotent output.

## Unknown and Unsupported Event Policy

1. Unknown codex event methods are ignored for output and recorded in adapter telemetry.
2. Unknown payload shapes for required events produce terminal in-stream `error` (`code=adapter_mapping_error`) if stream already started.
3. Unknown payload shapes before stream start return JSON error (`422`) with deterministic code (`invalid_event_payload`).
4. Unknown events must not cause out-of-order emission for known events.

## SSE Header Contract

Success responses must include:

1. `Content-Type: text/event-stream; charset=utf-8`
2. `Cache-Control: no-cache, no-transform`
3. `Connection: keep-alive`
4. `X-Accel-Buffering: no`
5. Compatibility window headers from control policy when available:
   - `x-oa-protocol-version`
   - `x-oa-compatibility-min-client-build-id`
   - `x-oa-compatibility-max-client-build-id` (optional)
   - `x-oa-compatibility-min-schema-version`
   - `x-oa-compatibility-max-schema-version`

Retired alias headers are not part of this contract.

## References

- `docs/adr/ADR-0003-spacetime-ws-only-replay-transport.md`
- `docs/adr/ADR-0008-bounded-vercel-sse-compatibility-lane.md`
- `docs/protocol/codex-worker-events-v1.md`
- `docs/protocol/COMPATIBILITY_NEGOTIATION_POLICY.md`
