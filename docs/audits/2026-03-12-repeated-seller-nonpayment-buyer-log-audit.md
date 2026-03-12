# Repeated Seller Nonpayment Buyer-Log Audit

Date: March 12, 2026
Scope: `apps/autopilot-desktop`
Related incidents: repeated seller-side `delivered / awaiting buyer Lightning payment` jobs observed on the open network

## Executive Summary

The buyer logs show that the repeated “seller not getting paid” reports are not one single bug.

There are at least 2 distinct failure modes:

1. A seller can execute a targeted request long after its originating buyer session has effectively died. In the current evidence, request `44392bc2fde69398453e2312f87248c94fccff5691d1ac0bb1339b33eee3d416` was dispatched by the buyer at `15:53:41` local time and then executed by the seller at `17:19:56` local time, well beyond its `75s` TTL. That request should never have remained claimable. This is a real seller nonpayment bug.
2. A buyer can queue a real Spark payment, the seller can actually confirm settlement, and the buyer can still remain stuck at `wallet_status=pending`. In the current evidence, request `7dbf4a5e3f4fe6659744361ee37f3d6d1be96c375c0780f6869ed28d083ac1ad` appears to have been paid to the seller, but the buyer never advanced its own payment state to terminal. This is a buyer-truth / settlement-observation bug, not a seller nonpayment for that specific request.

So the current pain is a combination of:

- stale targeted requests staying executable far past TTL
- buyer payment observation not reliably becoming terminal after send-side payment queueing
- seller UI/logs truthfully showing `awaiting buyer payment` for stale requests, while the system lacks strong guarantees that the buyer that created the request is still alive and able to settle

## What We Investigated

Seller evidence provided live by the operator showed:

- `44392bc2...` delivered, invoice created, `payment-required` published, then stuck at `awaiting buyer Lightning payment`
- earlier request `7dbf4a5e...` showing `provider.settlement_confirmed`

To verify the buyer side, we inspected the active buyer session logs on this machine:

- [20260312T205337Z-pid39707.jsonl](/Users/christopherdavid/.openagents/logs/autopilot/sessions/20260312T205337Z-pid39707.jsonl)
- [20260312T210338Z-pid43832.jsonl](/Users/christopherdavid/.openagents/logs/autopilot/sessions/20260312T210338Z-pid43832.jsonl)

We also inspected the buyer payment/state code paths in:

- [wallet.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/input/reducers/wallet.rs)
- [operations.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/state/operations.rs)
- [spark_wallet.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/spark_wallet.rs)
- [jobs.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/input/reducers/jobs.rs)
- [job_inbox.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/state/job_inbox.rs)
- [provider_nip90_lane.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/provider_nip90_lane.rs)

## Timeline and Evidence

### Case A: `44392bc2...` was dispatched by the buyer, then executed by the seller long after TTL

Buyer session evidence:

- In [20260312T205337Z-pid39707.jsonl](/Users/christopherdavid/.openagents/logs/autopilot/sessions/20260312T205337Z-pid39707.jsonl), the buyer queued and dispatched request `44392bc2fde69398453e2312f87248c94fccff5691d1ac0bb1339b33eee3d416` at local `15:53:41`.
- The buyer targeted provider `2c879cab8d1c3f5e21ca078e972ea8e76ff7549825ee7d2d171cff773f8b4161`.
- That buyer session never recorded invoice selection, payable-provider selection, queued payment, settled payment, or failed payment for `44392...`.
- Searching the local buyer session logs found no later session that continued `44392...` into invoice/payment settlement.

Seller session evidence from the operator:

- The seller accepted and executed `44392...` at local `17:19:56`.
- The same seller log showed `ttl_seconds=75` semantics on the request family.
- The seller then generated an invoice and published `payment-required`, but no payment followed.

Interpretation:

- `44392...` stayed in the seller inbox/preview backlog and remained accept-able long after its buyer context had effectively disappeared.
- This is not “buyer is slow.” This is stale targeted work being treated as fresh live work.
- In a seller-first product, this is unacceptable because it guarantees unpaid work.

### Case B: `7dbf4a5e...` looks like a real paid request that the buyer failed to finalize locally

