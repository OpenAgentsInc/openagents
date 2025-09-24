# Core: ModelClient and Streaming

This document explains how `core/src/client.rs` implements model streaming for
both the OpenAI Responses API and Chat Completions.

## Key types

- `ModelClient` — per‑conversation client with provider, auth, and context.
- `Prompt` — request inputs/tools/instructions (see `core-client-common.md`).
- `ResponseEvent` — internal stream of events consumed by `codex.rs`.

## Entry points

- `ModelClient::stream(&Prompt)` — public entry; dispatches to Responses or Chat
  based on `provider.wire_api`.
- `ModelClient::stream_responses(&Prompt)` — Responses API SSE implementation.

## Responses API flow

File: `codex-rs/core/src/client.rs`

1. Build `ResponsesApiRequest` (model, instructions, input, tools, reasoning).
2. Azure workaround: if the provider is Azure, set `store: true` and
   `attach_item_ids` so the model preserves incremental item IDs.
3. Make POST, set `Accept: text/event-stream` and provider headers.
4. On success, start processing SSE with an idle timeout appropriate for the
   provider (`provider.stream_idle_timeout()`).

### SSE event handling

- Implemented in `process_sse`, reading from `eventsource_stream`.
- Handles idle timeout and EOF:
  - If final `response.completed` was seen, emit `ResponseEvent::Completed`.
  - Otherwise, emit `CodexErr::Stream("stream closed before response.completed", …)`.
- Maps event kinds to internal events:
  - `response.output_item.done` → parse item into `ResponseItem` and emit
    `ResponseEvent::OutputItemDone(item)`.
  - `response.output_text.delta` → `ResponseEvent::OutputTextDelta`.
  - `response.reasoning_summary_text.delta` → `ReasoningSummaryDelta`.
  - `response.reasoning_text.delta` → `ReasoningContentDelta`.
  - `response.created` → `ResponseEvent::Created`.
  - `response.completed` → stores id/usage; completion emitted at stream end
    (or immediately at idle) to avoid duplicating output arrays.
  - `response.failed` → parse error payload; attempt to extract retry delay
    (`try_parse_retry_after`) and surface a helpful error message.

### Rate limit snapshots

- `parse_rate_limit_snapshot` reads `x-codex-*` headers and emits
  `ResponseEvent::RateLimits` before streaming begins.

### Retries and auth

- On non‑success statuses, refresh auth if `401` and using ChatGPT tokens.
- Surface structured error body for most 4xx rather than only status code.
- Backoff rules delegated to `provider.request_max_retries()`.

## Chat Completions flow

- Calls `chat_completions::stream_chat_completions` to obtain a raw stream.
- Wraps with `AggregatedChatStream::streaming_mode(..)` when
  `config.show_raw_agent_reasoning` is true, otherwise aggregates to match the
  Responses API semantics (one final assistant message per turn).
- Bridges the stream into `ResponseStream` by forwarding events over a channel.

## Utilities

- `attach_item_ids` — injects `id` fields into `input` items based on
  original `ResponseItem` values for Azure interoperability.
- Header parsers (`parse_header_f64`, `parse_header_u64`, `parse_header_str`).

## Tests

See `#[cfg(test)]` in `client.rs` for table‑driven event mapping, timeout/
error behaviors, and successful multi‑item completion.

