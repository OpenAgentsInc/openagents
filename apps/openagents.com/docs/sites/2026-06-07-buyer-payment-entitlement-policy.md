# Buyer Payment Entitlement Policy

Date: 2026-06-07
Issue: `#446`

## Summary

OpenAgents product surface now has a typed contract for deciding what a verified buyer payment can
do after payment evidence exists.

The policy layer separates:

- one-shot paid actions that can be consumed once;
- time-window entitlements;
- quota entitlements;
- resource-bound entitlements;
- actor-bound entitlements;
- route-bound entitlements;
- Site-bound entitlements; and
- hybrid policies that require multiple matching scopes.

The implementation lives in
`workers/api/src/buyer-payment-entitlement-policy.ts` with focused coverage in
`workers/api/src/buyer-payment-entitlement-policy.test.ts`.

## Contract Boundary

The policy consumes existing records from:

- `buyer-payment-ledger`;
- `paid-endpoint-product-catalog`; and
- the current payment policy audience/surface model.

It returns a public/customer/agent/operator-safe projection that says whether
the current request should:

- consume a one-shot entitlement;
- create a durable entitlement;
- renew an expired entitlement;
- decrement quota;
- allow an existing entitlement;
- ask for payment;
- reject a replay;
- reject exhausted or expired state; or
- reject mismatched actor, route, resource, Site, or scope.

The contract is deliberately pure. It does not mutate D1, call MDK, call
Stripe, create invoices, debit credits, dispatch payouts, or create
revenue-share records. Route handlers and workers can use the decision to make
those durable writes through the existing payment ledger boundaries.

## Authority Split

Payment proof is not authority for unrelated actions.

Every projection carries `authorityEffects` showing that a buyer payment does
not authorize user access, moderation bypass, confidential data access, owner
write access, Site deployment, or payout. If a route requires one of those
external authorities and it has not already been satisfied, the policy returns
`blocked` even when a redemption record is present.

This preserves the separation between:

- buyer payment evidence;
- entitlement state;
- authorization;
- moderation;
- owner grants;
- Site deploy authority;
- accepted work;
- payout intent;
- payout dispatch; and
- settled payout claims.

## Projection Rules

The projection is designed for agents and customers:

- stable refs are retained;
- raw payment material is rejected;
- raw timestamps are not required for customer-facing expiry copy;
- quota state is expressed as remaining units;
- next action is explicit; and
- receipt, redemption, and entitlement records are shown through the existing
  safe buyer-payment ledger projection.

The redaction guard rejects raw invoices, preimages, payment hashes, MDK
secrets, wallet state, provider tokens, private customer data, raw payloads,
and unsafe source material.

## Test Coverage

Focused tests cover:

- one-shot consumption;
- duplicate redemption/idempotency replay;
- quota decrement and exhaustion;
- time-window expiry and renewal;
- wrong resource, route, actor, and Site;
- scoped product entitlement creation;
- retired product policy rejection;
- raw payment material rejection; and
- payment proof failing to bypass authorization, moderation, owner-write,
  deploy, confidential-data, or payout policy.

## Remaining Work

This contract should be wired into the unified payment decision surface in the
next payment-hardening issue. That later surface should combine free-beta
allowance, internal credits, Stripe-funded credits, L402/MDK proof, and
existing entitlement state into one route-level policy decision.
