# Probe GEPA Outcome Metrics

Date: 2026-06-08

Status: implemented for `OpenAgentsInc/openagents#510`.

OpenAgents product surface now has a projection that connects Probe benchmark learning evidence to
Coding on Autopilot accepted-outcome metrics without turning benchmark wins
into product wins. The implementation lives in
`workers/api/src/probe-gepa-outcome-metrics.ts`.

## Metrics

The projection compares before/after snapshots for:

- acceptance rate;
- human review minutes;
- turns per accepted outcome;
- retries per accepted outcome;
- cost per accepted outcome ref;
- retry count;
- route scorecard refs;
- artifact completeness;
- proof bundle completeness;
- public/private proof state;
- failure family reduction;
- regression count; and
- closeout quality.

The projection now also carries selected Blueprint signature refs, Probe tool
menu refs, workroom comparison refs, workroom refs, and workroom outcome refs.
Those refs are the bridge from benchmark route scorecards to Coding on
Autopilot product evidence.

## Candidate States

OpenAgents product surface can display candidate state as:

- `benchmark_only`;
- `shadow`;
- `release_candidate`; or
- `active`.

`active` requires accepted coding outcome refs plus public and private proof
refs and workroom outcome refs. Benchmark validation alone can support
`benchmark_only`, `shadow`, or `release_candidate` display, but not
active-product authority.

The Stage 1 benchmark promotion gate is narrower than the general display
model. `workers/api/src/probe-gepa-stage1-shadow-promotion-gate.ts` can emit
only `shadow` or rejected `benchmark_only`. `release_candidate` and `active`
require a separate explicit OpenAgents product surface/Blueprint production gate.

## Claim Boundary

If accepted outcome refs and proof refs are missing, the claim text is:

```text
Benchmark validation only; no paid customer outcome improvement claim.
```

When accepted outcome refs and proof refs exist, the claim text becomes:

```text
Accepted coding outcome comparison; paid customer outcome improvement is linked to accepted outcome refs and proof refs.
```

This keeps benchmark validation separate from paid customer outcome
improvement.

## Audience Projection

`projectProbeGepaOutcomeMetricsForAudience` produces a public or operator
projection from the same validated record.

Public projection may show public-safe route scorecard refs, candidate refs,
selected signature refs, and tool menu refs. It withholds workroom refs,
workroom comparison refs, private proof refs, and accepted outcome refs until
the accepted-outcome gate passes. Operator projection can show richer workroom
deltas after the same safety checks reject raw traces, provider credentials,
customer material, private repos, wallet/payment material, and raw timestamps.

Artanis public reports now include a Probe GEPA summary generated from the
public audience projection. The summary can say retained smoke, retained
summary, or validation measured only. It cannot claim Coding on Autopilot
product improvement unless accepted-outcome refs, public proof refs, private
proof refs, and workroom outcome refs are present.

## Verification

Run:

```sh
bun run --cwd workers/api test -- probe-gepa-outcome-metrics.test.ts
bun run --cwd workers/api test -- probe-gepa-stage1-shadow-promotion-gate.test.ts
bun run --cwd workers/api test -- probe-gepa-forum-summary.test.ts
bun run --cwd workers/api test -- artanis-public-report.test.ts
bun run --cwd workers/api typecheck
```
