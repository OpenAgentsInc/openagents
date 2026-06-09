# Site Payment Primitive SDK Notes

Date: 2026-06-07
Issue: #444 / `OPENAGENTS-SITES-MDK-014`

## Summary

OpenAgents Sites now has a coherent payment primitive surface for generated
Sites, agents, and operators. The surface is intentionally OpenAgents product surface-hosted:
generated Sites call OpenAgents commerce APIs, and OpenAgents product surface owns checkout
configuration, webhook reconciliation, proof projection, bridge policy, and
secret handling.

This document is the concise SDK-style reference for the currently implemented
Site payment primitives. It does not claim production settlement, provider
payout, accepted-work payout, or wallet spend unless separate receipt evidence
exists.

## Current Implementation States

| Surface | State | Notes |
| --- | --- | --- |
| Payment discovery | Live | Public-safe read for catalog items, endpoints, prices, spend-cap hints, entitlement semantics, and implementation states. |
| Commerce review | Live | Customer/operator read projection of generated checkout products and paid actions. |
| Commerce review decisions | Operator-gated | Admin-token write path only. Does not create payment, access, deployment, payout, or settlement authority. |
| Checkout intent | Config-gated | Uses OpenAgents product surface's hosted MDK-compatible route client when configured; otherwise returns explicit missing-configuration state. |
| Checkout return | Live | Reads durable checkout state for `success`, `cancel`, or `status`; does not consume checkout query strings. |
| MDK webhook reconciliation | Config-gated | Exact-source verification for dashboard Standard Webhooks, daemon invoice HMAC, or SDK node-control callbacks. |
| Payment proof | Live | Public-safe buyer-side proof over checkout intent, receipt, reconciliation, and entitlement state. |
| L402 challenge | Live contract | Creates a challenge envelope for declared paid actions. Does not spend funds. |
| L402 redemption | Live contract | Records redacted proof refs for a challenge. Does not prove accepted-work payout or final settlement. |
| Customer-owned MDK account mode | Live read / operator-gated write | Hosted secret-binding refs only. No generated source credentials. |
| Payment-to-payout bridge | Operator-gated | Requires verified server-side buyer payment evidence and separate Nexus/Treasury/Pylon release gates. |
| Smoke tests | Live fake-provider CI | Fake-provider smoke proves flow shape and redaction, not production payment or settlement. |
| Generated helper contract | Live source contract | Static and Worker-compatible helper examples call OpenAgents product surface routes without MDK native runtime imports. |

## Discovery First

Agents and generated Sites should always start with discovery:

```bash
curl https://openagents.com/api/sites/SITE_ID/commerce/discovery
```

Discovery returns:

- checkout products and paid actions;
- stable catalog refs, product IDs, action IDs, methods, and paths;
- prices and denominations;
- checkout, return, proof, L402, review, and account-binding endpoints;
- sandbox/fake/live/gated implementation states;
- spend-cap hints;
- entitlement semantics;
- L402 header semantics;
- redaction guarantees.

Do not infer payment intent from arbitrary text. Select a typed catalog item by
`catalogRef`, `productId`, or `actionId`.

## Checkout Products

Use checkout products when a human buyer should open a hosted checkout flow.

```bash
curl -X POST https://openagents.com/api/sites/SITE_ID/commerce/checkout-intents \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: checkout-SITE_ID-PRODUCT_ID-REQUEST_ID" \
  -d '{
    "itemKind": "product",
    "productId": "consultation_deposit",
    "siteVersionId": "version_site_otec_v2",
    "expectedPrice": {
      "amountMinorUnits": 2500,
      "asset": "usd",
      "denomination": "usd_cent"
    },
    "customerDataRefs": ["email"],
    "successReturnPath": "/checkout/thanks",
    "cancelReturnPath": "/pricing"
  }'
```

Rules:

- `Idempotency-Key` is required.
- `successReturnPath` and `cancelReturnPath` must be clean Site-local paths.
- Do not include query strings, fragments, absolute URLs, raw provider checkout
  IDs, or checkout state in canonical URLs.
- Generated Site source must not contain MDK credentials, wallet material,
  raw invoices, payment hashes, preimages, webhook secrets, provider grants, or
  private customer values.

## Clean Checkout Returns

Generated Sites should read OpenAgents product surface's return state rather than trusting browser
query parameters:

