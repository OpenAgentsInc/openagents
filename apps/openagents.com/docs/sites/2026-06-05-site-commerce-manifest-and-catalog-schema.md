# Site Commerce Manifest And Catalog Schema

Date: 2026-06-05

Issue: `OpenAgentsInc/openagents#163`

Status: first schema and D1 catalog contract for checkout products and paid
agent actions.

## Purpose

Autopilot Sites need a source-visible way to declare commerce intent without
putting payment secrets into generated Site source. The manifest describes what
the Site wants to sell or protect. OpenAgents product surface remains the hosted payment boundary
that creates checkout intents, L402 challenges, entitlements, receipts, and
public-safe projections.

This issue adds:

- `workers/api/src/site-commerce.ts`
- `workers/api/src/site-commerce.test.ts`
- `workers/api/migrations/0065_site_commerce_catalog.sql`

## `.openagents/site.json` Payments Block

The source-visible contract is:

```json
{
  "payments": {
    "enabled": true,
    "provider": "openagents_hosted",
    "products": [
      {
        "id": "consultation_deposit",
        "name": "Consultation deposit",
        "price": { "asset": "usd", "amount": 5000 },
        "checkoutPath": "/checkout/consultation-deposit",
        "entitlementScope": "product",
        "agentReadable": true,
        "settlementMode": "checkout_only",
        "customerDataRequirements": [
          {
            "key": "email",
            "label": "Email",
            "required": true,
            "kind": "email"
          }
        ],
        "publicProjectionState": "listed"
      }
    ],
    "paidActions": [
      {
        "id": "download_report",
        "name": "Download report",
        "price": { "asset": "sats", "amount": 100 },
        "method": "GET",
        "path": "/api/reports/download",
        "checkoutPath": "/checkout/download-report",
        "entitlementScope": "action",
        "agentReadable": true,
        "settlementMode": "deferred",
        "customerDataRequirements": [],
        "publicProjectionState": "proof_only"
      }
    ]
  }
}
```

## Field Contract

Products and paid actions both carry:

- `id`: stable source key scoped to the Site;
- `name`: human-visible label;
- `price.asset`: `usd`, `sats`, or `credits`;
- `price.amount`: positive integer amount in the smallest useful unit for that
  asset contract;
- `checkoutPath`: clean Site-local absolute path for starting checkout;
- `entitlementScope`: `site`, `product`, `path`, `action`, or `account`;
- `agentReadable`: whether public agent manifests may describe this item;
- `settlementMode`: `checkout_only`, `deferred`, or `accepted_work_linked`;
- `customerDataRequirements`: non-secret customer fields such as email, name,
  text, or URL; and
- `publicProjectionState`: `hidden`, `listed`, `redacted`, or `proof_only`.

Paid actions additionally carry:

- `method`: `GET` or `POST`;
- `path`: clean Site-local absolute protected action path.

## D1 Catalog Tables

Migration `0065_site_commerce_catalog.sql` adds:

- `site_commerce_products`
- `site_commerce_paid_actions`

Both tables link records to:

- `site_id`;
- optional `site_version_id`;
- source key;
- price asset and amount;
- checkout path;
- entitlement scope;
- agent-readable flag;
- settlement mode;
- customer data requirement JSON;
- public projection state;
- creator; and
- active/archive timestamps.

The tables enforce clean local paths and positive prices at the database layer.
Secret-material rejection is enforced by the typed manifest validator before
catalog records are written.

## Rejected Material

The manifest validator rejects:

- provider-account secret-shaped values detected by the shared secret scanner;
- key names containing secret, token, mnemonic, preimage, invoice, wallet,
  credential, private key, webhook, grant, or payout;
- raw BOLT11 invoice prefixes such as `lnbc`, `lntb`, or `lnbcrt`;
- raw BOLT12 offer prefix `lno1`;
- preimage, mnemonic, xprv, MDK access-token, checkout-result, payment-hash,
  and payment-preimage strings; and
- checkout/action paths containing query strings or fragments.

The payments block is not a secret store. It must not contain MDK credentials,
wallet mnemonics, webhook secrets, raw invoices, preimages, provider grants,
payout destinations, checkout result query strings, or Treasury material.

## Boundary Between Money States

The schema intentionally separates:

| State | Owner | Meaning |
| --- | --- | --- |
| Checkout evidence | OpenAgents product surface/MDK checkout boundary | A buyer started, completed, failed, or cancelled a checkout. |
| Entitlement | OpenAgents product surface Site commerce catalog | A customer or agent may access a product/action after payment policy passes. |
| Accepted work | Autopilot/Nexus/Pylon authority | A work outcome was accepted by the relevant verifier. |
| Provider payout eligibility | Nexus/Pylon/Treasury policy | A provider may be paid for accepted work. |
| Settlement | Treasury/Nexus/Pylon receipt | The payment or payout has terminal settlement evidence. |

Checkout evidence does not imply accepted work. Entitlement does not imply a
provider payout. Accepted work does not imply settlement. Public projections
must show only the highest state backed by receipts.

## Current Limit

This issue defines and validates the manifest/catalog shape. It does not create
checkout intents, L402 challenges, entitlements, webhook reconciliation, or
public payment receipts. Those belong to the next issues in the commerce batch.
