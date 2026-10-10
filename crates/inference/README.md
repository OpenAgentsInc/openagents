# inference

The inference gateway library (`docs/inference/gateway.md`). This README
covers its wire layer (sections 3 and 12): the Open Responses types, the
server-sent event codec, and the Chat Completions translation, pure types
and functions with no I/O. `meter` is the measurement half (sections 5
and 6). `upstream` holds the adapters (section 4), the only module that
does I/O. The router and the HTTP routes build on all three.

## What is here

| Module | Contents |
| --- | --- |
| `request` | `CreateResponse` (the Open Responses request, and the gateway's internal request), tools, `tool_choice`, text format, reasoning settings |
| `response` | `Response` (`ResponseResource`), `Usage`, statuses, `Response::from_request`, `Response::validate` |
| `item` | `Item` (`message`, `function_call`, `function_call_output`, `reasoning`, `item_reference`, `compaction`), content parts |
| `event` | `Event` (a `sequence_number` and an `EventBody`), every event the 2026-04-24 spec names, plus `openagents:route` and `openagents:cost` |
| `error` | `ApiError` (`type`, `code`, `param`, `message`) with its HTTP status; the response's `error` object |
| `rates` | The public rate card (`Card`: list price, margin, and price per million tokens, sats beside, promotions as their own rows), provider names, the published card from the adapters' own rows, and the `/v1/models` catalog |
| `openagents` | The `openagents` request object (route, privacy, payer, max price, fallbacks) and response object (model, upstream, attempts, cost) |
| `sse` | `SseDecoder` (bytes in, frames out, split anywhere), `ResponsesDecoder`, `encode_event`, `DONE_FRAME` |
| `stream` | `Sequencer`, `Accumulator` (events folded into a response), `StreamCheck` (a stream checked against the spec's order) |
| `chat` | Chat Completions request, reply, and chunk types; translation both ways; `ChunkWriter` (events to chunks), `EventWriter` (chunks to events), `CompletionBuilder` |
| `session` | The stateful layer (P2): `Sessions::create` (our `resp_` ids, `previous_response_id`, `store`, the hosted tool loop), `Sessions::compact`, `Local` (a WebSocket connection's memory), `Owner` |
| `store` | Stored responses: `Record`, the `ResponseStore` trait, `MemoryStore`, `DirStore`; sealed, owner-scoped, expiring |
| `seal` | AES-256-GCM sealing bound to owner and purpose; the key from the environment or a 0600 key file; `random_id` |
| `ws` | The WebSocket transport's messages: `parse_create`, the `error` envelope, the 60-minute limit |
| `hosted` | Hosted tools: `openagents:web_search` as a function the model calls, the `WebSearch` trait, the Exa provider |
| `upstream::psionic` | `psionic-serve` as the `local` upstream: Open Responses to its `/v1/responses` types and back |
| `upstream::pylon` | Pylon provider registrations as `pylon:<pylon>` upstreams: NIP-CJ jobs through a `Jobs` transport, each answer's provider earning through `Earnings` |
| `upstream::progress` | Long-running work as Open Responses events: progress lines as a reasoning summary, then the answer; the text brief a run is handed |
| `upstream::coder` | Own coding capacity (#11080): each subscription account on the key owner's linked computers as `coder:<computer>/<account>`, offered only to that caller's `pay: "mine"`, most free sessions first, no charge |
| `upstream::{anthropic,openai,gemini}` | Direct caller keys: Anthropic Messages, OpenAI Responses, and Gemini Developer API; workspace credentials only, with `standard` privacy until the caller's data terms are verified |
| `upstream::whole` | An answer that arrived whole, replayed as the spec's event stream |
| `upstream` | The `Upstream` trait (capabilities, privacy terms, price rows, billed account, `send`) and adapters for Vertex AI (native Gemini, prepaid Google credit), Z.ai (`glm-5.3-flash`, prepaid credit), the Pro door's proxy, OpenRouter, and the Vercel AI Gateway; key lookup (env, mounted file, Secret Manager); Google tokens; the first-token `Gate`; `AttemptMeter`, which reports each attempt as a `meter::Attempt` |

## Router

`router::plan` turns a request into at most three (model, upstream)
attempts, a pure function of the request and a `router::Context`: the
`Offering`s adapters advertise (capabilities, zero retention, payer,
billed account), the `ClassTable` (the spec's starting table is the
default), the meter's rate card and ledger, live rates, Gym `Scores`, and
the `Bench`. The module docs list the steps. Choices the spec leaves open:

- Without Gym scores for a class, the table's model order comes first and
  the credit-first ranking orders each model's upstreams; with scores,
  candidates below the floor go and the ranking orders the rest.
- Free capacity (the Pro door) and prepaid balances rank as credit;
  among them the sooner expiry goes first.
- Price limits compare the caller's price (cost plus margin) per million
  tokens. When every candidate is over the limit the answer is
  `403 limit_reached` with `param: "openagents.max_price"`; otherwise an
  empty plan is `503 no_route`.
- `openagents/auto` without a judge falls back to `chat`.
- `router::fall_back`: an attempt that fails before the first output token
  falls back, except a `400` from the upstream (the next one would refuse
  it too). `Bench::observe` benches an upstream for five minutes on a 401
  or 402.

## Attempt loop

`run::Gateway` holds the adapters, the class table, Gym scores, the bench,
and the meter. `Gateway::run` plans a request and sends its attempts in
order (`tests/run.rs` covers each rule with stub adapters):

- Only adapters with their key are offered to the router, so an
  unconfigured upstream never costs an attempt.
- An attempt is committed at its first output token (a text, refusal,
  reasoning, or arguments delta, a function call item, or a terminal event
  with output). Before that, an HTTP error, a failure event, an empty
  stream, or no first token within the plan's `first_token_ms` falls back
  to the next attempt, except a `400` from the upstream, which is the
  caller's `400`. After it nothing falls back: a broken stream ends with
  `response.failed`, its error only `code` (`upstream_failed`) and
  `message`.
- Every attempt is recorded into the meter: fallbacks as they fail (a
  missed deadline as `first_token_deadline`), the committed one when its
  stream ends or the caller goes away. A 401 or 402 benches the upstream
  for five minutes.
- The committed stream is re-sequenced from zero and carries
  `openagents:route` before the first output item and `openagents:cost`
  (priced from the meter's rate card, decimal dollar strings from integer
  micros) before the terminal event, whose response carries the
  `openagents` object (model, upstream, every attempt, cost).
- `openagents/auto` asks a `run::PickClass` (the gateway's is a Jev
  System One choice over the six classes) and waits at most
  `run::JUDGE_BUDGET`; no judgment is `chat`.
- When every attempt fails before its first token the answer is
  `502 upstream_failed`, naming each attempt's error class.

`run::collect` folds a committed stream into one response for a caller
that did not ask for a stream. `Gateway::run` is `Gateway::prepare` (judge
and plan, nothing sent) then `Gateway::send`; a caller with a `run::Admission` (the public API)
is checked before planning, admitted (the free request or the worst-case
hold) between the two, and settled when the stream ends. `Meter::request` returns one
request's kept attempts, for `GET /v1/usage/{request_id}`.

## Stateful layer

`session::Sessions` sits on `run::Gateway` (`tests/session.rs` covers each
rule with a stub upstream and a stub search provider):

- Upstreams always get a stateless request: the whole context as items,
  `store: false`, no `previous_response_id`, compaction items opened.
- Every lifecycle response carries our id (`resp_` and 32 hex), the
  request's `previous_response_id` and `store`, and the caller's tools.
- `previous_response_id` looks in the connection's `Local` memory, then
  the owner's store; otherwise `400 previous_response_not_found`. A
  continuation's `function_call_output` must answer an earlier call. A
  failed continuation evicts its id from `Local`.
- `store: true` needs a configured store and an owner that is not
  zero-retention (`400 store_not_allowed`). Records are sealed to owner
  and id before they are kept, expire after the retention, and delete at
  once.
- Compaction returns the user's messages plus a `compaction` item holding
  the model's summary sealed to the owner; it opens into a developer
  message, and only for that owner.
- Hosted web search: the turns stream as one response (one
  `response.created`, items renumbered, the hosted function's calls
  hidden, an `openagents:web_search_call` item per search, summed usage and
  cost, one terminal event).

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
| `previous_response_id`, item references, compaction items | none | The stateful layer resolves the first and opens the last before routing; an item reference is `400` toward a Chat-only upstream |
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
translation. `tests/upstreams.rs` runs every adapter against a local stub
server: the streams in `fixtures/upstream/` and the recorded gateway
streams, request bodies, privacy fields, refusals, status errors, rate
limits, broken streams, and the attempt records.

What the recorded streams show about upstreams: Vercel's streams end
without `[DONE]`; its GLM lane sends raw reasoning deltas with no
`content_part.added`; OpenRouter follows the spec's order under OpenAI's
`reasoning_text` names.

Do not build this crate with `serde_json`'s `arbitrary_precision` feature:
`#[serde(flatten)]` cannot read floats under it.
