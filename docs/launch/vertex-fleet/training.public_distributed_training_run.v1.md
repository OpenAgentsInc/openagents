# training.public_distributed_training_run.v1

Promise state: **red** (unchanged — this change flips nothing).

## 2026-06-20 public scale-status projection

`GET /api/public/training/public-distributed-run-scale` now exposes a
public-safe, live-at-read projection for the public distributed training run
scale gate.

The projection composes the existing public training-run summary and settlement
reconciliation for `run.tassadar.executor.20260615`, then compares the live
counters against the documented network-scale threshold from
`docs/training/2026-06-19-public-distributed-training-run-scale-methodology.md`:

- `networkScaleQualifiedContributorThreshold: 50`
- `qualifiedContributorCount: 5`
- `acceptedTraceCount: 11`
- `realSettlementReceiptCount: 5`
- `providerConfirmedSettledPayoutSats: 1020`
- `networkScaleThresholdMet: false`
- `ownerSignedUpgradeAvailable: false`
- `greenGateSatisfied: false`

This clears no product blocker. It makes the current scale gap
machine-readable: the bounded run has real multi-contributor settlement
receipts, but it does not have comparable network-scale accepted-work receipts.

`blocker.product_promises.public_distributed_training_run_receipts_missing`
therefore remains active.

No participant-scale run, at-scale dispatch, spend, settlement, largest-run
claim, model-quality claim, public training capability claim, yellow/green
transition, or owner-signed upgrade is created by this projection.
