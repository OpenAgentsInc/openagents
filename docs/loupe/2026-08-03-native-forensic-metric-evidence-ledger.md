# Native forensic metric evidence ledger

Date: 2026-08-03

Status: code-complete OFR-005 persistence boundary. This document does not
claim that a live forensic campaign ran.

## Outcome

The Forensics API now retains the evidence used to build a scorecard. It no
longer depends only on caller-supplied arrays. The authenticated route accepts
four strict record types:

- `ForensicRunEvent.v1` for native T0-T8, finding, failure, and lifecycle
  events;
- `ForensicProviderUsageReceipt.v1` for exact, estimated, upper-bound, or
  unavailable provider usage and cost;
- `ForensicEvaluatorAdjudication.v1` for frozen later judgment that cannot
  rewrite the immutable finding event or its T5 timestamp; and
- `ForensicReviewerBurdenReceipt.v1` for bounded review time, corrections, and
  rejections bound to one retained `review_recorded` event.

`RecordMetricEvidence` writes one record. `ReadMetricEvidence` reads one exact
run for the authenticated owner. The route does not expose another owner's
records and returns no public projection.

## Persistence laws

Cloud SQL table `forensic_metric_evidence` owns the durable rows. Its primary
key is `(owner_ref, record_ref)`. Run events also have a unique
`(owner_ref, run_ref, event_sequence)` key. A PostgreSQL advisory transaction
lock serializes the next event for one owner/run before insert.

The service applies these laws:

- the first event has sequence 1 and subsequent events are dense;
- an identical record replay is idempotent;
- a reused record reference with different canonical bytes is a conflict;
- every record is strictly decoded before write and again after read;
- stored event sequences are validated before projection;
- run event, adjudication, usage, and reviewer receipts remain separate rows;
- an unavailable usage or review duration has a reason and no numeric value;
- run reads rebuild event and receipt digests from retained bytes; and
- scorecards include reviewer receipts in their receipt digest.

The ledger stores native evidence. It does not decide whether a finding is
true, promote a prompt, authorize disclosure, or make a run releasable.

## Scorecard rebuild

`rebuildForensicScorecard` consumes the ledger read projection with the frozen
dataset, evaluator, candidate, registry, population, and hard-gate inputs. It
continues to derive T5 from the immutable finding event. Later adjudication can
qualify that event but cannot change its timestamp. Eligible misses keep their
spent usage and a nonzero right-censor boundary. Dataset splits and vulnerable,
structural, fixed, clean, and incomplete populations remain separate.

Reviewer minutes per qualified finding use the retained review duration in
milliseconds, matching the frozen metric unit. Correction/rejection burden is
the retained sum of both counts. Missing or unavailable reviewer duration
remains unavailable rather than becoming zero.

## Verification

```sh
pnpm --filter @openagentsinc/forensic-contract test
pnpm --filter @openagentsinc/forensic-contract typecheck
pnpm exec vp test --run --project @openagentsinc/api-worker \
  apps/openagents.com/workers/api/src/forensic-metric-evidence.test.ts \
  apps/openagents.com/workers/api/src/forensic-managed-sandbox.test.ts
```

The focused tests cover immutable replay, conflict refusal, dense event
ordering, owner/run isolation, unavailable provider usage, reviewer receipts,
authenticated route record/read behavior, scorecard rebuilding, and separate
repetition identity.
