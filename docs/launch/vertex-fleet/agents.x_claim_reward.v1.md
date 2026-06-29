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

## Follow-up: pre-dispatch candidate gate (this run)

The aggregate preflight inspects ledger-wide stats and the post-settlement audit
inspects the settled row, but nothing validated the *specific* `eligible` row
chosen for the smoke before the operator runs `approve_dispatch`. This run adds
the front bookend — a pure, public-safe per-row candidate gate.

- `apps/openagents.com/workers/api/src/x-claim-reward-smoke-candidate.ts`
  — `assertXClaimRewardSmokeCandidate(reward)` plus the
  `XClaimRewardSmokeCandidateGate` / `XClaimRewardSmokeCandidateCheck` types.
  Checks: state is `eligible`, bounded 1000-sat amount, well-formed public
  receipt ref, no treasury payment id attached yet (clean start), and no leaked
  payment material (lightning invoice, BOLT12 offer, lightning address,
  preimage, or payment hash) in any public-facing field. Emits a public-safe
  `candidateSummary` (rewardId, state, amountSats, receiptRef only). It moves no
  funds and flips no state.
- `apps/openagents.com/workers/api/src/x-claim-reward-smoke-candidate.test.ts`
  — covers the clean pass, each blocking reason, every leaked-material pattern,
  and a no-secret assertion on the serialized candidate summary.
- `apps/openagents.com/docs/2026-06-09-x-claim-reward-dispatch-runbook.md`
  — a "Candidate pre-dispatch gate" section wiring the gate in before the
  `approve_dispatch` step.

## Follow-up: settlement-evidence pre-persistence gate (this run)

The candidate gate inspects the `eligible` row before dispatch and the
post-settlement audit inspects the `settled` row after the fact, but the
evidence refs the operator passes to `mark_settled` were only checked for
non-emptiness before being written to the public ledger — a malformed or leaky
ref would have been PERSISTED and only caught after landing. This run adds the
missing middle bookend that validates the settle input before persistence, and
wires it into the dispatch route so leaky material never reaches the row.

- `apps/openagents.com/workers/api/src/x-claim-reward-settlement-evidence.ts`
  — `assertXClaimRewardSettlementEvidenceRefs(evidenceRefs)` plus the
  `XClaimRewardSettlementEvidenceGate` / `XClaimRewardSettlementEvidenceCheck`
  types. Checks: at least one `settlement_evidence.public.*` ref present (also
  rejects empty) and no leaked payment material (lightning invoice, BOLT12
  offer, lightning address, preimage, or payment hash) in any submitted ref. It
  returns trimmed, deduped `acceptedRefs` for safe persistence.
- `apps/openagents.com/workers/api/src/agent-owner-claim-routes.ts`
  — the `mark_settled` branch of the dispatch route now runs the gate before
  persistence, returns HTTP 400 with `blockingReasonRefs` when it fails, and
  stores only the gate's `acceptedRefs`.
- `apps/openagents.com/workers/api/src/x-claim-reward-settlement-evidence.test.ts`
  — covers the clean pass, trim/dedupe, empty rejection, missing-public-ref
  rejection, every leaked-material pattern, and a no-payment-material assertion
  on the serialized gate.
- `apps/openagents.com/docs/2026-06-09-x-claim-reward-dispatch-runbook.md`
  — the `mark_settled` step documents the pre-persistence gate.

## Follow-up: worker-side dispatch run outcome auditor (this run)

The candidate gate, settlement-evidence gate, and post-settlement receipt audit
all inspect ledger *rows*; the aggregate preflight inspects *stats* before the
run. But when the smoke runs through the flag-gated worker-side dispatcher
(`runXClaimRewardTreasuryDispatch`), nothing audited the
`XClaimRewardTreasuryDispatchSummary` it returns — i.e. whether the run itself
did exactly the bounded single-reward smoke and nothing more. This run adds that
missing run-level gate.

- `apps/openagents.com/workers/api/src/x-claim-reward-smoke-dispatch-outcome.ts`
  — `assertXClaimRewardSmokeDispatchOutcome(summary)` plus the
  `XClaimRewardSmokeDispatchOutcomeReport` / `XClaimRewardSmokeDispatchOutcomeCheck`
  types. Checks: dispatch flag was on, exactly one reward settled, no reward
  failed, no payment left pending, the dispatch queue drained (no residual
  `dispatch_requested` or pending-payment rows), and the run skipped nothing
  (no liquidity/daily-cap stop). Emits a public-safe `outcomeSummary` (aggregate
  counters and skip-reason refs only). It moves no funds and flips no state.
