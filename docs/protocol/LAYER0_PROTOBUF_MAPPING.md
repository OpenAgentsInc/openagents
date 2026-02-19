# Layer-0 Protobuf Mapping (Proto -> JSON/SSE)

Proto3 definitions under `proto/openagents/protocol/v1/*` are canonical for shared contracts. Existing HTTP/SSE wire format can remain JSON-first while mapping to these schemas.

## Mapping Rules

- Proto source-of-truth defines field names and types.
- Runtime and Laravel continue to emit/consume JSON over HTTP/SSE.
- JSON payloads must follow protobuf JSON mapping conventions:
  - `snake_case` proto fields map to `camelCase` JSON keys.
  - enums map by string name where possible.
  - `oneof` payloads map to exactly one active payload object.

## SSE Event Mapping

Runtime SSE events map to `RunEvent` oneofs in `events.proto`:

- `run.started` -> `run_started`
- `text.delta`/`run.delta` -> `text_delta`
- `tool.call` -> `tool_call`
- `tool.result` -> `tool_result`
- `run.finished` -> `run_finished`
- unknown payloads -> `unknown`

## Receipt Mapping

`PredictReceipt` in `receipts.proto` is the canonical receipt contract for DS predict outputs. Runtime receipt maps should include both:

- enum-ready `reason_code` (`ReasonCode`)
- stable textual reason (`reason_code_text`) for backward compatibility in existing JSON surfaces

## Compatibility

- Additive changes only in `v1`.
- Breaking changes require a new proto package version.
- CI enforces this with Buf breaking checks.
