# X-Claim Reward Dispatch Runbook

Date: 2026-06-09
Updated: 2026-06-10 for the tweet-first claim flow in issue #4688.

Scope: operator dispatch of the promotional 1000-sat X owner-claim reward
(issue #4626, promise `agents.x_claim_reward.v1`). This runbook moves one
eligible reward through dispatch to settled with public-safe evidence. It does
not authorize Forum tip settlement, accepted-work payouts, or Treasury
movement.

## How eligibility appears

When `POST /api/agents/claims/{claimId}/x/verify` succeeds, the worker
verifies a friendly public tweet:

```text
Verifying my agent {displayName} is joining @OpenAgents

Code: {nonce}
```

The verifier binds the X account from the public tweet author. Old-format
tweets containing the nonce plus claim URL remain accepted during the
transition window. After verification, the worker records one reward row in
`x_claim_reward_ledger`:

- deduped by X account and by challenge (one reward per X account, ever);
- `state: eligible` (or `refused` with
  `reason.public.x_claim_reward_campaign_budget_exhausted` when the campaign
  cap is reached);
- `amountSats: 1000`, `receiptRef: x_claim_reward_receipt_*`.

The verify response includes the public-safe reward projection, so the
claiming owner sees eligibility immediately.

## Hard rules

- Never paste invoices, BOLT12 offers, preimages, mnemonics, or wallet paths
  into issue comments, Forum posts, or this repo. Evidence refs only.
- `mark_settled` requires public-safe settlement evidence refs; the route
  rejects it otherwise.
- One reward per X account is enforced by the ledger; do not work around it.
- The campaign wallet is a bounded marketing wallet, not the Forum tip payer,
  the edge wallet, or Treasury.

## Prepared operator smoke

This smoke is intentionally prepared but not executed by agents. It requires a
bounded operator wallet with at least 1000 sats plus fees, a current admin API
token, and one already-eligible reward row produced by the verified tweet
flow. Use exactly one reward for the first green-path smoke.

Set local shell variables without printing the token:

```bash
export OPENAGENTS_BASE_URL="https://openagents.com"
export OPENAGENTS_ADMIN_TOKEN="<redacted admin token>"
export X_CLAIM_REWARD_ID="x_claim_reward_..."
export X_CLAIM_SETTLEMENT_REF="settlement_evidence.public.mdk_campaign_wallet.x_claim_reward_YYYYMMDD_001"
```

Use the `reward.rewardId` returned by
`POST /api/agents/claims/{claimId}/x/verify`, or the matching operator DB row,
as `X_CLAIM_REWARD_ID`. Expected precondition:

- `state: "eligible"`
- `amountSats: 1000`
- `receiptRef: "x_claim_reward_receipt_*"`
- no raw invoice, preimage, payment hash, mnemonic, wallet path, or payout
  target in the response.

## Preflight readiness gate

Before enabling the live run, confirm the bounded wallet and ledger are clean
with the pure preflight evaluator
(`evaluateXClaimRewardSmokePreflight` in
`apps/openagents.com/workers/api/src/x-claim-reward-treasury-dispatcher.ts`).
It moves no funds and emits only aggregate, public-safe checks. The smoke is
ready only when every check passes:

- `dispatch_flag_enabled` — `TREASURY_DISPATCH_ENABLED=true`.
- `per_run_cap_allows_one` — per-run reward cap is at least 1.
- `exactly_one_approved_reward` — exactly one row in `dispatch_requested`
  (the first green-path smoke uses exactly one reward).
- `no_pending_payment_in_flight` — no `dispatched` row already has a treasury
  payment id, so the smoke starts clean.
- `daily_cap_headroom` — the daily sats cap leaves room for one 1000-sat reward.
- `treasury_liquidity_sufficient` — the campaign wallet's max sendable balance
  covers 1000 sats plus the liquidity buffer.

Feed it the aggregate dispatch stats from
`GET /api/operator/treasury/status` (`rewardDispatch`) plus the wallet's max
sendable balance; a non-empty `blockingReasonRefs` means do not start the live
smoke until each listed reason clears.

As of the 2026-06-20 wiring, the operator no longer needs to run the evaluator
by hand: `GET /api/operator/treasury/status` now computes the preflight inline
and returns it as `rewardDispatchSmokePreflight` whenever the dispatch-stats
reader is wired (it is omitted otherwise). The embedded report is public-safe —
it carries only the named `checks`, `blockingReasonRefs`, and the `ready`
boolean, never the balance figure, an invoice, a destination, or a preimage.
Start the live smoke only when `rewardDispatchSmokePreflight.ready` is `true`.

## Candidate pre-dispatch gate (per reward)

After the aggregate preflight is green, confirm the *specific* reward row you
picked for the smoke is a clean starting point with the pure per-row gate
(`assertXClaimRewardSmokeCandidate` in
`apps/openagents.com/workers/api/src/x-claim-reward-smoke-candidate.ts`). It is
the front bookend that complements the post-settlement receipt audit: it moves
no funds and passes only when the row is the right candidate to `approve_dispatch`:

- `state_is_eligible` — the row is still in `eligible` (not already approved,
  dispatched, settled, refused, or failed).
- `amount_is_campaign_reward` — `amountSats` is the bounded 1000-sat reward.
- `receipt_ref_well_formed` — `receiptRef` matches `x_claim_reward_receipt_*`.
- `no_treasury_payment_attached` — no `treasuryPaymentId` is attached yet, so the
  smoke starts clean.
- `no_payment_material_leaked` — no invoice, BOLT12 offer, lightning address,
  preimage, or payment hash appears in any public-facing field.

A non-empty `blockingReasonRefs` means do not run `approve_dispatch` on this row
until each listed reason clears. The public-safe `candidateSummary` (rewardId,
state, amountSats, receiptRef only) is safe to paste into issue #4626.

## Dispatch flow (per reward)

All calls use the worker admin API token as the bearer.

1. Approve dispatch:

   ```bash
   curl -fsS -X POST "$OPENAGENTS_BASE_URL/api/agents/claims/rewards/$X_CLAIM_REWARD_ID/dispatch" \
     -H "Authorization: Bearer $OPENAGENTS_ADMIN_TOKEN" \
     -H "Content-Type: application/json" \
     -d '{"action":"approve_dispatch"}'
   ```

   Expected row state: `dispatch_requested`.

2. Pay 1000 sats from the campaign wallet to the owner-provided receive code
   (collect the receive code out of band; never store it in the ledger).
   Confirm the payment reaches `completed` in the campaign wallet history.

   Funding amount: exactly `1000` sats to the owner-provided receive
   destination, plus network/routing fees paid by the campaign wallet. Do not
   paste the receive code, invoice, payment hash, preimage, wallet path, or
   raw wallet log into GitHub, Forum, docs, or issue comments.

3. Mark dispatched:

   ```bash
   curl -fsS -X POST "$OPENAGENTS_BASE_URL/api/agents/claims/rewards/$X_CLAIM_REWARD_ID/dispatch" \
     -H "Authorization: Bearer $OPENAGENTS_ADMIN_TOKEN" \
     -H "Content-Type: application/json" \
     -d '{"action":"mark_dispatched"}'
   ```

   Expected row state: `dispatched`.

4. Settle with evidence:

   ```bash
   curl -fsS -X POST "$OPENAGENTS_BASE_URL/api/agents/claims/rewards/$X_CLAIM_REWARD_ID/dispatch" \
     -H "Authorization: Bearer $OPENAGENTS_ADMIN_TOKEN" \
     -H "Content-Type: application/json" \
     -d "{\"action\":\"mark_settled\",\"evidenceRefs\":[\"$X_CLAIM_SETTLEMENT_REF\"]}"
   ```

   Expected row state: `settled`, with `evidenceRefs` containing only the
   public-safe settlement ref.

   The dispatch route validates the submitted `evidenceRefs` BEFORE persisting
   them with the pure pre-persistence gate
   (`assertXClaimRewardSettlementEvidenceRefs` in
   `apps/openagents.com/workers/api/src/x-claim-reward-settlement-evidence.ts`).
   It is the middle bookend between the candidate gate and the post-settlement
   audit: it rejects `mark_settled` (HTTP 400, `blockingReasonRefs` listed) when
   the refs carry no `settlement_evidence.public.*` ref or leak any payment
   material (invoice, BOLT12 offer, lightning address, preimage, or payment
   hash), so leaky material never lands on the public ledger row in the first
   place. It trims and dedupes the accepted refs before they are stored.

5. Failures: `{"action":"mark_failed","stateReasonRef":"reason.public.<why>"}`.
   Refusals (fraud, duplicate human, policy): `{"action":"refuse", ...}`.

## Worker-side dispatch run outcome audit

If the smoke runs through the flag-gated worker-side dispatcher
(`runXClaimRewardTreasuryDispatch`, behind `TREASURY_DISPATCH_ENABLED=true`)
rather than the manual curl flow above, audit the run's returned summary with
the pure outcome auditor (`assertXClaimRewardSmokeDispatchOutcome` in
`apps/openagents.com/workers/api/src/x-claim-reward-smoke-dispatch-outcome.ts`).
It complements the per-row post-settlement audit: that one confirms the settled
*record* is clean, while this confirms the *run* did exactly the bounded smoke.
It moves no funds and reads only the summary's aggregate counters. It passes
only when:

- `dispatch_run_enabled` — the run executed with the dispatch flag on.
- `exactly_one_settled` — exactly one reward reached `settled`.
- `no_reward_failed` — no reward failed during the run.
- `no_payment_pending` — no payment was left pending.
- `dispatch_queue_drained` — no `dispatch_requested` or pending-payment rows
  remain (`pending`, `stats.pendingPaymentCount`, and
  `stats.requestedDispatchCount` are all zero).
- `no_skipped_reasons` — the run skipped nothing (no liquidity or daily-cap
  stop).

A non-empty `blockingReasonRefs` means the run did not complete a clean
single-reward smoke; resolve each reason before running the per-row audit. The
public-safe `outcomeSummary` (aggregate counters and skip-reason refs only) is
safe to paste into issue #4626.

## Post-settlement receipt audit

Before recording the smoke as complete, run the pure post-settlement auditor
(`auditXClaimRewardSmokeReceipt` in
`apps/openagents.com/workers/api/src/x-claim-reward-smoke-receipt-audit.ts`) on
the settled reward row. It is the after-the-fact counterpart to the preflight
gate: it moves no funds and confirms the row is public-safe before the
transition receipt is published. The audit passes only when:

- `state_is_settled` — the row reached `settled`.
- `amount_is_campaign_reward` — `amountSats` is the bounded 1000-sat reward.
- `receipt_ref_well_formed` — `receiptRef` matches `x_claim_reward_receipt_*`.
- `settlement_evidence_present` — at least one `settlement_evidence.public.*`
  evidence ref is recorded.
- `no_payment_material_leaked` — no invoice, BOLT12 offer, lightning address,
  preimage, or payment hash appears in any public-facing field.

A passing audit returns `transitionReceiptSummary` — a public-safe object
(rewardId, receiptRef, state, amountSats, public settlement evidence refs only;
never the treasury payment id or destination) suitable for pasting into the
issue #4626 transition receipt. A non-empty `violationReasonRefs` means do not
publish the receipt until each listed reason clears.

## Transition-receipt proposal

Once the post-settlement audit passes, assemble the registry proposal with the
pure builder (`buildXClaimRewardSmokeTransitionRequest` in
`apps/openagents.com/workers/api/src/x-claim-reward-smoke-receipt-audit.ts`). It
runs the audit and, only when it passes, returns a public-safe
`transitionRequest` — the exact `POST /api/operator/product-promises/transitions`
body (`promiseId`, `toState: "green"`, deduped public evidence refs: the receipt
ref plus public settlement evidence refs). It defensively re-scans the assembled
refs for payment material and emits `transitionRequest: null` with
`blockingReasonRefs` if anything leaks or the audit fails.

Building this proposal flips no promise state and moves no funds: the registry
route re-evaluates blockers on submit, and the green flip still requires owner
sign-off. A non-empty `blockingReasonRefs` means do not submit until each listed
reason clears.

## Smoke completion gate (run + row, one go/no-go)

For the flag-gated worker-side path, run the composite completion gate
(`assertXClaimRewardSmokeCompletion` in
`apps/openagents.com/workers/api/src/x-claim-reward-smoke-completion.ts`) instead
of calling the transition-receipt builder directly. The standalone builder only
inspects the settled *row*; this gate also requires the worker-side dispatch
*run* to have been a clean bounded single-reward smoke before it will emit the
transition request. That closes the hole where a run that settled the wrong
number of rewards, left a payment pending, or skipped on liquidity/daily-cap
could still produce a green proposal as long as the single inspected row looked
clean.

Feed it `{ summary, reward }` — the `runXClaimRewardTreasuryDispatch` summary plus
the settled reward row. It runs the run-level outcome audit
(`assertXClaimRewardSmokeDispatchOutcome`) AND the per-row transition builder
(`buildXClaimRewardSmokeTransitionRequest`), and emits `transitionRequest` only
when BOTH pass. A non-empty `blockingReasonRefs` (the union of run-level and
row-level reasons) means do not submit until each listed reason clears. It moves
no funds and flips no promise state.

After the settled response, verify the public promise remains honest until the
operator records the transition receipt:

```bash
curl -fsS "$OPENAGENTS_BASE_URL/api/public/product-promises?cb=x-claim-smoke-$(date +%s)" \
  | jq '.promises[] | select(.id=="agents.x_claim_reward.v1") | {id,state,blockers,lastVerifiedAt}'
```

## Promise gate

`agents.x_claim_reward.v1` stays yellow until one live reward settles through
this flow with public-safe receipt refs; record that run on issue #4626 and
propose the registry update with a transition receipt
(`POST /api/operator/product-promises/transitions`).
