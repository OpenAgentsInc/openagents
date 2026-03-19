# Data Market Paid Buyer Access Audit

Date: 2026-03-18

## Scope

This audit covers the paid Data Market buyer-access path after the earlier Psionic sample sale audit exposed that the flow was not actually proven end to end. The goal for this pass was stricter:

- fund the buyer wallet with a real external Lightning payment
- publish a priced dataset
- publish a targeted grant
- publish a buyer request
- let the buyer pay the seller invoice
- have the seller detect settlement without manual wallet refresh
- issue the delivery bundle
- have the buyer observe the result event
- consume the delivery locally
- byte-verify the consumed payload against the source dataset

This audit also answers the control-plane question that came up during debugging: where `nexus-control` fits, what “hosted control” meant, and where Nostr is actually used.

## Short answer

The full paid buyer-access flow now works.

The successful proof run was `live6` under:

- `target/headless-data-market-e2e/live6-summary.json`
- `target/headless-data-market-e2e/consumed-dataset-live6/payload`

Successful final IDs:

- asset id:
  `data_asset.npub1q4pr7lmyp3q5e8z98et4d95h40h0rvj0zn8qlvnwcaad0f7gce8qxp36yg.conversation_bundle.Headless_Dummy_Dataset_Live6.sha256_c0e0cf661545f117bd4e0611531b758f89e09d3c62fc3c8aaa4eebe16114299f`
- grant id:
  `access_grant.npub1q4pr7lmyp3q5e8z98et4d95h40h0rvj0zn8qlvnwcaad0f7gce8qxp36yg.data_asset.npub1q4pr7lmyp3q5e8z98et4d95h40h0rvj0zn8qlvnwcaad0f7gce8qxp36yg.conversation_bundle.Headless_Dummy_Dataset_Live6.sha256_c0e0cf661545f117bd4e0611531b758f89e09d3c62fc3c8aaa4eebe16114299f.targeted_request.npub14r37gq524phnd9wewa9edac5gm8muwyf5d898acjq59dy9z94paq0juc7k`
- request id:
  `b945637edbe346ad46bddfd9f4f5aa7914433063db6a50138b9b169df191e3f4`
- seller payment pointer:
  `019d0488-c8f6-7ca4-8c5f-fe20157221b3`
- buyer result event id:
  `f93a429a9dde6af76779739b1fdaf13b2fcbf315e468b23b67f3bcfe6da076f6`

The buyer consumed the delivery into:

- `target/headless-data-market-e2e/consumed-dataset-live6/payload`

The consumed files matched the original packaged source dataset byte for byte.

## What was actually broken

There were multiple distinct failures, not one.

### 1. Buyer payment logic assumed result-before-payment

The current Data Market MVP is pay-before-delivery. The buyer request should accept a valid `payment-required` invoice before any result event exists.

Before the fix:

- buyer auto-payment selection still assumed the normal compute race shape
- invoice-only Data Market feedback could arrive without a result
- that prevented a valid seller invoice from becoming payable early enough

Fix:

- `apps/autopilot-desktop/src/state/operations.rs`
- Data Market requests now allow invoice-ready payment selection before the result event exists

Regression:

- `network_requests_data_market_accepts_payment_required_invoice_before_result`

### 2. Seller settlement detection depended on manual wallet refresh

The first live proof showed:

- seller published the `payment-required` invoice correctly
- buyer paid it correctly
- seller balance only advanced after a manual wallet refresh
- seller did not automatically transition the request into the paid state

That was the central reason the earlier proof was not good enough.

Fix:

- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/input/actions.rs`
- `apps/autopilot-desktop/src/input.rs`

The seller pane now keeps a payment-evidence refresh timer while a request is in `AwaitingPayment` and no `payment_pointer` has been observed yet. That queues wallet refreshes automatically until the seller sees the settled Lightning receive.

Regression:

- `data_seller_payment_watchdog_retries_until_payment_observed`

### 3. Seller request expiry hid useful state

When a request expired after partial progress, the pane collapsed the explanation into a generic “expired before seller evaluation” summary.

That made debugging much harder because it erased whether the request had:

- matched an asset
- matched a grant
- reached payment-related state

Fix:

- `apps/autopilot-desktop/src/app_state.rs`

Expired requests now preserve matched context in the summary.

Regression:

- `data_seller_expired_request_preserves_matched_context_and_payment_failure`

### 4. Spark invoice creation needed transient retry

During live runs, Spark-side invoice creation could fail transiently.

Fix:

- `apps/autopilot-desktop/src/spark_wallet.rs`

The wallet layer now retries transient invoice-creation failures before surfacing a hard error.

Regressions:

- `run_with_transient_retry_retries_networkish_errors_before_succeeding`
- `run_with_transient_retry_does_not_retry_non_transient_errors`

### 5. Seller publish replay hit idempotency conflicts

Repeated publish attempts against the same asset/grant identity could come back as kernel idempotency conflicts.

Fix:

- `apps/autopilot-desktop/src/data_seller_control.rs`

On kernel idempotency conflict, the seller publish path now rehydrates the existing authority object instead of failing the flow outright.

Regression:

- `detects_kernel_idempotency_conflicts`

### 6. Buyer dropped result events after payment

This was the last blocker that kept buyer access from being fully proven.

What happened:

- buyer successfully paid the seller invoice
- seller issued the delivery bundle and published a NIP-90 `result`
- buyer logs clearly showed the `result` event arriving
- but `buyer-status` still showed:
  - `status=paid`
  - `last_result_event_id=null`
  - `winning_result_event_id=null`

Root cause:

- `Paid` was treated as terminal for buyer response handling
- result events arriving after payment were ignored by the reducer
- even when the winner was already known, `winning_result_event_id` was not backfilled

Fix:

- `apps/autopilot-desktop/src/state/operations.rs`

Changes:

- buyer feedback/result reducers now allow post-payment processing for `Paid` requests
- post-payment result handling now records `last_result_event_id`
- if the result belongs to the already-selected winning provider, it also records `winning_result_event_id`

Regression:

- `network_requests_data_market_accepts_result_after_buyer_payment`

## What “hosted control” meant, and why this is not “instead of Nostr”

The earlier mention of “hosted control” was not saying the Data Market requires some hosted web service instead of Nostr.

The actual split today is:

- Nostr carries the NIP-90 request, feedback, and result events
- the control/authority plane owns the current kernel-backed market objects and desktop session auth

In practice, for the current MVP:

- assets
- grants
- delivery bundles
- receipts
- desktop control sessions

are projected through the control plane.

The successful proof in this audit did not use any hosted OpenAgents control service. It used:

- local `nexus-control`
- local relay transport
- local headless seller runtime
- local headless buyer runtime

So the correct statement is:

- Nostr is already in the flow for transport
- the current MVP is not fully “Nostr-only” for market authority
- “hosted control” was only one possible authority endpoint, not a hard requirement for the proof

Why the earlier desktop session hit that wording:

- the desktop app instance on this machine was not configured with a hosted control endpoint for its desktop-owned control APIs
- that did not block local headless proof
- we switched to the local authority path and proved the full flow there

## Funding step

Before the final paid proof, the buyer wallet was funded externally by paying a Lightning invoice generated from the local buyer wallet.

That external funding was separate from the dataset purchase itself.

The priced Data Market purchase in the successful run was still the seller’s own 5-sat invoice:

- invoice amount: `5000 msats`
- seller observed amount: `5 sats`
- seller balance advanced from `10 sats` to `15 sats`

## Successful live proof

### Environment

Local control and relay path:

- authority base: `http://127.0.0.1:40163`
- relay: `ws://127.0.0.1:48767`

Live manifests:

- seller manifest:
  `target/headless-data-market-e2e/seller-desktop-control.json`
- buyer manifest:
  `target/headless-data-market-e2e/buyer-desktop-control.json`

Seller and buyer homes:

- `target/headless-data-market-e2e/seller-home`
- `target/headless-data-market-e2e/buyer-home`

### Important rerun note

The first rebuilt proof rerun (`live5`) still failed, but for a different reason:

- it reused the earlier asset/grant identity
- the seller matched the request to a previously delivered grant
- seller evaluation moved into `grant_required` and then expired

That was not a payment or buyer-result bug anymore.

The successful proof (`live6`) used the same dataset bytes but changed the listing title and provenance suffix so the asset and grant IDs were fresh:

- title: `Headless Dummy Dataset Live6`
- provenance suffix: `/live6`

