# MDK Core-Backed Site Payment Helpers

Date: 2026-06-07
Issue: #443 / `OPENAGENTS-SITES-MDK-013`

## Summary

OpenAgents product surface now has a source-safe helper contract for generated Sites that need to
call OpenAgents-hosted Site commerce routes. The helpers keep payment authority
inside OpenAgents product surface. Generated static Sites and Worker-compatible Sites call OpenAgents product surface
APIs; they do not import MoneyDevKit native runtime packages, own MDK
credentials, hold wallet state, reconcile webhooks, or claim payout authority.

Implementation:

- `workers/api/src/site-mdk-generated-helpers.ts`
- `workers/api/src/site-mdk-generated-helpers.test.ts`

## Helper Coverage

The helper contract covers:

- payment discovery reads;
- checkout intent creation;
- checkout return reads for `success`, `cancel`, and `status`;
- payment proof reads;
- L402 challenge creation;
- L402 redemption;
- redacted helper error envelopes;
- static-site fetch examples;
- Worker-compatible / Workers for Platforms fetch examples.

All request bodies are validated against the current OpenAgents product surface Site commerce route
schemas:

- `SiteCheckoutIntentRequest`
- `SiteL402ChallengeRequest`
- `SiteL402RedemptionRequest`

The helper also declares parity with the already-ported MDK core conformance
fixtures for amount checkout, product checkout, metadata limits, safe return
paths, sandbox flags, L402 token parsing, price re-check, stale challenge, and
safe error envelopes.

## Static Site Example

A generated static Site should call OpenAgents product surface directly:

```js
const OPENAGENTS_API_BASE = 'https://openagents.com'

export async function readSitePaymentDiscovery(siteId) {
  return fetch(`${OPENAGENTS_API_BASE}/api/sites/${encodeURIComponent(siteId)}/commerce/discovery`, {
    method: 'GET',
  })
}

export async function createSiteCheckoutIntent(siteId, idempotencyKey, body) {
  return fetch(`${OPENAGENTS_API_BASE}/api/sites/${encodeURIComponent(siteId)}/commerce/checkout-intents`, {
    body: JSON.stringify(body),
    headers: {
      'content-type': 'application/json',
      'idempotency-key': idempotencyKey,
    },
    method: 'POST',
  })
}

export async function readSiteCheckoutStatus(siteId, checkoutIntentRef) {
  return fetch(`${OPENAGENTS_API_BASE}/api/sites/${encodeURIComponent(siteId)}/commerce/checkout-returns/${encodeURIComponent(checkoutIntentRef)}/status`, {
    method: 'GET',
  })
}

export async function requestSitePaidAction(siteId, idempotencyKey, body, agentBearerToken) {
  return fetch(`${OPENAGENTS_API_BASE}/api/sites/${encodeURIComponent(siteId)}/commerce/l402/challenges`, {
    body: JSON.stringify(body),
    headers: {
      authorization: `Bearer ${agentBearerToken}`,
      'content-type': 'application/json',
      'idempotency-key': idempotencyKey,
    },
    method: 'POST',
  })
}
```

## Worker-Compatible Site Example

A generated Worker-compatible Site should call an OpenAgents product surface commerce binding or
otherwise fetch the same OpenAgents product surface routes. The generated Site still does not own
MDK credentials.

```js
export async function callOpenAgentsSiteCommerce(env, path, init) {
  const url = new URL(path, 'https://openagents.com')
  return env.OPENAGENTS_COMMERCE.fetch(new Request(url, init))
}

export async function createSiteL402Challenge(env, siteId, idempotencyKey, body, agentBearerToken) {
  return callOpenAgentsSiteCommerce(
    env,
    `/api/sites/${encodeURIComponent(siteId)}/commerce/l402/challenges`,
    {
      body: JSON.stringify(body),
      headers: {
        authorization: `Bearer ${agentBearerToken}`,
        'content-type': 'application/json',
        'idempotency-key': idempotencyKey,
      },
      method: 'POST',
    },
  )
}

export async function redeemSiteL402(env, siteId, idempotencyKey, body, agentBearerToken) {
  return callOpenAgentsSiteCommerce(
    env,
    `/api/sites/${encodeURIComponent(siteId)}/commerce/l402/redemptions`,
    {
      body: JSON.stringify(body),
      headers: {
        authorization: `Bearer ${agentBearerToken}`,
        'content-type': 'application/json',
        'idempotency-key': idempotencyKey,
      },
      method: 'POST',
    },
  )
}
```

## Agent Instructions

When generating or modifying a Site that accepts payment:

1. Read `GET /api/sites/{siteId}/commerce/discovery` first.
2. Choose a typed catalog item by `catalogRef`, `productId`, or `actionId`.
   Do not infer payment intent from arbitrary text.
3. Use a stable `Idempotency-Key` for every checkout intent, L402 challenge,
   and L402 redemption.
4. Use clean Site-local return paths. Do not include query strings, fragments,
   absolute URLs, or checkout state in canonical return URLs.
5. For a human buyer flow, call
   `POST /api/sites/{siteId}/commerce/checkout-intents`.
6. For an agent-paid action, call
   `POST /api/sites/{siteId}/commerce/l402/challenges`, then redeem through
   `POST /api/sites/{siteId}/commerce/l402/redemptions` after payment proof is
   available. Both writes require an active registered OpenAgents agent bearer
   token. Do not embed that token in generated public Site source; pass it from
   the calling agent runtime.
7. Always set a spend cap for paid actions. For bitcoin-denominated L402 test
   calls, the current route field may use `sats` as the denomination value.
8. Read `GET /api/sites/{siteId}/commerce/payment-proofs/{checkoutIntentRef}`
   before making any public payment or entitlement claim.
9. Treat fake-provider or sandbox/signet evidence as smoke evidence only, not
   production payment, payout, or settlement evidence.

## Safety Rules

Generated Site source must never include:

- MDK credentials or access tokens;
- wallet mnemonics or wallet state;
- webhook secrets;
- raw invoices;
- payment hashes;
- payment preimages;
- provider grants;
- checkout query-state dependencies;
- customer private values;
- payout or settlement claims.

Generated Sites should not import `@moneydevkit/*`, `@moneydevkit/lightning-js`,
or any native Lightning runtime package. OpenAgents product surface owns the payment boundary and
the exact-source webhook reconciliation path.

## Verification

Run the focused helper suite:

```bash
bun run --cwd workers/api test -- src/site-mdk-generated-helpers.test.ts
```

Run the broader API checks:

```bash
bun run --cwd workers/api typecheck
bun run --cwd workers/api test
```
