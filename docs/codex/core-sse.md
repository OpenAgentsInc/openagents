# Core: SSE Processing Details

File: `codex-rs/core/src/client.rs` (`process_sse`)

Explains how Server‑Sent Events are consumed and mapped to internal events.

## Parsing and errors

- Each `eventsource_stream` item is parsed as JSON into `SseEvent` with fields
  `type`, `response`, `item`, `delta`.
- Unknown events are ignored until `response.completed` arrives.
- On JSON parse errors, the record is skipped with a debug log.

## Completion and idle timeout

- If the stream ends (EOF) without a prior completion, emit a structured stream
  error.
- Idle timer ensures a hung connection eventually yields an error/complete.

## Usage accounting

- `ResponseCompleted{ id, usage }` is converted to `TokenUsage` for UI display.
- For Azure, `attach_item_ids` ensures IDs in `input`/`output` match.

