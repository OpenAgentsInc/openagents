# Decisions

2026-10-12. Proposed. `POST /v1/decisions` is not mounted yet. Everything it
runs on is live: the decision itself is the [NIP-DEC](../../nips/openagents/NIP-DEC.md)
request that `POST /v1/systemone` answers today, keyless, at
`https://openagents.com/api/v1/systemone`.

The Decisions API reads text, images, or both and returns typed answers: the
probability that a condition is true, one choice from a fixed set, or a score
against ordered levels. It generates no text, so an answer costs one prefill
and comes back in a fraction of a second. Use it to classify, route, filter,
and rank inside an application, an agent, or a program.

The request and response match OpenAI's Decisions API field for field, so an
OpenAI SDK pointed at our base URL works unchanged. Underneath, every call is
one NIP-DEC decision, answered by the same doors that answer it over our
relay: open-weight Clef models on Pylons and our own GPUs first, then
backups. The same call can go over HTTP with an API key, over Nostr as a
decision job, or to a Pylon you run yourself.

It is a PUBLIC surface on `https://api.openagents.com/v1` (alias
`https://openagents.com/api/v1`) and follows [the API design](design.md):
one error object, `oak_` keys, sats, and no house rate limit on your own
keys. Headers, the workspace, and body size that every call shares are in
[Calling the API](calling.md).

## Contents

