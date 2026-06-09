# Generated Site Payment Smoke Runbook

Date: 2026-06-07
Issue: #458 / `OPENAGENTS-SITES-MDK-LIVE-005`

## Purpose

This runbook is the public-safe evidence map for the generated-Site payment
smoke batch.

It explains what the current OpenAgents product surface tests prove, how an agent should inspect a
generated Site payment surface, and what an operator must verify before making
stronger live-payment or settlement claims.

## Fixture

The deterministic generated-Site fixture is:

- Site: `site_payment_smoke`
- Version: `version_site_payment_smoke_v1`
- Human checkout product: `human_brief_checkout`
- Agent-paid action: `agent_research_note`
- Human checkout route: `/checkout/brief`
- Agent-paid action route: `/api/actions/research-note`
- Source: `workers/api/src/generated-site-payment-smoke-fixture.ts`

The fixture is safe for docs, manifests, and examples. It does not create a
live checkout, real invoice, deployment, wallet spend, payout, or settlement
authority.

## Evidence Map

| Issue | Evidence | What it proves |
| --- | --- | --- |
| #454 | `generated-site-payment-smoke-fixture.ts` and `generated-site-payment-smoke-fixture.test.ts` | A generated Site can publish a public-safe payment manifest with one human checkout product, one agent-paid action, helper plans, smoke projection refs, and no secret or raw payment material. |
| #455 | `runs generated Site human checkout fixture through commerce APIs without payment verification` | Discovery, checkout intent creation, durable challenge/intent persistence, clean return status, and checkout-created versus payment-verified separation. |
| #456 | `runs generated Site agent-paid L402 action smoke with registered agent gating` | Registered-agent gating, dry-run spend-cap preview, L402 challenge, over-cap rejection, unsafe proof rejection, entitlement-stub redemption, idempotent replay, and retry projection. |
| #457 | `runs generated Site checkout reconciliation through exact-source MDK webhook smoke` | Dashboard Standard Webhooks verification, payment-received transition, receipt/entitlement/reconciliation creation, duplicate provider-event replay, clean success return, and public payment proof. |

## Deterministic Verification

Run from the OpenAgents product surface repository root:

```bash
bun run --cwd workers/api test -- src/generated-site-payment-smoke-fixture.test.ts src/site-mdk-smoke.test.ts src/site-commerce-routes.test.ts
```

Expected result:

```text
Test Files  3 passed
Tests       26 passed
```

For route/projection coverage:

```bash
bun run --cwd workers/api test -- src/site-checkout-return.test.ts src/site-mdk-reconciliation.test.ts src/site-payment-proof.test.ts src/site-commerce-routes.test.ts src/site-mdk-webhooks.test.ts src/redaction-regression.test.ts
```

Expected result:

```text
Test Files  6 passed
Tests       72 passed
```

For release safety after Worker changes:

```bash
bun run --cwd workers/api typecheck
bun run --cwd workers/api test
git diff --check
bun run check:deploy
```

## Agent Inspection Flow

Agents should start with public reads:

```bash
curl https://openagents.com/.well-known/openagents.json
curl https://openagents.com/api/openapi.json
curl https://openagents.com/AGENTS.md
```

For a known Site id, inspect payment discovery first:

```bash
curl https://openagents.com/api/sites/SITE_ID/commerce/discovery
```

Discovery tells the agent which products and paid actions exist, which
endpoints are available, which surfaces are fake-provider, configured, gated,
or planned, and what spend-cap hints apply.

Before proposing generated checkout UI, inspect review state:

```bash
curl https://openagents.com/api/sites/SITE_ID/commerce/review
```

For a human checkout product, create a checkout intent only with an
`Idempotency-Key` and only when the owner has authorized the action:

```bash
curl -X POST https://openagents.com/api/sites/SITE_ID/commerce/checkout-intents \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: site-checkout-YOUR_UNIQUE_KEY" \
  -d '{
    "itemKind": "product",
    "productId": "PRODUCT_ID",
    "siteVersionId": "SITE_VERSION_ID",
    "customerDataRefs": ["PUBLIC_SAFE_CUSTOMER_DATA_REF"],
    "successReturnPath": "/checkout/complete",
    "cancelReturnPath": "/checkout/cancel"
  }'
```

Read clean return state from server-side records:

```bash
curl https://openagents.com/api/sites/SITE_ID/commerce/checkout-returns/CHECKOUT_INTENT_REF/status
curl https://openagents.com/api/sites/SITE_ID/commerce/checkout-returns/CHECKOUT_INTENT_REF/success
```

