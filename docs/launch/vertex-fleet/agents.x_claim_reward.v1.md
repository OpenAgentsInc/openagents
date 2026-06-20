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

## Follow-up: post-settlement receipt audit (this run)

The preflight evaluator gates *before* the smoke; nothing gated the settled
record *after* it. This run adds the after-the-fact counterpart — a pure,
public-safe **post-settlement receipt auditor** the operator runs on the
settled reward row before publishing the issue #4626 transition receipt.

- `apps/openagents.com/workers/api/src/x-claim-reward-smoke-receipt-audit.ts`
  — `auditXClaimRewardSmokeReceipt(reward)` plus the
  `XClaimRewardSmokeReceiptAudit` / `XClaimRewardSmokeReceiptCheck` types.
  Checks: settled state, bounded 1000-sat amount, well-formed public receipt
  ref, at least one `settlement_evidence.public.*` ref, and no leaked payment
  material (lightning invoice, BOLT12 offer, lightning address, preimage, or
  payment hash) in any public-facing field. Emits a public-safe
  `transitionReceiptSummary` (no treasury payment id or destination).
- `apps/openagents.com/workers/api/src/x-claim-reward-smoke-receipt-audit.test.ts`
  — covers the clean pass, each blocking reason, every leaked-material pattern,
  and a no-payment-material assertion on the serialized summary.
- `apps/openagents.com/docs/2026-06-09-x-claim-reward-dispatch-runbook.md`
  — a "Post-settlement receipt audit" section wiring the auditor in before the
  transition-receipt step.

## Follow-up: transition-receipt proposal builder (this run)

The preflight gates before the smoke and the receipt auditor gates the settled
row after it, but nothing assembled the actual registry-transition payload from a
passing audit. This run adds that final pure, fund-free bridge.

- `apps/openagents.com/workers/api/src/x-claim-reward-smoke-receipt-audit.ts`
  — `buildXClaimRewardSmokeTransitionRequest(reward)` plus the
  `XClaimRewardSmokeTransitionProposal` / `XClaimRewardSmokeTransitionRequest`
  types. It runs the post-settlement audit and, only when it passes, emits the
  public-safe `POST /api/operator/product-promises/transitions` body
  (`promiseId: agents.x_claim_reward.v1`, `toState: green`, deduped public
  evidence refs) with a defensive re-scan for leaked payment material. It flips
  no promise state and moves no funds — the registry route re-checks blockers and
  the green flip still needs owner sign-off.
- `apps/openagents.com/workers/api/src/x-claim-reward-smoke-receipt-audit.test.ts`
  — a `transition request` suite covering the ready proposal, audit-gated
  withholding, payment-material refusal, and a no-payment-material assertion on
  the serialized proposal.
- `apps/openagents.com/docs/2026-06-09-x-claim-reward-dispatch-runbook.md`
  — a "Transition-receipt proposal" section wiring the builder between the
  post-settlement audit and the registry transition call.

## What remains

The blocker is still open: an operator must run the live single-reward smoke —
enable the flag with the preflight green, dispatch one eligible reward to a real
owner receive code, let it settle with public-safe receipt refs, run the
post-settlement audit, and record the transition receipt on issue #4626. Only
then does this blocker clear and the promise become eligible for a green flip.
