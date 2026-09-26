---
name: decision-api
description: >
  Call the OpenAgents decision API: authenticate with an `oak_` bearer key,
  post `state` + typed `questions` to `POST /v1/systemone`, read Noul,
  Choice, and Score answers with their probabilities, and handle typed
  refusals, `Retry-After`, and idempotent retries. Use it when an agent or
  caller needs a programmatic decision — routing, tagging, ranking,
  verification — from a deployed door, or when writing client code against
  `crates/jev` or the `oak` CLI.
---

# The decision API for callers

The service answers questions instead of generating text. One request
carries one **state** (the JSON or text under judgment) and a map of typed
**questions**; the response carries one typed **answer** per question with
probabilities. There is no streaming prose and no chat history — each call
is complete and independent.

## The contract

```http
POST /v1/systemone
Authorization: Bearer oak_<id>.<secret>
Content-Type: application/json

{
  "model": "shared-kev",
  "state": "I was charged twice on the March invoice.",
  "questions": {
    "refund": {"type": "noul", "instructions": "Does the customer ask for money back?"}
  }
}
```

```json
{"model": "shared-kev", "answers": {"refund": {"type": "noul", "noul": 0.91}},
 "usage": {"input_tokens": 412, "output_tokens": 2}}
```

`GET /v1/models` lists the doors the credential may name in `model`. Three
question types:

- `noul` — probability a condition holds (`noul` field, 0–1).
- `choice` — pick one of `criteria`'s named options (`choice`,
  `confidence`, `probabilities`).
- `score` — probability-weighted position on 2–10 ordered `criteria`
  levels (`score`, `confidence`, `probabilities`, `legend`).

Question ids are caller-side only — the model sees instructions and
criteria, never the id. Include a no-match option in a `choice` when
nothing may fit.

## Refusals, retries, and identity

Errors are typed: `{"error": {"code", "message"}}`. Gateway-owned codes
are stable (`unauthenticated`, `door_not_bound`, `invalid_request`,
`too_many_questions`, `too_many_options`, `idempotency_conflict`,
`rate_limited`, `busy`, `overloaded`, `quota_exhausted`,
`door_unavailable`, `identity_mismatch`, `unavailable`); a backend may add
its own refusal codes. Treat `rate_limited`/`busy`/`overloaded` and `5xx`
as retryable — honor `Retry-After` — and `quota_exhausted` as a daily
budget that retrying cannot fix.

Send `Idempotency-Key` on work you may retry and increment `X-Attempt` per
attempt: the service settles quota by `(request, attempt)`, a retried
request shares one reservation, and a settled pair reused against changed
content answers `409`. The `x-request-id` response header is the identity
to quote when reporting a call.

## The CLI and the SDK

`oak` (`cargo run -p oak`) is the reference caller: `oak ask --questions
FILE [STATE]` for one call, `--input lines|ndjson` for bounded batches,
`--request-id` for idempotent work, `--select`/`--uncertain-below` to shape
output, and stable exit codes (0 answered, 1 failure, 2 usage, 3 refused,
4 unavailable, 5 mixed). Credentials come from `OPENAGENTS_API_KEY` or a
`0600` config file — never a flag. In Rust, `crates/jev` is the SDK the
CLI is built on.

## What an answer does not mean

Probabilities are model judgments, not permission or proof of correctness.
Calibration must be measured on the target workload; the wire contract does
not establish it. A Noul of 0.5 expresses equal model probability for yes and no,
not an independently established frequency.
`confidence` describes how concentrated a distribution is. Acting on an
answer — a threshold, a routing rule, an escalation — belongs to the
caller's policy and should be measured on the caller's labelled data.

## Where to read more

- `docs/decision-models/guides/caller.md` — the caller's guide end to end.
- `docs/decision-models/service/gateway.md` — the service's admission, bounds, and
  refusal codes.
- `docs/decision-models/api/openapi.yaml` — the machine-readable contract.
- `docs/decision-models/examples/` — runnable curl and `oak` examples.
- `docs/gym/measured-records.md` — how suites, runs, and report
  commitments measure a door.
