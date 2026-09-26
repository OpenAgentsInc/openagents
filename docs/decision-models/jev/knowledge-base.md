# Jev and TypeSafe: internal knowledge base

**Status (2026-09-16):** reference. Compiled from the live TypeSafe docs, the
official SDK sources, the Vercel AI SDK 7 source, and the AI Gateway docs on
2026-09-16. Re-check the [live docs index](https://docs.typesafe.ai/llms.txt)
before you rely on a limit or a field name; the vendored skill says the same.

This document is the local copy of what an agent needs to build against Jev
without a network read. It covers the model, the wire contract, the SDK
surfaces the Rust crate mirrors, the Gateway path, the design rules, and the
measured results the cookbooks report.

## What Jev is

Jev is TypeSafe AI's flagship model and the first of what TypeSafe calls
System One models. A System One model evaluates one **state** against a map of
typed **questions** and returns one typed **answer** per question, with
probabilities. It generates no text and no explanation. Code owns the
workflow; the model supplies a narrow judgment where code needs semantic
understanding.

Three question types exist:

| Type | Asks | Answer fields |
| --- | --- | --- |
| Choice | Which one of these named options? | `choice`, `probabilities`, `confidence` |
| Score | Which level on this ordered rubric? | `score`, `legend`, `probabilities`, `confidence` |
| Noul | Is this true? | `noul`, a probability of yes from 0 to 1 |

Every question in a request sees the same state and is evaluated
independently and in parallel. One answer never becomes context for another.
Adding a question adds tokens for the question text and little latency.

TypeSafe trains Jev with what it calls RLCD, reinforcement learning for
calibrated decisions: across many predictions, outcomes given probability
`0.8` should occur about 80% of the time. Calibration is a property of groups
of predictions, not a guarantee on one answer. The docs quote about 100 ms for
most queries, a token budget of about 32,000 tokens shared by state and
questions, and a target of more than 100 times the intelligence per unit of
speed and cost of a language model.

## The HTTP contract

```http
POST https://api.typesafe.ai/v1/systemone
Authorization: Bearer <API_KEY>
Content-Type: application/json
```

Request body:

| Field | Type | Notes |
| --- | --- | --- |
| `state` | string, object, or array | One state. An array is one state, not a batch. |
| `model` | string | `jev-latest` is the documented default. Cookbooks pin `jev-1.12`. |
| `questions` | map of id to question | You choose the ids. They are not sent to the model. |

Question shapes:

| Type | `instructions` | `criteria` |
| --- | --- | --- |
| `noul` | string, object, array, or null | Optional `{ "true": ..., "false": ... }`, each string, object, array, or null |
| `choice` | string, object, array, or null | Required map of option to description; a description may be null |
| `score` | string, object, array, or null | Required ordered array of at least two level descriptions; entries may be null |

Response body:

```json
{
  "model": "jev-latest",
  "answers": {
    "department": {
      "type": "choice",
      "choice": "technical",
      "probabilities": { "billing": 0.08, "technical": 0.85, "sales": 0.07 },
      "confidence": 0.82
    },
    "frustration": {
      "type": "score",
      "score": 1.6,
      "legend": { "0": "Calm", "1": "Frustrated", "2": "Very angry" },
      "probabilities": { "0": 0.05, "1": 0.3, "2": 0.65 },
      "confidence": 0.78
    },
    "is_urgent": { "type": "noul", "noul": 0.92 }
  },
  "usage": { "input_tokens": 312, "output_tokens": 48 }
}
```

Facts that matter to a client:

- Choice probabilities sum to one across the supplied options. Score
  probabilities sum to one across the levels, keyed by the zero-based level
  index as a string. A Score's `score` is the probability-weighted mean and
  can fall between levels.
- `confidence` is a statistic TypeSafe derives from the distribution's shape.
  Noul answers carry none. The AI SDK provider treats it as
  provider-specific metadata rather than a portable measure.
- The API rounds displayed probabilities and scores to two decimal places.
  The AI SDK provider declares `probabilityDecimals: 2, scoreDecimals: 2` so
  sum checks tolerate half a unit in the last place per value.
- A Choice question accepts at most 255 options. A Score question accepts at
  most 10 levels. The AI SDK provider rejects both before any I/O.
- The `usage` object in the generated OpenAPI schema still declares a
  `billing_units` field the API does not return. The SDKs treat
  `input_tokens` and `output_tokens` as optional.
- A second endpoint, `GET /v1/models`, returns `{ "models": [...] }` where
  each entry has `name`, `description`, and `release_date`.

Errors:

| Status | Meaning |
| --- | --- |
| `400` | The request is invalid. |
| `401` | Missing or invalid API key. |
| `403` | Access denied. |
| `404` | Not found. |
| `422` | Validation failed. The body names the field, in FastAPI's `detail` list shape: `[{ "loc": [...], "msg": "...", "type": "..." }]`. |
| `429` | Rate limit exceeded. Back off and retry. |
| `529` | TypeSafe is temporarily overloaded. Retry after a delay. |

Error bodies vary. Both SDKs extract a message from, in order: a string body,
`error` as a string, `error.message`, `message`, `detail` as a string,
`detail.message`, or a `detail` list rendered as `loc: msg` entries joined
with semicolons. A body over 200 characters is truncated in the message.

Headers the official SDKs send and read:

| Header | Direction | Value |
| --- | --- | --- |
| `Authorization` | request | `Bearer <key>` |
| `Accept` | request | `application/json` |
| `Content-Type` | request | `application/json` when a body is sent |
| `User-Agent` | request | `typesafe-sdk/<version>` |
| `X-TypeSafe-SDK` | request | `typesafe-sdk/<version>` |
| `X-TypeSafe-Runtime` | request | `node/22.1.0 (darwin; arm64)` or `python/3.12.1 (darwin; arm64)` |
| `X-TypeSafe-Retry-Count` | request | The attempt number on a retry; absent on the first attempt |
| `x-typesafe-request-id` | response | The request id, surfaced on responses and errors |
| `retry-after-ms`, `Retry-After` | response | Server retry delay; `retry-after-ms` wins; `Retry-After` may be seconds or an HTTP date |

## The official SDKs

The Rust crate mirrors these two. Both are MIT, both at version 0.6.0 as of
2026-09-15, and both had their first public release on 2026-09-11 (JS) and
2026-09-14 (Python). The 0.6.0 breaking change in both: `Score.criteria` is
an ordered sequence, no longer a dictionary keyed by integers.

### Shared defaults and environment

| Setting | Environment variable | Default |
| --- | --- | --- |
| API key | `TYPESAFE_API_KEY` | none; required |
| Base URL | `TYPESAFE_BASE_URL` | `https://api.typesafe.ai` |
| Default model | `TYPESAFE_DEFAULT_MODEL` | `jev-latest` |
| Log level | `TYPESAFE_LOG_LEVEL` | `warn` |
| Timeout per attempt | none | 10 seconds |

Explicit constructor values win over the environment. Empty or
whitespace-only environment values are ignored. Trailing slashes are stripped
from the base URL.

Note the AI SDK provider `@ai-sdk/typesafe-ai` reads a different variable,
`TYPESAFE_AI_API_KEY`, and defaults its base URL to
`https://api.typesafe.ai/v1`.

### Retry policy

| Field | Default | JS name | Python name |
| --- | --- | --- | --- |
| Retries after the first attempt | 2 | `maxRetries` | `max_retries` |
| First backoff | 500 ms | `backoffInitialMs` | `backoff_initial` (seconds) |
| Backoff cap | 5,000 ms | `backoffMaxMs` | `backoff_max` (seconds) |
| Jitter fraction subtracted | 0.25 | `backoffJitter` | `backoff_jitter` |
| Retried statuses | 408, 429, 500 to 599 | `httpStatuses` | `http_statuses` |
| Honor server delay headers | true | `respectRetryAfter` | `respect_retry_after` |
| Longest honored server delay | 60,000 ms | `maxRetryAfterMs` | not exposed |
| Retry connection errors | true | `apiConnectionError` | `api_connection_error` |
| Retry timeouts | true | `apiTimeoutError` | `api_timeout_error` |
| Extra exception types | none | not exposed | `exceptions` |
| Retry predicate | none | not exposed | `predicate` |
| Total retry budget | none in JS; 30 seconds in Python | not exposed | `timeout` |

Backoff is `initial * 2^attempt`, capped, then multiplied by
`1 - random() * jitter`. A server delay from `retry-after-ms` or
`Retry-After` replaces the backoff when honored and, in JS, when it does not
exceed the cap. Python's total budget stops before a retry whose delay would
reach the budget and re-raises the last error. A retry sends
`X-TypeSafe-Retry-Count` with the attempt number.

### Errors

| Condition | JS class | Python class |
| --- | --- | --- |
| Base | `TypeSafeError` | `TypeSafeError` |
| Non-2xx response | `APIError` with `status`, `headers`, `body`, `requestId` | `TypeSafeAPIError` with `status`, `body`, `headers`, `endpoint`, `request_id` |
| 400 | `BadRequestError` | `TypeSafeBadRequestError` |
| 401 | `AuthenticationError` | `TypeSafeAuthenticationError` |
| 403 | `PermissionDeniedError` | `TypeSafePermissionDeniedError` |
| 404 | `NotFoundError` | `TypeSafeNotFoundError` |
| 422 | `UnprocessableEntityError` | `TypeSafeUnprocessableEntityError` |
| 429 | `RateLimitError` with `retryAfterMs` | `TypeSafeRateLimitError` with `retry_after_ms` |
| 5xx | `InternalServerError` | `TypeSafeInternalServerError` |
| Connection failure | `APIConnectionError` | `TypeSafeAPIConnectionError` |
| Timeout | `APITimeoutError` extends the connection error, with `timeoutMs` | `TypeSafeAPITimeoutError` extends the connection error, with `timeout` |
| Caller cancelled | `APIUserAbortError` | none; cancellation is the caller's |
| 2xx body that fails the schema | none; the body parses leniently | `TypeSafeAPIResponseValidationError` with a dotted `field_path` |

Client-side validation raises the base error before any I/O when: the
question map is empty; a Score's criteria is not a list (JS) or is empty
(Python); the JS builder `score()` receives a map or `choice()` receives a
list; a timeout is not positive and finite; a retry field is out of range;
the API key is missing; or the JS client runs in a browser without
`dangerouslyAllowBrowser`.

### JavaScript surface (`@typesafe-ai/sdk` 0.6.0)

Exports: `TypeSafeClient`, `APIPromise`, `WithResponse`, `ENV`, `EnvVar`,
the twelve error classes above, `LOG_LEVELS`, `choice`, `noul`, `score`,
`Models`, every type in `types.ts`, and `VERSION`.

`TypeSafeClient` construction takes `apiKey`, `baseURL`, `defaultModel`,
`logLevel`, `logger`, `retry`, `timeout`, `defaultHeaders`,
`dangerouslyAllowBrowser`, and `fetch`. Public readonly fields expose every
setting except the key. `client.models` is a `Models` resource.

`client.systemOne(request, options)` takes `{ state, questions, model? }`
plus per-call `{ signal?, timeout?, retry?, headers? }` and returns an
`APIPromise<SystemOneResult<Q>>`. The result has `model`, `answers` typed by
question, and `usage`. The `APIPromise` adds `asResponse()`,
`withResponse()` returning `{ data, response, requestId }`, and `map(fn)`.

`client.models.list(options)` returns `APIPromise<ModelCard[]>`, unwrapping
the `{ models }` envelope and throwing the base error on any other shape.

The response body is parsed as JSON when possible and kept as text
otherwise. A user-supplied header cannot override authentication, `Accept`,
or the content type. Log levels are `debug`, `info`, `warn`, `error`, and
`off`; `info` logs one line per attempt with status, elapsed time, and
request id; `debug` adds headers, with credential headers masked to their
scheme and last four characters, and bodies unmasked.

### Python surface (`typesafe-sdk` 0.6.0)

Public members: `TypeSafeClient`, `AsyncTypeSafeClient`, `Models`,
`AsyncModels`, `Choice`, `Score`, `Noul`, `NoulCriteria`, the `ChoiceModel`,
`ScoreModel`, `NoulModel`, `QuestionModel`, `Question`, and `Questions`
dictionary types, `Answer`, `ChoiceAnswer`, `ScoreAnswer`, `NoulAnswer`,
`SystemOneResponse`, `ListModelsResponse`, `ModelMetadata`, `Usage`,
`RetryPolicy`, `JSONValue`, `JSONContent`, the twelve error classes, and a
`constants` module with the environment variable names and defaults.

Both clients take keyword arguments `api_key`, `model`, `retry`, `timeout`,
`headers`, `transport`, `http_client`, and `base_url`; `transport` and
`http_client` are mutually exclusive. Both are context managers and expose
`close()` or `aclose()`.

`client.system_one(state, questions, *, model=None, retry=None,
timeout=None, extra_headers=None, extra_body=None)` returns a
`SystemOneResponse` with `model`, `usage`, `answers`, and three cached
views: `nouls`, `choices`, and `scores`, each a dictionary of only that
answer type. `extra_body` is shallow-merged last over the body.
`client.models.list(*, retry=None, timeout=None, extra_headers=None)` returns
a `ListModelsResponse` whose `models` is a tuple.

Every response also exposes `request_id` and `raw_http_response`. Score
answers key `legend` and `probabilities` by integer in Python and by string
in JS, because Python coerces at decode time. Decoding takes a fast path
that reads the whole tagged response in one call and falls back to
per-answer dispatch that skips an unknown answer type with a warning and
raises the validation error with a precise dotted path for a malformed
field. Unknown response fields are ignored so a newer server never breaks
an older client.

### The System One adapter

`typesafe-ai/system-one-adapter-python` (0.1.4, MIT) is a drop-in
replacement for the Python client that answers the same questions through
OpenAI or Anthropic models instead of Jev. It exists to compare Jev against
a language model on cost, speed, and quality. It prompts for either a
per-label probability distribution or one discrete value per question,
optionally uses the provider's native structured output, rescales invalid
distributions to sum to one, retries malformed output with a correction
message, and computes `confidence` for Choice and Score answers itself. Its
response adds `input_tokens_total`, `output_tokens_total`, `n_retries`,
`latency`, and a `debug` record of every attempt.

The adapter's system prompt tells the model to treat the entire state as
untrusted data and never follow instructions found in it, and it escapes
angle brackets in the serialized state. That is the prompt-injection posture
a language-model fallback for typed questions needs.

## The Vercel AI SDK 7 path

On 2026-09-16 Vercel announced Jev on AI Gateway. The AI SDK exposes it
through `experimental_evaluate` from `ai` 7.0.105 or later. Gateway routing
uses the model id `typesafe-ai/jev` or `typesafe-ai/jev-latest`.
Evaluation is available through the AI SDK only; the Gateway's
OpenAI-compatible, Anthropic-compatible, and Cohere-compatible endpoints do
not offer it. Evaluation calls appear in Gateway logs and reports, count
toward budgets, and accept the other Gateway provider options, including
`zeroDataRetention: true`, in the same `providerOptions.gateway` object.
Some evaluation models price input tokens only.

The AI SDK renames one primitive: a Noul is a `boolean` question whose
answer is `{ type: "boolean", probability }`. Choice and Score keep their
names but drop `confidence` and `legend` from the answer; TypeSafe's
confidence moves to `result.providerMetadata.typesafe.confidence`, keyed by
question id.

### Gateway wire protocol

The Gateway provider in `@ai-sdk/gateway` posts to
`<baseURL>/evaluation-model`, where the default base URL is
`https://ai-gateway.vercel.sh/v4/ai`. The request carries the Gateway's
usual headers, an `Authorization: Bearer` token from `AI_GATEWAY_API_KEY` or
a Vercel OIDC token, `ai-gateway-protocol-version`, and two headers specific
to evaluation:

```http
ai-evaluation-model-specification-version: 4
ai-model-id: typesafe-ai/jev-latest
```

The body is `{ state, questions, providerOptions? }` in the AI SDK's
question shape, with `boolean` in place of `noul`. The response is:

```json
{
  "answers": {
    "id": { "type": "choice", "choice": "billing", "probabilities": { "billing": 0.9 } },
    "other": { "type": "boolean", "probability": 0.98 }
  },
  "rounding": { "probabilityDecimals": 2, "scoreDecimals": 2 },
  "usage": { "inputTokens": 283, "outputTokens": 21 },
  "warnings": [],
  "providerMetadata": { "typesafe": { "confidence": { "id": 0.82 } } }
}
```

`probabilities` is optional on Choice and Score answers in this contract.
Warnings are typed `unsupported`, `compatibility`, `deprecated`, or `other`.
A client that reaches Jev through the Gateway speaks this protocol; a client
that reaches TypeSafe directly speaks the `/v1/systemone` contract above.
The owner chose the direct API on 2026-09-16, so the
[Rust SDK](rust-sdk.md) speaks `/v1/systemone` and this protocol is recorded
for a later `gateway` feature.

### AI SDK semantics worth copying

- One call evaluates one state. No streaming, no multilabel, no batching of
  unrelated states.
- The model's supported question types are checked before any I/O, and one
  unsupported question fails the whole call.
- A successful call returns an answer for every question; there is no
  partial success and no automatic model substitution.
- A Choice distribution, when present, includes every option and the
  selected choice has the maximal probability. Distributions must sum to
  one within a tolerance of one millionth plus the declared rounding. A
  Score with a distribution equals its weighted mean. Invalid output is
  rejected, never normalized.
- Language-model adapters for OpenAI, Anthropic, and Google exist behind the
  same API but evaluate all questions in one prompt, return no Choice or
  Score distributions, and make no calibration promise.

## Design rules

These are the rules the docs, the skill, and the cookbooks repeat. Apply them
when you write a question set for Coder.

1. **Keep control flow, deterministic rules, exact lookups, and side effects
   in code.** Insert a judgment only where code needs semantic
   understanding. Do not build an agent loop where a workflow works.
2. **Ask one narrow judgment per question.** A judgment a knowledgeable
   person makes in a second given the right context. Split a broad judgment
   into independent dimensions and combine them in code with weights you
   own.
3. **Send only the relevant state, structured.** Prefer a JSON object with
   named fields. Point a question at a field with a backticked path such as
   `` `ticket.messages[0].text` ``.
4. **Put the judgment in `instructions` and the answer space in
   `criteria`.** Question ids are not sent to the model. Use objects or
   arrays for instructions and criteria when definitions, contrasts,
   exclusions, or examples clarify a boundary. Give Choice options
   contrastive descriptions with the same field names across options.
   Score levels must describe concrete situations and stand alone.
5. **Include a no-match outcome.** Add `other` or `none` to a Choice when
   the list may not cover every input. Use a separate Noul for presence when
   that answer is useful on its own. Choice probabilities always sum to one,
   so a ranking alone cannot say that nothing fits.
6. **Ask every question that shares a state in one request, including
   speculative ones.** Code ignores the answers on unused branches. Make a
   second request only when the first answer is needed to fetch evidence,
   build new state, or choose the next options.
7. **Route on probability and confidence with thresholds tuned on your own
   labeled data.** Confidence summarizes distribution shape, not workflow
   correctness. A Noul near 0.5 means yes and no are equally likely, not a
   medium intensity. Different actions deserve different thresholds by
   consequence. If you only want the best option, take the highest
   probability and skip the threshold.
8. **Keep questions and thresholds in one reviewable place.** The agent-skill
   page says agents write weak questions and expects a person to edit them.
9. **Keep the key server-side.** Never ship the key to a browser or a client
   surface.
10. **Test representative cases and the resulting behavior.** For a failure,
    inspect the exact state, questions, candidates, answers, composition,
    and outcome. Separate missing evidence, model error, code error, and
    service failure. Treat cookbook thresholds as examples to evaluate.

## Patterns

| Pattern | Shape | Benefit |
| --- | --- | --- |
| Speculative fan-out | Ask category, severity, refund, and frustration together; branch in code and ignore the rest. | Cost, speed |
| Confidence-gated routing | Below a floor, hand to a person. Above it, each action has its own threshold by stakes. | Reliability, safety |
| Composite scoring | Score each dimension, normalize, weight in code, re-weight without re-running inference. | Cost, reliability, speed |
| Intent routing | One Choice picks a handler: deterministic code, a specialist model with its own context, or a person. A Score on complexity decides between the last two. | Cost, speed |

## Cookbook index with measured results

Each cookbook ships its cached API responses, so it replays without a key.
The numbers are TypeSafe's, on their data, with the model version each page
names. Treat them as claims to re-measure on Coder's data.

| Cookbook | Recipe | Reported result |
| --- | --- | --- |
| Line-by-line search | Tag each line with an id, one Choice over all line ids ranks them, one Noul says whether the document answers at all. | GitHub's terms of service, 218 lines, one request. Present answers read at or above 0.9 on the Noul; absent ones at or below 0.05. |
| Skill suggestion | One request ranks 182 skills with a Choice and asks three Nouls whether the turn wants an action; a second request rereads the top three with full text and may reject all. | Wrong skill loads fell from 16.8% to 7.3% and needless loads from 9.8% to 4.0% over 488 turns on a small Anthropic model. |
| Parallel questions | Thirteen questions over one 54,000-character article, batched against one at a time. | Batching was 12.2 times cheaper and 10.0 times faster with no change in answers. |
| Re-ranking | BM25 shortlist of 30, then one Choice or Noul per query and candidate pair. | Top-1 accuracy on 40 legal queries rose from 5% to 18%; top-10 from 38% to 62%. |
| Classifying RAG passages | Four questions per query and passage pair; code keeps, flags, or drops each passage before the answering model. | Two thirds of retrieved passages excluded; only false-premise queries routed anything to a conflict block. |
| Guardrails | Four Nouls per message on hazard criteria plus a Score on severity; two thresholds per Noul decide pass, review, or block. | The same probabilities under a stricter and a looser policy give different decisions without re-running inference. |
| Self-consistency: nouls | One 14-question rubric over one insurance claim, 15 times. | Jev at 111 ms and $0.000043 a call; small chat models 10 to 16 times slower and 22 to 42 times costlier; reasoning models 100 to 125 times slower and 779 to 805 times costlier. |
| Self-consistency: choices | Moderation labels with an explicit uncertain outcome; compares label agreement with the share of automatic actions. | Adding an uncertain outcome trades automation share for agreement. |
| Function calling | Map function names and closed-set arguments to Choice questions; consume only the chosen branch's arguments. | Natural-language trading requests become typed calls. |
| Structure recovery | Request one: is each line break a sentence split. Request two: classify each merged block. | Markdown rebuilt from flattened text. |
| Citation check | One Choice decides whether the quoted context supports the claim; low confidence flags for review. | Catches wrong or invented citations against the source. |
| SDE cascade | Extract with a small model, verify fields with Jev, escalate failures to a reasoning model. | Most of the large model's quality at a fraction of the cost. |
| Date extraction | Ask for the parts a document names; resolve and validate in code. | Absolute and relative dates with confidence-based review. |
| Pre-parsed value extraction | Regex finds candidate emails, phones, and amounts; Jev selects the intended span; code normalizes verbatim. | The model cannot pick a value the regex did not surface, so coverage is checked in code. |
| Hierarchical classification | One Choice per taxonomy level with subtrees as option values; parallel beam search on the probabilities. | Patent, retail, biomedical, and source-code hierarchies. |
| Entity alignment | One Score whose three levels are the three actions: merge, leave unlinked, hand to a curator. | 450 candidate pairs with no threshold to fit. |
| Classification using confidence | One Choice into 75 industry groups; low confidence reports the broader division instead. | SEC annual reports. |
| Autoresearch feature discovery | A loop proposes questions, turns text into numeric features, and trains a gradient-boosted regressor. | Model errors drive the next question proposals. |

## The announcement, 2026-09-16

Vercel's changelog states that TypeSafe reports Jev was up to 193.6 times
faster and 444.6 times cheaper than language models on its workflow
evaluations, and lists the use cases Vercel names for an agent loop:
choosing the next tool or subagent, deciding whether to continue, retry, ask
the user, or stop, scoring urgency or risk before an action, and verifying
model outputs and enforcing guardrails. Those four are the seams the
[integration map](../../coder/design/decision-function-inventory.md) checks against Coder.

## Current integration and credentials

The earlier Coder integration described in this September 16 reference is
historical. The current SDK is [`crates/jev`](../../../crates/jev/); its
[current client matrix](../guides/clients.md) and [caller guide](../guides/caller.md)
are authoritative for supported settings. `jev` accepts an explicit `ApiKey`
or `TYPESAFE_API_KEY` from the environment. Host applications can provide their
own protected configuration loader; the SDK does not search a private sibling
repository or automatically load the older `coder_jev` configuration file.

Keep credentials out of source, logs, transcripts, fixtures, and issue text.
A model endpoint setting and possession of a key do not authorize a benchmark
campaign, another recipient, or disclosure of private inputs.

## Sources

- TypeSafe docs index: <https://docs.typesafe.ai/llms.txt>
- API reference: <https://docs.typesafe.ai/api.md>
- Primitives: <https://docs.typesafe.ai/primitives.md>, plus the Choice,
  Score, Noul, and advanced-structure pages
- Confidence: <https://docs.typesafe.ai/confidence.md>
- Building guide: <https://docs.typesafe.ai/concepts/how-to-build-with-system-one.md>
- Patterns: <https://docs.typesafe.ai/patterns.md>
- Agent skill: <https://docs.typesafe.ai/agent-skill.md>
- JavaScript SDK: <https://github.com/typesafe-ai/typesafe-sdk-js> at v0.6.0
- Python SDK: <https://github.com/typesafe-ai/typesafe-sdk-python> at 0.6.0
- System One adapter: <https://github.com/typesafe-ai/system-one-adapter-python>
- Skill source: <https://github.com/typesafe-ai/skills>
- Vercel changelog: <https://vercel.com/changelog/typesafe-ai-jev-now-available-on-ai-gateway>
- AI SDK evaluation: <https://ai-sdk.dev/docs/ai-sdk-core/evaluation>
- AI Gateway evaluation: <https://vercel.com/docs/ai-gateway/modalities/evaluation>
- AI SDK source: `packages/typesafe-ai`, `packages/gateway`, and
  `packages/provider/src/evaluation-model/v4` in <https://github.com/vercel/ai>
- Line-by-line search cookbook: <https://docs.typesafe.ai/cookbooks/semantic_find.md>
