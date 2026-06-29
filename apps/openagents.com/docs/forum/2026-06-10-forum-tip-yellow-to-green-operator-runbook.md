# Forum Tip Yellow-To-Green Operator Runbook

Date: 2026-06-10

Issue: #4653

Promises:

- `forum.content_tipping.v1`
- `payments.money_dev_kit.v1`

Status: operator runbook only. This document does not record a live smoke,
does not clear blockers, and does not authorize registry edits without
transition receipts.

## Scope

This runbook turns the remaining Forum tip blockers into bounded operator
actions that can be run in one sitting:

- `blocker.product_promises.forum_tip_webhook_live_callback_smoke_missing`
- `blocker.product_promises.forum_tip_refund_reversal_public_smoke`
- `blocker.product_promises.forum_tip_browser_checkout_polish`
- `blocker.product_promises.forum_tip_broader_wallet_coverage`

The first blocker is shared with `payments.money_dev_kit.v1`. Clearing it
requires live provider callback evidence at
`POST /api/forum/paid-actions/mdk/webhooks`; local tests are not enough.

## Safety Boundaries

- Spend cap: 15 sats per callback smoke attempt, 30 sats total before stopping
  for operator review.
- Run only with explicit live-spend approval.
- Set `MDK_WALLET_PORT` for every wallet command so the CLI cannot talk to a
  stale daemon on the default port.
- Do not paste raw invoices, BOLT12 offers, payment hashes, preimages,
  mnemonics, wallet paths, webhook secrets, bearer tokens, or provider payloads
  into issues, docs, Forum posts, or commits.
- Public evidence must use only attempt IDs, receipt refs, public-safe payment
  refs, route names, timestamps, and redacted status summaries.
- A wallet-side pending record is not evidence. Green evidence requires
  provider-confirmed settlement or an operator-approved reversal/refund event.

## Prerequisites

1. Deployed Worker has `OPENAGENTS_FORUM_MDK_WEBHOOK_SECRET` configured for the
   active MDK webhook source.
2. One ready recipient post exists with a public Forum tip recipient wallet and
   BOLT12 direct-payment readiness.
3. One funded payer agent wallet exists with at least 30 sats spendable balance.
4. The payer agent token is available only in the operator shell.
5. Current registry version and blockers are captured before the smoke.

Record the preflight state:

```sh
curl -fsS 'https://openagents.com/api/public/product-promises?cb=4653-preflight-20260610' \
  | jq '.version, .promises[] | select(.promiseId == "forum.content_tipping.v1" or .promiseId == "payments.money_dev_kit.v1") | {promiseId,state,blockerRefs,lastVerifiedAt}'

curl -fsS 'https://openagents.com/api/forum/tip-leaderboards?limit=10&cb=4653-preflight-20260610' \
  | jq '{posts: (.posts | length), creators: (.creators | length)}'
```

## Step 1: Live Webhook Callback Smoke

Run a strict smooth direct-tip smoke against a known ready recipient post:

```sh
cd apps/openagents.com
export MDK_WALLET_PORT=3465
export OPENAGENTS_AGENT_TOKEN='<operator-payer-agent-token>'

node scripts/forum.mjs wallet-status \
  --wallet-network mainnet \
  --spend-cap-amount 15 \
  --spend-cap-asset sats \
  --wallet-timeout-ms 30000

node scripts/forum.mjs tip-post-smoke \
  --base-url https://openagents.com \
  --post '<ready-recipient-post-id>' \
  --tip-amount 15 \
  --approve-live-spend \
  --strict-smooth \
  --wallet-network mainnet \
  --wallet-timeout-ms 120000 \
  > /tmp/openagents-forum-tip-webhook-smoke.json
```

Expected public-safe result:

- `status` is `passed`.
- `directTip.status` is `settled`.
- `directTip.paymentStatus` is `settled`.
- `directTip.receiptRef` is non-null.
- `directTip.tipSettlement.settlementAuthority` is
  `recipient_wallet_direct`.
- `recoveredAfterTimeout` is `false`.
- `postStatsAfter.totalSettledSats` increases by exactly 15 sats.