```bash
curl https://openagents.com/api/sites/SITE_ID/commerce/checkout-returns/CHECKOUT_INTENT_REF/status
curl https://openagents.com/api/sites/SITE_ID/commerce/checkout-returns/CHECKOUT_INTENT_REF/success
curl https://openagents.com/api/sites/SITE_ID/commerce/checkout-returns/CHECKOUT_INTENT_REF/cancel
```

The return projection can show pending, unpaid, paid, entitled, expired, or
blocked state depending on durable checkout, receipt, and entitlement records.
It is not provider payout evidence and is not accepted-work payout evidence.

## Payment Proofs

Read buyer-side proof before showing entitlement or payment claims:

```bash
curl https://openagents.com/api/sites/SITE_ID/commerce/payment-proofs/CHECKOUT_INTENT_REF
```

Payment proof reads summarize:

- checkout intent state;
- buyer payment receipt, if present;
- MDK reconciliation event, if present;
- entitlement state, if present;
- explicit denial of accepted-work payout, provider payout, wallet-state, and
  final settlement claims.

Payment proof is suitable for public-safe Site status displays. It is not
Treasury settlement evidence.

## Paid Actions And L402

Use paid actions when an agent or user should pay before calling a generated
Site route.

Challenge:

```bash
curl -X POST https://openagents.com/api/sites/SITE_ID/commerce/l402/challenges \
  -H "Authorization: Bearer $OPENAGENTS_AGENT_TOKEN" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: l402-challenge-SITE_ID-ACTION_ID-REQUEST_ID" \
  -d '{
    "paidActionId": "generate-report",
    "method": "POST",
    "path": "/api/actions/generate-report",
    "entitlementScope": "action",
    "price": {
      "amount": 1200,
      "asset": "sats"
    },
    "spendCap": {
      "amount": 1200,
      "asset": "sats"
    }
  }'
```

Redemption:

```bash
curl -X POST https://openagents.com/api/sites/SITE_ID/commerce/l402/redemptions \
  -H "Authorization: Bearer $OPENAGENTS_AGENT_TOKEN" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: l402-redemption-SITE_ID-ACTION_ID-REQUEST_ID" \
  -d '{
    "challengeId": "SITE_L402_CHALLENGE_ID",
    "challengeExpiresAt": "CHALLENGE_EXPIRY_ISO",
    "credentialId": "PUBLIC_SAFE_CREDENTIAL_REF",
    "paymentProofRef": "mdk_payment_proof_PUBLIC_SAFE_REF",
    "paidActionId": "generate-report",
    "method": "POST",
    "path": "/api/actions/generate-report",
    "entitlementScope": "action",
    "price": {
      "amount": 1200,
      "asset": "sats"
    }
  }'
```

Use bitcoin in prose and user-facing product language. The current L402 route
field uses `sats` as a denomination value in these contract examples.

Rules:

- The price must fit within the supplied spend cap.
- Challenge and redemption writes require an active registered OpenAgents
  agent bearer token plus an `Idempotency-Key`. Generated public Site source
  must not embed or display that token; the calling agent supplies it from its
  own private runtime.
- Payment cannot grant missing owner, privacy, safety, moderation, repository,
  deployment, or payout authority.
- Redemptions accept redacted proof refs only. Do not submit raw invoices,
  payment hashes, preimages, wallet secrets, or provider secrets.
- Expired challenges must be replaced with fresh challenges.
- The current redemption route returns an entitlement stub. Live proof
  verification, durable reconciliation, and settlement evidence are separate
  release-gated work.

## Generated Helper Contract

Use the helper contract in:

- `workers/api/src/site-mdk-generated-helpers.ts`
- `docs/sites/2026-06-07-mdk-core-backed-site-helpers.md`

The helper contract builds source-safe request plans for:

- discovery reads;
- checkout intent creation;
- checkout return reads;
- payment proof reads;
- L402 challenge creation;
- L402 redemption;
- redacted helper error envelopes.

It validates request bodies against current OpenAgents product surface route schemas and rejects
query-state paths, invalid idempotency keys, spend-cap overflow, unsafe API
bases, raw payment material, and MDK native runtime imports.

Static generated Sites should call `https://openagents.com` directly. Worker
or Workers for Platforms Sites should call an OpenAgents commerce binding or
the same OpenAgents product surface route boundary. Neither should own MDK credentials.

## Customer-Owned MDK Account Mode

