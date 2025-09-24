# Chat Completions Adapter

File: `codex-rs/core/src/chat_completions.rs`

Implements the classic Chat Completions streaming API and maps it onto the same
internal `ResponseEvent` stream as the Responses API.

## Request building

- Builds a `messages` array from `Prompt` items, attaching reasoning text
  segments to adjacent assistant anchors after the last user message.
- Deduplicates repeated assistant messages to avoid echoing final aggregates.
- Converts function/local shell/custom tool calls into the OpenAI `tool_calls`
  schema.
- Uses `create_tools_json_for_chat_completions_api` to format tool definitions.

## Streaming

- `process_chat_sse` reads SSE chunks, forwarding:
  - `delta.content` → `OutputTextDelta`
  - `delta.reasoning` (string or object) → `ReasoningContentDelta`
  - tool call deltas → accumulated call state
- On `finish_reason`:
  - `tool_calls` → emit a `FunctionCall`/`LocalShellCall` ResponseItem and
    complete the turn.
  - `stop` → finalize the assistant message with accumulated text and
    reasoning content.

## Retries and timeouts

- Similar retry policy as Responses, with capped backoff.
- Idle timeouts enforced to fail hung streams.

