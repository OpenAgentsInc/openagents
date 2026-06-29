# Site Payment Proof

Date: 2026-06-07
Issue: #439

## Summary

OpenAgents product surface now has a public-safe read route for buyer-side Site payment proof:

```text
GET /api/sites/{siteId}/commerce/payment-proofs/{checkoutIntentRef}
```

The route projects durable server-side evidence for a Site checkout intent. It
can be shown inside a generated Site, an OpenAgents customer surface, or an
agent-facing read flow without exposing raw payment material.

## What The Proof Reads

The projection summarizes:

- Site id;
- Site version id;
- checkout intent ref;
- checkout status;
- catalog/product refs;
- provider/environment refs;
- buyer payment challenge;
- buyer payment receipt when it exists;
- MDK reconciliation event when it exists;
- entitlement state when it exists;
- proof state;
- claim state;
- public caveat refs; and
- friendly customer, agent, and operator labels.

The main proof states are:

- `pending_checkout`;
- `pending_reconciliation`;
- `payment_observed`;
- `verified_entitlement`; and
- `blocked`.

The proof reaches `verified_entitlement` only when the checkout intent, receipt,
matched reconciliation event, and active entitlement all agree on the durable
server-side refs.

## What It Does Not Prove

This proof is buyer-side checkout evidence only. It does not prove:

- accepted-work completion;
- provider payout authority;
- Pylon payout eligibility;
- wallet state;
- payout target approval;
- bitcoin movement to a provider;
- final settlement; or
- any customer-private data value.

Those states belong to the separate Nexus/Treasury payout and settlement
surfaces.

## Redaction Boundary

The proof route and projector reject or omit:

- raw invoices;
- payment hashes;
- preimages;
- MDK credentials;
- webhook secrets;
- wallet material;
- raw provider payloads;
- customer private data;
- payout targets;
- payout provider grants;
- checkout query strings; and
- raw timestamps in customer-facing projection copy.

Public proof projections omit customer/operator-only details. Customer and
operator projections can include redacted payment refs and operator refs through
typed projections, but those still do not expose raw payment material or create
payout authority.

## Route Behavior

The route is read-only and does not require an idempotency key.

It requires the durable checkout-intent and buyer-payment stores. If those are
not available, the route returns a Site-commerce unavailable response. If the
checkout intent does not exist for the requested Site id, the route returns the
same checkout-intent-not-found response used by checkout returns.

If source records cannot be projected safely, the route returns a public-safe
`409 payment_proof_unsafe` response rather than returning partial private
state.

## Related Surfaces

- `GET /api/sites/{siteId}/commerce/discovery` now advertises the payment proof
  endpoint and `paymentProof: "available"`.
- `/api/openapi.json` exposes the `readSiteCommercePaymentProof` operation.
- `/.well-known/openagents.json` advertises the `site_payment_proof` resource
  and `site_payment_proof_read` action.
- `/AGENTS.md` lists the route in the Site commerce section.

## Verification

Covered tests:

- `workers/api/src/site-payment-proof.test.ts`;
- `workers/api/src/site-payment-discovery.test.ts`;
- `workers/api/src/site-commerce-routes.test.ts`;
- `workers/api/src/openagents-openapi-routes.test.ts`;
- `workers/api/src/openagents-capability-manifest-routes.test.ts`; and
- `workers/api/src/openagents-agent-onboarding-routes.test.ts`.