- `apps/openagents.com/workers/api/src/x-claim-reward-smoke-dispatch-outcome.test.ts`
  — covers the clean fresh-dispatch pass, the pending-payment polling pass, each
  blocking reason, and a no-payment-material assertion on the serialized summary.
- `apps/openagents.com/docs/2026-06-09-x-claim-reward-dispatch-runbook.md`
  — a "Worker-side dispatch run outcome audit" section wiring the auditor in
  before the per-row post-settlement receipt audit for flag-gated smoke runs.

## Follow-up: composite run+row smoke completion gate (this run)

The run-level outcome auditor inspects the dispatch *summary* and the
transition-request builder inspects the settled *row*, but nothing required BOTH
before proposing the green flip. That left a hole: a worker-side run that settled
the wrong number of rewards, left a payment pending, or skipped on
liquidity/daily-cap could still produce a green transition proposal as long as
the single inspected row happened to look clean. This run adds the composite gate
that closes that hole.

- `apps/openagents.com/workers/api/src/x-claim-reward-smoke-completion.ts`
  — `assertXClaimRewardSmokeCompletion({ summary, reward })` plus the
  `XClaimRewardSmokeCompletionReport` / `XClaimRewardSmokeCompletionInput` types.
  It runs `assertXClaimRewardSmokeDispatchOutcome(summary)` (run-level) AND
  `buildXClaimRewardSmokeTransitionRequest(reward)` (row-level) and emits the
  public-safe `transitionRequest` only when BOTH pass; `blockingReasonRefs` is
  the deduped union of both gates' reasons. It moves no funds and flips no state.
- `apps/openagents.com/workers/api/src/x-claim-reward-smoke-completion.test.ts`
  — covers the both-pass emit, run-not-clean withhold (clean row), pending-payment
  withhold, settled-row-not-ready withhold (clean run), both-fail aggregation, and
  a no-payment-material assertion on the serialized report.
- `apps/openagents.com/docs/2026-06-09-x-claim-reward-dispatch-runbook.md`
  — a "Smoke completion gate (run + row, one go/no-go)" section directing the
  operator to use this composite instead of the row-only builder.

## Follow-up: wire the preflight evaluator into the operator status route (this run)

Every smoke gate above was a pure function exercised only by tests — including
`evaluateXClaimRewardSmokePreflight`, which the runbook told the operator to feed
by hand from `GET /api/operator/treasury/status`. That hand-wiring was the last
manual, error-prone step before arming the live run. This run closes it by
computing the preflight inside the status route itself.

- `apps/openagents.com/workers/api/src/treasury-routes.ts`
  — `handleOperatorTreasuryStatusApi` now evaluates
  `evaluateXClaimRewardSmokePreflight` from the wallet's max sendable balance and
  the `rewardDispatch` stats and returns it as `rewardDispatchSmokePreflight`
  (omitted when no dispatch-stats reader is wired). It reuses the existing
  `balancePayload` parser and emits only the public-safe report (`checks`,
  `blockingReasonRefs`, `ready`) — never the balance figure, an invoice, a
  destination, or a preimage. It moves no funds and flips no state.
- `apps/openagents.com/workers/api/src/treasury-routes.test.ts`
  — adds an `operator treasury status` cases for the ready preflight (with a
  no-balance-figure assertion on the serialized report), a blocking preflight
  when flag/caps/pending fail, and omission when no stats reader is wired.
- `apps/openagents.com/docs/2026-06-09-x-claim-reward-dispatch-runbook.md`
  — the "Preflight readiness gate" section now documents the inline
  `rewardDispatchSmokePreflight` field and the `ready` go signal.

## What remains

The blocker is still open: an operator must run the live single-reward smoke —
enable the flag with the preflight green, dispatch one eligible reward to a real
owner receive code, let it settle with public-safe receipt refs, run the
post-settlement audit, and record the transition receipt on issue #4626. Only
then does this blocker clear and the promise become eligible for a green flip.
