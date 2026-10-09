# The OpenAgents inference gateway (spec, 2026-10-09)

One system in front of every model account we hold. Our own apps call it,
and so can anyone with an API key. It speaks
[Open Responses](https://www.openresponses.org/) first and the OpenAI Chat
Completions API second, routes each request to the best upstream for the
job, measures every call itself, and charges a published price: what the
upstream charges plus a stated margin.

Think of it as our own OpenRouter, with three differences: Open Responses
is the primary API, the upstreams include accounts only we hold (prepaid
Google and Z.ai credit, the Pro door) and Pylon providers, and every price
on the rate card shows its cost and margin.

Status: P0 is implemented for our own services and runs locally
(section 13); production is not deployed. Section 2 records what ran before
it. Anything else in this document is design unless it says so.

## Contents

1. [Goals](#1-goals)
2. [What exists today](#2-what-exists-today)
3. [The API we serve](#3-the-api-we-serve)
4. [Upstream accounts and adapters](#4-upstream-accounts-and-adapters)
5. [Routing](#5-routing)
6. [Measurement](#6-measurement)
7. [Accounts, keys, and billing](#7-accounts-keys-and-billing)
8. [Pricing](#8-pricing)
9. [Privacy and data use](#9-privacy-and-data-use)
10. [Compared with OpenRouter](#10-compared-with-openrouter)
11. [Public docs](#11-public-docs)
12. [Where the code goes](#12-where-the-code-goes)
13. [Rollout](#13-rollout)
14. [Owner steps](#14-owner-steps)
15. [Open decisions](#15-open-decisions)
16. [Lineage: Episodes 241–247](#16-lineage-episodes-241247)

## 1. Goals

| Goal | What it means |
| --- | --- |
| One system for every account | Each provider key and prepaid balance is configured once, in the gateway. No service holds its own provider key. |
| Internal and public use | Our chat worker, decision worker, Coder, and Alice call the same endpoint as an outside developer, with the same metering. |
| Smart routing | A request names a model or a task class. The gateway picks the upstream by quality, cost, latency, privacy, and which prepaid credit fits, and falls back before the first token when an upstream fails. |
| Our own numbers | Latency, time to first token, tokens, cost, and errors are measured by us on every attempt, not read from a provider dashboard. |
| Transparent pricing | One public rate card. Each row shows the upstream's price, our margin, and the sum. |
| More people on our API | Standard APIs (Open Responses and Chat Completions), so existing SDKs work by changing the base URL and key. |
| Limits only users set | We impose no usage limits. A user can set caps on their own account and keys (`docs/api` D16). |

Non-goals for now: image, audio, and video generation; embeddings (later, as
a separate route); fine-tuning; hosting third-party model weights ourselves
outside Pylon and Psionic.

## 2. What exists today

Every model call today picks its provider, key, privacy fields, and
fallbacks in its own code. There is no shared front for generation.

| Caller | Upstreams and models | Keys | Fallbacks and privacy | Metering |
| --- | --- | --- | --- | --- |
| Chat worker (`crates/coder/src/bin/coder-worker.rs`, `crates/coder/src/generate.rs`) | Open Responses (`POST /v1/responses`) to OpenRouter (primary, `CODER_WORKER_PRIMARY`) and the Vercel AI Gateway (`google/gemini-3.8-flash`; `zai/glm-5.3-flash` lane) | `OPENROUTER_API_KEY`, `CODER_DOOR_KEY` or `CODER_AI_GATEWAY_KEY`, in the host's env file | `FallbackDoor`: primary, then Gemini, before the first words (4 s, 8 s). `CODER_PROVIDER_PRIVACY=strict` sends OpenRouter `zdr` and Vercel `zeroDataRetention`; `store: false` always | JSONL usage log per job (`crates/coder/src/relay/usage.rs`): model, door, timings, tokens, `cost_usd` when the door reports one, payer; no text |
| Jev judge and decision worker (`crates/jev/src/doors.rs`, `crates/gateway/src/relay_worker.rs`) | Vercel `typesafe-ai/jev`, OpenRouter decisions, TypeSafe direct | `AI_GATEWAY_API_KEY`, `OPENROUTER_API_KEY`, `TYPESAFE_API_KEY` | Fail over on 402, 408, 429, 5xx, timeouts; 401 and 402 bench a door for five minutes | Decision usage JSONL (`decision_usage.rs`) |
| Decision gateway (`crates/gateway`) | Decision doors only (`/v1/systemone`, `/v1/classify`); no text generation | `oak_` keys (`tenancy::keys`) | Registry authorization, quota holds, receipts | Holds and receipts; `tenancy::money` prices (`Rate` per input, cached, output, reasoning tokens); billing, plans, prepaid, Stripe card funding, Lightning funding, `pay-ledger` |
| Pro inference door (private `pro` repo, `pro.openagents.com`) | Stripe LLM proxy, Chat Completions; serves `gpt-5.6-sol`, `-terra`, `-luna` of the 56 ids the key may call ([details](../gateway/README.md)) | `PRO_UPSTREAM_KEY` (test mode), `PRO_API_KEY` | None | None; no customer meter attached |
| BYOK (`crates/model-access`) | The person's OpenRouter, Vercel, TypeSafe keys; `models.payer` is `ours` or `mine` | Keychain or `~/.openagents`; hosted chat gets them NIP-44-sealed per job | Fixed order per use; desktop "Use my keys for everything" | Payer and key fingerprint in the usage log |
| Web own-Claude key (`crates/openagents-web/src/cloud/custody.rs`, `crates/oa-seal`) | Anthropic, Bedrock, Vertex, or Foundry credential for Claude Code on the user's computer | Sealed with AES-256-GCM | Not an OpenAgents model call | None |
| OpenRouter client (`crates/openrouter`) | Chat Completions, embeddings, structured outputs, streaming tools | `OPENROUTER_API_KEY` | `no_retention` sets `data_collection` and `zdr` | Usage and cost from the response |
| Other direct calls | OpenAI (`crates/coder/src/codebase.rs`), Gemini API (`product_kb/eval.rs`), embeddings (`crates/knowledge`), Vertex (`crates/microcoder-loop/src/vertex.rs`), eval runner | Separate env vars each | Ad hoc | Per tool, if any |
| Pylon and Psionic | `psionic-serve` serves `/v1/chat/completions` and `/v1/responses` on loopback; a Pylon runs jobs on it, paid over NIP-X402 | Provider's own | None | Job receipts |

Gaps this spec closes: keys are spread across hosts; Gemini is paid through
Vercel rather than our Google credit; GLM is paid through Vercel rather
than our Z.ai credit; nothing tracks a prepaid balance; latency and cost
are measured per caller, in different shapes; and nothing outside our own
apps can call a model through us. `crates/microcoder-loop` points at an
`openagents.com/v1/responses` route that no server in this repository
serves.

## 3. The API we serve

Base URL: `https://api.openagents.com/v1`, the host the
[OpenAgents API design](../api/2026-10-02-openagents-api.md) already chose
(D6). It points straight at the gateway, so the website's sign-in cookies
never ride along with API calls and the API caches, limits, and fails
apart from the site. `https://openagents.com/api/v1` is an alias: the
website passes `/api/*` through to the same gateway, for callers who want
one domain. One service either way (decided 2026-10-09). The agent API and the inference gateway share the host, keys,
balances, and error shapes. The agent is one more model id there
(`openagents`); raw models are the rest of the catalog.

| Method and path | What it does | Phase |
| --- | --- | --- |
| `POST /v1/responses` | Open Responses, JSON or server-sent events. Primary API. | P0 internal, P1 public |
| `POST /v1/chat/completions` | OpenAI Chat Completions, JSON or server-sent events. Full backup surface, translated onto the same request model. | P0 internal, P1 public |
| `GET /v1/models` | Catalog: every model id, its upstreams, capabilities, context, price row, and live latency and uptime. OpenAI list shape plus an `openagents` object per model. | P1 |
| `GET /v1/rates` | The rate card as JSON, one row per model and upstream, with cost, margin, and price. | P1 |
| `GET /v1/usage/{request_id}` | Tokens, cost, upstream, attempts, and timings for one finished request. | P1 |
| `GET /v1/key` | The calling key's balance, spend, and the limits its owner set. | P1 |
| `POST /v1/responses/compact` | Open Responses compaction. | P2 (built) |
| WebSocket `/v1/responses` | Open Responses WebSocket transport (optional in the spec). | P2 (built) |
| `GET /v1/responses/{id}`, `DELETE /v1/responses/{id}` | The caller's stored response (`store: true`), read or deleted at once. Not in the spec; OpenAI's shape. | P2 (built) |

### Open Responses

We implement the specification at version 2026-04-24
([specification](https://www.openresponses.org/specification),
[OpenAPI reference](https://www.openresponses.org/reference),
[acceptance tests](https://www.openresponses.org/compliance)):

- **Items.** `message`, `function_call`, `function_call_output`, and
  `reasoning`, each with `id`, `type`, and `status`. Reasoning items carry
  `summary` and `encrypted_content` when the upstream gives them, and raw
  reasoning `content` only when the caller asks and the upstream allows.
- **Streaming.** The spec's order: `response.created`,
  `response.in_progress`, then per item `response.output_item.added`,
  `response.content_part.added`, deltas, `*.done`,
  `response.content_part.done`, and `response.output_item.done`, then a
  terminal `response.completed`, `response.incomplete`, or
  `response.failed`. Every event has `sequence_number`. The stream we serve
  ends with `[DONE]`. Upstreams are read leniently: the Vercel AI Gateway's
  streams never send `[DONE]` (the terminal event ends them), and its GLM
  lane sends raw reasoning deltas with no `response.content_part.added`
  first, so the codec and every adapter accept both.
- **Tools.** Function tools and `tool_choice` (`auto`, `required`, `none`,
  a named function, and `allowed_tools`, enforced as a hard constraint).
  Hosted tools are P2, each as its own prefixed type: web search is
  built (see "Stored responses, compaction, WebSocket, hosted tools"
  below).
- **State.** P1 was stateless, as OpenRouter's Responses route is. P2 adds
  `store: true` with encrypted retention the caller can delete, and
  `previous_response_id` (below).
- **Errors.** The spec's object (`type`, `code`, `param`, `message`) and
  statuses: `invalid_request` 400, `not_found` 404, `too_many_requests` 429,
  `server_error` and `model_error` 500. Our additions use the same shape:
  `unauthorized` 401, `insufficient_balance` 402, `limit_reached` 403 (a
  limit the key's owner set, named in `param`), `upstream_failed` 502, and
  `no_route` 503 (no upstream meets the request's constraints). An error
  after the first token arrives as `response.failed`. A failed response's
  `error` object carries only `code` and `message` (the spec's
  `ResponseResource.error`); `type` and `param` belong to the HTTP error
  body.
- **Compliance.** The acceptance suite (10 HTTP tests, 7 WebSocket tests)
  runs against staging before each release and against production in P1.
  The WebSocket tests' flows pass locally against stub upstreams
  (`crates/gateway/tests/inference_state.rs`); the suite itself runs
  against staging once it is deployed.

### Stored responses, compaction, WebSocket, hosted tools

Built for #11071 in `crates/inference/src/session.rs` (with `store.rs`,
`seal.rs`, `ws.rs`, `hosted.rs`) and mounted by `crates/gateway`
(`inference_state`). Upstreams never see state: every request leaves the
gateway as the whole context in `input`, with `store: false` and no
`previous_response_id`.

- **Ids.** Every response gets our id, `resp_` and 32 hex characters, on
  every lifecycle event; the upstream's id is not shown.
- **`store: true` is opt-in per request.** The default is `false`: nothing
  is kept. A stored response is the response plus the context it answered,
  sealed with AES-256-GCM to its owner and id, under the gateway's state
  directory (`inference/responses/<sha256(tenant)>/<id>.json`). It belongs
  to the tenant whose key made it: another tenant's read, delete, or
  `previous_response_id` gets "not found". It expires after
  `inference.store.retention_days` (30 by default) and
  `DELETE /v1/responses/{id}` removes it at once. Without
  `inference.store` configured, `store: true` is `400`.
- **Zero retention.** A tenant in `inference.zero_retention_tenants` keeps
  nothing with us: `store: true` is `400 store_not_allowed`. Its responses
  can still be continued on the same WebSocket connection, from that
  connection's memory, which ends with the connection.
- **`previous_response_id`** puts the new input after the earlier
  response's context and output, as the spec orders them. The id is looked
  up in the connection's memory (WebSocket), then in the owner's stored
  responses; otherwise `400 previous_response_not_found`. A continuation
  whose `function_call_output` answers no earlier call is `400`.
  `instructions` are not carried over (OpenAI's rule).
- **Compaction.** `POST /v1/responses/compact` (`model` required) asks the
  model for a summary of the conversation and returns the user's messages
  plus one `compaction` item whose `encrypted_content` is the summary
  sealed to the caller's tenant. Sent back as input, the item opens into a
  developer message holding the summary; for another tenant, or another
  gateway's key, it is `400 invalid_compaction`. The summary pass is
  metered and priced like any request.
- **WebSocket.** `GET /v1/responses` upgraded, with the same key in
  `Authorization`. Each `response.create` message runs one response, one at
  a time; events come back one JSON message each; failures come back as
  `{"type": "error", "status", "error": {"code", "message", "param"}}`.
  `stream`, `stream_options`, and `background` are refused. The connection
  remembers its most recent response (so `store: false` turns continue on
  the same socket), forgets it when a continuation of it fails, and closes
  after 60 minutes with `websocket_connection_limit_reached`.
- **Hosted tools.** `{"type": "openagents:web_search"}` in `tools`
  (`max_results` 1 to 10, default 5). The model sees a function
  (`openagents_web_search`); when it calls it, the gateway searches, hands
  back the results, and runs the next model turn, until the model answers
  without searching or `max_tool_calls` (default 8) is spent. Each search
  is an `openagents:web_search_call` item (`action.query`, `results` with
  `title`, `url`, `snippet`); the turns stream as one response with one
  sequence, summed usage, and a cost that adds the search provider's list
  price plus the margin. The provider is Exa (`EXA_API_KEY`), unverified
  for `strict` until `EXA_TERMS_VERIFIED=zero-retention`, so the default
  `strict` request is refused web search until then. Chat Completions has
  no hosted tools.
- **Sealing key.** `INFERENCE_STORE_KEY` (base64 of 32 bytes; the name is
  `inference.store.key_env`) or its `_FILE`; without either, the gateway
  makes `inference/seal.key` (mode 0600) in its state directory. Changing
  the key makes every stored response and compaction item unreadable.

Our extensions follow the spec's rules: optional fields, and prefixed names
for any new item or event type.

| Extension | Where | Contents |
| --- | --- | --- |
| `openagents` request object | request body | `route` (`order`, `only`, `ignore`, `sort`: `quality`, `price`, or `latency`), `privacy` (`strict` or `standard`), `pay` (`ours` or `mine`), `max_price` as `{"input": "0.50", "output": "1.50"}`: decimal US dollar strings per million tokens, compared against our price including the margin, `fallbacks` (model ids). All optional. |
| `openagents` response object | response body | `model` and `upstream` that answered, `attempts` (each upstream tried, outcome, milliseconds), `cost` (`upstream_usd`, `margin_usd`, `price_usd` as decimal dollar strings converted from the meter's integer micros, so no amount passes through a float; `price_sats` beside them). |
| `text.format` `json_object` | request body | Accepted on `/v1/responses` as an extension (the spec names `text` and `json_schema`), so a Chat Completions `response_format: json_object` keeps its meaning on both APIs. |
| `stop`, `seed`, `user` | request body | Carried as extension fields of the same names (they are not in Open Responses), so Chat Completions callers keep them; an upstream that does not support one ignores it. |
| `openagents:route` event | stream | Which model and upstream took the request, sent before the first output item. Only when the request carries `x-openagents-events: route,cost`: the Open Responses acceptance suite refuses event types it does not know ([run](2026-10-09-acceptance-run.md)). |
| `openagents:cost` event | stream | The cost object, sent before the terminal event. Only on request, as `openagents:route`. |
| Headers | every response | `x-request-id`, `x-openagents-model`, `x-openagents-upstream`, `x-openagents-cost-usd`. |

Model ids are `publisher/model` (`google/gemini-3.8-flash`,
`zai/glm-5.3-flash`, `openai/gpt-5.6-luna`). Router ids pick by task class:
`openagents/auto`, `openagents/classify`, `openagents/fast`,
`openagents/chat`, `openagents/code`, `openagents/long`, and
`openagents/reason` (section 5).

### Chat Completions

A full backup surface, shipped in the first public phase, so OpenAI-SDK
clients work by changing only the base URL and key. It is a translation
layer: the request becomes the same internal Open Responses request, takes
the same route, and the result is translated back. Nothing routes
differently because of which API a caller used.

| Chat Completions | Internal (Open Responses) | Fidelity |
| --- | --- | --- |
| `system` and `developer` messages | `instructions` and developer `message` items | 1:1 |
| `user` text and `image_url` parts | `input_text` and `input_image` | 1:1 |
| `assistant` content and `tool_calls` | `message` and `function_call` items | 1:1, except that adjacent assistant messages merge into one in either direction, and a message's `name` and a part's `annotations` are dropped |
| `tool` messages | `function_call_output` | 1:1 |
| `tools` (function), `tool_choice`, `parallel_tool_calls` | the same | 1:1 |
| `response_format` (`json_object`, `json_schema`) | `text.format` (`json_object` is our extension there) | 1:1 |
| `max_tokens`, `max_completion_tokens` | `max_output_tokens` | 1:1 |
| `reasoning_effort` | `reasoning.effort` | 1:1 |
| `temperature`, `top_p` | the same | 1:1 where the upstream supports them; otherwise ignored, as OpenRouter does |
| `stop`, `seed`, `user` | extension fields of the same names | 1:1 where the upstream supports them; otherwise ignored |
| `stream`, `stream_options.include_usage` | SSE; usage in the final chunk before `[DONE]` | 1:1 |
| `finish_reason` | from status: `completed` gives `stop` or `tool_calls`; `incomplete` gives `length` or `content_filter`; `failed` gives `error` | 1:1 |
| `usage` (prompt, completion, cached, reasoning tokens) | `usage` | 1:1, plus the `openagents` cost object |
| Reasoning items | not in the format | Degrades: summary text goes in a `reasoning` delta field (OpenRouter's convention); `encrypted_content` is dropped, so a reasoning model cannot carry hidden reasoning across turns |
| Hosted tools, MCP tools, annotations and citations | not in the format | Degrades: function tools only; citations dropped |
| `store`, `previous_response_id` | not in the format | Not available on Chat Completions; send the whole conversation (Open Responses has both) |
| `n` greater than 1, `logprobs` | no equivalent | `n > 1` is `400`; `logprobs` only where the upstream supports it, otherwise `400` |
| Several output items in one turn | one `message` with `tool_calls` | Flattened, order kept |

## 4. Upstream accounts and adapters

An **account** is a credential plus a balance we hold or a user holds. An
**adapter** speaks one upstream's wire format and turns it into Open
Responses items and events. One account can feed several models, and one
model can be reachable through several accounts. Our keys live in Secret
Manager, and only the gateway reads them.

| Upstream | Account | Wire | Good for | Notes |
| --- | --- | --- | --- | --- |
| Google Vertex AI (Gemini) | Ours: about $30,000 of prepaid Google inference credit | Vertex `generateContent` or its OpenAI-compatible endpoint | Default for chat and long context where Gemini meets the quality floor | Spend first (section 5). Today Gemini reaches the chat worker through Vercel, so the credit does not pay for it. |
| Z.ai (GLM) | Ours: prepaid Z.ai credit | Chat Completions | `zai/glm-5.3-flash` as the cheap, fast tier: 1M context, 128K output, list $0.15 input, $0.03 cached, $0.50 output per million tokens | The owner's "GLM 513 Flash" is this model, the one the chat worker's `glm` lane already names; Z.ai's catalog has no GLM-5.13 on 2026-10-09. Z.ai's docs say its thinking cannot be turned off, so reasoning streams as reasoning items, never answer text. Today it goes through Vercel. |
| Pro door (Stripe LLM proxy) | Ours: Stripe key, no customer meter | Chat Completions | Free capacity for classification, extraction, and basic tasks: GPT-5.6 Sol, Terra, Luna, and the other ids the proxy marks supported | See [Pro inference door](../gateway/README.md). The proxy's README says no tool calling, so the router never sends tools there. The adapter replaces the separate `pro.openagents.com` service. |
| OpenRouter | Ours: OpenRouter credits | Open Responses (stateless) | The long tail of models we hold no direct account for | `provider.zdr` and `data_collection: deny` carry our privacy level. Also a BYOK upstream. |
| Vercel AI Gateway | Ours | Open Responses and Chat Completions | A second broad route, and Jev's first door | One adapter among many, not the router. `providerOptions.gateway.zeroDataRetention` carries privacy. |
| OpenAI direct | Ours, when the owner adds a key | Open Responses (native) | GPT models without a middle hop | Not configured for generation today. |
| Anthropic direct | Ours when added; users' own keys | Messages API | Claude models | Thinking blocks map to reasoning items, and their signature to `encrypted_content`. |
| TypeSafe (Jev) | Ours | TypeSafe decision API | Typed judgments, not text | Stays on the decision gateway and `crates/jev`. The inference gateway calls Jev to pick a task class for `openagents/auto`. |
| User keys (BYOK) | The user's | Per provider | Anything the user's key reaches | Sealed per user (`oa-seal`), as the web own-key path does today. `pay: "mine"` uses only these. Builds on `crates/model-access`. |
| User subscriptions (ChatGPT, Claude) | The user's | Codex or Claude Code on the user's own computer | The user's own coding runs | Not an HTTP upstream. A subscription serves only its owner's requests on their own computer through Coder; the gateway never pools or resells it. |
| Pylon providers | Community | NIP-PYLON jobs to `psionic-serve` | Open-weight models at a provider-set price | P2, built (below). Paid out through `pay-ledger`. |
| Local Psionic | The user's machine | `psionic-serve` `/v1/responses` on localhost | Private, free, offline | P2, built (below). Only for `route.only: ["local"]`. |

Each adapter implements one Rust trait: what it can do (tools, reasoning,
JSON schema, images, context, privacy guarantees), its price rows, which
account it bills, and `send`, which returns a stream of Open Responses
events. Each adapter keeps a recorded stream per model as a test fixture,
as `crates/coder/fixtures/gateway/` does today.

Implemented (#11061) in `crates/inference/src/upstream/`: the
`Upstream` trait and adapters for Vertex (native `streamGenerateContent`),
Z.ai, the Pro door's proxy, OpenRouter, and Vercel. Z.ai and the Pro door
take only `standard` requests until their terms are confirmed
(`ZAI_TERMS_VERIFIED=zero-retention`, `PRO_TERMS_VERIFIED=zero-retention`),
and the Z.ai adapter refuses as unconfigured until `zai-api-key` exists.

### Pylon providers and local Psionic

Built for #11070 in `crates/inference/src/upstream/` (`psionic.rs`,
`pylon.rs`, `whole.rs`) and mounted by `crates/gateway`
(`inference_pylon`).

**Local Psionic.** `psionic-serve` on the caller's machine is the
upstream `local`; its models are `local/<psionic id>` (`PSIONIC_BASE_URL`,
`PSIONIC_MODELS`). It answers only a request that sends
`openagents.route.only: ["local"]`, so a gateway serving others never
routes their requests to a model server on its own host. It charges
nothing and the text never leaves the machine, so it is eligible under
`strict`. `psionic-serve`'s types are its own; the adapter translates:

| Open Responses | `psionic-serve` |
| --- | --- |
| `message` items | `input` as chat messages (`role`, text `content`); `developer` becomes `system` |
| `instructions`, `max_output_tokens`, `temperature`, `top_p`, `seed`, `stop` | the same names |
| a stream | `stream: false` (its `/v1/responses` does not stream); the whole answer is replayed as the spec's events |
| `message` with `output_text` | `output[].content[]` `output_text` (or the top-level `output_text`) |
| `reasoning` item | `reasoning_text` parts inside its message |
| `usage` | `input_tokens`, `output_tokens` (no details) |
| function tools, images, files, JSON schema | not offered (its tool calls come back outside the items); refused before sending |

**Pylon providers.** A provider registers with the gateway
(`inference.pylons`): its key, its pylon, the models it offers, its own
price per million tokens, and its stated data policy. Each registration is
the upstream `pylon:<pylon>`.

- The rate card row is the provider's price plus the margin. A job is the
  conversation as NIP-CJ text turns (instructions, transcript, task; at
  most 32 turns and 16 KiB), sent over `inference.pylon_relay` and signed
  by the gateway's buyer key (`<registry>/inference/pylon-buyer.key`),
  which the provider allowlists. No tools, images, or schema.
- When the answer is used, the provider earns exactly its price for the
  tokens: a settlement in the split ledger (`earnings.ledger`, resource
  `openagents.inference.pylon.v1`, key `debit:inference:<job request
  id>`) with the caller's price received from their balance, the
  provider's price as its share (`Earned`, role `provider`, paid to the
  pylon's owner when one is named), and the margin left to OpenAgents. It
  is converted to millisats at `inference.sats_rate`. The existing payout
  worker (`pay-ledger`'s `payout::tick` over its rails) pays it out; the
  gateway never sends money itself. An answer whose earning cannot be
  recorded is not served (the router falls back), so no provider work is
  used unpaid; an answer that arrives after the router gave up on it is
  not used and earns nothing.
- Privacy: a provider whose stated policy is no training and no retention
  is eligible under `strict`; any other, including one that stated
  nothing, serves `standard` only, like every other row.
- Without `earnings.ledger` and `inference.sats_rate` there is no way to
  pay a provider, so no Pylon upstream is mounted.
- `crates/gateway/tests/inference_pylon.rs` runs the whole path with a
  stub provider and fake payout rails: a `standard` API request served by
  a Pylon provider, the provider's 200-sat share in the ledger, and the
  payout worker paying it.

## 5. Routing

A request names a model id or a task class. The router turns it into an
ordered list of (model, upstream) attempts.

1. **Candidates.** A model id expands to every upstream that serves it
   (`google/gemini-3.8-flash` through Vertex, Vercel, or OpenRouter). A task
   class expands to its model list.
2. **Hard filters.** Drop a candidate that lacks a capability the request
   needs (read from the request's structure: tools, JSON schema, images,
   context length), fails the privacy level, is the wrong payer for
   `pay: "mine"`, costs more than the caller's `max_price` or their own
   limit, is benched (a 401 or 402 benches an upstream for five minutes, as
   `jev::doors` does), or draws on an exhausted credit balance.
3. **Quality floor.** Keep only candidates whose measured score for the
   task class meets that class's floor (section 6). With no score yet, the
   class table's order stands, and it keeps standing until Gym scores for
   the class exist; until then credit, price, and latency only order each
   model's upstreams.
4. **Rank.** Prepaid credit first (Google, Z.ai, the Pro door's free
   capacity), then price, then measured time to first token. A caller's
   `route.sort` or `route.order` replaces this ranking.
5. **Attempt.** Try in order. Fall back only before the first output token:
   on an HTTP error, a failure event, an empty stream, or no first token
   within the class's deadline (the chat worker's 4 s and 8 s rules today).
   After the first token, a failure is the caller's `response.failed`. At
   most three attempts. The last planned attempt has nothing to fall back
   to, so it runs to the class's ceiling (`last_ms`: chat 30 s, fast 15 s,
   classify 10 s, code and long 60 s, reason 90 s, a named model 30 s)
   rather than failing at the first-token deadline, and a caller that
   passes an observer to `Gateway::send_observed` is told once
   (`Progress::StillWorking`) when it outlives that deadline. The chat
   worker sends the same notice as `status: still_working`, and the chat
   shows "Still working on it…". Only when the ceiling passes does the
   request fail, in plain words.

`openagents/auto` picks the task class with a typed judgment (Jev, as the
chat router does), never by matching keywords in the prompt.

Added from episodes 242–246 (section 16), design until its issue lands:

- **Verified value.** The quality floor in step 3 will also read each
  model's accepted-outcome rate in real paid work, beside its Gym score
  ([#11079](https://github.com/OpenAgentsInc/openagents/issues/11079)).
  Only real runs count; fixture or synthetic runs never feed the floor.
- **The caller's own keys fail over too.** Under `pay: "mine"`, each of the
  caller's keys is its own candidate and is benched like ours (a 401, 402,
  or exhausted quota), so a spent key falls over to the caller's next one
  before the first token, never to ours.
- **Own coding capacity.** `openagents/code` under `pay: "mine"` will also
  consider the key owner's own linked Coder and subscriptions, own capacity
  only, never pooled or resold
  ([#11080](https://github.com/OpenAgentsInc/openagents/issues/11080)).

Starting class table, before measurement takes over:

| Class | For | Order |
| --- | --- | --- |
| `classify` | Labels, extraction, short yes or no | Pro door `gpt-5.6-luna`, then `zai/glm-5.3-flash`, then Gemini Flash-Lite on Vertex |
| `fast` | Short replies, openers, summaries | `zai/glm-5.3-flash`, then Gemini Flash on Vertex, then OpenRouter |
| `chat` | General conversation | Gemini Flash on Vertex, then Pro door `gpt-5.6-terra`, then OpenRouter |
| `code` | Coding and tool use | Highest Terminal-Bench and Gym score within the price; the user's own key or subscription when `pay: "mine"` |
| `long` | More than 200,000 tokens of context | Gemini on Vertex, then `zai/glm-5.3-flash` (both 1M) |
| `reason` | Hard multi-step problems | Pro door `gpt-5.6-sol`, then Gemini Pro on Vertex, then OpenRouter |

### Credit-aware routing

Each account we hold has a ledger row: balance, currency, expiry, and cost
basis (prepaid credit, free capacity, or pay-as-you-go). Among candidates
that meet the quality floor, the router spends prepaid credit before cash,
and credit that expires sooner first. A credit route never wins on credit
alone: below the floor it is not a candidate.

The gateway records each attempt's cost against its account and keeps a
burn-down per account: spend today, seven-day average, projected days
left, and days to expiry. It alerts the owner when an account crosses 50%,
25%, and 10% remaining, when projected runway drops under 30 days, and when
credit is on course to expire unspent. The gateway dashboard shows the same
numbers. Balances are reconciled daily against the provider's own billing
(Google's billing export, the Z.ai console).

## 6. Measurement

Implemented (#11062): `inference::meter` in `crates/inference/src/meter/`
keeps the attempt records, live rates, credit ledger, burn-down alerts
(log lines), and the daily reconciliation hook (`ProviderBilling`; real
billing sources come with the adapters). The gateway serves the state at
`GET /v1/admin/inference/status` to the bearer in the env var that
`inference.admin_token_env` names.

Every attempt writes one record. It holds counts and timings, never prompt
or completion text, like the chat worker's usage log today.

| Field | Notes |
| --- | --- |
| Request id, attempt number | Up to three attempts per request |
| Caller tenant and key id, API (`responses` or `chat`) | |
| Task class, requested model, model and upstream used, account billed | |
| Outcome | `ok`, `fallback` (and why), `failed`, `canceled` |
| Queue time, time to first token, total time, output tokens per second | |
| Input, cached input, cache write, output, and reasoning tokens | From the upstream's usage; counted by us when it gives none |
| Upstream cost, margin, price | Upstream cost from our rate row, checked against the upstream's own cost field when it sends one |
| Error class and upstream status | |

From those records:

- **Live rates.** p50 and p90 time to first token, throughput, error rate,
  and uptime per model and upstream over 5 minutes, 1 hour, and 24 hours.
  They feed routing and are published on `/v1/models` and the models page.
- **Cost reconciliation.** Daily, our computed cost against OpenRouter's and
  Vercel's reported cost and the providers' billing. A gap over 2% raises
  an alert.
- **Quality.** Gym suites per task class run nightly against each candidate
  model; the score is the quality floor check in section 5. Shadow runs
  send a small sample to a challenger model and compare, as the cost shadow
  baseline does today.
- **Tokens served** (added from episode 243, section 16). The one public
  number for the API: tokens served per UTC day from answering attempts,
  split into our own services and outside callers, and free and paid
  ([#11081](https://github.com/OpenAgentsInc/openagents/issues/11081)).
- **Published comparisons** (added from episode 243). A number we publish
  against another model or router comes from real runs; fixture or
  synthetic runs are labeled as illustrations and never published as
  measurements.

## 7. Accounts, keys, and billing

No new machinery. The gateway reuses what `crates/gateway`,
`crates/tenancy`, and `crates/pay-ledger` already run for the decision
gateway.

| Need | Existing piece |
| --- | --- |
| API keys | `tenancy::keys`: `oak_<id>.<secret>` bearer keys, stored as digests |
| Accounts and workspaces | `tenancy::accounts` and the gateway's account routes and SSO |
| Prices | `tenancy::money::Price` and `Rate` (input, cached input, output, reasoning tokens) |
| Hold, then settle | `tenancy::quota` and the gateway's `money` module (`observed-usage-v1`: reserve the worst case, settle the reported usage) |
| Balances and top-ups | `billing`, prepaid grants, `card_funding` (Stripe), Lightning `funding`; keyless 402 through `crates/x402` (OpenAgents API D3, D13) |
| Receipts | The gateway's sealed receipt per forwarded call |
| Usage views | `/v1/workspaces/{id}/usage` |
| Our own services | Service keys on a house tenant: metered the same way, not charged |

Limits: we impose none. A key's owner can set a spending cap, a maximum
price per request, allowed models and upstreams, a rate cap, and an expiry
(OpenAgents API D16; the gateway's `budgets` module). A request that hits
one gets `limit_reached` naming the limit. The jobs-at-once guard that
protects the service stays, as the chat worker's does.

## 8. Pricing

The rate card is public, with one row per model and upstream:

| Column | Example: `zai/glm-5.3-flash` |
| --- | --- |
| Upstream list price per million tokens (input, cached input, output) | $0.15, $0.03, $0.50 |
| Our margin | 5% |
| Price per million tokens | $0.1575, $0.0315, $0.525 |
| Price in sats | At the current rate, beside the dollar price |

Rules:

- **List price, whoever pays.** A request costs the upstream's list price
  plus margin, whichever of our accounts answered. Prepaid credit lowers our
  cost, not the posted price, so a caller's price never depends on our
  balances. Promotions (for example, a free model while a credit lasts) are
  separate rows, marked as promotions.
- **One stated margin.** We recommend 5% on tokens (under OpenRouter's 5.5%
  card fee), no fee on Lightning top-ups, and the card processor's fee
  passed through on card top-ups.
- **Free tier.** Keyless callers and new keys get free requests on the
  free-capacity models (Pro door Luna, and a GLM or Gemini Flash promotion
  while credit lasts), then a balance or a 402.
- **Bring your own key: no fee** in P1. The request is still metered and
  shows on the caller's usage.
- **Pylon providers** set their own price. The rate card shows it plus the
  same margin, and the provider receives their price.

Built (#11066): `inference::rates` turns the meter's rate card into the
public card. The gateway serves it at `GET /v1/rates` without a key, and
`GET /v1/models` (with a key) adds the OpenAI list shape (`object`,
`data`) beside the decision models, each model with its providers' rows
and last-hour live rates. Sats are figured at `inference.sats_rate`
(`usd_per_btc`, `as_of`) when the config sets it. A promotion is a
`promotion` on a rate row (label and caller prices): the meter charges
it and the card shows it as its own row. The Pro door's rows name the
provider "OpenAgents (Pro)". The website's `/docs/api/models` draws the
card from the gateway named by `--inference` (`OPENAGENTS_WEB_INFERENCE`),
or the adapters' published card when it has none; tests hold the page and
`/v1/rates` to the same card.

## 9. Privacy and data use

- **Default `strict`.** Every request goes only to endpoints that neither
  train on nor keep the request: OpenRouter `zdr`, Vercel
  `zeroDataRetention`, Vertex with caching and request logging off, and
  OpenAI and Anthropic zero retention where our accounts have it. This is
  today's chat worker default (`CODER_PROVIDER_PRIVACY=strict`), moved into
  the gateway.
- **`standard`** is the caller's opt-in to endpoints that may keep data for
  abuse review but do not train on it.
- Each catalog row states the endpoint's policy (`trains`, `retains`,
  `zdr`) and the source of that claim. An endpoint whose policy we have not
  verified is not eligible under `strict`. The Pro door and Z.ai need that
  check before they carry default traffic (section 14).
- **We keep no prompt or completion text.** Attempt records hold counts and
  timings. `store: true` (P2, built) is the only way a response is kept:
  only when the request asks, sealed to its owner, for 30 days unless the
  caller deletes it first, and never for a zero-retention tenant. A
  compaction item is the caller's to hold; we keep no copy.
- **User-key-only mode.** `pay: "mine"` uses only the caller's own keys and
  never falls back to ours: the gateway form of the desktop's "Use my keys
  for everything".
- The served privacy page
  (`crates/openagents-web/content/docs/privacy-and-security.md`) gains an
  API section in P1.

## 10. Compared with OpenRouter

Read from OpenRouter's [API reference](https://openrouter.ai/docs/api_reference/overview),
provider selection, BYOK, errors, and FAQ pages on 2026-10-09.

| OpenRouter | Us |
| --- | --- |
| Chat Completions at `/api/v1/chat/completions` | Match, as the backup surface |
| Responses API, stateless | Match, as our primary API, held to the Open Responses spec and its acceptance tests; stateful in P2 (built: `store`, `previous_response_id`, compaction, WebSocket) |
| `model` as `publisher/model`; a `models` fallback list | Match (`openagents.fallbacks`) |
| `provider`: `order`, `only`, `ignore`, `sort`, `allow_fallbacks`, `max_price` | Match, in `openagents.route` |
| `provider.zdr`, `data_collection` | Match as one `privacy` level, `strict` by default (opt-in there) |
| `require_parameters` | Always on: no request goes to an upstream that cannot honor it |
| `:nitro`, `:floor` suffixes | Leave out; `route.sort` covers them |
| Default balancing, weighted by inverse square of price | Different: quality floor, then prepaid credit, then price, then latency |
| Auto router | Match as `openagents/auto` and the task-class ids, chosen by a typed judgment |
| Structured outputs, tool calling | Match |
| Prompt caching and cached-token prices | Match: cached rows on the rate card |
| Usage in every response; generation stats endpoint | Match: `usage`, the `openagents` cost object, `GET /v1/usage/{id}` |
| Errors with `code`, `message`, `metadata`; HTTP 200 with an error mid-stream | Match in the Open Responses error shape; mid-stream as `response.failed` |
| Models API with prices | Match, plus live latency and uptime per upstream |
| Credits with a 5.5% card fee and no token markup | Different: a stated per-token margin and no Lightning top-up fee |
| BYOK at 5% after a monthly allowance | Different: no fee in P1 |
| Free models with daily request caps | Different: a free tier on free-capacity models; no other caps |
| App attribution headers and rankings | Leave out for now |
| Plugins (web search, file parser, response healing) | Hosted tools as Open Responses item types instead: web search built in P2 (`openagents:web_search`) |
| — | Ours only: prepaid Google and Z.ai credit, the Pro door, Pylon providers, a rate card that shows its margin |

## 11. Public docs

Served developer docs live with the site's other docs in
`crates/openagents-web/content/docs/` and follow the plain-copy rules in
`AGENTS.md`: no internal words such as door, lane, or tenancy. This spec
stays the internal reference. The outline follows OpenAI's developer docs
and OpenRouter's: start in three lines, then reference, then guides.

| Page | Contents |
| --- | --- |
| Quickstart | Three lines: get a key, `curl` to `/v1/responses`, then the OpenAI SDK with its base URL set to ours |
| Models and prices | Every model with its live rate card row, context, capabilities, and current speed |
| Open Responses reference | Request, response, items, streaming events, tools, errors |
| Chat Completions reference | The same, and what does not carry over (section 3's table) |
| Routing and credits | Model ids, task classes, `route`, fallbacks, and how we pick |
| Bring your own key | Adding keys, `pay: "mine"`, and what we charge (nothing) |
| Errors | Every code, its status, and what to do |
| Limits you set | Spending caps, price caps, allowed models, rate caps |
| Privacy and data use | `strict` and `standard`, what we keep (counts, not text), and each upstream's policy |
| Use with your tools (added from episode 243) | One page per tool, each a base URL, key, and model id: OpenCode first, then the Vercel AI SDK and the OpenAI SDKs. The acceptance run (#11068) includes OpenCode's request shapes: content-part arrays and streamed tool-call deltas, the two the June endpoint rejected |
| Decisions | Below |

The Decisions guide, our version of OpenAI's guide for choosing an
approach:

| If you need | Use | Why |
| --- | --- | --- |
| Labels, routing, a yes or no | `openagents/classify` | Cheapest and fastest; small models do short labels well |
| A short, fast reply | `openagents/fast` | GLM-5.3 Flash: low price, quick first token |
| General chat | `openagents/chat` | Balanced price and quality, measured daily |
| Coding with tools | `openagents/code`, or your own key | Picked by coding benchmark scores |
| Very long input | `openagents/long` | 1M-token models |
| The cheapest answer that is good enough | Any class with `route.sort: "price"` | |
| The fastest first token | `route.sort: "latency"` | |
| Your own provider contract or data terms | Your own key with `pay: "mine"` | Your terms apply, and we charge nothing |
| Open-weight models, or paying a community provider | Pylon providers (P2) | Provider-set prices, paid in bitcoin |

## 12. Where the code goes

Recommendation: a new library crate, `crates/inference`, mounted by the
existing gateway service.

| Piece | Home | Why |
| --- | --- | --- |
| Open Responses types, event codec, Chat Completions translation | `crates/inference` | Pure types and translation, testable without a server. `psionic-serve` keeps its own types; the `local` and Pylon adapters translate at the edge (section 4). |
| Adapters, router, rate card, credit ledger, attempt records | `crates/inference` | One library the gateway and our tools share |
| HTTP routes, keys, holds and settlement, balances, receipts | `crates/gateway` | Already the one admission path for decision calls, with keys and money; a new route keeps one place for both |
| OpenRouter client | `crates/openrouter`, used by its adapter | Exists |
| Who pays (`ours`, `mine`) | `crates/model-access` | Exists |
| Jev | `crates/jev`, unchanged | Decisions are a separate product |
| Pro door | An adapter in `crates/inference`; the private `pro` service retires after P1 | One fewer service, and its upstream key moves to the gateway |

Putting all of this in `crates/gateway` would load a provider layer into a
crate that knows nothing about models today. A separate service would
duplicate keys, balances, and receipts. The library-plus-mount split
avoids both.

## 13. Rollout

| Phase | Scope | Done when |
| --- | --- | --- |
| P0, internal | `crates/inference`: Open Responses types, Chat Completions translation, adapters (Vertex, Z.ai, Pro door, OpenRouter, Vercel), the router with credit-aware ranking, attempt records, burn-down alerts. The gateway serves `/v1/responses` and `/v1/chat/completions` to service keys. The chat worker sends its model calls through it and drops its provider keys. | Every chat worker turn names an upstream the gateway chose, every attempt is recorded, and the Google and Z.ai burn-downs show on the dashboard |
| P1, public beta | Public keys, both APIs, `/v1/models`, `/v1/rates`, `/v1/usage`, `/v1/key`, the free tier, BYOK, user-set limits, served docs, and a passing Open Responses acceptance run | An outside developer goes from key to first answer with the OpenAI SDK in under five minutes, and the acceptance suite passes against production |
| P2, marketplace | Pylon providers and local Psionic as upstreams with payouts, `store: true`, compaction, WebSocket, hosted tools | A Pylon provider earns from a public API request |

P0 as built (#11060 to #11064):

- `crates/inference`: the wire types and translation, the adapters
  (`upstream`), the meter, the router's plan, and the attempt loop
  (`run::Gateway`). The crate README lists the rules.
- `crates/gateway` mounts `POST /v1/responses` and
  `POST /v1/chat/completions` when `inference` is configured
  (`inference_routes`). Only an `oak_` key whose tenant is in
  `inference.service_tenants` is admitted (metered, not charged); a key
  scoped to actions needs `inference`. Adapters read their keys from the
  environment or mounted `*_FILE`s; one without its key stays out of
  routing. `openagents/auto` is judged by Jev (`TYPESAFE_API_KEY`); a
  judged class with no route right now answers as `chat`.
  `inference.classes` replaces the class table.
- `GET /admin/inference` is the dashboard: each credit account's burn-down,
  the alerts that hold, and live rates for the last hour and day, behind
  the admin token (`GET /v1/admin/inference/status` is the same as JSON).
- The chat worker sends every model call through the gateway when
  `CODER_INFERENCE_KEY` holds a service key (`CODER_INFERENCE_URL`,
  default `http://127.0.0.1:8790`; `CODER_INFERENCE_MODEL`, default
  `openagents/chat`). `CODER_WORKER_INFERENCE=direct` keeps the provider
  doors. Each result names the model the gateway chose, and the usage log's
  `door` names the upstream (`vertex via 127.0.0.1:8790`).
- `scripts/dev/inference-local.sh` runs the gateway, a chat worker on it,
  and the website on one machine.

P2 as built so far:

- #11071: stored responses, `previous_response_id`, compaction, the
  WebSocket transport, and hosted web search (section 3, "Stored
  responses, compaction, WebSocket, hosted tools").
- #11070: local Psionic and Pylon providers as upstreams, with provider
  earnings in the split ledger (section 4, "Pylon providers and local
  Psionic"). Not yet run against a live provider over a relay.

P1 public API as built (#11065), local only until deployed:

- `inference.public` in the gateway config opens `/v1/responses` and
  `/v1/chat/completions` to every active `oak_` key
  (`crates/gateway/src/inference_public.rs`); without it, only service
  tenants call. A public key's `Caller` carries a `run::Admission`, so
  every run (plain, stored turns, compaction, hosted tool loops, the
  WebSocket) is checked against the key's limits, planned
  (`Gateway::prepare`), admitted, sent (`Gateway::send`), and settled.
- Free tier: `inference.public.free_tier` (`requests_per_day`, `models`),
  counted per workspace (tenant) per UTC day so minting keys does not add
  free requests. A request is free only when every planned attempt is a
  listed model; one that fails before its first token gives its count back.
- Paying: one `tenancy::money` hold per distinct model the plan may try,
  priced at that model's rate card row plus margin (capacity `inference`,
  policy `observed-usage-v1`, a price version per model and rates), over
  the worst case: the request's bytes as input tokens plus framing (capped
  at the context window), and its `max_output_tokens` or the model's
  maximum as output. The answering model's hold settles at the reported
  usage; the others are released, since attempts that fall back before
  their first token are never charged. No usage at the end, or a caller
  who leaves mid-stream, leaves the hold outstanding for reconciliation.
  The workspace is `X-Workspace-Id` or the one workspace on the key's
  tenant, checked for current membership. No account or too little
  balance is `402 insufficient_balance`.
- Limits only the key's owner sets (`GET`/`PUT
  /v1/workspaces/{ws}/keys/{key}/limits`, a signed-in member, never the
  key itself): `spend_cap` (`usd`, `period` `day`, `month`, or `total`,
  checked against spend plus the request's worst case), `max_price`
  (dollars per million tokens, into the router's price limit), `models`,
  `requests_per_minute`, and `expires_at`. Each answers `403
  limit_reached` naming it in `param`. The key book is
  `inference-keys.json` beside the registry.
- `GET /v1/models` is public and in OpenAI's list shape (`object`,
  `data`), each model with an `openagents` object: context, maximum
  output, what it can do, whether the free tier covers it, and per upstream
  its price per million tokens with margin, zero retention, and the last
  hour's requests, uptime, and median time to first token; then the router
  ids. A keyed caller also gets the decision doors in `models`, as before.
- `GET /v1/usage/{request_id}` (the meter keeps 25 hours) and `GET
  /v1/key` (limits, spend today, this month, and in total, free requests
  left, balance), to the key's own tenant only.
- Web Settings, API keys (`/settings/api-keys`): make a key with a name
  and an optional monthly spending limit (shown once), and revoke keys.
- `openagents-web --inference http://HOST:PORT` (or
  `OPENAGENTS_WEB_INFERENCE`, the flag the models page reads the rate card
  through) also passes `/api/v1/...` to the gateway as
  `/v1/...` with the site's cookies removed: the `openagents.com/api/v1`
  alias.
- Not yet: keyless `402` through `crates/x402` (pay per request with no
  key), and hierarchical team budgets on inference holds (a shared-spend
  workspace is refused with a plain message).

Issues, in build order:

| Issue | Piece | Blocked by |
| --- | --- | --- |
| [#11060](https://github.com/OpenAgentsInc/openagents/issues/11060) | P0: `crates/inference` types, streaming codec, Chat Completions translation | — |
| [#11061](https://github.com/OpenAgentsInc/openagents/issues/11061) | P0: adapters (Vertex, Z.ai, Pro door, OpenRouter, Vercel) | #11060 |
| [#11062](https://github.com/OpenAgentsInc/openagents/issues/11062) | P0: measurement, live rates, credit burn-down | #11060 |
| [#11063](https://github.com/OpenAgentsInc/openagents/issues/11063) | P0: router | #11061, #11062 |
| [#11064](https://github.com/OpenAgentsInc/openagents/issues/11064) | P0: gateway routes for our services; chat worker moves onto them | #11063 |
| [#11065](https://github.com/OpenAgentsInc/openagents/issues/11065) | P1: public API beta | #11064 |
| [#11066](https://github.com/OpenAgentsInc/openagents/issues/11066) | P1: public rate card | #11062, #11065 |
| [#11067](https://github.com/OpenAgentsInc/openagents/issues/11067) | P1: bring your own key | #11061, #11065 |
| [#11068](https://github.com/OpenAgentsInc/openagents/issues/11068) | P1: Open Responses acceptance suite and OpenAI SDK run | #11065 |
| [#11069](https://github.com/OpenAgentsInc/openagents/issues/11069) | P1: public developer docs and Decisions guide | #11065, #11066 |
| [#11070](https://github.com/OpenAgentsInc/openagents/issues/11070) | P2: Pylon providers and local Psionic | #11065, #11066 |
| [#11071](https://github.com/OpenAgentsInc/openagents/issues/11071) | P2: stored responses, compaction, WebSocket, hosted tools | #11068 |

## 14. Owner steps

These are also in the workspace `NEEDS_OWNER.md`.

| Step | Why |
| --- | --- |
| Name the Google Cloud project and billing account that hold the roughly $30,000 inference credit, and its expiry | So the Vertex adapter bills that account and the burn-down starts from the right balance |
| Put a Z.ai API key in Secret Manager as `zai-api-key` in `openagentsgemini`, and note the credit balance and expiry | The Z.ai adapter reads it there |
| Confirm the Pro door's free-capacity terms: how much, until when, and whether a live-mode key replaces the test-mode one | The router spends it first for `classify` |
| Confirm the Stripe LLM proxy's and Z.ai's data terms (training and retention) | Neither carries `strict` traffic until then |
| Decide the margin (decision 1 below) | It is printed on every rate card row |

## 15. Open decisions

| # | Decision | Recommendation |
| --- | --- | --- |
| 1 | Margin on tokens | 5% flat, shown on every row |
| 2 | Rate card currency | Dollars per million tokens, as every upstream prices, with sats beside it; charge in sats from the balance. This refines the OpenAgents API's "prices always in sats" (D15) for the rate card only |
| 3 | Price of credit-funded models | List price plus margin; run promotions as separate, labeled rows |
| 4 | BYOK fee | None in P1; revisit if BYOK traffic costs us real money |
| 5 | Stateful Responses (`store: true`) | Stateless in P1, like OpenRouter; encrypted storage in P2 (built: opt-in, owner-scoped, 30 days, deletable) |
| 6 | Code home | New `crates/inference` library, mounted in `crates/gateway` (section 12) |
| 7 | The private Pro door service | Fold it into an adapter and retire `pro.openagents.com` after P1 |
| 8 | Credit routes whose data terms are unverified | Keep them out of `strict` until verified, even though it slows credit burn |
| 9 | Free tier size | A fixed number of free requests per new key per day, on free-capacity models only |
| 10 | Public model ids for the Pro door's models | Plain `openai/gpt-5.6-*` ids; the rate card names the upstream as "OpenAgents (Pro)" rather than the proxy vendor, matching the Pro door's own rule |
| 11 | The Khala name (episodes 242–245) | Not a product or model name: the roadmap lists Khala as historical. `openagents/auto` stays the routed model. Accept `openagents/khala` and `khala` as unlisted aliases of `openagents/auto`, so tool configs written for the June endpoint keep working |
| 12 | Free tier paid for with data (episodes 243 and 245) | No. The free tier keeps `strict` and keeps no text, like every request. Any trace sharing for pay is a separate opt-in product with its own terms, not a condition of the free tier |

## 16. Lineage: Episodes 241–247

Added 2026-10-09. The June 2026 episodes on Khala (a routed,
OpenAI-compatible "collective intelligence" API) are the first version of
this gateway. Khala is historical (`docs/glossary.md`, `docs/roadmap.md`);
this table records which of its ideas the gateway keeps, and where.
Sources: [241](../transcripts/241.md), [242](../transcripts/242.md),
[243](../transcripts/243.md), [244](../transcripts/244.md),
[245](../transcripts/245.md), [246](../transcripts/246.md),
[247](../transcripts/247.md).

| Episode | Idea | Verdict | Where it lives |
| --- | --- | --- | --- |
| 241 | Many models behind one OpenAI-compatible API, so no single vendor can cut access | Kept | Sections 1, 3, 4 |
| 241 | Open and inspectable, unlike a closed router over closed models: the caller sees which model answered and can opt out of upstreams | Kept | Section 3 (`openagents` response object, `openagents:route`, headers), `route.ignore` |
| 242 | One routed model id with free and paid use | Kept, renamed | `openagents/auto` and the class ids (section 3); free tier (section 8); Khala name, decision 11 |
| 242 | Selected by paid, verified value, not graded on its own benchmarks | Now in spec, build later | Section 5, [#11079](https://github.com/OpenAgentsInc/openagents/issues/11079) |
| 242 | Open pool: anyone running a Pylon serves and is paid in bitcoin | Kept | Section 4 (Pylon providers), P2 [#11070](https://github.com/OpenAgentsInc/openagents/issues/11070) |
| 242, 247 | Programs and other people's agents compose answers that are work (code, sites, briefs), with contributors paid | Later | [#11082](https://github.com/OpenAgentsInc/openagents/issues/11082), after #11070 |
| 242 | Confidential compute as one more route for private work | Later | A future privacy level above `strict` (section 9); no account today |
| 243 | Tokens served per day as the honest public number, ours kept apart from outside demand | Now in spec, build later | Section 6, [#11081](https://github.com/OpenAgentsInc/openagents/issues/11081) |
| 243 | One-config-line drop-in for OpenCode and other tools; the content-array and tool-call-delta bugs | Kept | Section 11 ("Use with your tools"), acceptance [#11068](https://github.com/OpenAgentsInc/openagents/issues/11068) |
| 243 | Free tier sized from a cost model, not a guess | Kept | Section 5 burn-down feeds decision 9 |
| 243 | Head-to-head Gym and honest published numbers | Kept | Section 6 (Quality, Published comparisons) |
| 243, 245 | Free tier paid for with data; selling traces | Dropped | Decision 12; section 9 keeps no text |
| 244 | Coding requests run on the caller's own subscriptions through their own Pylon or Coder; own capacity only, no resale, semantic routing | Now in spec, build later | Sections 4 and 5, [#11080](https://github.com/OpenAgentsInc/openagents/issues/11080) |
| 244 | Pooling other people's subscription capacity | Dropped | Section 4: never pooled or resold |
| 245 | "Your coding agent pays you": payouts from trace-derived plugins | Dropped for the gateway | No automatic royalties (roadmap); paid agent work is #11082 instead |
| 246 | Fail over across a user's several accounts when one is spent | Kept | Section 5 (the caller's own keys fail over too) |
| 247 | Referral commissions on new buyers | Later, not here | Belongs to the site's referral work, not the gateway |
