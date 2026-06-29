# Probe GEPA Stage 1 Shadow Promotion Gate

Date: 2026-06-08

Status: implemented for `OpenAgentsInc/openagents#515`.

OpenAgents product surface now has a typed Stage 1 gate for Probe GEPA candidate movement. The gate
can emit only:

- `shadow`; or
- rejected `benchmark_only`.

It cannot emit `active` or `release_candidate`. Those states require a separate
explicit OpenAgents product surface/Blueprint production gate with accepted coding outcome evidence.

The implementation lives in
`workers/api/src/probe-gepa-stage1-shadow-promotion-gate.ts`.

## Inputs

The gate consumes:

- retained result refs;
- validation result refs;
- Psionic candidate-frontier refs;
- route scorecard refs;
- proof bundle refs;
- proof completeness basis points;
- validation delta basis points;
- typed policy findings with explicit severity;
- OpenAgents product surface gate refs;
- Blueprint gate refs;
- requested target state.

The gate uses typed policy-finding severity instead of parsing blocker meaning
from prose. A finding marked `blocking` blocks shadow movement.

## Decision Rules

The gate returns `shadow` only when all are true:

- requested state is `shadow`;
- retained refs exist;
- validation refs exist;
- Psionic frontier refs exist;
- route scorecard refs exist;
- proof bundle refs exist;
- proof completeness is at least `8000` basis points;
- validation delta is non-negative;
- OpenAgents product surface gate refs exist;
- Blueprint gate refs exist;
- no policy finding has `blocking` severity.

Requests for `active` return rejected `benchmark_only` with
`blocker.probe_gepa.stage1.active_not_allowed_by_shadow_gate`.

Requests for `release_candidate` return rejected `benchmark_only` with
`blocker.probe_gepa.stage1.release_candidate_requires_separate_gate`.

## Public Claim Boundary

The accepted public label is:

```text
shadow candidate; validation measured only
```

The gate does not claim:

- public Terminal-Bench score;
- active production deployment;
- release-candidate status;
- paid customer outcome improvement;
- payout or settlement;
- automatic activation.

## Verification

Run:

```sh
bun run --cwd workers/api test -- probe-gepa-stage1-shadow-promotion-gate.test.ts probe-gepa-outcome-metrics.test.ts
bun run --cwd workers/api typecheck
```
