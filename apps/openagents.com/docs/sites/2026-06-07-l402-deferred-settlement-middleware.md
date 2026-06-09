# L402 Deferred Settlement Middleware

Issue: #445 / OPENAGENTS-H-008

Date: 2026-06-07

## Summary

OpenAgents product surface now has a typed L402 deferred-settlement contract for paid endpoints
whose useful work should only be charged after a success boundary. This is for
generated Site paid actions, agent-paid APIs, and future workroom routes where a
handler can fail after the client presents payment evidence.

The contract lives in:

- `workers/api/src/l402-deferred-settlement.ts`
- `workers/api/src/l402-deferred-settlement.test.ts`

It builds on the existing buyer-payment ledger, L402 credential verification,
Site payment middleware, checkout return, and MDK reconciliation surfaces. It
does not add a new payment rail and does not create payout authority.

## Settlement Modes

The contract models five modes:

- `immediate`
- `deferred_until_success`
- `deferred_until_artifact_receipt`
- `deferred_until_response_closeout`
- `manual_operator_review`

The projected outcomes are:

- `allow`
- `payment_required`
- `settlement_pending`
- `settled`
- `retryable_failure`
- `blocked`

For deferred modes, a valid payment credential can remain reusable while the
protected handler is still pending or failed before charge. Once the configured
success boundary is reached, the contract projects receipt and entitlement refs
and marks the credential consumed.

## Redaction Boundary

The contract rejects raw invoices, payment preimages, raw payment hashes, MDK
credentials, wallet material, provider grants, private customer data, and raw
payload material. Customer/public/agent projections expose safe refs and status
only.

Operator projections may include a credential ref, but still not raw credential
material.

## Non-Goals

This slice does not:

- debit Stripe credits;
- configure a live MDK sidecar or hosted platform route;
- create payout intents;
- mark accepted work paid;
- settle provider payouts;
- alter Artanis, Pylon, Treasury, or Nexus authority.

## Tests

The regression tests cover:

- settle after success;
- retryable failure before charge;
- pending until artifact receipt;
- active entitlement allow path;
- existing receipt/entitlement idempotent settled projection;
- expired challenge blocking;
- invalid credential payment-required projection;
- raw payment material rejection.
