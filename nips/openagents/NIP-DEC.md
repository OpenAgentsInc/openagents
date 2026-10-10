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

TypeSafe's own API also takes a JSON array as `state`; a NIP-DEC host does
not. A client that holds an array or a scalar sends its compact JSON text
(`jev::State::from` does), which every host admits and every door reads.

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

`nostr::decision::canonical_model` resolves an alias, and
`jev::nip_dec::openrouter_model` gives OpenRouter's name for a canonical
one (`jev-1.13.0` → `typesafe/jev-1.13`; another bare name `n` →
`typesafe/n`). The hosted decision worker's deployed release
(`5710e1311c`) admits `jev-1.13.0`, `jev-latest`, and the alias
`typesafe/jev-1.13`. A model a worker does not admit is refused
`not_admitted`.

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
unchanged. The OpenAgents gateway's `POST /v1/systemone` (`crates/gateway`,
`serve`) checks every bound above before a door is consulted
(`nostr::decision::check_body`), asks a model alias's door by its canonical
name, and answers its own refusals with the status table below; a refusal
code only that gateway defines (such as `out_of_scope`) keeps its own
status. OpenRouter's Decisions API puts the HTTP status number in
`error.code`; a client or worker reads a numeric or absent code through
`code_for_http_status`.

### The OpenAgents decision API

