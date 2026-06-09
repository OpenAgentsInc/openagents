# Payment Destination Input Parser

Date: 2026-06-07
Issue: #451 / OPENAGENTS-H-014

## Summary

OpenAgents product surface now has a typed payment destination input classifier for Sites, Forum,
Pylon, Nexus, and future agent-wallet flows.

Implementation:

```text
workers/api/src/payment-destination-input.ts
workers/api/src/payment-destination-input.test.ts
```

The classifier is intentionally narrow. It accepts private raw input, returns a
public-safe redacted projection, and never creates checkout, payout, wallet,
settlement, or dispatch authority.

## Supported Classifications

The current Worker-safe boundary classifies:

- BOLT11 invoices;
- BOLT12 offers;
- LNURL payloads;
- Lightning Addresses;
- BIP353-style human-readable names;
- `bitcoin:` URI payloads;
- on-chain addresses when that input type is enabled;
- Cashu payment requests when that input type is enabled;
- unsupported formats;
- malformed Lightning-like formats; and
- ambiguous pasted inputs that contain multiple candidate destination types.

LNURL, Lightning Address, and BIP353-style inputs are marked as requiring
resolution outside the Worker parser. The parser does not fetch DNS, HTTP, or
wallet state.

## MDK Source Decision

MoneyDevKit's local `bitcoin-payment-instructions` crate is the conformance
source. It supports more complete Rust parsing and resolver behavior, but its
network resolver paths are not a direct Cloudflare Worker import.

OpenAgents product surface should either compile a future small WASM parser adapter or delegate
resolution to an explicit sidecar/CLI boundary. Until that exists, generated
Sites should call the OpenAgents product surface classifier only for typed validation and safe
projection.

## Site Builder Rule

Generated Sites that need paid actions should not publish raw payment
destinations in public HTML, proof JSON, OpenAPI docs, Site metadata, or
customer dashboards.

They should store and display only:

- payment destination kind;
- method refs;
- network hint;
- redacted destination ref;
- resolution-required state;
- approval-required state; and
- checkout/action policy refs.

A parsed destination is not enough to charge, tip, boost, fund, or pay. Site
checkout and L402 actions still need the Site payment catalog, spend caps,
idempotency, provider configuration, reconciliation, receipt, and entitlement
policy.
