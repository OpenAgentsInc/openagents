# Review and fallback policies for classification

A classify call's policy document may declare a `review` block with schema
`openagents.classify-review.v1`. The block is explicit opt-in: absent, the
call runs strict model-pinned operation — the bound door's answers are the
call's answers and no second backend ever sees the inputs. Declared, the
call runs a bounded secondary phase after the primary fan-out completes:
declared-cause fallbacks first, then the re-judgments the declared trigger
names.

This is a partial implementation of #9485. It makes the review workflow
auditable end to end. It is not a quality guarantee: the bounds cap work
and spend, the records preserve what each model said, and nothing here
claims a reviewer's answer is better than the primary's. A caller that
wants to know whether review helps must measure that on its own data.

## The policy block

The block lives under `policy.review` in the classify request. Every
field is declared; nothing activates from a missing field.

```json
"policy": {
  "v": "openagents.classify-policy.v1",
  "name": "example",
  "select": { "single_label": { "ties": "first-declared",
              "uncertain_below": 0.9, "no_match": {"kind": "null"} } },
  "review": {
    "v": "openagents.classify-review.v1",
    "reviewer": "acme-kev-review",
    "trigger": "uncertain",
    "on_failure": "keep-original",
    "max_items": 8,
    "max_attempts": 8,
    "latency_ms": 5000,
    "max_spend": 200000,
    "fallback": [
      { "on": "transport", "model": "acme-kev-fb" },
      { "on": "refused", "model": "acme-kev-fb",
        "codes": ["quota_exceeded"] }
    ]
  }
}
```

- `v` is the schema tag, `openagents.classify-review.v1`. Any other value
  refuses the call as `invalid_request`.
- `reviewer` is a door name — a bound door, admitted under the caller's
  own credentials exactly as `model` is. The review forward asks the
  door's bound artifact, and an answer that names another model is a
  review that did not answer.
- `trigger` names which units the pass re-judges: `uncertain` for units
  carrying the declared `uncertain` flag, `no-match` for units whose
  selection resolved to the no-match outcome, `always` for every unit the
  primary forward produced a result for.
- `on_failure` names what a triggered-but-unanswered review means:
  `keep-original` leaves the primary's selection as the final answer and
  keeps the failed review recorded on the unit; `strict` lets the review
  govern, leaving the unit with the review's own outcome and no
  selection — the caller's declaration that the primary's answer is not
  to be trusted unconfirmed.
- `max_items` caps the units one call's review pass may dispatch.
- `max_attempts` caps the backend dispatches the whole secondary phase
  may spend — review and fallback together — past the primary forwards.
- `latency_ms` is the phase's own wall-clock bound, measured from the
  primary fan-out's completion and never beyond the call's own deadline.
- `max_spend` caps the worst-case spend the phase's monetary holds may
  take — the sum of the doors' quoted maximum reservations, in the
  account's millionths. A declared spending bound requires a known
  configured price; without one, the secondary work remains unattempted.
- `fallback` is the list of retry destinations, first match per cause
  winning.

A zero bound is refused as `invalid_request`: a bound that admits no work
is a misconfiguration, not a feature.

## Fallback causes

Fallback covers declared failure classes, and the classes are distinct
causes, not interchangeable retries:

- `transport` covers items the primary forward never got a decided answer
  for — a dead door, a 5xx, a deadline, or an unreadable response.
- `capacity` covers items the call's own bounds stopped before dispatch —
  queue, slot, or deadline waits that ended unattempted.
- `refused` covers the door's typed 4xx refusal. An entry covering
  `refused` must also enumerate the exact refusal `codes` it may carry,
  and an item whose refusal's cause is not in the list keeps its refusal.
  A semantic refusal is an answer; bypassing one is a declared decision,
  never a default.

An item whose cause has no declared entry reports the skip on its
`fallback` record rather than retrying silently.

## Admission and identity

Every secondary dispatch — review or fallback — passes the same admission
path as the primary call, under its own recorded request and attempt
identity:

1. Authorize the named door against the caller's tenant and bindings.
2. Apply the door's declared rate window, concurrency pool, and the
   process's forward bound, waited on inside the phase deadline.
3. Reserve quota durably under a new request identity derived from the
   call's (`{request}:rev:{seq}` or `{request}:fb:{seq}`).
4. Hold monetary admission under the same identity, when the gateway is
   configured with prices.
5. Verify the backend's published model card against the door's binding
   before a byte is forwarded.
6. Forward the sub-envelope inside the phase deadline.
7. Settle the reservation and the hold from the dispatch's own outcome
   and reported usage.
8. Write a sealed receipt.

A dispatch refused at any step is recorded on the item with the refusal's
typed code — `door_not_bound`, `rate_limited`, `quota_exhausted`,
`insufficient_funds`, `unpriced`, `identity_mismatch` — and never reaches
a backend. A review policy cannot widen a caller's reach: a reviewer or
fallback door the caller is not bound to refuses at authorization. The
review envelope asks the same questions over the same input state the
primary asked — the reviewer's read is independent, and the primary's
answer is never handed back for confirmation.

## What the response reports

Each item keeps its primary output and gains the records the phase
produced:

- `attempts` — every dispatch's row: role (`primary`, `review`,
  `fallback`), the door and the artifact that answered, the outcome and
  cause, the attempt identity, latency, and the settlement and usage
  reference the reservation carried. Secondary attempt records also include
  the sealed receipt reference and full served identity. A receipt-write failure
  leaves the receipt reference null, rather than inventing evidence.
- `units[].original` — the primary unit's whole result, present whenever
  a review or fallback rewrote the unit.
- `units[].review` — the review's own record: the trigger's reason, the
  attempt identity, the outcome, the reviewer's reported usage, and the
  reviewed selection when the answer held to the unit's contract.
- `units[].final_source` — `primary` or `reviewer`, which model's output
  the unit's `selected` and `raw` report.
- `original` — a fallback-replaced item's whole primary result.
- `fallback` — the cause class, the door retried, the outcome, and the
  attempt identity; `skipped` when the policy named no entry for the
  cause or the refusal's code was not declared.
- `review_status` — `not-reviewed`, `reviewed`, or `review-incomplete`
  when a bound or a failed review left triggered work unresolved.

A reviewed answer is validated against the same contract the primary's
answers are held to — the question type, the label set, the distribution,
the rubric. An answer that cannot be validated is a review that did not
answer; it is never averaged, patched, or completed with invented
probabilities. The reviewer's output stands on its own; the primary's
confidence is never copied onto it.

The call-level `review` summary reports the schema version, the digested
policy document, the reviewer door, the trigger and failure behavior, the
declared bounds, the counts of reviewed and fallback work, and — under
monetary admission — the worst-case spend the phase's holds reserved.
`usage.review` and `usage.fallback` account the secondary dispatches'
reported tokens separately from the primary's counters, and each reports
`*_complete: false` rather than a partial total presented as whole. A
hold that could not be priced stays outstanding; an item the phase never
reached reports the bound that stopped it.

## Bound semantics

The bounds stop work in order, and every stop is recorded where it
landed: an item's `fallback` record or a unit's `review` record carries
the outcome `unattempted` and the cause naming the bound — `max_items`,
`max_attempts`, `latency_ms`, or `max_spend`. The spend check compares
the sum of worst-case holds already taken against the next dispatch's
quoted maximum, so the phase never takes a hold the declared budget could
not cover.

A stopped review obeys `on_failure` too. Under `strict`, budget exhaustion
retains the primary under `original` but leaves the final selection null.
Unknown prices cannot satisfy a declared spending limit, and spend arithmetic
overflow stops further secondary work.
