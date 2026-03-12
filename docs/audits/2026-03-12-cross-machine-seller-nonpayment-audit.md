# Cross-Machine Seller Nonpayment Audit

Date: March 12, 2026
Scope: `apps/autopilot-desktop`
Related issues: `#3414`, `#3422`, `#3423`, `#3424`, `#3425`, `#3426`, `#3427`, `#3428`

## Executive Summary

The repeated “seller was not paid” reports are real, but they are not one single bug.

The cross-machine evidence shows 2 distinct failure modes:

1. A seller can still execute a targeted open-network request long after the originating buyer session has moved on, far beyond the request TTL. In that case the seller really does deliver work and then wait forever for payment that will never come.
2. A buyer can queue a real Spark Lightning payment, the seller can record wallet-confirmed settlement, and the buyer can still remain stuck at `wallet_status=pending`. In that case the seller may have actually been paid, but the buyer UI and Mission Control remain locally nonterminal and misleading.

So the current operational problem is:

- real unpaid seller work from stale targeted requests
- buyer-side payment reconciliation that can stay stranded after payment was queued
- UI/log wording that does not cleanly distinguish “seller unpaid” from “seller paid but buyer-local observation is still pending”

This is release-blocking for a seller-first product.

## Evidence Sources

Seller-side evidence came from the operator logs and Active Job pane shared in the incident report.

Buyer-side evidence came from local desktop session logs on this machine:

- [20260312T205337Z-pid39707.jsonl](/Users/christopherdavid/.openagents/logs/autopilot/sessions/20260312T205337Z-pid39707.jsonl)
- [20260312T210338Z-pid43832.jsonl](/Users/christopherdavid/.openagents/logs/autopilot/sessions/20260312T210338Z-pid43832.jsonl)

Relevant code paths reviewed:

- [jobs.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/input/reducers/jobs.rs)
- [job_inbox.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/state/job_inbox.rs)
- [provider_nip90_lane.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/provider_nip90_lane.rs)
- [operations.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/state/operations.rs)
- [wallet.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/input/reducers/wallet.rs)
- [actions.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/input/actions.rs)
- [nip90_compute_flow.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/nip90_compute_flow.rs)

## Incident A: Request `44392bc2...` Was Real Unpaid Seller Work

### Buyer evidence

In [20260312T205337Z-pid39707.jsonl](/Users/christopherdavid/.openagents/logs/autopilot/sessions/20260312T205337Z-pid39707.jsonl):

- the buyer queued request `44392bc2fde69398453e2312f87248c94fccff5691d1ac0bb1339b33eee3d416`
- the buyer dispatched it at local `15:53:41`
- the buyer targeted provider `2c879cab8d1c3f5e21ca078e972ea8e76ff7549825ee7d2d171cff773f8b4161`
- the request was a `mission_control.buy_mode.5050` request with `budget_sats=2` and `timeout_seconds=75`

That buyer session does not show later invoice selection, queued payment, settled payment, or failed payment for `44392...`.

### Seller evidence

The seller log shows:

- the provider accepted and executed `44392...` at local `17:19:56`
- the provider delivered the result
- the provider created a `2 sats` Lightning invoice
- the provider published `payment-required`
- the provider remained at `delivered / awaiting buyer Lightning payment`

### Diagnosis

This request was stale. It was dispatched by the buyer roughly `86` minutes before the seller executed it, while its declared timeout was `75s`.

The seller should never have been allowed to execute it.

This is a true seller nonpayment failure, not just a buyer UI lag problem.

## Incident B: Request `7dbf4a5e...` Looks Like Real Seller Settlement With Buyer Still Stuck Pending

### Buyer evidence

In [20260312T210338Z-pid43832.jsonl](/Users/christopherdavid/.openagents/logs/autopilot/sessions/20260312T210338Z-pid43832.jsonl):

- the buyer queued request `7dbf4a5e3f4fe6659744361ee37f3d6d1be96c375c0780f6869ed28d083ac1ad`
- the buyer selected provider `2c879cab8d1c3f5e21ca078e972ea8e76ff7549825ee7d2d171cff773f8b4161` as:
  - `selected_provider_pubkey`
  - `result_provider_pubkey`
  - `invoice_provider_pubkey`
  - `payable_provider_pubkey`
- the buyer recorded:
  - `payment_pointer=019ce3eb-8c6d-76e3-9e85-df3c5f81aec0`
  - `pending_bolt11=lnbc20n1...`
- the buyer logged:
  - `Wallet: Payment sent (019ce3eb-8c6d-76e3-9e85-df3c5f81aec0); awaiting Spark confirmation for balance refresh`
  - `Provider: buyer payment pending Spark confirmation request=7dbf... pointer=019ce3eb-8c6d...`
- the buyer then ingressed provider success feedback:
  - `status=success`
  - `status_extra=wallet-confirmed settlement recorded`
  - `event_id=77aa29045e246321464295c471350fedbd2e3d799d9879f8530dd38cae2d14de`

Despite that, the buyer remained at:

- `in_flight_phase=awaiting-payment`
- `in_flight_status=payment-required`
- `wallet_status=pending`
- `next_expected_event=wallet settlement`

### Seller evidence

The seller log shows:

- `provider.settlement_confirmed`
- `payment_id=019ce3eb-9160-7be0-a15d-4736a9505b00`
- `amount_sats=2`
- `fees_sats=0`

The seller’s receive-side payment id is expected to differ from the buyer’s send-side payment pointer. That difference by itself is not evidence of failure.

### Diagnosis

The most likely reading is:

- seller settlement really happened
- buyer payment send was real
- buyer-local Spark observation never became terminal

