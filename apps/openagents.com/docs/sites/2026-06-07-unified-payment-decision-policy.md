# Unified Payment Decision Policy

Date: 2026-06-07
Issue: `#447`

## Summary

OpenAgents product surface now has a single typed decision contract for route and action payment
state across:

- free-beta allowance;
- internal account credits;
- Stripe-funded credit top-up state;
- L402/MDK payment evidence;
- existing product entitlements;
- safety and private-authority blocks;
- manual review; and
- unavailable payment provider state.

The implementation lives in `workers/api/src/unified-payment-decision.ts` with
focused coverage in `workers/api/src/unified-payment-decision.test.ts`.

## Contract Boundary

The unified policy composes existing OpenAgents product surface payment surfaces:

- `payment-limit-policy`;
- `buyer-payment-ledger`;
- `buyer-payment-entitlement-policy`;
- existing billing credit refs; and
- redacted Stripe top-up state refs.

It does not create Stripe Checkout Sessions, migrate Stripe billing, debit real
credits, create invoices, call MDK, redeem L402 credentials, create payout
intents, or settle revenue-share records.

## Outcomes

The helper returns stable statuses:

- `allow`;
- `recoverable_by_credits`;
- `recoverable_by_l402_mdk`;
- `recoverable_by_either`;
- `manual_review`;
- `hard_blocked`;
- `exhausted`; and
- `provider_unavailable`.

The projection also carries agent-safe `nextActions`, including:

- `spend_internal_credits`;
- `use_entitlement`;
- `use_free_beta`;
- `add_credits`;
- `pay_l402_mdk`;
- `request_manual_review`;
- `retry_later`; and
- `stop`.

If L402/MDK recovery is not configured, the agent-facing next action does not
claim it can be paid. If Stripe top-up is not available, the projection does
not claim credits can be added.

## Source Refs

The safe projection can include:

- credit ledger refs;
- Stripe top-up state refs;
- L402 redemption refs;
- MDK checkout receipt refs;
- entitlement refs;
- spend-cap refs; and
- payment policy refs.

It rejects Stripe customer IDs, payment method data, invoice IDs, raw webhooks,
MDK credentials, raw payment hashes, preimages, provider tokens, wallet state,
raw credit ledger payloads, customer emails, and private route payloads.

## Entitlement Equivalence

The focused tests prove that a credit-paid access path and an L402/MDK-paid
access path can project the same entitlement decision when policy permits both.

That means agents and generated Sites can ask one question:

```text
Can this route/action proceed, and if not, what recovery path is actually live?
```

They do not need to reason about separate branches for free-beta, account
credits, Stripe credit purchases, existing entitlements, or L402/MDK evidence.

## Remaining Work

The next payment-hardening issue should add spend-cap and dry-run preview
behavior for agents before they pay, including max spend per call/window,
configured product price, settlement mode, idempotency requirements, and
provider availability. That dry-run path should call this unified policy
contract without mutating credits, creating invoices, redeeming credentials, or
granting entitlements.
