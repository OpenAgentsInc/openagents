# NIP-DEC — Decisions

`draft` `optional` — v1, 2026-09-30. **Implemented** in `crates/nostr`
(`decision`), served by the hosted `decision-worker` (`crates/gateway`).
The [shared contracts](contracts.md) are normative.

A decision is one state and a set of typed questions about it, answered with
probabilities. This NIP defines the request, the answer, and the job that
carries one decision over Nostr. The same body is what an HTTP gateway takes
at `POST /v1/systemone`, what TypeSafe's Jev API takes, and what OpenRouter's
Decisions API takes (`alpha.decisions.create` with model
`typesafe/jev-1.13`). A decision job and an HTTP call with the same body ask
the same thing.

Decisions were a section of [NIP-CJ](NIP-CJ.md) ("Typed decision jobs"),
which now points here. The kinds and the wire version did not change, so
every request valid under that section is valid here.

A decision records a judgment. It does not certify calibration, and a
probability is never permission: whoever consumes an answer pins its own
threshold, abstention, and interpretation policy.

## Kinds and version

| Kind | Name | Direction |
| --- | --- | --- |
| `25910` | Decision request or cancel | Caller → worker |
| `26910` | Decision result | Worker → caller |
| `27010` | Decision status | Worker → caller |

All three are ephemeral. Transport, encryption, tagging, signer and
recipient checks, freshness, and deduplication are NIP-CJ's
["Families and transport"](NIP-CJ.md#families-and-transport): one `p` naming
the worker on a request; one `p` naming the caller and one `e` naming the
exact request on each answer; NIP-44 v2 content. A handler refuses another
family's kind or payload rather than reinterpret it.

Every payload leads with `v: "openagents.systemone.v1"`. This NIP keeps
that version rather than adding a `v2`, for three reasons:

- The v1 wire already carried these bodies. The v1 validator in
  `crates/nostr` bounded `state` and each question by size and never read
  the shape of `state`, `instructions`, or criteria values, and every
  deployed worker forwards them to the door verbatim. A structured request
  sent before this NIP reached TypeSafe unchanged and was answered.
- A `v2` would split callers from workers for no difference in meaning:
  a v1 worker would refuse a `v2` request that it could have answered.
- Every string-only request stays valid. What this NIP adds is the
  documented shape and its bounds. The bounds refuse only bodies that
  NIP-CJ's section never allowed: bare numbers where text belongs, unknown
  question types, and nesting past the depth limits.

NIP-CAP's decision-service `interface` stays `openagents.systemone.v1` for
the same reason.

## EntryType

An **EntryType** is a string, a JSON object, a JSON array, or `null`.
Structure is allowed anywhere a question describes something: its
`instructions`, a `choice` option's description, a `score` level, and a
`noul` question's `true` and `false`. A bare number or boolean is not an
entry. Values inside an object or array are any JSON, numbers and booleans
included.

`null` leaves the thing undescribed: a `choice` option with a `null`
description is a bare option. An object or array is read as structured
guidance, not as a schema. Field names such as `what`, `not_for`,
`examples`, `summary`, `signals`, `question`, `compare`, and `focus` are
conventions that make a rubric easier to read. The model reads them; the
protocol does not interpret them.

## Request

A request has `v`, `requires`, `type: "systemone"`, and:

| Field | Meaning |
| --- | --- |
| `request` | Caller-chosen logical ID, stable across retries (the HTTP lane's `Idempotency-Key`). |
| `attempt` | Positive integer (the HTTP lane's `X-Attempt`). |
| `model` | Nonempty host-admitted model name, such as `jev-1.13.0`. See [Models](#models). |
| `state` | A string, or any JSON object. Every question reads the same state. |
| `questions` | A nonempty object of question ID to question. IDs are for the caller and are not sent to the model as meaning. |
| `deadline` | Unix seconds, the latest time an answer is useful. It matches the event's NIP-40 `expiration`. |

Each question has `type` (`noul`, `choice`, or `score`) and `instructions`,
an EntryType saying what to judge. An absent `instructions` is `null`. The
`criteria` depend on the type:

| Type | `criteria` | Answer |
| --- | --- | --- |
| `noul` | Optional: `{"true": EntryType, "false": EntryType}`, either key optional. Absent or `null` leaves both outcomes undescribed. | `{type: "noul", noul}`, the probability of yes. |
| `choice` | Required: a nonempty object of option ID to EntryType description. | `{type: "choice", choice, confidence, probabilities}`. |
| `score` | Required: an ordered array of 2 to 10 EntryType level descriptions, level 0 first. | `{type: "score", score, confidence, legend, probabilities}`. |

A `noul` question whose `criteria` is a string is the undescribed form some
callers sent before this NIP. It stays valid and passes through unchanged.
Fields a question carries beyond `type`, `instructions`, and `criteria` pass
through to the door unchanged; a door may refuse them.

One question does not read another's answer in the same request. Dependent
questions (walking a taxonomy one level at a time, for example) are separate
requests.

### Bounds

A worker refuses a request that exceeds any of these with `invalid_request`,
except where a row names another code. `crates/nostr` exports each as a
constant.

| Bound | Value |
| --- | --- |
| Decrypted payload | 256 KiB (`limit_exceeded` from a worker that meters ciphertext). |
| `request` ID | 128 bytes. |
| `model` | 1 to 128 bytes. |
| `state` | 128 KiB serialized; an object nests at most 32 levels. |
| Questions | 1 to 64 (`too_many_questions` above 64). |
| Question ID | 1 to 64 bytes. |
| One question | 32 KiB serialized, its entries included. |
| Entry nesting | 8 levels: a string or `null` is depth 0, and each object or array adds one. |
| `choice` options | 1 to 255 (`too_many_options` above 255); option IDs 1 to 128 bytes. |
| `score` levels | 2 to 10 (`too_many_options` above 10). |

The per-question byte bound is the size bound on every structured entry in
it. The depth bound stops a small body from costing a deep descent.

## Answers

Answers are unchanged from NIP-CJ's decision family. All probabilities and
confidences are finite in `[0, 1]`, and categorical probabilities sum to one
within `0.000001`.

- **noul**: `noul` is the probability that the answer is yes.
- **choice**: `probabilities` names exactly the requested options.
  `choice` is a maximum-probability option, ties broken by lexicographic
  option ID, and `confidence` is its probability.
- **score**: `probabilities` maps level indexes as decimal strings (`"0"`,
  `"1"`, …) to probabilities; `legend` maps the same indexes to the
  requested level descriptions, structured ones included; `score` is the
  probability-weighted index, within `0.000001`; `confidence` is the largest
  level probability.

A door may add `selected` (its own pick) to a noul or score answer.

## Result, status, and cancel

A `26910` result carries `v`, `requires`, `type: "result"`, `request`,
`attempt`, `outcome`, `dispatched`, `response`, `receipt`, and `code`.
`receipt` is an ArtifactRef to the shared execution receipt, bound to the
request's digest. On `completed`, `response` holds `model` (the served
identity), `answers` keyed exactly as the questions were, `usage`
(ArtifactRef or null), and optionally `service` (`{door, version}`) and
`latency_ms`, and `code` is null. On any other outcome `response` is null
and `code` is a bounded cause. A refusal, a transport failure, and a model
answer are distinct outcomes.

A `27010` status is `type: "status"` with `request`, `attempt`, and
`status` (`queued`, `processing`, or `error`); an error adds `code`,
`message`, and optional `retry_after_ms`. An error is not evidence that an
admitted call cost nothing.

Idempotency, retries, and cancellation are NIP-CJ's: the key is `(worker,
principal, request, attempt)`; the fingerprint covers the JCS of the whole
request body, structured entries included, so changing any entry under the
same key is `idempotency_conflict`; a redelivery of a settled pair returns
the recorded result without a second charge or model call. A `type:
"cancel"` from the original signer stops a job before dispatch and asks a
dispatched one to stop; its receipt is not proof of stop.

## Models

The worker, not the caller, admits a model; naming a model authorizes
nothing. A host that admits a canonical name also admits its documented
aliases and sends the door the canonical name:

| Name | Meaning |
| --- | --- |
| `jev-1.13.0` | Jev 1.13.0, TypeSafe's System One model. |
| `jev-latest` | TypeSafe's current Jev. A receipt's served `model` names the version that answered. |
| `typesafe/jev-1.13` | OpenRouter's name for Jev 1.13; an alias of `jev-1.13.0`. |

`nostr::decision::canonical_model` resolves an alias. The hosted decision
worker's deployed release (`04113fec9d`) admits `jev-1.13.0` and
`jev-latest`; a worker built from this NIP's commit on also admits
`typesafe/jev-1.13`, with no config change. A model a worker does not admit
is refused `not_admitted`.

## HTTP-gateway equivalence

A decision job and an HTTP call are the same decision:

| Relay job | HTTP |
| --- | --- |
| `model`, `state`, `questions` | The JSON body of `POST /v1/systemone`, byte-for-byte after JCS. |
| `request` | `Idempotency-Key` header. |
| `attempt` | `X-Attempt` header. |
| `deadline` | The client's timeout. |
| `response` on `completed` | The `200` body. |
| A refusal `code` | `{"error": {"code", "message"}}` with the status below. |
| `retry_after_ms` | `Retry-After` in seconds, rounded up. |
| Receipt `usage` | `X-Receipt` header. |

A gateway that relays a job to an HTTP door sends `state` and `questions`
unchanged. A client that turns a relay refusal back into an HTTP-shaped
error (`jev_hosted` does, so every Jev caller sees one error shape) uses the
same table.

### Refusals and HTTP status

Gateways answer refusals with the statuses OpenRouter's Decisions API uses
(`nostr::decision::http_status`):

| Status | Codes | Meaning |
| --- | --- | --- |
| 400 | `malformed`, `invalid_request`, `too_many_questions`, `too_many_options`, `unsupported_version`, `stale`, `idempotency_conflict`, `uncalibrated` | The request is wrong; do not retry it unchanged. |
| 401 | `unauthenticated` | No valid credential or signer. |
| 402 | `payment_required` | The account cannot pay for the call. |
| 403 | `not_admitted` | The caller or model is not admitted here. |
| 404 | `door_not_bound`, `not_found` | No such model or route for this caller. |
| 413 | `limit_exceeded` | The request is too large. |
| 429 | `rate_limited`, `quota_exhausted` | Over a rate or a quota; `Retry-After` says when. |
| 500 | `internal` | The gateway failed. |
| 502 | `door_unavailable`, `identity_mismatch`, and any code not listed | The model server failed or answered as the wrong identity. |
| 503 | `busy`, `unavailable`, `registry_unavailable`, `membership_unavailable`, `ledger_unavailable` | No capacity or a dependency is down; retry later. |
| 524 | `timeout` | The model server did not answer in time. |
| 529 | `overloaded` | The model server is overloaded. |

A door that answers with one of these statuses and no typed error is read
back as the first code of its row, and any other status as `unavailable`
(`nostr::decision::code_for_http_status`). A daily open-lane quota is
`quota_exhausted` at `429`, not `402`: nothing the caller pays changes it
before the day turns. Relay-only causes (`worker_absent`, `cancelled`) have
no HTTP status.

## Examples

Each example is a `{state, questions}` pair in
[`crates/nostr/fixtures/decisions/valid/`](../../crates/nostr/fixtures/decisions/valid/),
and `decision::tests::every_structured_example_round_trips_the_wire` seals
each one into a `25910` event and checks that the worker admits exactly the
state and questions sent. Requests the bounds refuse are in
[`invalid/`](../../crates/nostr/fixtures/decisions/invalid/).

**Strings only** (`string-only.json`). The form NIP-CJ documented, valid
unchanged.

**A shared `field` in instructions** (`invoice-field.json`). Extracting
from an invoice asks several questions about the same field; each question's
`instructions` carries the same `field` object beside its own `question`:

```json
{
  "type": "choice",
  "instructions": {
    "field": {"name": "total_due", "meaning": "The amount the buyer owes on this invoice, after tax.", "where": "Usually the last money line."},
    "question": "Which currency is the field in?"
  },
  "criteria": {"EUR": "Euro", "USD": "US dollar", "GBP": "Pound sterling", "other": "Any other currency, or none is shown."}
}
```

**A rubric on each option** (`rubric-choice.json`). Options describe what
they are for, what they are not for, and examples:

```json
"criteria": {
  "billing": {
    "what": "Charges, invoices, refunds, and payment methods.",
    "not_for": "Questions about plan features or seat limits.",
    "examples": ["I was billed twice", "Update my card"]
  },
  "technical": {
    "what": "Something in the product is broken or behaves unexpectedly.",
    "not_for": "Money questions, even when a bug caused them.",
    "examples": ["The export button does nothing", "Login loops"]
  }
}
```

**Walking a taxonomy** (`taxonomy-walk.json`). Option values nest objects
and arrays to show what lies below each branch; the next level is a new
request whose options are the chosen branch's children:

```json
"criteria": {
  "electronics": {
    "what": "Devices and the parts that power or connect them.",
    "children": {"accessories": ["cables", "chargers", "cases"], "computers": ["laptops", "desktops"], "phones": []}
  },
  "apparel": {"what": "Clothing, shoes, and worn accessories.", "children": ["tops", "bottoms", "footwear"]}
}
```

**Object score levels** (`score-levels.json`). Each level has a `summary`
and the `signals` that point to it; the answer's `legend` returns them:

```json
"criteria": [
  {"summary": "Cosmetic", "signals": ["typo", "misaligned layout", "no user blocked"]},
  {"summary": "Degraded", "signals": ["slow responses", "a workaround exists"]},
  {"summary": "Major", "signals": ["one feature fails for many users", "no workaround"]},
  {"summary": "Critical", "signals": ["revenue path down", "a whole region affected", "data at risk"]}
]
```

**A structured noul** (`structured-noul.json`). `true` and `false` each say
what they mean, with examples:

```json
{
  "type": "noul",
  "instructions": "Does the customer ask for money back?",
  "criteria": {
    "true": {"what": "The message asks for a refund, credit, or reversal of a charge.", "examples": ["Please refund the duplicate charge", "Credit my account"]},
    "false": {"what": "The message asks for anything else, including an explanation of a charge.", "examples": ["Why was I charged $49?", "Send me the invoice PDF"]}
  }
}
```

**Instructions with `question`, `compare`, and `focus`**
(`compare-focus.json`). The state holds two candidates; the instructions
name which to compare and what matters:

```json
"instructions": {
  "question": "Which patch better does the task?",
  "compare": ["a", "b"],
  "focus": ["correctness for non-retryable errors", "test coverage"]
}
```

## Recording decisions in ATIF

A decision a program or agent makes is recorded in its ATIF trajectory as a
tool call whose `extra.schema` is `openagents.decision-call.v1`
(`crates/atif`, `Decision`): `door`, `model`, `state_digest` and
`questions_digest` (SHA-256 of the canonical `state` and `questions`,
structured entries included), `question_ids`, the selected `answers`, the
consuming `route`, any `error`, every dispatch `attempt`, and any `review`.
The call's arguments hold the request body, so the questions and answers
read beside each other. The final metrics count decision calls by name.
[NIP-ATIF](NIP-ATIF.md) carries the trajectory. A recorded decision call
records a judgment, not proof that it was right.

## How programs, the router, and Coder use decisions

- **Programs.** A [NIP-PRG](NIP-PRG.md) `decide` step puts a pinned question
  set to a decision door and records the decision call with the set's
  identity and digest beside the answer. Its questions may use any entry
  form here.
- **The chat router.** The chat worker asks its routing question set
  (such as `chat-router-v1`) of a decision door for each routed turn and
  reports the result as NIP-CJ `judgment` feedback (`route`, `route_p`,
  `lane`, `risk`, `tier`, …). The thread's ATIF records the judgment as a
  decision call.
- **Coder.** Every Jev caller (the Microcoder repository adapter, the
  delegate door's Jev and judge, and the hands seam) finds Jev through
  `jev_hosted::resolve`: a local TypeSafe key calls TypeSafe over HTTP;
  otherwise the call goes to the hosted decision worker as a decision job.
  The `jev` SDK's `Entry`, `NoulCriteria`, `Choice`, and `Score` build every
  entry form here, and both paths send the same body.

## Conformance

A conforming worker validates every bound above before dispatch and
forwards `state` and `questions` to its door unchanged. Tests in
`crates/nostr` (`decision`) cover a wire round trip for each example, the
refusal of each invalid example, entry and state depth at and past the
limit, oversize entries and state, the noul criteria forms, model aliases,
and the status table. `crates/gateway` (`tests/relay_worker.rs`,
`structured_entries_pass_through_and_model_aliases_admit`) checks that a
structured request reaches the door byte-for-byte and that
`typesafe/jev-1.13` goes to the door as `jev-1.13.0`.
`crates/jev-hosted` (`tests/live.rs`,
`live_hosted_structured_decision_answers`) asks the deployed worker a
structured `choice` and `noul`; it is opt-in and needs no key.
