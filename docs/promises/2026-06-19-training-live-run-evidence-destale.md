# Training.* Live-Run Evidence Destale (Registry 2026-06-19.7)

Date: 2026-06-19

## Decision

This is a `training.*` evidence/copy destale that makes the product-promise
registry match the LIVE Tassadar run `run.tassadar.executor.20260615`. It flips
NO promise state. The green count stays exactly 20. Every change below is
anchored to a dereferenceable, real settled-Bitcoin receipt and its backing
Verified `exact_trace_replay` challenge — no fabricated evidence, no scope
widening beyond what the receipts prove.

The trigger: the live per-run settled feed has moved ahead of the prior
`2026-06-18.3` registry copy (which described two contributors / 1,005 sats).

## The Proven Fact (live source of truth)

`GET /api/public/training/runs/run.tassadar.executor.20260615/settlements` now
enumerates **five counted `realBitcoinMoved:true` settlements** to **five
distinct independent contributor pylons**, plus one excluded
`realBitcoinMoved:false` simulation row:

| Receipt | Sats | Pylon | Challenge | Real |
| --- | --- | --- | --- | --- |
| `receipt.nexus.tassadar_run_settlement.idempotency.tassadar.run_settlement.5b7f92fe.canary1k.v6.20260618` | 1000 | `pylon.448ba824…` | `071445c5-…` | true |
| `receipt.nexus.tassadar_run_settlement.settlement.tassadar.retro.10c3b01b.trigger.v1` | 5 | `pylon.81f0facfe…` | `10c3b01b-…` | true |
| `receipt.nexus.tassadar_run_settlement.idempotency.tassadar.ao6.final.20260619T003201.manual.v1` | 5 | `pylon.f0504556…` | `335df7e8-…` | true |
| `receipt.nexus.tassadar_run_settlement.idempotency.tassadar.ao6.patched.20260619T004804.manual.v1` | 5 | `pylon.58b7f3c0…` | `33d4ca81-…` | true |
| `receipt.nexus.tassadar_run_settlement.idempotency.tassadar.ao6.patched2.20260619T010148.manual.v1` | 5 | `pylon.fa4e9049…` | `9fd49062-…` | true |
| `receipt.nexus.tassadar_run_settlement.idem.tassadar.settlement.59ba1f30.orrery.v2` | 5 | `pylon.448ba824…` | `59ba1f30-…` | **false (simulation, excluded)** |

- Real settled total: **1,020 sats** (1,000 canary + 4×5 self-serve).
- All real rows: `moneyMovement:real_bitcoin`, `state:settled`,
  `adapter:spark_treasury`, each backed by a Verified `exact_trace_replay`
  challenge.
- Run summary: `qualifiedContributorCount` 5, `acceptedTraceCount` 11,
  `providerConfirmedSettledPayoutSats` 1,020.

## Per-Promise Effects (all receipt-anchored, none a state flip)

- **`training.decentralized_training_launch.v1` — stays GREEN.** Copy moves from
  "two distinct independent contributors / 1,005 sats" to "five distinct
  independent contributors / 1,020 sats". Green→green; no `promise_transition`
  required. An exception receipt may be recorded against the deployed
  `2026-06-19.7` version per `proof.claim_upgrade_receipts.v1` if owner review
  wants one for the copy upgrade.
- **`training.public_distributed_training_run.v1` — stays RED, narrowed.** The
  "payment and settlement refs for more than one contributor" criterion is now
  MET (five distinct settled contributors). The remaining gate is a documented
  participant-count/network-scale methodology plus broad accepted-work receipts
  beyond these five canary-scale settlements. This is the `training.*` record
  CLOSEST to a yellow upgrade and the fastest owner-gated win.
- **`training.verification_classes.v1` — stays YELLOW.** `exact_trace_replay`
  is now exercised on real dispatched work across five distinct paid
  contributors, broadening the three-classes-on-real-work evidence. The only
  blocker remains the `#4674` aggregate-only-vs-per-contribution written
  decision (`aggregate_only_policy_redecision_missing`), unchanged.
- **`training.post_training_arc.v1` — stays PLANNED, evidence added.** Cites the
  previously-uncited 2026-06-11 CS336 A5 alignment paid run
  (`run.cs336.a5.alignment.demo`, eval `eval.cs336_a5.synthetic_math.bounded_combined.4682.1`
  at `GET /api/training/evals/a5`, four Verified challenges incl.
  `training.verification.challenge.cb1d4f39-5b33-4650-8659-afcc33131af5`,
  ~40-sat real settlement) proving rollout-generation and reward-grading as
  paid, independently verified network work. Stays planned: no SFT/preference
  stage dispatched, no reviewed vibe-test artifact.
- **`training.data_refinery_corpus.v1` — stays PLANNED, evidence freshened.**
  The `a4_eval_delta` leaderboard lane is live-but-empty in
  `training-leaderboards.ts`; the payment policy is
  `2026-06-10-cs336-a4-data-refinery-payment-policy.md`. Single missing receipt
  for green: one Verified `deterministic_recompute` refinery-shard challenge
  whose closeout records an eval-delta payment.

Unchanged (no new settled receipts exist for them):
`training.public_gradient_windows.v1`, `training.full_pipeline_program.v1`,
`training.ablation_system.v1`, `training.model_ladder.v1`,
`training.marathon_operations.v1`, `training.device_capability_dataset.v1`.

## Green-Ready Flags (owner-gated)

None of these are flipped here. The owner-signed, receipt-first path per
`proof.claim_upgrade_receipts.v1`:

1. **`training.public_distributed_training_run.v1` red→yellow** — fastest win:
   author and publish a participant-count/network-scale methodology; the five
   settled contributors already satisfy the multi-contributor settlement leg.
2. **`training.decentralized_training_launch.v1`** — green→green copy-upgrade
   exception receipt for the 2→5 contributor / 1,005→1,020 sats change.

## Public Copy Rule

- Safe: five distinct independent contributors have been paid real Bitcoin
  (1,020 sats total) on the live run, each verified by independent
  `exact_trace_replay` replay, with public receipts (`realBitcoinMoved:true`).
- Unsafe: network-scale, paid-at-scale, hundreds-paid, largest-run,
  canonical-model-mutation, or unbounded-payout copy. The proof is exactly five
  bounded canary-scale settlements — scope the copy to that.

## Authority Boundary

A destale of evidence/copy to match the live feed grants no new payout,
settlement, provider, wallet, deployment, network-scale, or public-claim
authority. No `promise_transition` is required because no state changes; any
future green flip remains receipt-first and owner-signed per
`proof.claim_upgrade_receipts.v1`.