So `7dbf...` is primarily a buyer payment-observation / UI-truth failure, not strong evidence that the seller was unpaid for that specific request.

## What Actually Went Wrong

### 1. Stale targeted requests were being treated as executable live demand

Before the freshness fix, targeted open-network demand could become `safe-auto` simply because it named the provider and passed basic validation.

That was too weak for a seller-first product.

The request path carried TTL, but acceptance was not enforcing “this request is already expired in wall-clock time” strongly enough in the build that produced the incident.

This is exactly how a seller can come online later, pick up old targeted work, do the compute, and wait forever for a buyer that no longer has a live payment path for that request.

### 2. Buyer auto-payment observation could get stranded after send-side queueing

The buyer clearly reached:

- invoice received
- payable provider selected
- Spark payment sent

But it did not reliably reach:

- settled buyer wallet payment
- explicit terminal buyer-local failure

That leaves the request stuck in a nonterminal `pending` state even after provider success feedback says settlement was recorded on the seller side.

### 3. The product still conflates multiple truths

The current operational story has 3 distinct truths that need to be separated:

- seller unpaid
- seller settled, buyer-local wallet observation still pending
- buyer settled and locally confirmed

If those are not distinct, operators cannot tell whether they are looking at:

- a real unpaid seller
- a local observation lag
- or a dead/stale request that should never have executed

## What Is Already Fixed on `main`

Some of the failure chain is already addressed on current `main`, but those fixes are not enough by themselves to declare the release safe.

### `#3423` Reject expired targeted requests before execution

Landed in:

- [jobs.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/input/reducers/jobs.rs)
- [job_inbox.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/state/job_inbox.rs)
- [provider_nip90_lane.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/provider_nip90_lane.rs)

Current main now stores:

- `created_at_epoch_seconds`
- `expires_at_epoch_seconds`

and blocks open-network requests that have already expired with:

- `Request already expired ... and cannot be executed safely`

This should prevent the exact `44392...` class of stale-targeted execution once the seller is running a build that includes that change.

### `#3424` Persist freshness truth through inbox and Active Job

Landed in:

- [app_state.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/app_state.rs)
- [pane_renderer.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/pane_renderer.rs)

Current main now carries request freshness metadata into:

- Job Inbox
- Active Job pane
- Active Job clipboard text

That makes it much easier to audit whether a request was stale when accepted.

### `#3425` Add buyer-side payment watchdog

Landed in:

- [operations.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/state/operations.rs)
- [actions.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/input/actions.rs)
- [wallet.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/input/reducers/wallet.rs)
- [input.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/input.rs)

Current main now has:

- `active_auto_payment_observation_request_id()`
- `buyer_payment_watchdog_due(...)`
- `run_pending_buyer_payment_watchdog_tick(...)`

That means the buyer should keep queueing periodic Spark `Refresh` operations while a payment pointer exists but the request is still nonterminal.

Important nuance:

- the live incident log for `7dbf...` does not show the new watchdog refresh lines that current main emits
- so either the affected buyer binary predates that fix, or that running binary still did not execute the watchdog path correctly

Either way, the incident proves we cannot trust the old release binary.

## What Still Needs To Be Finished

### `#3426` Distinguish seller-settled from buyer-locally-unconfirmed

Still open.

We need an explicit buyer-side phase for:

- seller success feedback received
- seller settlement appears confirmed
- buyer local Spark reconciliation still pending

Without that, operators still read everything as “awaiting payment,” which hides the difference between:

- real seller nonpayment
- buyer-local lag after seller settlement

### `#3427` Tighten `targeted-open-network / safe-auto`

Still open.

Freshness must be mandatory for `safe-auto`.

Targeting alone is not enough. A request explicitly naming this provider should still fall back to manual-only or hard reject if:

- it is expired
- its freshness window is no longer credible
- the seller cannot trust that the buyer is still alive enough to settle

### `#3428` Release gates must catch both failure classes

Still open.

The release harness must fail if:

- a stale targeted request can still be executed
- a buyer payment can remain indefinitely pending without recovery
- seller payout truth is not actually proven by wallet-authoritative evidence

## Why `autopilot-v0.1.1` Should Not Be Trusted

The release currently in the wild is not trustworthy for seller payments.

Reason:

- it was cut before this full stale-targeted / buyer-pending chain was closed and validated end-to-end
- live evidence shows both failure classes are real in the released behavior

So even though parts of the fix chain now exist on `main`, the current release asset should be treated as invalid for seller payment correctness until:

- freshness rejection is present in the shipped seller binary
- buyer payment watchdog behavior is proven in the shipped buyer binary
- release verification explicitly fails on both stale-targeted and stuck-pending regressions

## Required Final Fix Sequence

1. Ship sellers with the stale-targeted expiry rejection from `#3423`.
2. Ensure buyers are on a build that includes the payment watchdog from `#3425`.
3. Finish `#3426` so Mission Control and Buy Mode distinguish:
   - seller unpaid
   - seller settled / buyer-local pending
   - buyer fully settled
4. Finish `#3427` so targeted demand is not `safe-auto` merely because it names the provider.
5. Finish `#3428` so packaged-release validation proves these cases before any replacement release is published.

## Bottom Line

The cross-machine evidence supports the operator complaint.

- `44392...` is a real “seller worked and was not paid” incident caused by stale targeted request execution.
- `7dbf...` is a real “buyer got stuck pending after payment path already started, and likely after seller settlement” incident.

Those are different failures, but both are severe for a seller-first release.

The only honest release posture right now is:

- current release asset is not acceptable
- rebuild only after the full `#3423` through `#3428` chain is complete and revalidated
