# Generated Site Checkout Evidence Gate

Date: 2026-06-08

## Summary

Generated Site payment smoke now carries a machine-readable
`paymentLaunchGate`. The gate separates:

- checkout evidence;
- live Bitcoin checkout evidence;
- payout settlement evidence.

This prevents generated Site public copy from treating a clean return page or
verified buyer payment as accepted-work payout settlement.

## Current State

The retained generated Site fixture is `checkout_evidence_only`.

It includes a public receipt bundle with:

- checkout intent refs;
- payment proof refs;
- buyer receipt refs;
- active entitlement refs;
- matched reconciliation refs.

It does not include payout settlement receipt refs, so:

- `liveBitcoinCheckoutClaimAllowed:false`;
- `payoutSettlementClaimAllowed:false`;
- public copy must use `copy.generated_site_payment.checkout_evidence_only`.

## Claim Rules

Checkout return URL state is never payout authority. A browser success view can
read server-side state, but it cannot create receipts, entitlements, payout
intents, or settlement.

Generated Site live Bitcoin checkout copy requires a live-provider checkout
fixture with a verified receipt bundle. Generated Site payout settlement copy
also requires separate payout settlement receipt refs.

Payout bridge readiness still requires accepted-work refs, payout target
approval, fresh wallet readiness, spend cap, release-gate real-movement
evidence, and duplicate buyer-payment rejection.

## Regression Coverage

- `workers/api/src/generated-site-payment-smoke-fixture.test.ts`
- `workers/api/src/site-commerce-routes.test.ts`
- `workers/api/src/site-payment-proof.test.ts`
- `workers/api/src/site-payment-to-payout-bridge.test.ts`
