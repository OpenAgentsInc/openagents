# Agent Spend-Cap Preview Contract

Date: 2026-06-07
Issue: #448 / OPENAGENTS-H-011

## Summary

OpenAgents product surface now has a typed, side-effect-free spend-cap preview contract for paid
agent actions. It lets an agent or browser client ask whether a proposed paid
route is under the configured spend cap before the client creates a payment,
uses account credits, redeems an L402 credential, or receives an entitlement.

The contract is intentionally a dry run. It can report that a paid action is
under cap, exactly at cap, over cap, unsupported, stale, private, owner-grant
only, malformed, or blocked, but it never mutates payment, payout, entitlement,
credit, or MDK state.

## Covered Inputs

The preview input describes:

- the paid action and product refs;
- the actor ref, audience, and authenticated-agent state;
- the route ref and whether the route is public-previewable or owner-grant
  only;
- the configured price, max-per-call cap, max-per-window cap, and current
  window spend;
- the requested payment rail, supported rails, settlement mode, retry refs, and
  idempotency guidance;
- the current unified payment decision from the shared payment policy surface.

Prices use the same paid-endpoint price contract as the product catalog. For
bitcoin-priced actions, the denomination is `bitcoin_millisatoshi`, so callers
can keep precise machine-readable values while UI copy can still say
`bitcoin`.

## Statuses

The preview returns one of these public-safe statuses:

| Status | Meaning |
| --- | --- |
| `under_cap` | The requested paid action fits under both per-call and window caps. |
| `exact_cap` | The requested action exactly reaches either cap and is still allowed. |
| `over_cap` | The requested action would exceed the per-call or window cap. |
| `unauthenticated_agent` | A registered agent token is required before payment. |
| `catalog_missing` | The action/product catalog entry is missing. |
| `malformed_amount` | Price or cap values are invalid. |
| `wrong_currency` | Price, caps, or window spend do not use the same asset/denomination. |
| `unsupported_rail` | The requested rail is not supported for this action. |
| `stale_catalog_entry` | The catalog entry exists but is not active. |
| `private_route` | The route is not public-previewable. |
| `owner_grant_required` | The route requires owner-granted authority outside default agent auth. |
| `blocked` | The unified payment decision hard-blocks payment or requires review. |

## Next Actions

The projection includes normalized next actions such as:

- `pay_l402_mdk`;
- `spend_internal_credits`;
- `add_credits`;
- `use_entitlement`;
- `use_free_beta`;
- `lower_spend_or_raise_cap`;
- `provide_agent_token`;
- `ask_owner_for_grant`;
- `request_manual_review`;
- `fix_catalog`;
- `fix_currency`;
- `stop`.

These are guidance only. A preview response does not grant authority to take the
next action.

## Redaction And Side Effects

The contract rejects raw private or payment-secret material before projection.
It rejects emails, raw invoices, preimages, MDK credentials, Stripe secrets,
wallet state, provider grants, raw webhooks, runner payloads, and source
archives. The returned projection is checked again before it leaves the helper.

The side-effect summary is always false for:

- `callsMdk`;
- `createsPaymentArtifact`;
- `redeemsCredentials`;
- `createsEntitlement`;
- `debitsCredits`;
- `mutatesPayout`.

That invariant is the point of the contract: agents can inspect budget and
authority before money moves.

## Current Boundary

This is a reusable Worker/Effect contract module, not a public HTTP route yet.
Do not document it in public `AGENTS.md` as a live API endpoint until a route is
added and deployed.

The next implementation step is to attach this preview contract to the live
agent-readable paid-action routes that already expose product catalog and L402
metadata.
