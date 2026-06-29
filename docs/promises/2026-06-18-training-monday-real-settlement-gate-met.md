# Training Monday Real Paid-Settlement Gate Met

Date: 2026-06-18

## Decision

The real paid-settlement gate on
`training.monday_decentralized_training_launch.v1` is now satisfied for a
bounded scope. A single 1,000-sat real Bitcoin run-settlement settled, native
over Spark, to an independent contributor (Orrery,
`pylon.448ba824…`). This upgrades the promise's evidence and copy from
"simulation-record path only" to "one real paid settlement proven", without
broadening any network-scale, paid-at-scale, largest-run, canonical-model, or
unbounded-payout claim.

This supersedes the simulation-only reading in
[`2026-06-17-training-monday-simulation-settlement-policy.md`](2026-06-17-training-monday-simulation-settlement-policy.md)
for the real-settlement question only. The simulation receipt remains valid
historical evidence that the settlement-record/projection path can be projected
publicly; it is retained as context, not removed.

## The Proven Fact

- Public settlement receipt
  `receipt.nexus.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618`
  with `realBitcoinMoved:true`, `moneyMovement:real_bitcoin`, `state:settled`,
  `adapter:spark_treasury`.
- Backed by Verified challenge
  `training.verification.challenge.071445c5-6ad6-4136-87e3-253b01914b4c`
  (independent validator replay on a distinct device; digests matched).
- #5232 closed. Public settled feed moved 0 → 1. No raw address in the public
  projection.

## Public Copy Rule

- Safe: say one bounded 1,000-sat real Bitcoin run-settlement settled to an
  independent contributor, native over Spark, with a public receipt
  (`realBitcoinMoved:true`) and an independent validator challenge.
- Unsafe: say the network is paying contributors at scale, that hundreds were
  paid, that this is the largest decentralized training run, that public
  gradients mutate a canonical model, or that any unbounded payout authority
  exists. The proof is exactly one 1,000-sat canary settlement — scope the copy
  to that.
- Required to widen beyond one canary: additional public settled receipts with
  `realBitcoinMoved:true`, a participant-count methodology, and accepted-work
  receipts for more than one contributor.

## Projection Rule

`/api/public/pylon-stats` continues to count only receipts whose public
projection proves real Bitcoin movement and settled state toward accepted-work
sats and `publicRealSatsSettled*` totals. The 1,000-sat canary qualifies; the
prior simulation receipt does not. Training qualification/progress and real
payout settlement stay separate counters.