Since 2026-10-10 ([#11225](https://github.com/OpenAgentsInc/openagents/issues/11225))
every OpenAgents caller sends its decisions to our own API,
`POST https://openagents.com/api/v1/systemone`, with no key and no
dependency on TypeSafe. The gateway (`crates/gateway`, `decision_dispatch`)
answers any `jev-…` or `typesafe/…` model name, `openagents/decide`, and
`clef-flash`, and asks these doors in order, each only when every door
before it could not answer:

1. **Connected Pylons.** The gateway reads [NIP-PYLON](NIP-PYLON.md)
   beacons on its relay and keeps the fresh, online ones from its trusted
   pylon keys that advertise a decision service (`<pylon key>:pylon/decision`
   on the `cj-decision` lane) with a free slot, best standing first (fewest
   failures, then the fastest). It sends the decision as a job of this NIP
   (`25910`, signed by the gateway's dispatch key), waits up to 8 s, checks
   the answer's shape (every question answered with its own type,
   probabilities finite and summing to one over exactly the options asked)
   and that the served model and identity are the beacon's, and otherwise
   benches that pylon for a minute and tries the next. At most two pylons
   are tried.
2. **Our hosted Clef** (`clef_url`), a Psionic `/v1/systemone` over HTTP.
3. **Gemini on Vertex AI** (`gemini-3.8-flash`, the prepaid Google credit):
   the same state and questions with a response schema that asks one
   probability per option (`p_yes` for a noul), clamped and normalized
   into the answer shape above.
4. **Jev**, optional and last, off unless the operator turns it on.

Each answer names its door in `service.door` (`pylon:<slug>`, `clef`,
`vertex`, `jev`), the pylon's address, key, and identity, the served
`model`, and `latency_ms`, and the response carries `X-Decision-Door`.
Every decision is one line in `<registry>/decisions/YYYY-MM-DD.jsonl` with
each attempt's door, outcome, code, and milliseconds. One answer in twenty
is asked again at a second door after it is sent, and the agreement (the
questions whose pick agrees, the largest probability gap) goes to
`decisions/shadow-YYYY-MM-DD.jsonl`.

Clients find it through `jev_hosted::resolve` (below): TypeSafe's door
resolves to our API unless the person or operator sets
`OPENAGENTS_DECISIONS=jev`. `OPENAGENTS_DECISIONS_URL` names another base
URL (staging, a local gateway).

### Doors and the backup door

The body is the same at every door; only the model's name differs:

| Door | Route | Body |
| --- | --- | --- |
| TypeSafe | `POST https://api.typesafe.ai/v1/systemone` | `jev::DecisionRequest::to_value` |
| An OpenAgents gateway | `POST /v1/systemone` | the same |
| Vercel AI Gateway | `POST https://ai-gateway.vercel.sh/typesafe/v1/systemone` | the same, the model as `typesafe-ai/jev` |
| OpenRouter | `POST https://openrouter.ai/api/alpha/decisions` | `jev::DecisionRequest::openrouter_body` (the model as `typesafe/jev-1.13`) |
| A decision job | kind `25910` | `jev_hosted::wire_body` |

The Vercel AI Gateway's route is its TypeSafe-compatible API, which takes
and answers TypeSafe's shapes under an AI Gateway key; it serves one Jev,
`typesafe-ai/jev` (its current one, unversioned), and adds
`provider_metadata.gateway` with the routing and the cost as a decimal
string, which a reader takes as `usage.cost`. Its own errors are
`{"message", "error_type"}`, read as `{"error": {"code": error_type,
"message"}}`. OpenRouter's answer adds `id`, `provider`, and `usage.cost`,
and names the dated model it served (`typesafe/jev-1.13-20260917`); a
reader ignores fields it does not use.

A server that holds door keys may keep backup doors. The chat worker's
judge (`jev::doors::Failover`, through
`jev_hosted::resolve_with_fallbacks`) asks a decision in the order Vercel
AI Gateway → OpenRouter → TypeSafe (`Failover::primary_last`): the gateway
is the primary and routes Jev to TypeSafe itself, with the owner's key as
its own fallback, and TypeSafe direct is the final backup; every other
route still goes to TypeSafe. The decision worker's open lane (the jobs
it forwards under its server-held TypeSafe key; `decision-worker.json`
`open.upstream_last`) asks in the same order: its `backups`
(`jev::doors::FALLBACKS`) first and its TypeSafe upstream last. A
provisioned principal's jobs, under their own key, ask the upstream first
and the backups after it. A door is asked only when
every door before it could not answer for its own reasons
(`jev::doors::fails_over`): it timed out or could not be reached, answered
`402`, `408`, `429`, or any `5xx`, or refused with one of its own codes
(its key, its account, its model list, its quota: `unauthenticated`,
`payment_required`, `not_admitted`, `rate_limited`, `quota_exhausted`,
`internal`, and the `502`–`529` rows). A refusal of the question itself
(`invalid_request` and the other `400` and `413` rows) never fails over,
and a door after the first that refuses the question ends the chain. Each
backup is off unless its key (`AI_GATEWAY_API_KEY`, `OPENROUTER_API_KEY`)
is in the server's environment, and the server says which at start. An
answer from any door but TypeSafe names it in `service.door`, so the
decision record (`openagents.decision-call.v1`) says which door answered;
when no door answers, the first door's refusal stands. The chat judge and
the decision worker remember a door that refused for its key or account
(`401`, `402`; `jev::doors::benches`) and skip it for `jev::doors::BENCH`
(five minutes), then ask it again; a skipped door counts as having
refused the same way again. The decision worker never benches the
upstream for a provisioned principal, whose key is the caller's.

A computer's own TypeSafe key (`TYPESAFE_API_KEY`, else `api_key` in
`~/.openagents/jev.json`) is asked first, as the person's own, and is not
the only door (`jev_hosted::resolve`): when it cannot answer for its own
reasons, a decision goes on to the Vercel AI Gateway and OpenRouter when
their keys are set on that computer, then to the hosted decision service
as a decision job, keyless, the same door a computer with no key uses
(`jev::doors::Door::carried`; off under `OPENAGENTS_JEV_HOSTED=off`). The
hosted service is itself gateway-first on its open lane, so a key out of
credits still gets answers. The refusing door is benched as above. An
answer the hosted service carried keeps the `service` object it added
(`door`, `version`) and gains `service.exchange`, the service that carried
it, and its decision record says `via` `hosted`. A server's backup doors
(`jev_hosted::resolve_with_fallbacks`) are the keys it holds and never
include the hosted service.

