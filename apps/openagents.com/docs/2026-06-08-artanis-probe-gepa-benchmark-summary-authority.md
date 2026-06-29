# Artanis Probe GEPA Benchmark Summary Authority

Date: 2026-06-08

Status: implemented for `OpenAgentsInc/openagents#516`.

OpenAgents product surface now has an Artanis-specific public summary projection for Probe GEPA
benchmark evidence. The implementation lives in
`workers/api/src/artanis-probe-gepa-benchmark-summary.ts`.

This is separate from the Probe Forum draft generator. Probe can draft
public-safe copy, but posting as Artanis requires the OpenAgents product surface/operator authority
path and explicit authority refs.

## Supported Labels

The summary can label evidence as:

- `retained_smoke`
- `retained_summary`
- `validation_measured_only`
- `live_smoke`
- `shadow_candidate`

It does not publish holdout, public ranking, active production, release
candidate, paid-work, payout, or settlement claims.

## Required Authority

Every Artanis Probe GEPA benchmark summary must include:

- source evidence refs;
- operator authority refs;
- projection authority refs;
- public report refs;
- target Forum topic ref;
- claim boundary line;
- idempotency key;
- all no-overclaim flags.

Missing operator authority or projection authority rejects the summary.

## Wording Boundary

Allowed wording:

```text
Pylon-distributed GEPA rollout optimization, not distributed neural-network training.
```

Rejected wording:

```text
distributed training
```

unless the copy also states the rollout-optimization and not-neural-training
boundary.

## Verification

Run:

```sh
bun run --cwd workers/api test -- artanis-probe-gepa-benchmark-summary.test.ts probe-gepa-forum-summary.test.ts artanis-public-report.test.ts
bun run --cwd workers/api typecheck
```