Buyer session evidence in [20260312T210338Z-pid43832.jsonl](/Users/christopherdavid/.openagents/logs/autopilot/sessions/20260312T210338Z-pid43832.jsonl):

- Buyer queued request `7dbf4a5e3f4fe6659744361ee37f3d6d1be96c375c0780f6869ed28d083ac1ad`.
- Buyer observed the targeted provider invoice and selected the seller as the payable provider.
- Buyer queued a Spark Lightning payment and recorded payment pointer `019ce3eb-8c6d-76e3-9e85-df3c5f81aec0`.
- Buyer logged:
  - `Buyer Spark payment pending wallet sync request_id=7dbf... pointer=019ce3eb-8c6d...`
  - `Wallet: Payment sent (019ce3eb-8c6d...); awaiting Spark confirmation for balance refresh`
- Buyer later ingressed seller feedback:
  - `status=success`
  - `status_extra=wallet-confirmed settlement recorded`
  - feedback event `77aa29045e246321464295c471350fedbd2e3d799d9879f8530dd38cae2d14de`
- Despite that, the buyer remained in:
  - `phase=awaiting-payment`
  - `status=payment-required`
  - `wallet_status=pending`

Seller evidence from the operator:

- Seller logged `provider.settlement_confirmed` for `7dbf...`
- Seller recorded settlement amount `2 sats` and `fees_sats=0`

Interpretation:

- The most likely reading is that `7dbf...` really did pay the seller.
- The buyer-side app failed to observe or reconcile its own outbound Spark payment into a terminal local state.
- That is a buyer settlement-observation failure, not the same thing as seller nonpayment.

## Root Cause 1: Stale Targeted Requests Remain Claimable Far Beyond TTL

This is the most serious seller-facing defect in the current evidence.

Current state:

- Request ingestion stores `ttl_seconds` in [job_inbox.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/state/job_inbox.rs).
- Acceptance checks in [jobs.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/input/reducers/jobs.rs) verify only:
  - request validity
  - minimum TTL
  - price floor
  - provider blockers
  - inflight limit
- There is no visible acceptance-time rejection for “request is older than its TTL / execution window.”

Evidence from code:

- `request_accept_block_reason(...)` in [jobs.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/input/reducers/jobs.rs) checks `request.ttl_seconds < MIN_PROVIDER_TTL_SECONDS`, but not whether the request has already expired in wall-clock time.
- `JobInboxRequest` in [job_inbox.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/state/job_inbox.rs) stores price, TTL, risk, and arrival sequencing, but not a derived “expired / deadline passed” rejection state.
- `event_to_inbox_request(...)` in [provider_nip90_lane.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/provider_nip90_lane.rs) parses `ttl_seconds`, but the current acceptance path does not enforce freshness against the event’s actual age.

Operational consequence:

- A seller who comes online later can work dead demand from a buyer that no longer exists, no longer has the app running, or is no longer tracking that request.
- The seller then truthfully gets stuck at `awaiting buyer payment`.

## Root Cause 2: Buyer Payment State Depends on Local Spark Observation but Lacks Strong Recovery

The buyer payment path is real up to the point where Spark send is queued.

Code path:

- `prepare_auto_payment_attempt_internal(...)` in [operations.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/state/operations.rs) sets `pending_auto_payment_request_id` and records the invoice.
- `SparkWalletCommand::SendPayment` eventually calls `send_payment(...)` in [spark_wallet.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/spark_wallet.rs).
- `send_payment(...)` stores:
  - `last_payment_id`
  - `pending_balance_confirmation_payment_id`
  - then triggers `refresh_balance_and_payments(...)`
- Buyer reconciliation happens in `reconcile_pending_buyer_payment_confirmation(...)` in [wallet.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/input/reducers/wallet.rs).

The weak point:

- Buyer reconciliation only advances when a Spark worker update provides a matching payment record or terminal wallet error.
- We do not have a seller-style periodic buyer payment-evidence refresh loop while `pending_auto_payment_request_id` is still active.
- Seller side does have that pattern in [actions.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/input/actions.rs): it refreshes Spark every `5s` while delivered work is awaiting wallet evidence.
- Buyer side, by contrast, appears to rely on the initial send-time refresh plus any incidental later wallet updates.

Operational consequence:

