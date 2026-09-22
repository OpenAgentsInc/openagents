# Durable classification jobs

`POST /v1/classify` answers inside one HTTP call: a caller with a corpus
that outlives a request needs work the gateway accepts, runs, and lets
the caller collect later. `POST /v1/jobs` is that surface — the same
classify request, the same admission path, executed asynchronously and
recorded durably before it is acknowledged.

This document is the contract: the routes, the durable record, the
states, the recovery rules, and the notification lane. A relay event or
an in-memory queue is not durable job storage; the job's manifest and
item ledger live beside the registry the gateway already writes.

## Submission

```text
POST /v1/jobs
Authorization: Bearer oak_<id>.<secret>
Idempotency-Key: caller-supplied key
Content-Type: application/json
```

```json
{
  "v": "openagents.job.v1",
  "kind": "classify",
  "request": { "v": "openagents.classify.v1", "model": "…", "…": "…" },
  "notify": { "url": "https://caller.example/hook", "secret": "optional" }
}
```

The request body is the `POST /v1/classify` envelope, verbatim. The
gateway authenticates the caller, validates the request against the
same authorization, lane, limits, plan, and context checks the
synchronous route runs, writes the job's manifest and status to disk,
and only then acknowledges:

```json
{ "v": "openagents.job.v1", "job": "job_<digest>", "status": "queued",
  "counts": { "expected": 4, "attempted": 0, "answered": 0, "refused": 0,
              "unavailable": 0, "unattempted": 0, "unknown": 0 } }
```

A validation failure is the same typed refusal the synchronous route
returns; nothing is persisted for a rejected submission. Quota is
reserved at execution, not submission — an accepted job is admitted
work, and its spend settles once when it runs.

### Idempotency

Submission binds the `Idempotency-Key` to the credential and the
manifest's content digest, recorded in the job index after the manifest
and status land. A resubmission with the same key and identical content
returns the existing job — its current status, not a second execution.
A resubmission with the same key and changed content is a conflict:
`409` with `idempotency_conflict`, never a silently replaced job. A
submission without a key is minted one and cannot be replayed as the
same job.

The job id is derived from the key and the manifest digest, so the
idempotent replay names the same job rather than a lookup.

## The durable record

Each job owns a directory under the registry:

```text
registry/jobs/<job_id>/manifest.json   the accepted request, tenant, and notify config
registry/jobs/<job_id>/status.json     the current state and outcome counts
registry/jobs/<job_id>/items.jsonl     one line per finished item, in completion order
registry/jobs/<job_id>/events.jsonl    notification events, when notify was declared
registry/jobs/<job_id>/deliveries.jsonl one line per webhook delivery attempt
registry/jobs/<job_id>/cancelled       the cancellation marker — its
                                       presence is the intent
registry/jobs/index.jsonl              idempotency key → job and manifest digest
```

The manifest and status land before the acknowledgement; the index
entry is written last. A process that dies between them leaves an
orphaned job directory the index never named, which recovery removes —
the submission was never acknowledged, so nothing was accepted.

## States

```text
queued → running → completed | failed | cancelled
             ↘ cancelling → cancelled
```

- `queued` — accepted, not yet dispatched.
- `running` — executing: items are forwarding and recording.
- `cancelling` — cancellation acknowledged; dispatched items settle,
  undispatched items do not run.
- `completed` — every item reached a terminal outcome.
- `cancelled` — cancellation finished.
- `failed` — the job did not run to its end: a vanished binding, a
  backend that failed identity, or a gateway restart that left the
  execution ambiguous.

`completed` is not complete successful coverage. The counts say what
happened: `expected` inputs; `attempted`, `answered`, `refused`,
`unavailable`, `unattempted`, and `unknown` among them. A completed job
with `unknown > 0` ended with work whose outcome the gateway cannot
honestly claim.

## Status and cancellation

```text
GET  /v1/jobs/{id}            → 200 status document, or 404
POST /v1/jobs/{id}/cancel     → 202 status document, or 404
```

Both require the same tenant's credential. A job another tenant owns
answers `job_not_found` — existence is not disclosed across tenants.
Anonymous submissions are visible to anonymous callers only, the same
boundary the shared lane already draws.

