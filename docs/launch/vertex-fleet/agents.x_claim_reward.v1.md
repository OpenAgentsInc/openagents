# agents.x_claim_reward.v1 — worker note

Promise: `agents.x_claim_reward.v1` (state: yellow — unchanged by this work).

## Blocker advanced

`blocker.product_promises.x_claim_reward_live_dispatch_smoke_missing`
(NOT cleared — no live reward has settled yet; the blocker stays listed).

## What was built

A pure, public-safe **live-smoke preflight evaluator** so the operator can
confirm every precondition is green before flipping `TREASURY_DISPATCH_ENABLED`
on for the first single-reward dispatch smoke. It moves no funds and emits only
aggregate readiness checks (no invoices, preimages, payment ids, or receive
codes).

- `apps/openagents.com/workers/api/src/x-claim-reward-treasury-dispatcher.ts`
  — `evaluateXClaimRewardSmokePreflight(input)` plus the
  `XClaimRewardSmokePreflightReport` / `XClaimRewardSmokePreflightInput` /
  `XClaimRewardSmokePreflightCheck` types. Checks: dispatch flag enabled,
  per-run cap allows one, exactly one approved reward, no pending payment in
  flight, daily-cap headroom for one 1000-sat reward, and sufficient treasury
  liquidity (amount + buffer).
- `apps/openagents.com/workers/api/src/x-claim-reward-treasury-dispatcher.test.ts`
  — a `live smoke preflight` suite covering ready, each blocking reason, and a
  no-payment-material assertion on the serialized report.
- `apps/openagents.com/docs/2026-06-09-x-claim-reward-dispatch-runbook.md`
  — a "Preflight readiness gate" section wiring the evaluator to
  `GET /api/operator/treasury/status` before the live dispatch flow.

## What remains

The blocker is still open: an operator must run the live single-reward smoke —
enable the flag with the preflight green, dispatch one eligible reward to a real
owner receive code, let it settle with public-safe receipt refs, and record the
transition receipt on issue #4626. Only then does this blocker clear and the
promise become eligible for a green flip.
