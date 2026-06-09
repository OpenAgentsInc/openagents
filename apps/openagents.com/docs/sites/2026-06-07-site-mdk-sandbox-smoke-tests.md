# Site MDK Sandbox Smoke Tests

Date: 2026-06-07
Issue: #442 / `OPENAGENTS-SITES-MDK-012`

## Summary

OpenAgents product surface now has repeatable Site MDK smoke coverage for the current
fake-provider Site commerce lane. The smoke proves that the generated-Site
payment surface can run through discovery, checkout intent creation, clean
return status, provider reconciliation, replay handling, payment proof,
L402 challenge creation, L402 redemption, stale challenge rejection, spend-cap
rejection, and redaction.

This is smoke evidence only. Passing fake-provider or sandbox/signet smoke does
not prove production payment, wallet spend, provider payout, accepted-work
payout, or settlement authority.

## CI Smoke Command

Run the focused smoke suite from the OpenAgents product surface repository root:

```bash
bun run --cwd workers/api test -- src/generated-site-payment-smoke-fixture.test.ts src/site-mdk-smoke.test.ts src/site-commerce-routes.test.ts
```

Expected result:

```text
Test Files  3 passed
Tests       26 passed
```

The focused suite includes:

- `site-mdk-smoke.test.ts`, which validates the smoke projection helper,
  required check names, implementation-state labels, fake/sandbox/live
  classification, authority denials, and raw secret/payment-material rejection.
- `generated-site-payment-smoke-fixture.test.ts`, which validates the
  generated-Site fixture artifact added in #454: one human checkout product,
  one agent-paid action, manifest/catalog/discovery projections, helper request
  plans, record-only smoke evidence, and redaction coverage.
- `site-commerce-routes.test.ts`, which exercises the fake-provider Site route
  flow end-to-end through checkout, clean return status, verified webhook
  reconciliation, replay handling, payment proof, L402 challenge, redemption,
  stale challenge rejection, spend-cap rejection, response redaction, and the
  #455 generated-Site human checkout fixture path, the #456 generated-Site
  agent-paid L402 action smoke, and the #457 generated-Site reconciliation
  smoke. Together those generated-Site cases prove registered-agent bearer
  gating, spend-cap preview, challenge creation, unsafe proof rejection,
  entitlement-stub redemption, deterministic idempotent replay, exact-source
  dashboard Standard Webhooks verification, duplicate provider-event replay,
  receipt/entitlement creation after verified payment state, and redaction.

## Smoke Helper

`workers/api/src/site-mdk-smoke.ts` is the reusable test helper and projection
contract. It models:

- implementation state: `fake_provider`, `sandbox_signet`, or `live_provider`;
- required checks: discovery, checkout intent, clean return status, provider
  reconciliation, provider replay, payment proof, L402 challenge, L402
  redemption, stale rejection, spend-cap rejection, and redaction;
- record-only authority: no wallet spend, no live mainnet requirement, no
  provider payout, no accepted-work payout, no settlement, and no deployment
  authority;
- public-safe refs only, with raw invoice, payment hash, preimage, mnemonic,
  MDK token, webhook secret, wallet material, customer private value, provider
  credential, private source, and raw timestamp rejection.

The helper returns a projection with `notProductionPaymentEvidence: true` for
all fake-provider and sandbox/signet runs. A live-provider smoke can be
classified as `live_provider`, but it still cannot create payout, settlement,
wallet-spend, or deployment authority by itself.

## Covered Flow

The route smoke uses the current `site_otec` fixture and an intentionally fake
MDK hosted provider:

1. `GET /api/sites/site_otec/commerce/discovery`
2. `POST /api/sites/site_otec/commerce/checkout-intents`
3. `GET /api/sites/site_otec/commerce/checkout-returns/{checkoutIntentRef}/status`
4. `POST /api/sites/site_otec/commerce/mdk/webhooks`
5. repeated webhook delivery to verify replay handling
6. `GET /api/sites/site_otec/commerce/payment-proofs/{checkoutIntentRef}`
7. `POST /api/sites/site_otec/commerce/l402/challenges`
8. `POST /api/sites/site_otec/commerce/l402/redemptions`
9. stale redemption rejection
10. spend-cap rejection
11. response scan for prohibited payment or credential material

The generated `site_payment_smoke` fixture now covers the same boundary for a
generated customer Site: checkout creation starts unpaid, a deterministic
dashboard Standard Webhooks event verifies payment receipt, a duplicate
provider event becomes a replay, and the clean return plus payment-proof routes
project an active entitlement without creating payout authority.

The L402 challenge and redemption routes require an active registered
OpenAgents agent bearer token and an `Idempotency-Key`. The L402 fixture
currently uses the existing route asset field for a bitcoin denomination value.
Do not broaden that into a public promise that production MDK settlement is
live.

## Optional Live Or Sandbox Provider Run

The code is ready to classify an operator-run smoke as `sandbox_signet` or
`live_provider` once the provider route is configured. The required operator
inputs must stay in secret bindings or ignored local env files:

- `MDK_ACCESS_TOKEN`
- `MDK_MNEMONIC`
- `MDK_WEBHOOK_SECRET`, or the exact webhook secret for the configured MDK
  event source
- any provider route secret used by OpenAgents product surface's hosted MDK route client

Do not print those values. Do not put them in fixtures, public output, GitHub
issues, docs, commit messages, or customer-visible projections.

For an operator live/sandbox run, expected public output is limited to:

- smoke ref;
- implementation state;
- check names and pass/skip/fail status;
- public-safe checkout intent refs;
- public-safe challenge or redemption refs;
- public-safe receipt/proof refs;
- explicit authority denials.

If the run uses fake-provider or sandbox/signet evidence, the result must say
it is not production payment or settlement evidence.

## Production Release Boundary

These smokes do not replace the release evidence required for production Site
commerce. Production claims still need:

- live MDK provider configuration evidence;
- exact webhook-source verification;
- replay-safe reconciliation retained in D1;
- buyer receipt and entitlement projection;
- clean return status without checkout query strings;
- no secret-shaped response material;
- operator-approved bridge from verified buyer payment to any payout intent;
- separate Pylon/Nexus/Treasury release gates before accepted-work payout
  claims.

Passing this smoke means the Site commerce shape is coherent and regression
covered. It does not mean a customer payment has settled or that any worker,
agent, provider, or Pylon can spend bitcoin.
