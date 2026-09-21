# Calling the decision API

This page is the caller's guide: how to authenticate, ask questions, read
answers, and survive the failure modes. The deployed service is
[the gateway](gateway.md); the measurement side is
[the Gym](../gym/measured-records.md).

The contract has one route that decides — `POST /v1/systemone` — and one
that lists — `GET /v1/models`. Both examples below work against any door
that speaks the contract, including a local `kev-serve` and TypeSafe's own
`api.typesafe.ai`.

## Credentials

A key looks like `oak_<id>.<secret>` and arrives as a bearer token:

```http
Authorization: Bearer oak_acme.9f…c4
```

An operator issues keys with `tenant-keys`; a caller never mints one. Keep
the key in `OPENAGENTS_API_KEY` or in a config file nothing else can read —
never in a command-line flag, a URL, a log line, or a source file:

```bash
export OPENAGENTS_API_KEY="oak_acme.9f…c4"
# or ~/.config/openagents/oak.json, mode 600:
# {"api_key": "oak_acme.9f…c4", "base_url": "https://gateway.example.com",
#  "model": "acme-kev"}
chmod 600 ~/.config/openagents/oak.json
```

`oak` refuses a config file that group or others can read. Anonymous
callers reach only the deployment's `shared` doors, at the operator's
bound — a tenant's dedicated doors are invisible to them, not merely
unreachable.

## Ask one question

```bash
cat > questions.json <<'EOF'
{
  "refund": {
    "type": "noul",
    "instructions": "Does the customer ask for money back?"
  }
}
EOF

oak ask --questions questions.json \
  "I was charged twice on the March invoice."
```

```json
{"id":"state","outcome":"answered","model":"acme-kev","request_id":"01J…",
 "answers":{"refund":{"type":"noul","noul":0.91,"selected":"yes"}},
 "usage":{"input_tokens":412,"output_tokens":2}}
```

A *state* is the JSON — or plain text — the questions read. A *questions
file* is a JSON object from question id to question, in the wire schema:

| `type` | Question | Criteria | Answer |
|---|---|---|---|
| `noul` | Does a condition hold? | optional `{"true": …, "false": …}` | `noul`: the probability of yes |
| `choice` | Which of these? | object of option → description | `choice`, `confidence`, `probabilities` |
| `score` | Where on this rubric? | array of 2–10 level descriptions | `score`, `confidence`, `probabilities`, `legend` |

```json
{
  "department": {
    "type": "choice",
    "instructions": "Which team should own this ticket?",
    "criteria": {
      "billing": "Charges, invoices, and refunds",
      "technical": "Bugs and outages",
      "none": "No team fits"
    }
  }
}
```

Read the probabilities as calibrated judgments, not as permission: a Noul
near 0.5 means yes and no are about equally likely, not "medium", and a
Choice `confidence` describes how concentrated the distribution is — it
says nothing about whether acting on the answer is allowed. The threshold
that turns a probability into an action belongs to your code and is worth
measuring on your own labelled data.

## Batch

Three input modes, all bounded:

```bash
# One call per line of text — each line is the state.
cat tickets.txt | oak ask --questions questions.json --input lines

# One call per JSON line — {"id", "state", "request_id"} or a bare state.
cat rows.ndjson | oak ask --questions questions.json --input ndjson \
  --concurrency 8 --request-id backfill-2026-10
```

NDJSON output preserves input order and keeps every outcome visible —
an invalid input line reports `invalid` rather than vanishing:

```json
{"id":"a","outcome":"answered","answers":{"refund":{"noul":0.91}}}
{"id":"b","outcome":"refused","error":{"code":"guardrail","message":"…"}}
{"id":"line-3","outcome":"invalid","error":{"code":"invalid_row","message":"not JSON"}}
```

`--select id` trims each row's `answers` to the ids you name.
`--uncertain-below 0.7` tags a row `"uncertain": true` when the winning
probability of any answer falls under the bound. Progress and the closing
count go to standard error; `--quiet` leaves only the JSON.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Every row answered |
| 1 | The run failed — bad credential, unbound door, unreadable config |
| 2 | Usage error — bad flag, missing questions file |
| 3 | Refusals only — the door declined every row it saw |
| 4 | Unavailable only — timeouts, capacity, transport |
| 5 | Mixed — some answered, some did not |

## Retries and idempotency

`oak` retries timeouts, transport failures, `429`, and `5xx` up to
`--retries` times (default 3), honoring `Retry-After` up to 60 seconds.
`rate_limited`, `busy`, and `overloaded` are temporary capacity — retry.
`quota_exhausted` is the daily budget — it is reported `refused` and not
retried, because retrying a spent allowance cannot help.

With `--request-id KEY` the call carries `Idempotency-Key: KEY` and
`X-Attempt: n`; the service settles quota by that pair, so a retry of the
same request shares one reservation and a settled pair is never charged
twice. In batch mode each row derives `KEY/<id>`; a row may carry its own
`request_id` to override. Reusing a settled key against different content
answers `409 idempotency_conflict` — change the key, not the content.

## What a call costs and leaves behind

`usage` carries the token counts the door reported; either field may be
absent when a backend does not count it. Monetary cost is a separate
ledger the service keeps — the caller-visible record of a call is its
receipt. Every settled attempt appends a sealed `ExecutionReceipt` to the
service's `receipts.jsonl` binding the request and attempt ids, the
requested and served model identities, the registry revision that
admitted the call, the outcome, usage, and digests of the request and
result — never their content. Quote `request_id` when you report a call.

## The measurement loop

The same contract is what `gym` scores. The caller-side flow:

1. `gym build` freezes a suite of labelled items into a digest.
2. `gym eval --suite … --partition …` runs a door against it into the
   receipt-chained store.
3. `gym report --suite … --expect …` renders coverage — expected versus
   recorded — and refuses to call a partial run complete.
4. `gym report --commitment path` emits a versioned commitment binding the
   chain head, row count, and selection; `gym verify --commitment path`
   checks a store against it anywhere it travels.

`docs/gym/measured-records.md` is the reference. A caller's own labelled
items build the same way — `docs/gym/` covers the suite schema.

## Install and support

`oak` ships in this repository and builds with the workspace toolchain:

```bash
cargo build -p oak --release   # binary at target/release/oak
oak version                    # reports the crate version
```

Versioning follows the crate version in `crates/oak/Cargo.toml`; packaged
installers, checksums, and foreign-language SDKs are tracked separately in
the roadmap and do not exist yet.

| Surface | Status |
|---|---|
| `oak ask` — state, lines, NDJSON, `--select`, `--uncertain-below`, `--request-id`, bounded concurrency and retries | Supported |
| `oak models` | Supported |
| `oak version` | Supported |
| Labels, dimensions, multi-label output | Proposed — lands with the classification route |
| Capacity and review selection | Proposed — lands with the review route |
| Packaged SDKs (Python, Go) | Proposed — tracked under the packaging work |

## Limits and honesty

- The service bounds body bytes, questions per call, total options,
  per-door concurrency and rate, and per-tenant daily quota. Oversize
  envelopes are refused at `422` before a door is consulted.
- The served `model` and its `artifact_identity` are a serving claim
  verified against the registry binding, not a remote attestation.
- Receipts record digests, not content — request bodies are not retained.
- `oak` is the first client; packaged SDKs are tracked separately. Labels,
  dimensions, multi-label output, and capacity/review selection arrive
  with the classification and review routes — they are specified in
  `decision-api.md` and do not exist yet.
