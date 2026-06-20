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

5. Failures: `{"action":"mark_failed","stateReasonRef":"reason.public.<why>"}`.
   Refusals (fraud, duplicate human, policy): `{"action":"refuse", ...}`.

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
