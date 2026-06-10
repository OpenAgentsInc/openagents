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
- verified closeout refs
- receipt refs
- settled sats when provider-confirmed
- source refs

The route keeps empty lanes visible with blocker refs so public pages can show
which assignment lanes are waiting for real verified receipt rows.

## Web Surface

The public Training Runs page renders a compact leaderboard panel linking to
the all-lanes feed and each lane feed. It does not invent rows client-side; the
Worker JSON feed remains the ranking authority.

## Current Live Boundary

The framework is deployed before real verified rows exist. Empty lane blockers
use:

- `blocker.training_leaderboard.<lane>.requires_verified_receipts`

The issue should remain open until at least one lane is populated from real
verified closeout receipts in production.
