# CS336 A1 Real-Gradient Public Status

Date: 2026-06-10

Issue: [#4678](https://github.com/OpenAgentsInc/openagents/issues/4678)

This document records the OpenAgents monorepo side of the A1 leaderboard-class
training run. The issue remains Lane B until an operator-funded run executes on
at least two real contributor devices and produces provider-confirmed payout
settlement receipts.

## Psionic External Ask

Psionic must expose a connector-consumable lane:

- `psion_cs336_a1_real_gradient_v1`;
- TinyStories/OWT shard inputs for CS336 A1;
- real-gradient training, not the bounded finite-difference reference trainer;
- public-safe Freivalds commitment refs for gradient closeouts;
- public-safe merge and eval refs;
- validation-loss points tied to receipt/source refs;
- worker closeout refs that the Worker can bind to verification challenges.

Until that exists, public run summaries keep
`blocker.cs336_a1.real_gradient_psionic_lane_external`.

## Public Projection

`GET /api/training/runs` and
`GET /api/training/runs/{trainingRunRef}` now include
`summary.realGradient`.
`GET /api/training/leaderboards/a1` exposes the same public-safe A1
leaderboard rows directly for plan-step-11 consumers.

The projection carries:

- the Psionic lane ref and requirement refs;
- the two-real-contributor-device requirement;
- Freivalds, gradient closeout, merge, and eval refs;
- validation-loss-under-budget status and loss curve points;
- leaderboard rows derived from public leases and verified challenge refs;
- scope boundaries that this A1 rehearsal does not replace the Qwen fine-tune
  gate in #4670 and cannot flip first-real-training-run copy green alone.

Pending, offered, claimed, wallet-side, or locally staged payment records are
not counted as settled. Leaderboard row `settledPayoutSats` remains `0` until
provider-confirmed settlement receipts are linked.

## Verification

Run from `apps/openagents.com/workers/api`:

```sh
bunx vitest run src/training-run-window-authority.test.ts src/training-run-window-routes.test.ts src/training-run-public-copy-gate.test.ts
```

The tests prove the no-evidence path stays blocked and the observed path needs
two devices, Freivalds commitments, merge/eval refs, a verified challenge, and
loss under budget.
