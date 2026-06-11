# CS336 A3 IsoFLOP Dashboard

Date: 2026-06-10

Issue: [#4679](https://github.com/OpenAgentsInc/openagents/issues/4679)

This document records the OpenAgents monorepo side of the CS336 A3
scaling-sweep homework lane.

## Psionic Contract

The Psionic-side external ask has landed in `OpenAgentsInc/psionic#1103` as
`psion_cs336_a3_scaling_reference_v1`.

The monorepo treats that lane as the source of:

- planner-selected `(N, D)` cells;
- one tiny training run per cell;
- validation-loss results;
- fit artifacts containing budget optima, exponents, and predicted best config;
- public receipt/source refs suitable for Worker projection.

Fitted laws are analysis artifacts that cite cell receipts. They are never
capability claims by themselves.

## Public Feed

`GET /api/training/isoflop/a3` returns
`openagents.training.isoflop_dashboard.v1`.

The feed includes:

- receipt-backed sweep cells;
- verified-cell counts;
- public fit artifacts when available;
- blockers until at least 20 verified cells and a fit artifact exist;
- scope refs that settlement requires provider confirmation.

The feed reads optional `a3ScalingSweep` evidence from a training run's
`public_projection_json`, so no D1 migration is required for this projection.

## Smoke

Run from `apps/openagents.com/workers/api`:

```sh
bun run smoke:cs336-a3:isoflop
```

The smoke proves the public projection stays blocked without cells and publishes
a fit only when 20 public verified cells and a fit artifact are present.

## Live evidence

The first live crowd-sourced sweep (24 paid cells, sampled
`deterministic_recompute` re-runs, the published Psionic fit artifact,
and the admission seam
`POST /api/training/runs/{trainingRunRef}/scaling-sweep-evidence`) is
recorded in `2026-06-11-cs336-a3-isoflop-paid-sweep-evidence.md`.