That minted a fresh asset/grant pair for the same buyer.

### Successful sequence

1. Seller drafted and published the fresh `live6` asset.
2. Seller drafted and published the matching targeted grant.
3. Buyer refreshed market state and published the targeted request.
4. Seller evaluated the request as `ready_for_payment_quote`.
5. Seller published a `payment-required` Lightning invoice.
6. Buyer paid the invoice automatically.
7. Seller auto-refreshed wallet evidence and observed settlement without manual intervention.
8. Seller prepared and issued the delivery bundle.
9. Seller published the NIP-90 result event:
   `f93a429a9dde6af76779739b1fdaf13b2fcbf315e468b23b67f3bcfe6da076f6`
10. Buyer recorded the result event in `buyer-status`.
11. Buyer resolved and consumed the delivery bundle locally.
12. Consumed payload bytes matched the original dataset.

### Final successful artifacts

Key run files:

- `target/headless-data-market-e2e/live6-publish-asset.json`
- `target/headless-data-market-e2e/live6-publish-grant.json`
- `target/headless-data-market-e2e/live6-buyer-request.json`
- `target/headless-data-market-e2e/live6-request-payment.json`
- `target/headless-data-market-e2e/live6-seller-payment-settled.json`
- `target/headless-data-market-e2e/live6-issue-delivery.json`
- `target/headless-data-market-e2e/live6-buyer-result.json`
- `target/headless-data-market-e2e/live6-consume-delivery.json`
- `target/headless-data-market-e2e/live6-summary.json`

## How a buyer agent can buy and access the dataset

For the current MVP, assume the buyer agent is pointed at the first-party Data Market control surface rather than trying to infer the protocol from raw Nostr alone.

The minimal agent path is:

1. Refresh the buyer market snapshot.
2. Select the target asset.
3. Publish the targeted access request.
4. Wait for `payment-required`.
5. Pay the seller invoice.
6. Wait until `buyer-status.latest_request.last_result_event_id` is non-null.
7. Resolve and consume the delivery locally.

In `autopilotctl`, the concrete path is:

```bash
autopilotctl --manifest ./buyer-desktop-control.json --json data-market buyer-refresh
autopilotctl --manifest ./buyer-desktop-control.json --json data-market buyer-publish-request --asset-id <asset_id> --refresh-market
autopilotctl --manifest ./buyer-desktop-control.json --json data-market buyer-status
autopilotctl --manifest ./buyer-desktop-control.json --json data-market consume-delivery --request-id <request_id> --grant-id <grant_id> --output-dir ./consumed --refresh-market --overwrite
```

The key status fields for an agent are:

- `payload.buyer.latest_request.last_feedback_event_id`
- `payload.buyer.latest_request.last_payment_pointer`
- `payload.buyer.latest_request.last_result_event_id`
- `payload.buyer.latest_request.winning_result_event_id`

For the successful `live6` proof, the consume command could be issued with:

- request id:
  `b945637edbe346ad46bddfd9f4f5aa7914433063db6a50138b9b169df191e3f4`
- grant id:
  `access_grant.npub1q4pr7lmyp3q5e8z98et4d95h40h0rvj0zn8qlvnwcaad0f7gce8qxp36yg.data_asset.npub1q4pr7lmyp3q5e8z98et4d95h40h0rvj0zn8qlvnwcaad0f7gce8qxp36yg.conversation_bundle.Headless_Dummy_Dataset_Live6.sha256_c0e0cf661545f117bd4e0611531b758f89e09d3c62fc3c8aaa4eebe16114299f.targeted_request.npub14r37gq524phnd9wewa9edac5gm8muwyf5d898acjq59dy9z94paq0juc7k`

Output root used in the proof:

- `target/headless-data-market-e2e/consumed-dataset-live6`

## Current state after this audit

What is now proven:

- external Lightning funding into buyer wallet
- priced seller invoice generation
- buyer Lightning payment
- seller automatic settlement detection
- seller result publication
- buyer result observation after payment
- local delivery consumption
- payload byte verification

What remains true about the MVP:

- the authority/control plane still owns market object truth
- the transport plane is still Nostr
- repeated resale of the exact same delivered grant identity is still a separate policy/lifecycle question from the buyer-access bug fixed here

The buyer-access proof gap is closed.