Customers may eventually bring their own MDK merchant account through a
reviewed hosted-secret path. The live customer-safe read is:

```bash
curl https://openagents.com/api/sites/SITE_ID/commerce/mdk-account-binding
```

Operator writes are admin-token gated:

```bash
curl -X POST https://openagents.com/api/sites/SITE_ID/commerce/mdk-account-bindings \
  -H "Authorization: Bearer OPENAGENTS_ADMIN_API_TOKEN" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: mdk-account-binding-SITE_ID-REQUEST_ID" \
  -d '{
    "bindingRef": "site_mdk_account:SITE_ID:customer_wallet",
    "requestedProviderMode": "customer_owned_mdk",
    "reviewStatus": "approved",
    "environment": "sandbox",
    "siteVersionId": "VERSION_ID",
    "customerRef": "customer.SITE_ID",
    "orderRef": "order.SITE_ID",
    "secretBindingRefs": ["hosted_secret.site_mdk_account.SITE_ID.mdk"],
    "allowedCatalogRefs": ["site_payment.SITE_ID.VERSION_ID.product.example"],
    "allowedProductRefs": ["example"],
    "allowedActionRefs": [],
    "reviewerRefs": ["operator.site_mdk_account"],
    "caveatRefs": ["caveat.site_mdk_account.binding_reviewed"]
  }'
```

The request may contain hosted secret-binding refs only. Do not put MDK tokens,
mnemonics, webhook secrets, wallet state, raw invoices, payment hashes,
preimages, provider grants, or private customer values in the request body or
generated source.

## Webhook Reconciliation

MDK webhook reconciliation is a provider callback path, not a normal agent
route:

```text
POST /api/sites/{siteId}/commerce/mdk/webhooks
```

OpenAgents product surface verifies the configured source family:

- dashboard Standard Webhooks;
- daemon invoice HMAC;
- SDK node-control callback secret.

Verified callbacks can update durable checkout status and create buyer payment
receipts and entitlements. Replays are idempotent. Webhook reconciliation does
not create provider payout, accepted-work payout, wallet spend, deployment, or
settlement authority.

## Payment-To-Payout Bridge

The payment-to-payout bridge is intentionally separate:

```text
POST /api/sites/{siteId}/commerce/payout-bridges
```

It is operator-authorized and requires:

- verified server-side checkout intent state;
- buyer payment receipt;
- matched MDK reconciliation event;
- accepted-work refs;
- payout target approval;
- wallet readiness;
- amount and spend-cap checks;
- Pylon/Nexus/Treasury release-gate evidence.

Checkout return URLs, client success claims, raw provider events, duplicate
buyer payment refs, and public agent claims cannot create payout intents.

## Operator Runbook

For production MDK configuration:

1. Configure `MDK_ACCESS_TOKEN`, `MDK_MNEMONIC`, and the exact webhook secret
   as Cloudflare Worker secrets or equivalent provider-side bindings.
2. Configure the MDK-compatible route sidecar or pure hosted platform route.
   Worker code must not import `@moneydevkit/lightning-js` or run MDK native
   node control inside Cloudflare Workers.
3. Configure the exact webhook source and verify only that signature family.
4. Run fake-provider CI smoke:

   ```bash
   bun run --cwd workers/api test -- src/site-mdk-smoke.test.ts src/site-commerce-routes.test.ts
   ```

5. Run helper parity tests:

   ```bash
   bun run --cwd workers/api test -- src/site-mdk-generated-helpers.test.ts
   ```

6. Run the full API suite before deploy:

   ```bash
   bun run --cwd workers/api typecheck
   bun run --cwd workers/api test
   bun run check:deploy
   ```

7. Treat fake-provider or sandbox/signet success as shape and redaction
   evidence only. Keep production payment, payout, and settlement claims gated
   on retained receipts.

## Evidence Separation

Keep these states distinct:

- **Payment evidence:** buyer checkout or L402 proof refs.
- **Entitlement evidence:** server-created access record for a product/action.
- **Accepted-work evidence:** customer/operator accepted outcome refs.
- **Payout intent:** Treasury/Nexus intent to pay after policy gates.
- **Payout dispatch:** approved dispatch attempt to a payout target.
- **Settlement evidence:** retained proof of actual bitcoin movement.

Do not collapse any one of those into another in docs, generated source,
customer UI, API responses, GitHub issues, or agent instructions.
