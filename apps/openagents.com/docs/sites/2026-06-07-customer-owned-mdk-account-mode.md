# Customer-Owned MDK Account Mode

Date: 2026-06-07
Issue: #441 / OPENAGENTS-SITES-MDK-011

## Purpose

Generated Sites need to distinguish two payment-account modes:

- `openagents_hosted_mdk`: OpenAgents holds the hosted checkout boundary.
- `customer_owned_mdk`: an operator-reviewed customer-owned MDK account is
  referenced through hosted secret-binding refs.

The customer-owned mode is a binding and projection layer only. It does not
grant generated Sites, customers, or agents direct MDK credential access, live
wallet spend authority, checkout authority, payout authority, settlement
authority, access authority, or deployment authority.

## Live Surfaces

The public/customer-safe read path is:

```text
GET /api/sites/{siteId}/commerce/mdk-account-binding
```

It returns one of:

- `unavailable`
- `pending_review`
- `configured`
- `blocked`
- `revoked`

The operator write path is:

```text
POST /api/sites/{siteId}/commerce/mdk-account-bindings
```

That write requires an OpenAgents admin API token and an `Idempotency-Key`.
The request can include customer/order/site refs, sandbox or production
classification, catalog/product/action scope refs, reviewer/caveat refs, and
hosted secret-binding refs. It must not include MDK access tokens, wallet
mnemonics, webhook secrets, wallet material, raw invoices, payment hashes,
preimages, provider grants, or private customer values.

## Checkout Behavior

Checkout intent creation still goes through the OpenAgents product surface-hosted Site commerce
boundary. When an approved customer-owned binding applies to the catalog item,
the checkout intent response includes:

```json
{
  "provider": "openagents_hosted_mdk",
  "providerMode": "customer_owned_mdk",
  "mdkAccountBinding": {
    "bindingState": "configured",
    "providerMode": "customer_owned_mdk",
    "secretBindingRefs": [],
    "secretBindingState": "redacted"
  }
}
```

This lets generated Sites and agents know the selected payment-account mode
without exposing credentials or implying payout settlement.

## Redaction Rules

Customer and public projections redact:

- hosted secret-binding refs;
- customer and order refs;
- reviewer refs;
- MDK credentials;
- wallet material;
- invoices, payment hashes, and preimages;
- provider grants;
- raw timestamps.

Operator projections may include hosted secret-binding refs so an operator can
verify which server-side secret binding is configured. Operator projections
still cannot include raw secret values.

## Follow-On Work

This issue does not complete MDK checkout runtime parity. The next batch must
add sandbox/signet smoke tests, MDK core-backed helper parity, and Site payment
primitive SDK docs before treating this as a customer-facing production payment
story.