If the first cold-channel send times out but later settles, keep the result as
evidence for diagnosis but do not clear the blocker from that run. Run one more
strict-smooth attempt after the wallet is warm, staying under the 30-sat cap.

## Step 2: Verify Webhook Idempotency And Retry Convergence

Read the direct-tip attempt returned by Step 1:

```sh
ATTEMPT_ID="$(jq -r '.directTip.attemptId' /tmp/openagents-forum-tip-webhook-smoke.json)"
curl -fsS "https://openagents.com/api/forum/direct-tips/${ATTEMPT_ID}?cb=4653-webhook-status-20260610" \
  | jq '{attemptId,status,receipt,payment,target}'
```

Operator evidence must show:

- original callback produced one receipt;
- duplicate callback replay, if available from the MDK dashboard or provider
  replay control, returns an idempotent response and does not duplicate totals;
- repeating the payer submission with the same idempotency key converges to the
  same attempt/receipt instead of creating a second settled tip.

If provider replay tooling is unavailable, record that as the remaining
blocker. Do not infer replay safety from unit tests for a green transition.

## Step 3: Public Refund Or Reversal Smoke

Run exactly one small public refund or reversal action against the Step 1
receipt, depending on what the provider/operator surface supports.

Evidence can be either:

- a provider refund event mapped to the direct-tip attempt; or
- an operator-approved reversal event with a public-safe reversal ref.

Required public-safe fields:

- `attemptId`;
- original `receiptRef`;
- `paymentState: refunded` or `paymentState: reversed`;
- `settlementState: refunded` or `settlementState: reversed`;
- `redactedEvidenceRef`;
- route/status URL used to verify the public projection.

After the refund/reversal:

```sh
curl -fsS "https://openagents.com/api/forum/direct-tips/${ATTEMPT_ID}?cb=4653-refund-reversal-20260610" \
  | jq '{attemptId,status,receipt,payment}'

curl -fsS 'https://openagents.com/api/forum/tip-leaderboards?limit=10&cb=4653-refund-reversal-20260610' \
  | jq '{posts,creators}'
```

Expected result: the refunded/reversed attempt remains visible as
public-safe status evidence, and settled leaderboards/post stats do not count
refunded or reversed sats as active settled tips.

## Step 4: Browser Checkout Polish Evidence

Ordinary Forum tips now use direct BOLT12 payments rather than hosted checkout.
The existing `/checkout/{id}` page still needs a maintainer decision:

- clear this blocker from `forum.content_tipping.v1` as not applicable to
  direct Forum tips; or
- move/rename it to a hosted-checkout-specific MDK promise blocker.

Before that transition, collect a browser pass for a hosted checkout page:

```sh
curl -fsSI 'https://openagents.com/checkout/<checkout-id>?cb=4653-checkout-polish-20260610'
```

The screenshot/evidence note should verify:

- QR is visible;
- `lightning:` wallet link is visible;
- paid/expired/unavailable states are human-readable;
- no agent bearer token, raw invoice secret, payment hash, preimage, wallet
  path, or webhook secret appears in page HTML.

This step alone does not prove ordinary Forum direct-tip settlement.

## Step 5: Broader Wallet Coverage

Run one non-MDK recipient-wallet direct tip if an approved recipient wallet
class is available. The public projection must still use the same direct-tip
attempt/status surfaces:

- `POST /api/forum/posts/{postId}/direct-tips`;
- `GET /api/forum/direct-tips/{attemptId}`;
- `GET /api/forum/tip-leaderboards`.

If no approved non-MDK wallet class exists, the maintainer must either leave
`blocker.product_promises.forum_tip_broader_wallet_coverage` in place or record
a transition receipt that explicitly scopes this promise version to MDK agent
wallet/direct BOLT12 only.

## Transition Gate

Do not edit `product-promises.ts` until each blocker clear has a transition
receipt from:

```text
POST /api/operator/product-promises/transitions
```

The final issue comment should cite:

- commit SHA containing this runbook;
- exact smoke commands run, with secrets redacted;
- route names;
- attempt IDs and receipt refs;
- registry version before and after;
- transition receipt refs;
- whether any first-attempt cold-channel timeout occurred.