Cancellation is recorded before it is signaled: a durable marker beside
the status file, never a rewrite of it, so a cancel that races the
runner's terminal write cannot clobber the outcome the run recorded.
The status a caller reads reports `cancelling` while the marker stands
unresolved. An item that already dispatched finishes and records its
real outcome; an item that never dispatched records `unattempted`. A
second cancel on a terminal job returns the terminal status —
idempotent, not an error.

## Results export

```text
GET /v1/jobs/{id}/results?cursor=<opaque>&limit=<n>
```

Returns finished items in completion order:

```json
{ "v": "openagents.job-results.v1", "job": "job_<digest>",
  "items": [ { "index": 0, "input": "…", "attempt_id": "…",
               "dispatched": true, "item": { "…": "…" } } ],
  "next_cursor": "…", "terminal": "completed" }
```

Each item preserves its request-order index, caller input id, attempt
id, dispatch flag, and the full per-input result the classify pipeline
produced — outcomes, selections, units, latency, and queue time. The
job's terminal document names its sealed receipt; per-dispatch receipts
sit in `receipts.jsonl` under the job's request identity.

A cursor is opaque and expires (`job_cursor_ttl_ms`, default one hour);
an expired cursor answers `410` with `cursor_expired` rather than a
quiet restart. `limit` defaults to 100 and caps at 1,000. Export while
the job runs returns what has finished; `terminal` names the state.

## Retention and deletion

Terminal jobs are deleted on demand:

```text
DELETE /v1/jobs/{id} → 200 { "deleted": true }, or 409 job_running
```

A job still queued, running, or cancelling cannot be deleted — cancel
first. The operator's `job_retention_ms` (default seven days) bounds
how long terminal results stay; a sweep at startup and at each
submission removes what expired. Deletion removes the manifest, status,
items, events, and deliveries — the index entry is retained so a late
idempotent replay still names the digested submission it once meant.

## Restart recovery

A gateway restart finds every non-terminal job and reconciles it
explicitly, in place:

- An item with a recorded outcome keeps it.
- An item with no recorded outcome is `unknown`: the forward may or may
  not have run, and claiming either is dishonest. The quota ledger
  already orphans its unsettled holds the same way.
- The job ends `failed` with `gateway_restart` as its cause — it did
  not run to its end, and the record says so rather than resuming
  blindly. A job that was `cancelling` ends `cancelled`.
- Undelivered notification events re-enter the delivery queue; the
  event id makes a duplicate harmless.

Nothing accepted is lost: every input the manifest named is accounted
for in the counts. Nothing is double-settled: settlement keys are the
job's request identity and attempt, and recovery settles nothing twice.
Nothing is claimed exactly-once: ambiguous items are `unknown`, not
`unattempted`.

## Webhook notification

Polling is the required lane; webhooks are opt-in. A submission that
declares `notify.url` gets one signed event per terminal transition:

```json
{ "v": "openagents.job-event.v1", "id": "<job>-1", "type": "job.completed",
  "job": "job_<digest>", "at": "…", "counts": { "…": "…" } }
```

Delivery is `POST` to the declared URL with
`x-openagents-event: <id>` and
`x-openagents-signature: sha256=<hmac-sha256(secret, id + "." + body)>`.
The secret is generated when the caller does not supply one and
returned once, in the submission response; it is stored with the job
and never logged. `POST /v1/jobs/{id}/notify/rotate` replaces it and
answers the new secret once — later events sign under the new key and
the old one stops working.

A destination is validated at submission: `https`, or `http` to a
loopback address only, and never a URL carrying credentials. Each event
is delivered at most four times — the initial POST plus three retries
with backoff — and every attempt is recorded in `deliveries.jsonl` with
its outcome. A `2xx` completes delivery; anything else retries.
Receivers deduplicate on `x-openagents-event`: an event id delivered
twice means one transition.

## What it does not do

A job is a classify call with a durable envelope — the same admission,
reservation, verification, and receipt the synchronous route runs. It
does not schedule by deadline or priority, does not deduplicate work
across submissions, and does not resume a crashed run: an ambiguous
item is `unknown`, not replayed. Webhook delivery is a notification,
not a result channel — the export route is the record.