- The buyer can remain stuck at `wallet_status=pending` even after the outbound payment was sent and the seller later confirmed settlement.
- This makes the buyer UI and Mission Control truth lag behind reality and obscures which requests were truly unpaid versus merely unconfirmed locally.

## Secondary Design Problem: Seller Accepts Open-Network Targeted Work Without Buyer Liveness Guarantee

Even if TTL freshness is fixed, the product currently assumes that “targeted request names this provider” is enough to treat the demand as safe auto-accept.

That is only partially true.

The current policy correctly distinguishes:

- `targeted-open-network`
- `speculative-open-network`

But it still assumes targeted demand is acceptable if the request itself looks valid. It does not ask:

- is the buyer session still active enough to settle?
- is the request still inside a sane payment window?
- has the buyer session already moved on or restarted?

For v0.1 seller-first behavior, that is too trusting.

## What the Evidence Does Not Show

The current logs do **not** support these weaker explanations:

- “the seller pane is fake and just lying”
- “Apple FM failed locally and no result was actually delivered”
- “the buyer never saw the invoice for every case”

Those explanations are contradicted by the logs.

Specifically:

- For `7dbf...`, the buyer clearly saw the invoice, chose the seller, and queued a payment.
- For `44392...`, the seller clearly worked and invoiced the request, but far too late.

## Diagnosis

The repeated reports of seller nonpayment are best understood as:

1. **Real seller nonpayment from stale targeted request acceptance**
   - request `44392...`
   - buyer dispatched at `15:53:41`
   - seller accepted/executed at `17:19:56`
   - request should have expired long before seller execution

2. **Buyer-side false nonterminal state after actual payment**
   - request `7dbf...`
   - seller likely got paid
   - buyer remained stuck at `wallet_status=pending`

These are different bugs, but they compound operational confusion because they both surface as “something is wrong with payment.”

## Recommendations

### 1. Reject expired targeted requests before seller execution

Required change:

- During inbox validation and again at acceptance time, compute request age from Nostr event creation time and reject any request older than its TTL / allowed execution window.

Minimum product rule:

- A seller must never execute a request whose buyer-side TTL has already expired.

### 2. Carry explicit request freshness into `JobInboxRequest`

Required change:

- Persist request created-at epoch and a derived expiry epoch in [job_inbox.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/state/job_inbox.rs).
- Surface freshness in the inbox and active-job reasoning, not just TTL length.

### 3. Add a buyer-side payment-evidence watchdog

Required change:

- Mirror the seller’s wallet refresh loop for buyer pending auto-payments.
- While `pending_auto_payment_request_id` is set and the buyer request is nonterminal, enqueue periodic `SparkWalletCommand::Refresh` until the send-side payment is observed terminal or the request fails.

### 4. Distinguish “seller truly unpaid” from “buyer locally unconfirmed”

Required change:

- If seller success feedback arrives (`wallet-confirmed settlement recorded`) for the selected provider, the buyer UI should mark the request as seller-settled / buyer-observation-lagging, even if local Spark send reconciliation has not yet finalized.
- That must not be treated as buyer-side wallet-authoritative success, but it must be visible as a separate state.

### 5. Add deterministic regressions for both failure modes

Needed tests:

- stale targeted request older than TTL must be rejected before seller execution
- buyer payment queued + seller settlement success + no immediate local payment record must not remain opaque pending forever
- release harness must fail if a request older than TTL can still be executed

### 6. Tighten targeted-open-network “safe-auto” policy

Required change:

- A request being targeted is necessary but not sufficient.
- The auto-accept decision should also require freshness and a sane buyer settlement window.

## Release Risk

This is release-blocking for seller-first use.

Why:

- The MVP promise is that a seller can go online, receive work, and get paid.
- Executing expired targeted requests violates that promise directly.
- Buyer pending-state drift then makes it harder to understand which requests really paid and which did not.

## Bottom Line

The live evidence shows that the current repeated seller nonpayment complaints are grounded in reality.

The main hard failure is not “Spark randomly failed” and not “the seller UI is lying.” The main hard failure is that we let sellers work dead targeted requests long after the originating buyer context is gone. On top of that, the buyer app can fail to finalize its own payment state even after a seller has already confirmed settlement.

That combination is exactly the kind of ambiguity a seller-first MVP cannot afford.
