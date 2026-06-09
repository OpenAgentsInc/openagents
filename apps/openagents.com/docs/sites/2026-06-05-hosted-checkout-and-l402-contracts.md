# Hosted Checkout And L402 Site Contracts

Issue #164 added the first OpenAgents product surface-hosted commerce contracts for Autopilot Sites.
These are contract stubs, not live MDK settlement yet. Their purpose is to give
static Sites, Worker-for-Platforms Sites, and agent clients a stable boundary
that never exposes MDK credentials, wallet material, raw invoices, preimages, or
provider payout secrets in generated Site source.

## Static Site Checkout Intent Pattern

A static/R2 Site should create checkout intents through OpenAgents product surface:

```http
POST /api/sites/{siteId}/commerce/checkout-intents
Idempotency-Key: site-order-123-product-abc-v1
Content-Type: application/json
```

```json
{
  "productId": "consultation-deposit",
  "price": { "amount": 25, "asset": "usd" },
  "spendCap": { "amount": 25, "asset": "usd" },
  "entitlementScope": "product",
  "successReturnPath": "/checkout/thanks",
  "cancelReturnPath": "/pricing",
  "customerData": {
    "email": "customer@example.com"
  }
}
```

The response returns a public-safe `checkoutIntent.id`, hosted `checkoutUrl`,
clean success/cancel paths, pending entitlement state, and explicit redaction
flags. It does not return raw invoices, payment hashes, wallet credentials, MDK
tokens, webhooks, or payout material.

Generated Sites must keep success and cancel paths local and clean: no query
strings, no fragments, no checkout result state in public URLs. OpenAgents product surface owns the
durable checkout result and entitlement reconciliation.

## WFP Site L402 Challenge Pattern

A Worker-for-Platforms or generated API Site should ask OpenAgents product surface for an L402
challenge when an agent calls a paid action:

```http
POST /api/sites/{siteId}/commerce/l402/challenges
Authorization: Bearer <OPENAGENTS_AGENT_TOKEN>
Idempotency-Key: site-action-123-generate-report-v1
Content-Type: application/json
```

```json
{
  "paidActionId": "generate-report",
  "method": "POST",
  "path": "/api/actions/generate-report",
  "price": { "amount": 1200, "asset": "sats" },
  "spendCap": { "amount": 1200, "asset": "sats" },
  "entitlementScope": "action"
}
```

OpenAgents product surface requires an active registered OpenAgents agent bearer token for this
write. It returns `402 Payment Required` with a standard `WWW-Authenticate`
L402 header that uses `challenge_ref` and `invoice_ref="redacted"` in this stub
phase. The JSON body contains the challenge id, expiry, method/path binding,
price, spend cap, entitlement state, and redaction flags.

When the hosted MDK payment boundary is live, the same contract should mint
real invoice/payment evidence server-side while still returning only safe refs
to generated Site code.

## Redemption Pattern

After a hosted payment flow returns a public-safe proof ref, the Site can ask
OpenAgents product surface to redeem the challenge:

```http
POST /api/sites/{siteId}/commerce/l402/redemptions
Authorization: Bearer <OPENAGENTS_AGENT_TOKEN>
Idempotency-Key: site-action-123-generate-report-redemption-v1
Content-Type: application/json
```

```json
{
  "challengeId": "site_l402_challenge_site_otec_l402-challenge-1",
  "challengeExpiresAt": "2026-06-05T18:10:00.000Z",
  "credentialId": "site_l402_credential_1",
  "paidActionId": "generate-report",
  "method": "POST",
  "path": "/api/actions/generate-report",
  "price": { "amount": 1200, "asset": "sats" },
  "entitlementScope": "action",
  "paymentProofRef": "mdk_payment_proof_12345678"
}
```

Stale challenges are rejected and must be refreshed. Fresh stub redemptions
return a granted entitlement stub. Future issues must replace the stub proof
with durable hosted MDK verification, one-shot credential consumption, and D1
ledger-backed replay protection.

## Current Contract Guarantees

- Every mutating call requires an `Idempotency-Key`.
- Product/action ids must be stable catalog ids.
- Prices must be greater than zero and within the declared spend cap.
- Action paths and return paths must be local absolute paths with no query
  string or fragment.
- Responses expose only public-safe refs and redaction flags.
- Raw MDK credentials, wallet material, raw invoices, preimages, private keys,
  and payment secrets are outside the Site source boundary.

## Current Limits

- No MDK invoice is created yet.
- No real checkout page is rendered yet.
- No durable checkout intent, L402 challenge, redemption, or entitlement ledger
  is written yet.
- Replay safety is contract-level and deterministic in this issue; durable
  one-shot redemption belongs in the next MDK/ledger implementation slice.
