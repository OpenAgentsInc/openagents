# Laravel SSE Mapping

Runtime stream events are mapped to Laravel AI SDK-compatible SSE frames via:

- `OpenAgentsRuntime.Integrations.LaravelEventMapper`
- Layer-0 contract source: `docs/protocol/LAYER0_PROTOBUF_MAPPING.md` and `proto/openagents/protocol/v1/events.proto`

## Runtime Event → SSE Data Mapping

- `run.started` -> `{ "type": "start", ... }`
- `run.delta` / `text.delta` -> `{ "type": "text-delta", "delta": ... }`
- `tool.call` -> `{ "type": "tool-call", ... }`
- `tool.result` -> `{ "type": "tool-result", ... }`
- `run.finished` -> `{ "type": "finish", ... }` then `data: [DONE]`
- unknown event types -> `{ "type": "event", "eventType": ... }`

## SSE Invariants

- Every emitted frame includes `event: message`.
- Cursor continuity uses `id: <seq>` on emitted frames.
- Terminal flows emit literal `[DONE]` as final data payload.
