# Probe GEPA Pylon Production-Equivalent Smoke

Status: retained evidence contract for OpenAgents product surface issue #511.

This smoke is the current source-backed production-launch evidence for the
Probe GEPA/Pylon lane. It does not claim that Probe beats Terminal-Bench, that
Pylons have settled paid benchmark work, or that Artanis can administer
production without the separate scheduled-runner gate.

## What It Proves

The retained record in `workers/api/src/artanis-gepa-production-smoke.ts`
models a bounded Stage 0 Probe GEPA smoke with:

- SHC and Harbor run refs;
- Terminal-Bench 2 split and suite refs;
- Probe closeout refs and closeout bundle refs;
- artifact manifest, proof bundle, verifier, and resource refs;
- route scorecard refs;
- accepted and rejected Pylon closeout refs;
- explicit `unpaid_smoke` payment mode;
- Psionic import refs for the same closeouts;
- a public-safe Forum summary ref.

The projection is evidence only. It denies wallet spend, settlement mutation,
provider mutation, model training, automatic candidate promotion, public
benchmark score claims, payout claims, and automatic Forum posting.

## Launch Gate Effect

`exampleArtanisProductionLaunchGateRecord` now feeds the
`production_e2e_smoke` check from this retained GEPA/Pylon smoke. That removes
the public launch-gate blocker:

`blocker.public.artanis.launch_gate.production_e2e_smoke.blocked`

The scheduled-runner blocker remains separate from this smoke. OpenAgents product surface #512 adds
that separate bounded proof in
`docs/artanis/2026-06-08-bounded-gepa-scheduled-runner.md`.

So this smoke may support retained production-equivalent GEPA/Pylon copy, but
it still does not authorize public Terminal-Bench scores, paid-work settlement
claims, Probe candidate activation, provider mutation, wallet spend, or
unbounded production administration.

## Relation To Live Work

This retained contract is designed to consume the next live tranche tracked in
Probe #188:

- OpenAgents #4563: real SHC Harbor Terminal-Bench smoke with Probe closeout
  bundles;
- OpenAgents #4564: public Benchmark Cloud runner executing real Probe tasks;
- Psionic #1093: live OpenAgents product surface/Pylon closeout imports into the GEPA coordinator;
- OpenAgents product surface #512: bounded scheduled runner proof;
- OpenAgents product surface #513: route scorecards connected to Coding on Autopilot outcomes.

Until the live SHC/Harbor run lands, the retained refs remain public-safe
evidence refs rather than public benchmark results.

## Verification

Focused coverage:

- `workers/api/src/artanis-gepa-production-smoke.test.ts`
- `workers/api/src/artanis-production-launch-gate.test.ts`
- `workers/api/src/artanis-public-report.test.ts`

Expected public state immediately after #511 and before #512:

- `productionLaunchGate.failedOrPendingRequiredCount` is `1`;
- the only launch-gate blocker is `scheduled_runner`;
- `productionLaunchGate.canClaimContinuouslyRunning` remains `false`;
- public GEPA/Pylon copy is retained-smoke copy only, not a public benchmark
  score or paid-work settlement claim.

Expected public state after #512:

- `productionLaunchGate.failedOrPendingRequiredCount` is `0`;
- `productionLaunchGate.canClaimBoundedStatusProjection` is `true`;
- `productionLaunchGate.canClaimContinuouslyRunning` is `false`;
- the scheduled-runner check is passed by the bounded status-runner proof, not
  by this smoke contract.
