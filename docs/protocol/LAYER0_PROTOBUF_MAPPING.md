# Layer-0 Protobuf Mapping (Proto -> JSON/SSE)

Proto3 definitions under `proto/openagents/protocol/v1/*` are canonical for shared contracts. Existing HTTP/SSE wire format can remain JSON-first while mapping to these schemas.

## Mapping Rules

- Proto source-of-truth defines field names and types.
- Runtime and Laravel continue to emit/consume JSON over HTTP/SSE.
- Runtime boundary adapters enforce proto-compatible shapes before payloads cross boundaries:
  - `OpenAgentsRuntime.Contracts.Layer0TypeAdapters.run_event/4`
  - `OpenAgentsRuntime.Contracts.Layer0TypeAdapters.predict_receipt/1`
  - `OpenAgentsRuntime.Contracts.Layer0TypeAdapters.comms_send_intent/3`
  - `OpenAgentsRuntime.Contracts.Layer0TypeAdapters.comms_send_result/1`
  - `apps/lightning-ops/src/controlPlane/protoAdapters.ts`
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

## Codex Worker Stream Mapping

Codex worker SSE payloads are proto-compatible projections of `openagents.protocol.v1.CodexWorkerEvent`
from `proto/openagents/protocol/v1/codex_events.proto`.

Envelope mapping:

- `seq` (JSON number) -> `CodexWorkerEvent.seq`
- `eventType` or `event_type` (JSON string) -> `CodexWorkerEvent.event_type`
- `payload` (JSON object) -> `CodexWorkerUnknownPayload.body` in proto terms when method-specific
  desktop/iOS payloads are emitted through worker events.

Handshake envelope mapping (runtime-mediated desktop/iOS flow):

- `event_type == "worker.event"` is required.
- `payload.source == "autopilot-ios"` + `payload.method == "ios/handshake"`:
  - required: `handshake_id`, `device_id`, `occurred_at`
- `payload.source == "autopilot-desktop"` + `payload.method == "desktop/handshake_ack"`:
  - required: `handshake_id`, `desktop_session_id`, `occurred_at`

Proto-first boundary rule for clients:

- Desktop and iOS stream consumers must decode through proto-derived envelope adapters before
  handshake correlation/dedupe logic.
- Missing required handshake fields are treated as invalid envelope payloads (ignored for ack
  matching and not eligible for handshake success state transitions).

## Receipt Mapping

`PredictReceipt` in `receipts.proto` is the canonical receipt contract for DS predict outputs. Runtime receipt maps should include both:

- enum-ready `reason_code` (`ReasonCode`)
- stable textual reason (`reason_code_text`) for backward compatibility in existing JSON surfaces

## Compatibility

- Additive changes only in `v1`.
- Breaking changes require a new proto package version.
- CI enforces this with Buf breaking checks.
