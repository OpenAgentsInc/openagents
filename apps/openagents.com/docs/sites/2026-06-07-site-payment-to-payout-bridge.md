# Site Payment To Payout Bridge

Date: 2026-06-07
Issue: #437

## Summary

OpenAgents product surface now has an operator-authorized bridge from verified Site buyer payment
state into Nexus/Treasury payout-intent creation:

`POST /api/sites/{siteId}/commerce/payout-bridges`

The route is not a public checkout action. It requires an OpenAgents admin API
token plus `Idempotency-Key`, and it uses only server-side durable evidence:

- Site checkout intent record;
- buyer payment receipt;
- matched MDK reconciliation event;
- current Pylon/Nexus release-gate evidence;
- accepted-work refs;
- payout target approval ref;
- wallet readiness;
- amount; and
- spend cap.

Checkout return URLs, client-side success pages, raw provider events, and public
agent claims are never payout authority.

## What It Creates

When all gates pass, the route creates one Nexus/Treasury payout intent after
the same Treasury authority policy gates pass:

- accepted-work evidence is required;
- payout target approval is required;
- wallet readiness must be fresh;
- amount must fit within spend cap; and
- replay and duplicate funding are rejected.

The bridge also checks buyer-payment duplicate state before authority creation.
The same buyer payment receipt cannot fund another payout intent under a
different idempotency key.

## What It Does Not Do

This route does not dispatch the payout, mutate payout targets, spend wallet
funds, or declare settlement complete. Public settlement claims still require
Nexus/Pylon receipt evidence for dispatch, terminal result, verification, and
settlement.

The public-safe projection can link buyer payment, accepted-work refs, payout
intent refs, payout attempt refs, verification refs, and settlement receipt refs
when those records exist. Until those records exist, it reports payout-intent
readiness only.

## Live Route Contract

Request body fields include:

- `checkoutIntentRef`;
- `acceptedWorkRefs`;
- `payoutTargetApprovalRef`;
- `payoutTargetRef`;
- `policySnapshotRef`;
- `amount`;
- `spendCap`;
- `walletReadiness`;
- optional `adapterKind`;
- optional `assignmentRef`;
- optional `pylonJobRef`;
- optional `artanisDispatchRef`; and
- optional public-safe metadata/projection refs.

Responses use the existing Site commerce envelope with:

- `action: "payout_bridge_create"`;
- `bridge`;
- `payoutIntent` on success; and
- redaction metadata.

Blocked responses use HTTP 409 and include blocker refs such as:

- `missing_verified_buyer_payment`;
- `checkout_return_not_authority`;
- `duplicate_buyer_payment_ref`;
- `missing_accepted_work_ref`;
- `missing_payout_target_approval`;
- `stale_or_absent_wallet_readiness`;
- `spend_cap_exceeded`;
- `release_gate_not_ready`; and
- `missing_real_movement_gate`.

## Verification

Covered tests:

- `workers/api/src/site-payment-to-payout-bridge.test.ts`;
- `workers/api/src/site-commerce-routes.test.ts`;
- `workers/api/src/treasury-payment-authority.test.ts`;
- `workers/api/src/pylon-marketplace-payout-flow.test.ts`;
- `workers/api/src/artanis-nexus-pylon-adapters.test.ts`; and
- `workers/api/src/treasury-payment-mdk-agent-wallet-adapter.test.ts`.
