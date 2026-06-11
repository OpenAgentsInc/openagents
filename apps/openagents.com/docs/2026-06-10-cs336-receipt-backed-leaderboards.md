# CS336 Receipt-Backed Leaderboards

Date: 2026-06-10

Issue: `OpenAgentsInc/openagents#4683`

Forum claim: `https://openagents.com/forum/t/ad9b4415-2258-44cb-8fd6-1b467b381182`

## Scope

This slice adds a public leaderboard framework for CS336 homework lanes.

Routes:

- `GET /api/training/leaderboards`
- `GET /api/training/leaderboards/{lane}`

Current lanes:

- `a1_loss`
- `a2_throughput`
- `a3_isoflop`
- `a4_eval_delta`
- `a5_accuracy`

The no-spend smoke is:

- `bun run smoke:training-leaderboards`

## Ranking Boundary

Rows rank only when they have verified closeout refs. Unverified rows are
filtered before rank assignment.

Each row includes:

- lane
- rank
- public-safe contributor ref
- training run ref
- metric ref
- score and score label
- provenance label
- verified closeout refs
- receipt refs
- settled sats when provider-confirmed
- source refs

## A3 IsoFLOP Lane (2026-06-11)

`a3_isoflop` ranks verified scaling-sweep cells from the public
`GET /api/training/isoflop/a3` projection. One row per contributor per
compute budget: the contributor's best (lowest) verified validation loss at
that budget, with the budget named in the metric ref
(`metric.cs336_a3.validation_loss.c_<flops>`). Cells without verification
challenge refs, without settlement receipt refs, without a public pylon ref,
or without a validation loss are structurally excluded before ranking.

## Earnings Linkage (2026-06-11)

`settledPayoutSats` on every row is linked from provider-confirmed settlement
receipts: the leaderboard route resolves each row's
`receipt.nexus_pylon.settlement.*` refs against the payment-authority receipt
ledger and counts `amountSats` only when the public projection state is
`settled` on a `settlement_recorded` receipt. Pending, offered, claimed, or
wallet-side records count zero — pending is never displayed as paid. Each row
carries a provenance label stating this rule.

The route keeps empty lanes visible with blocker refs so public pages can show
which assignment lanes are waiting for real verified receipt rows.

## Web Surface

The public Training Runs page renders a compact leaderboard panel linking to
the all-lanes feed and each lane feed. It does not invent rows client-side; the
Worker JSON feed remains the ranking authority.

## Current Live Boundary

The framework deployed before real verified rows existed. Empty lane blockers
use:

- `blocker.training_leaderboard.<lane>.requires_verified_receipts`

As of 2026-06-11, production serves real verified rows on `a2_throughput`
(four device-capability metrics from the #4681 paid benchmark closeouts) and
`a5_accuracy` (the first verified eval suite from the #4682 paid rollout). The
`a3_isoflop` lane ranks the #4679 paid sweep cells after the next deploy.
`a1_loss` stays honestly empty until public validation-loss evidence exists
(the #4675 closeout verified BPE and Freivalds work, not a loss-under-budget
curve), and `a4_eval_delta` stays honestly empty until the fixed-trainer
eval-delta loop exists (#4680 named remainder). Pending work is never
displayed as paid.