A client that turns a relay refusal back into an HTTP-shaped
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
(`crates/atif`, `Decision`): `door`, `model` (the served one),
`state_digest` and `questions_digest` (SHA-256 of the canonical `state` and
`questions`, structured entries included), `question_ids`, the selected
`answers`, the consuming `route`, any `error`, every dispatch `attempt`,
any `review`, and what served it: `via` (`direct` with this computer's key,
`hosted` through the hosted decision service, `local` for a loopback or
private door), `service` (the relaying service's `{door, version}`),
`request_id`, `usage` (`input_tokens`, `output_tokens`, and `cost` when the
door priced it), `cost_usd` (the door's cost, else input tokens at Jev's
list price), and `latency_ms`. The call's arguments hold the request body,
so the questions and answers read beside each other; a body over 32 KiB
(`atif::REQUEST_BOUND`) is recorded bounded instead: its model, its
questions when they fit (else their digest and size), and its state as a
digest, a size, and a 2 KiB excerpt, with `extra.request_bounded`. The
digests always cover the whole body. `jev_hosted::decision_record` builds
the record the same way for every caller. An answer from a backup door
carries `service.door` naming it; the chat thread's router record, whose
judgment the chat worker relays, names the Jev door that answered as
`service.upstream` (the judgment's `door`) and the served model. The final metrics count
decision calls by name.
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
- **Coder.** Every caller that asks TypeSafe's door finds its door through
  `jev_hosted::resolve`, which since #11225 answers with our decision API
  (above), keyless. Only under `OPENAGENTS_DECISIONS=jev` does it take the
  older order: a local TypeSafe key calls TypeSafe over HTTP; otherwise the
  call goes to the hosted decision worker as a decision job; otherwise there
  is no Jev, and the caller says why. With no decision profile configured,
  the chat worker's and the terminal's router ask our API too
  (`coder::decision::from_env`; `OPENAGENTS_DECISIONS=off` turns it off). The callers: the
  Microcoder repository adapter and bench, the delegate door's Jev and
  judge, Coder One's episode, checks, and tools, the hands seam, a
  program's `decide` step, the chat router and CLI route (through the
  decision profile: its keyed doors resolve the same way, its `relay`
  profile is the hosted door, and a terminal with no profile and no key
  runs ordinary chat without a classifier, as it always has), the Gym,
  external-eval graders, and Voyager's live door. A door on this machine
  (`kev-serve`, a local Lev) is a plain client with no key. Every request is built with the shared
  model in the `jev` SDK: `jev::State`, `jev::Questions` of `Noul`,
  `Choice`, and `Score` with `Entry` fields and `NoulCriteria`, and
  `jev::DecisionRequest`; a question file's JSON reads into the same types
  (`Question::from_value`, which keeps anything it does not model as it
  stands). Answers read back as `jev::Answer` and serialize in the shape
  above; an HTTP error reads as a `jev::Refusal` with its NIP-DEC code.

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
`crates/jev` (`nip_dec`) round-trips every documented shape through the
shared model, and `crates/jev-hosted` checks that every example becomes the
same decision job and that the SDK's alias and status tables are the
wire's. `crates/gateway` (`tests/serve.rs`,
`the_decisions_api_answers_the_typesafe_docs_examples`) sends TypeSafe's
documentation examples (the invoice `field`, the `billing`/`orders`/
`account` rubric, the taxonomy walk, the PR-scope levels, and the
credentials noul) to the gateway's HTTP API under `typesafe/jev-1.13` and
checks each refusal's status;
`the_backup_door_answers_only_when_the_upstream_cannot` and
`typesafe_then_the_gateway_then_openrouter` (`tests/relay_worker.rs`)
check the backup doors and their order, and `crates/jev` (`tests/doors.rs`)
checks the same order, the shared answer shape, and that a refusal of the
question never fails over, with stand-in doors.