1. [Quick start](#quick-start)
2. [The request](#the-request)
3. [Question types](#question-types)
4. [The response](#the-response)
5. [Models](#models)
6. [Asking many questions at once](#asking-many-questions-at-once)
7. [Images](#images)
8. [Interpreting answers](#interpreting-answers)
9. [Three ways to call it](#three-ways-to-call-it)
10. [How a call is served](#how-a-call-is-served)
11. [Limits](#limits)
12. [Errors](#errors)
13. [Price and payment](#price-and-payment)
14. [Privacy](#privacy)
15. [Relation to `/v1/systemone`](#relation-to-v1systemone)
16. [Building it](#building-it)

## Quick start

```bash
curl https://api.openagents.com/v1/decisions \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "openagents/decide",
    "input": "I was charged twice for my order.",
    "questions": [{
      "type": "choice",
      "name": "department",
      "instructions": "Which department should handle this complaint?",
      "choices": [
        {"value": "billing", "description": "Payments, invoices, and refunds."},
        {"value": "technical", "description": "Problems using the product."},
        {"value": "shipping", "description": "Delivery and tracking."},
        {"value": "other", "description": "Requests outside these categories."}
      ]
    }]
  }'
```

```json
{
  "id": "dec_01JA8Q6V8M4T2C9W3X5Y7Z0B1D",
  "object": "decision",
  "model": "clef-flash",
  "answers": [
    {
      "type": "choice",
      "name": "department",
      "choice": "billing",
      "probabilities": [
        {"value": "billing", "probability": 0.95},
        {"value": "technical", "probability": 0.02},
        {"value": "shipping", "probability": 0.01},
        {"value": "other", "probability": 0.02}
      ],
      "confidence": 0.95
    }
  ],
  "usage": {"input_tokens": 212},
  "openagents": {
    "door": "pylon:coderos-4080-clef",
    "latency_ms": 184,
    "cost": {"price_sats": 0, "price_usd": "0.000000"}
  }
}
```

With an OpenAI SDK, set the base URL and the key; nothing else changes:

```python
from openai import OpenAI

client = OpenAI(base_url="https://api.openagents.com/v1", api_key=OPENAGENTS_API_KEY)
decision = client.decisions.create(
    model="openagents/decide",
    input="Export fails in Safari but works in Chrome.",
    questions=[{
        "type": "score",
        "name": "severity",
        "instructions": "How severe is this issue?",
        "levels": [
            {"label": "Cosmetic", "description": "Appearance only; no lost functionality."},
            {"label": "Workaround available", "description": "A task fails, but another way works."},
            {"label": "Fully blocked", "description": "A task fails with no workaround."},
        ],
    }],
)
print(decision.answers[0].score)
```

In Rust, `crates/jev` already speaks the same decision; see
[Three ways to call it](#three-ways-to-call-it).

## The request

| Field | Type | Meaning |
| --- | --- | --- |
| `model` | string | The model that answers. See [Models](#models). Required. |
| `input` | string, or an array of user messages | The shared evidence every question reads. A message has `role: "user"` and `content`: a string, or parts of `{"type": "input_text", "text"}` and `{"type": "input_image", "image_url": "data:image/...;base64,..."}`. Required. |
| `questions` | array, 1 to 64 | What to judge. Each has `type`, `name`, `instructions`, and its type's options. Required. |
| `metadata` | object | Up to 16 string pairs, echoed back and kept with the call's record. Optional. |

A question:

| Field | Applies to | Meaning |
| --- | --- | --- |
| `type` | all | `predicate`, `choice`, or `score`. |
| `name` | all | Your id for the question, 1 to 64 bytes, unique in the request. The answer echoes it. Optional; omitted names become `q0`, `q1`, … in request order. Names are for you: they are not shown to the model as meaning. |
| `instructions` | all | What to judge, as a string. Write it around something observable. |
| `choices` | `choice` | 2 to 255 options of `{"value", "description"}`. `value` is a string, 1 to 128 bytes, unique. |
| `levels` | `score` | 2 to 10 ordered levels of `{"label", "description"}`, lowest first. Level indices start at 0. |
| `true`, `false` | `predicate` | Optional descriptions of what yes and no mean, when the plain question leaves room. |

Headers: `Authorization` (an `oak_` key, a session, or none for the free
lane), `X-Workspace-Id` for a session with more than one workspace,
`Idempotency-Key` (the same key and body within 24 hours returns the first
answer without a second charge; a different body is `409`
`idempotency_conflict`), and `X-Attempt` on a deliberate retry.

## Question types

| Type | Use it to | Main result | NIP-DEC type |
| --- | --- | --- | --- |
| `predicate` | Check a condition: visible damage, a file is relevant, a message is urgent. | `probability`, from 0 to 1, that the condition is true. | `noul` |
| `choice` | Pick one option without an order: a department, a route, a category. | `choice`, one of your values, with `probabilities` over all of them. | `choice` |
| `score` | Rate against ordered levels: severity, quality, priority. | `score`, the probability-weighted level index (it can fall between levels), with `probabilities` per level. | `score` |

Include a fallback option such as `other` when your choices do not cover
every input, and send its answers to a review queue. Do not add a "none of
these" option to a question that already has an explicit "nothing wrong"
option: open-weight models read it differently from Jev and pick it far too
often ([clef-self-host.md](../inference/clef-self-host.md), "The `none`
problem").

## The response

| Field | Meaning |
| --- | --- |
| `id` | `dec_` plus an opaque id. |
| `object` | `"decision"`. |
| `model` | The model that answered, by its served name (for `openagents/decide`, the one that was chosen). |
| `answers` | One answer per question, in request order. |
| `usage` | `input_tokens`, as the answering model counted them; `cached_input_tokens` when a prefix was reused. There are no output tokens. |
| `metadata` | Your `metadata`, echoed. |
| `openagents` | Ours, beyond OpenAI's shape: `door` (which door answered: `pylon:<slug>`, `clef`, `jev`, `vertex`), `latency_ms`, `cost` (`price_sats`, `price_usd`), and `receipt` (the sealed execution receipt's id). SDKs ignore fields they do not know. |

Answers:

```json
{"type": "predicate", "name": "visible_damage", "probability": 0.92}

{"type": "choice", "name": "department", "choice": "billing",
 "probabilities": [{"value": "billing", "probability": 0.95}, {"value": "other", "probability": 0.05}],
 "confidence": 0.95}

{"type": "score", "name": "severity", "score": 1.1,
 "probabilities": [{"value": 0, "label": "Cosmetic", "probability": 0.1},
                   {"value": 1, "label": "Workaround available", "probability": 0.7},
                   {"value": 2, "label": "Fully blocked", "probability": 0.2}],
 "confidence": 0.7}

{"type": "refusal", "name": "visible_damage", "code": "unsupported_input",
 "message": "This model does not read images."}
```

- `probabilities` list every option or level you sent, in your order, and
  sum to 1 within 0.000001.
- `confidence` is the probability of the top option or level. It is not an
  accuracy estimate, and the open-weight servers each define it differently
  if read raw; we always return the top probability. Read `probabilities`
  for decisions; read `confidence` only as a summary.
- A `refusal` answers one question that could not be judged (an input type
  the model does not read, a question the model's server refused). The other
  answers in the response stand. A request that is wrong as a whole is an
  [error](#errors), not a refusal.

## Models

| Model | What answers | Reads images | Notes |
| --- | --- | --- | --- |
| `openagents/decide` | The best available door for the request: Clef on Pylons and our GPUs first, then the backups ([How a call is served](#how-a-call-is-served)). The default. | When the chosen door does | `model` in the response names what answered. |
| `clef-flash` | Cloudflare's open-weight Clef-Flash (9B), Q4_K_M, served natively by Psionic | Planned (Psionic serves images on CPU and Metal; CUDA refuses them today) | Fastest. Under-confident on file relevance; calibrate thresholds per use. |
| `clef` | Clef (27B), the same family's larger model | Planned, as above | The quality tier. Best of our models on file relevance (AUC 0.91 on closed issues); about 2.6× slower than Flash on an H100. |
| `jev-latest` | TypeSafe's Jev, relayed | No | A backup door. Callers with their own TypeSafe key can still call TypeSafe directly. |

`GET /v1/models` lists them with `capabilities: ["decision"]`, each model's
limits, and whether it reads images. A model name authorizes nothing; the
gateway admits it and says which door served it.

## Asking many questions at once

Put independent questions about the same input in one request. The input is
read once, and every extra question costs only its own text, about 120–130
tokens with its options. Measured on our own servers
([clef-batching.md](../inference/clef-batching.md),
[decision-throughput.md](../inference/decision-throughput.md)):

| Machine | One question per request | Batched |
| --- | ---: | ---: |
| One H100, `clef-flash` | 9.9 decisions/s | 39 decisions/s at 13 questions |
| One H100, `clef` | 3.8 decisions/s | 13.5 decisions/s at 13 questions |

- Throughput rises until about 13–26 questions a request; past that it stops
  rising and only latency grows. 64 is the ceiling.
- Questions in one request are judged together and can see each other.
  Adding or removing one can move the others' answers slightly. For
  decisions that depend on an earlier answer, send a second request.
- Keep the input to what the questions need. For many candidates (files,
  documents, products), send each as a short, bounded summary rather than
  its full text. On our closed-issue test, sending each file's path, why it
  was a candidate, and an outline of its declarations beat sending its
  contents, at a fraction of the tokens
  ([clef-batching.md](../inference/clef-batching.md), "How to pack many
  files into one request").

## Images

Images go in a user message as `input_image` parts with a base64 data URL
(PNG, JPEG, or WebP), up to 4 images and the 8 MiB body limit. Hosted URLs
and file ids are not fetched. The image is placed before the text, as the
Clef reference does. A model or door that does not read images answers that
request's questions with `refusal` `unsupported_input`, and
`openagents/decide` routes an image request only to doors that read images.

Images travel over HTTP only. A NIP-DEC decision job carries text; its
payload bound (256 KiB) is too small for images, and an image job form
is open work (see [Building it](#building-it)).

## Interpreting answers

- A probability is a measurement, not permission. Pick each threshold from
  labeled examples of your own, by the cost of a false yes against a false
  no.
- Thresholds do not carry across models. Clef-Flash's raw probabilities run
  low on relevance questions: the right cut on our file test was about 0.35
  for 27B batched, but 0.15–0.2 for Flash. Our own callers apply per-model
  calibration maps, keyed by the served `model`.
- Same server, same request, same answer: our Clef servers return bitwise
  identical answers to a repeated request. Different doors (a Pylon, our
  GPU, a backup) can differ slightly; `openagents.door` says which answered.

## Three ways to call it

The same decision, three transports:

| Transport | Who it is for | What you send |
| --- | --- | --- |
| HTTP, `POST /v1/decisions` | Apps, sites, other agents, OpenAI SDKs | This page's request, with an `oak_` key, a session, or nothing for the free lane. |
| Nostr, a NIP-DEC decision job | Agents on our relay, and our own apps | Kind `25910` to a decision worker or a Pylon, NIP-44 encrypted; the result is kind `26910`. The body is the NIP-DEC request the translation below produces. The CLI and Coder do this today through `jev_hosted`. |
| Your own Pylon | Anyone with a GPU | `openagents pylon serve --decide` answers decision jobs with a local Clef server and advertises `pylon/decision`; the gateway's Pylon-first path sends calls to it ([the Pylon guide](../compute/pylon.md#answer-decisions-clef)). |

In Rust, `crates/jev` is the client for all three: `jev::Config::new` for a
hosted door, `Config::local` for a server on this computer, and
`jev_hosted::resolve` for the failover chain. It speaks the NIP-DEC body;
it gains a `decisions()` call that takes and returns this page's shapes.

### The translation

Every `POST /v1/decisions` becomes one NIP-DEC request, and its answers come
back in this page's shape. Nothing is lost either way.

| `/v1/decisions` | NIP-DEC (`/v1/systemone`, kind `25910`) |
| --- | --- |
| `model` | `model` |
| `input` as a string | `state` |
| `input` as messages | `state`: the text parts joined in order; `images`: the image parts (HTTP doors only) |
| `questions[i].name` | the question's key in `questions` |
| `predicate` with `instructions`, `true`, `false` | `noul` with `instructions` and `criteria: {"true", "false"}` |
| `choice` with `choices: [{value, description}]` | `choice` with `criteria: {value: description}`; answers are read back in your order |
| `score` with `levels: [{label, description}]` | `score` with `criteria: ["label: description", ...]`, level 0 first |
| `Idempotency-Key`, `X-Attempt` | `request`, `attempt` |
| answer `noul` | `probability` |
| answer `choice`, `probabilities` (object) | `choice`, `probabilities` (array of `{value, probability}`) |
| answer `score`, `probabilities` (object keyed `"0"`…), `legend` | `score`, `probabilities` (array of `{value, label, probability}`) |
| `service.door`, `latency_ms`, the receipt | `openagents.door`, `openagents.latency_ms`, `openagents.receipt` |

## How a call is served

`openagents/decide` asks these doors in order, each only when every door
before it could not answer for its own reasons (busy, unreachable, out of
credit); a refusal of the question itself never moves on:

1. **Clef on our capacity.** Pylons that advertise `pylon/decision` and have
   a free slot, best standing first, over NIP-DEC on our relay; then our
   hosted Clef servers (Psionic on CUDA, the RTX 4080 today, H100s as the
   [throughput plan](https://github.com/OpenAgentsInc/openagents/issues/11446) lands).
2. **Jev** on TypeSafe, under our house key.
3. **Gemini on Vertex**, with a schema that asks one probability per option.

`/v1/systemone` today asks Jev first and the Pylons second, because the
Pylon judge was too slow and unsure for production routing on one RTX 4080
(NIP-DEC, "The OpenAgents decision API"). `/v1/decisions` starts with the
same order, and moves Clef first once the H100 lanes and the router gate
pass. `X-Decision-Order: pylon` asks the Pylons first on either route.
Naming `clef-flash` or `clef` asks only our Clef capacity; it fails rather
than falling through to another model family. Every call is one line in the
decision record with each attempt's door, outcome, and time, and one answer
in twenty is asked again at the next door to measure agreement.

Speed today, per request at about 1.5k input tokens: 0.11 s on an H100 and
0.24 s on the RTX 4080 for `clef-flash`, 0.26 s on an H100 for `clef`
([decision-throughput.md](../inference/decision-throughput.md)).

## Limits

These are the NIP-DEC bounds; a request over one is refused before any model
runs. None of them is the model's own limit
([clef-batching.md](../inference/clef-batching.md), "Are the limits real or
arbitrary?").

| Limit | Value |
| --- | --- |
| Questions | 1 to 64 |
| Choices | 2 to 255 a question |
| Levels | 2 to 10 a question |
| Text input | 128 KiB |
| One question | 64 KiB with its options |
| Body | 8 MiB with images |
| Images | 4 |
| Input tokens | 16,384 by default, the length Clef was trained at |

## Errors

One error object, as every route answers it:
`{"error": {"type", "code", "message", "param", "request_id"}}`. The codes
and statuses are NIP-DEC's (`nostr::decision::http_status`):

| Status | Codes | Retry? |
| --- | --- | --- |
| 400 | `invalid_request`, `too_many_questions`, `too_many_options`, `idempotency_conflict` (409) | No; fix the request. |
| 401 | `unauthenticated` | No. |
| 402 | `payment_required` | After a top-up. |
| 403 | `not_admitted` | No. |
| 413 | `limit_exceeded` | No; send less. |
| 429 | `rate_limited`, `quota_exhausted` | After `Retry-After`. |
| 502 / 503 / 524 / 529 | `door_unavailable`, `busy`, `unavailable`, `timeout`, `overloaded` | Yes, with backoff. |

## Price and payment

You pay for input tokens only: no output, cache-read, or cache-write
charges, and no charge for a call that produced no answer. Prices are in
sats on the [rate card](../inference/gateway.md), drawn from the workspace
balance (top up with Lightning) or paid per call over x402 with Lightning by
a keyless agent. Until the rate card lists decisions, calls are free, as
`/v1/systemone` is today; the free lane without a key is rate-limited.

What it costs us, for scale (hardware hours divided by measured throughput,
batched 13 questions a request): about $0.002 per 1,000 decisions on the RTX
4080 and $0.045 per 1,000 on a spot H100 ([decision-throughput.md](../inference/decision-throughput.md)).

## Privacy

- The gateway never stores request or answer payloads. A call leaves its
  sealed execution receipt (identities, token counts, timing, and SHA-256
  digests of the request and answer, never their content) and one line in
  the decision record (doors, outcomes, milliseconds)
  ([privacy and retention](../decision-models/service/privacy.md)).
- The door that answers sees the payload while it answers. A Pylon is
  someone else's machine; Jev and Vertex are third parties. A request that
  must stay on our machines names `clef-flash` or `clef` with
  `X-Decision-Order: hosted` (proposed; today the header takes only
  `pylon`), which skips Pylons and third-party doors.
- Sealed decisions: our H100 in confidential-computing mode answers over
  NIP-ATT, with the endpoint's attestation verified per call (about 3
  decisions/s today; [decision-throughput.md](../inference/decision-throughput.md),
  `sealed-gpu`). The proposed HTTP form is `X-Decision-Order: sealed`.

## Relation to `/v1/systemone`

`POST /v1/systemone` stays: it is the NIP-DEC body over HTTP, the shape Jev,
TypeSafe, Ollama, llama.cpp, and Psionic all serve, and the one our own
callers (`crates/jev`, the chat router, the solver) send today. `/v1/decisions`
is the same decision in OpenAI's shape, so OpenAI SDKs and code written
for OpenAI's Decisions API can switch base URL and run. Both go through the
same admission, doors, receipts, and record; only the translation at the
edge differs.

## Building it

| Step | Where | Notes |
| --- | --- | --- |
| Route and translation | `crates/gateway`, a `decisions` module beside `decision_dispatch` | Parse this page's request, translate to the NIP-DEC body, call the existing dispatch, translate answers back, add `id`, `object`, `openagents`. Bounds checked once by `nostr::decision::check_body` after translation. |
| OpenAPI and coverage | `GET /v1/openapi.json`, `crates/gateway/tests` | PUBLIC entry; the coverage test fails without it. |
| Conformance against OpenAI's SDKs | a test that runs the Python and JavaScript SDK examples against a local gateway | Same method as the [acceptance run](../inference/2026-10-09-acceptance-run.md) for `/v1/responses`. |
| Rust client | `crates/jev` | `decisions()` with these shapes, over HTTP or NIP-DEC. |
| Images | Psionic CUDA image path; NIP-DEC image jobs | CUDA refuses images today; a job form for images needs a by-reference input. |
| Per-question refusals | `decision_dispatch` | Today a door refuses a whole request; split unsupported questions out so the rest are answered. |
| Clef first | after the H100 lanes and the router gate | [#11446](https://github.com/OpenAgentsInc/openagents/issues/11446): CUDA batching (#11441), prefix reuse (#11442), Hopper kernels (#11443, #11444), FP8 (#11445). |
| Rate card | `crates/inference` rate card, the gateway's metering | Price per million input tokens in sats; the receipt already records tokens. |
