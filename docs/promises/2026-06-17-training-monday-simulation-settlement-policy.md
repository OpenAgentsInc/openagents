# Training Monday Simulation Settlement Policy

Date: 2026-06-17

## Decision

Keep `training.monday_decentralized_training_launch.v1` green only for the
scoped launch proof: a public Tassadar run exists, a non-owner worker submitted
useful executor-trace work, an independent validator verified it, and the
settlement-record projection path is public.

The Orrery receipt
`receipt.nexus.tassadar_run_settlement.idem.tassadar.settlement.59ba1f30.orrery.v2`
is simulation-backed (`realBitcoinMoved:false`). It is accepted as evidence that
the settlement-record path can be projected publicly, not as evidence that real
Bitcoin moved or that the contributor received spendable sats.

## Public Copy Rule

- Safe: say the scoped launch/run/verification/settlement-record path is live.
- Unsafe: say the Orrery receipt proves real sats moved, that a contributor was
  paid real Bitcoin, or that public accepted-work sats settled because of this
  receipt.
- Required for real settlement copy: a public-safe receipt with
  `realBitcoinMoved:true`, settlement state `settled`, and no private payment
  material.

## Projection Rule

`/api/public/pylon-stats` must continue excluding simulation settlement receipts
from accepted-work sats and `publicRealSatsSettled*` totals. Training
qualification/progress and real payout settlement stay separate counters.
