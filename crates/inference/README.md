# inference

The inference gateway library (`docs/inference/gateway.md`). This README
covers its wire layer (sections 3 and 12): the Open Responses types, the
server-sent event codec, and the Chat Completions translation, pure types
and functions with no I/O. `meter` is the measurement half (sections 5
and 6). Adapters, the router, and the HTTP routes build on both.

## What is here

| Module | Contents |
| --- | --- |
| `request` | `CreateResponse` (the Open Responses request, and the gateway's internal request), tools, `tool_choice`, text format, reasoning settings |
| `response` | `Response` (`ResponseResource`), `Usage`, statuses, `Response::from_request`, `Response::validate` |
| `item` | `Item` (`message`, `function_call`, `function_call_output`, `reasoning`, `item_reference`, `compaction`), content parts |
| `event` | `Event` (a `sequence_number` and an `EventBody`), every event the 2026-04-24 spec names, plus `openagents:route` and `openagents:cost` |
| `error` | `ApiError` (`type`, `code`, `param`, `message`) with its HTTP status; the response's `error` object |
| `openagents` | The `openagents` request object (route, privacy, payer, max price, fallbacks) and response object (model, upstream, attempts, cost) |
| `sse` | `SseDecoder` (bytes in, frames out, split anywhere), `ResponsesDecoder`, `encode_event`, `DONE_FRAME` |
| `stream` | `Sequencer`, `Accumulator` (events folded into a response), `StreamCheck` (a stream checked against the spec's order) |
| `chat` | Chat Completions request, reply, and chunk types; translation both ways; `ChunkWriter` (events to chunks), `EventWriter` (chunks to events), `CompletionBuilder` |

## Strict and lenient

- **Strict where the spec is strict.** A known item, part, or event `type`
  must decode with its required fields. Roles, `tool_choice` modes,
  truncation, image detail, verbosity, and reasoning summary are closed
  enums. Every event needs an integer `sequence_number`.
- **Forward-compatible where the spec says so.** Unknown item, part, tool,
  and event types decode to an `Unknown` variant holding the JSON, as the
  spec asks clients to do with prefixed extensions. Statuses, service
  tiers, error types, reasoning effort, and include values are open
  enums: an unnamed value is kept as `Other(String)`. Every object keeps
  fields it does not name in `extra`, so a decoded upstream object
  re-encodes with nothing lost.
- **Lenient on upstream responses.** A response field the spec requires
  but an upstream left out decodes to its zero value. What we encode always
  carries every required field (`null` where nullable).

`EventBody::normalized` maps OpenAI's `response.reasoning_text.*` (which
OpenRouter sends) to the spec's `response.reasoning.*`.

## Chat Completions mapping

A Chat Completions request becomes a `CreateResponse` plus a `ChatShape`
(how the reply must look: `include_usage`, which max-tokens field the
caller used). It routes like any other request; the result comes back
through `completion_from_response` or `ChunkWriter`. The reverse
(`from_responses_request`, `response_from_completion`, `EventWriter`)
serves upstreams that speak only Chat Completions. `tests/chat_table.rs`
has a round-trip test per row.

| Chat Completions | Open Responses | Fidelity |
| --- | --- | --- |
| first `system` message (string content) | `instructions` | 1:1 |
| later `system` messages, `developer` messages | `message` items, role `system` / `developer` | 1:1 |
| `user` text, `image_url`, inline `file` parts | `input_text`, `input_image`, `input_file` | 1:1 |
| `user` `input_audio` part, `file.file_id` | none | `400`, naming the part |
| `assistant` `content` and `refusal` | `message` item with `output_text` and `refusal` parts | 1:1 |
| `assistant` `tool_calls` | `function_call` items after the message | 1:1; two assistant messages in a row come back as one |
| `assistant` `reasoning` (OpenRouter) | `reasoning` item, summary text | 1:1 for text; `encrypted_content` has no Chat field and is dropped going back |
| `tool` messages | `function_call_output` | 1:1 |
| `name` on a message | none | Dropped |
| `tools` (function), `tool_choice` (modes, named, `allowed_tools`), `parallel_tool_calls` | the same | 1:1; hosted tools cannot go to a Chat-only upstream (`400`) |
| `response_format` (`text`, `json_object`, `json_schema`), `verbosity` | `text.format`, `text.verbosity` | 1:1 |
| `max_tokens`, `max_completion_tokens` | `max_output_tokens` | 1:1 (`max_completion_tokens` wins when both are sent) |
| `reasoning_effort` | `reasoning.effort` | 1:1; `reasoning.summary` has no Chat field |
| `temperature`, `top_p`, `presence_penalty`, `frequency_penalty`, `metadata`, `service_tier`, `safety_identifier`, `prompt_cache_key`, `store` | the same | 1:1 |
| `stop`, `seed`, `user` | extension fields of the same names | 1:1 (not in Open Responses) |
| `logprobs`, `top_logprobs` | `include: ["message.output_text.logprobs"]`, `top_logprobs` | 1:1 |
| `n` | none | `n > 1` is `400`; `n: 1` is accepted |
| `stream`, `stream_options.include_usage` | `stream`; `ChatShape.include_usage` | 1:1; usage chunk (no choices) before `[DONE]` |
| `openagents`, unknown provider fields (`provider`, `top_k`) | `openagents`, `extra` | Passed through |
| `previous_response_id`, item references, compaction items | none | `400` toward a Chat-only upstream |
| reply `finish_reason` | `status`: `completed` gives `stop` or `tool_calls`; `incomplete` gives `length` or `content_filter`; `failed` gives `error` | 1:1 |
| reply `usage` (prompt, completion, cached, reasoning) | `usage` | 1:1; the `openagents` cost object rides alongside |
| several output items | one message: text concatenated, tool calls in order, reasoning text joined | Flattened, order kept |
| annotations and citations, hosted tool items | none | Dropped |

In a stream, text, refusal, and reasoning deltas (summary or raw) become
`content`, `refusal`, and `reasoning` deltas; each `function_call` item is
a tool call with its own index; the terminal event becomes the finish
chunk. `openagents:route` and `openagents:cost` arrive as the `openagents`
object on the last chunk. A failure becomes a chunk with
`finish_reason: "error"` and an `error` object. Going the other way, Chat
`reasoning` deltas become a reasoning summary.

## Tests

No network. `tests/spec.rs` decodes the spec's examples
(`fixtures/spec/`). `tests/recorded.rs` runs the recorded upstream streams
in `crates/coder/fixtures/gateway/` through decoding (split at random
points), re-encoding, the order check, folding, and the Chat Completions
stream. `tests/chat_table.rs` and `tests/chat_stream.rs` cover the
translation.

What the recorded streams show about upstreams: Vercel's streams end
without `[DONE]`; its GLM lane sends raw reasoning deltas with no
`content_part.added`; OpenRouter follows the spec's order under OpenAI's
`reasoning_text` names.

Do not build this crate with `serde_json`'s `arbitrary_precision` feature:
`#[serde(flatten)]` cannot read floats under it.
