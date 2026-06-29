# pylon.largest_decentralized_training_claim.v1

Date: 2026-06-20
State: red
Registry pass: 2026-06-20.43

## Status Projection

`GET /api/public/pylon/largest-decentralized-training-claim` is the
public-safe status projection for the largest decentralized training run claim.
It reads the existing public Tassadar run summary through the same scale-status
path used by `training.public_distributed_training_run.v1`, then compares the
current qualified-contributor count to the documented comparable benchmarks.

Current bounded run status:

- qualified contributors: 5
- accepted exact-trace work units: 11
- real settlement receipts: 5
- provider-confirmed settled payout: 1,020 sats
- current scale label: canary scale

Comparison benchmarks:

- Templar Covenant-72B public comparable: about 70 contributors
- Episode 236 target benchmark: 200 contributors

The current run is below both. The route therefore reports
`concreteComparableThresholdMet=false`,
`transcriptTargetThresholdMet=false`, `ownerSignedUpgradeAvailable=false`, and
`greenGateSatisfied=false`.

## Boundary

This projection does not create or widen a claim. It grants no contributor
admission, training dispatch, spend, settlement, benchmark victory, largest-run
claim, network-scale claim, or product-promise transition authority.

The only remaining blocker stays active:

- `blocker.product_promises.public_training_contributor_receipts_missing`

Green requires a comparable-scale run with public per-contributor qualified
receipts, plus an owner-signed receipt-first upgrade under
`proof.claim_upgrade_receipts.v1`.