Read public-safe payment proof:

```bash
curl https://openagents.com/api/sites/SITE_ID/commerce/payment-proofs/CHECKOUT_INTENT_REF
```

For an agent-paid action, create an L402 challenge only with a registered
OpenAgents agent bearer token, an `Idempotency-Key`, and a human-approved spend
cap:

```bash
curl -X POST https://openagents.com/api/sites/SITE_ID/commerce/l402/challenges \
  -H "Authorization: Bearer <OPENAGENTS_AGENT_TOKEN>" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: site-l402-YOUR_UNIQUE_KEY" \
  -d '{
    "paidActionId": "ACTION_ID",
    "path": "/api/actions/example",
    "method": "POST",
    "entitlementScope": "action",
    "price": {"amount": 250, "asset": "sats"},
    "spendCap": {"amount": 250, "asset": "sats"}
  }'
```

The route uses `sats` as the current wire value for the bitcoin-denominated
asset field. Public prose should still say bitcoin unless clarifying that wire
denomination.

Redeem only with a public-safe MDK proof ref and owner approval:

```bash
curl -X POST https://openagents.com/api/sites/SITE_ID/commerce/l402/redemptions \
  -H "Authorization: Bearer <OPENAGENTS_AGENT_TOKEN>" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: site-l402-redeem-YOUR_UNIQUE_KEY" \
  -d '{
    "challengeId": "CHALLENGE_ID",
    "credentialId": "CREDENTIAL_ID",
    "paidActionId": "ACTION_ID",
    "path": "/api/actions/example",
    "method": "POST",
    "entitlementScope": "action",
    "challengeExpiresAt": "CHALLENGE_EXPIRY_REF_OR_TIMESTAMP",
    "paymentProofRef": "mdk_payment_proof_PUBLIC_SAFE_REF",
    "price": {"amount": 250, "asset": "sats"}
  }'
```

The current generated-Site L402 redemption route accepts public-safe proof refs
into an entitlement stub. It does not prove live proof verification, real
bitcoin movement, accepted-work payout, or settlement.

## Operator Live-Provider Checklist

Before claiming live generated-Site MDK checkout support, an operator must
verify:

- the Worker environment has a configured MDK-compatible checkout route or
  sidecar;
- the route kind is `hosted_platform` or `self_hosted_mdkd_sidecar`, not the
  fake provider;
- server-only MDK credentials and webhook verification material are present as
  secrets or equivalent server-only bindings;
- the exact webhook source is selected: dashboard Standard Webhooks, daemon
  invoice HMAC, or SDK node-control;
- the live callback writes a durable checkout status transition;
- duplicate callbacks are replay-safe;
- receipt, entitlement, reconciliation, checkout-return, and payment-proof
  projections stay public-safe;
- no public output contains raw invoices, payment hashes, preimages, wallet
  state, MDK credentials, webhook secrets, provider grants, customer private
  values, private payout targets, or exact private balances; and
- any payment-to-payout bridge uses separate Nexus/Treasury authority and does
  not rely on browser return state or client success claims.

## Evidence Classification

| Evidence type | Meaning | Production claim allowed |
| --- | --- | --- |
| Deterministic fake-provider smoke | Route and projection shape is coherent and redaction-safe. | No live payment claim. |
| Configured hosted-provider or sidecar smoke | OpenAgents product surface verified a configured provider callback in the selected environment. | Live checkout evidence only, if receipts are retained. |
| Real bitcoin movement proof | A separate approved wallet movement occurred through OpenAgents product surface authority and receipt boundaries. | Movement claim only for the named bounded smoke. |
| Accepted-work payout settlement | Nexus/Treasury/Pylon gates accepted work and settlement receipts. | Only after separate accepted-work payout evidence exists. |

## Redaction Rules

Public runbooks, manifests, emails, proof pages, logs, and issue comments must
not include:

- MDK access tokens, mnemonics, webhook secrets, or route secrets;
- raw invoices or offers;
- raw payment hashes or preimages;
- wallet paths, wallet config, exact private balances, or wallet material;
- provider grants, provider credentials, or raw provider payloads;
- private customer identifiers or customer-submitted values;
- private payout targets; or
- raw source archives, private repositories, or runner logs.

If any of those appear in a projected response, public doc, issue comment, or
runbook, treat it as a release blocker and add redaction regression coverage
before continuing.
