# MDK Agent-Wallet Sandbox Smoke Plan

Issue #165 defines the internal smoke path that must pass before OpenAgents
publishes public MDK wallet instructions for Autopilot Sites. This is a
runbook for operators and agents. It does not enable live production payment
authority by itself.

## Boundary

MDK is buyer-side payment evidence for Sites checkout and L402 paid actions.
It is not Pylon payout authority, not accepted-work eligibility, and not
provider settlement truth.

The split is:

```text
generated Site / agent
-> receives public-safe checkout or L402 challenge from OpenAgents product surface
-> pays with MDK agent-wallet, pay402, or another Lightning wallet
-> returns public-safe proof refs to OpenAgents product surface
-> OpenAgents product surface grants entitlement and records receipt refs

Pylon / Nexus / Treasury
-> remain responsible for provider identity, accepted-work eligibility,
   company spend authority, payout dispatch, reconciliation, and settlement
```

## Evidence Reviewed

The smoke plan is based on:

- `docs/2026-06-02-mdk-l402-agent-checkout-audit.md`
- `docs/sites/2026-06-05-hosted-checkout-and-l402-contracts.md`
- `projects/moneydevkit/repos/mdk-checkout/packages/agent-wallet/README.md`
- `projects/moneydevkit/repos/mdk-checkout/packages/core/src/pay402.ts`
- `projects/moneydevkit/repos/mdk-checkout/packages/core/src/mdk402/with-payment.ts`
- `projects/moneydevkit/repos/mdk-checkout/packages/core/src/mdk402/token.ts`
- `projects/moneydevkit/repos/mdk-checkout/packages/core/tests/mdk402-with-payment.test.ts`
- `projects/moneydevkit/repos/mdk-checkout/packages/core/tests/pay402.test.ts`

Important MDK behaviors from the reference code:

- `@moneydevkit/agent-wallet` is self-custodial, stores local wallet state under
  `~/.mdk-wallet/`, starts a local daemon, and supports `init --network signet`.
- `pay402` expects a `WWW-Authenticate: L402 macaroon="...", invoice="..."`
  challenge, checks invoice amount against `maxAmountSats`, pays, then retries
  with `Authorization: L402 <credential>:<preimage>`.
- `withPayment` and `withDeferredSettlement` mint L402 credentials and bind
  amount, currency, resource, expiry, and sandbox state.
- Sandbox checkouts can emit `sandbox="true"` in the L402 header and JSON body;
  sandbox credentials can bypass real preimage verification only when signed as
  sandbox.
- Deferred settlement lets paid retries continue until the service succeeds and
  the credential is consumed.

## Required Local Secrets And Env Names

Do not commit values for any of these.

OpenAgents hosted MDK boundary:

```text
MDK_ACCESS_TOKEN
MDK_API_BASE_URL
MDK_WEBHOOK_SECRET
MDK_ENVIRONMENT
OPENAGENTS_SITE_COMMERCE_SANDBOX
```

Local buyer wallet:

```text
MDK_WALLET_NETWORK
MDK_WALLET_PORT
MDK_WALLET_MNEMONIC
```

Optional smoke controls:

```text
OPENAGENTS_SITE_COMMERCE_SMOKE_SITE_ID
OPENAGENTS_SITE_COMMERCE_SMOKE_PRODUCT_ID
OPENAGENTS_SITE_COMMERCE_SMOKE_ACTION_ID
OPENAGENTS_SITE_COMMERCE_SMOKE_MAX_AMOUNT_SATS
```

Rules:

- Use `signet` or explicit MDK sandbox mode for this smoke.
- Never paste mnemonic, access token, raw invoice, preimage, payment hash, or
  webhook secret into tracked docs, public proof, issue comments, screenshots,
  generated Site source, static JS, or model-visible prompts.
- Public logs may include only redacted refs such as `checkoutIntentId`,
  `challengeId`, `redemptionId`, `paymentProofRef`, `receiptId`, and
  entitlement state.

## Smoke Topology

Use two roles:

1. **OpenAgents product surface hosted boundary.** Creates checkout intents, creates L402
   challenges, verifies/redacts hosted MDK evidence, grants entitlements, and
   writes D1 receipt/projection records.
2. **Buyer agent wallet.** Pays invoices through `@moneydevkit/agent-wallet`,
   `pay402`, or another Lightning/L402 client.

Do not run merchant wallet authority inside generated Site source. Static Sites
call OpenAgents product surface. WFP Sites call OpenAgents product surface through fetch/service binding. Customer-owned
MDK accounts are a later reviewed secret-binding mode.

## Preparation

1. Confirm the current Site commerce contracts are deployed in an internal
   environment.
2. Pick a disposable smoke Site and action:

```text
siteId=site_mdk_smoke
productId=consultation-deposit
paidActionId=generate-report
price=100 sats
spendCap=100 sats
expiry=5 to 15 minutes
```

3. Initialize a local buyer wallet on signet:

```bash
MDK_WALLET_NETWORK=signet npx @moneydevkit/agent-wallet@latest init
npx @moneydevkit/agent-wallet@latest status
npx @moneydevkit/agent-wallet@latest balance
```

4. Confirm the wallet output is JSON and that no mnemonic is printed unless an
   operator intentionally uses `init --show` in a private shell.

## Smoke Flow

### 1. Unpaid L402 challenge

Call the paid Site action with an active registered OpenAgents agent bearer
token. The token comes from the calling agent runtime; do not embed it in
generated public Site source.

Expected:

- HTTP status is `402`.
- `WWW-Authenticate` uses `L402`.
- Sandbox smoke may include `sandbox="true"`.
- Response includes action id, method, path, price, spend cap, expiry, and
  public-safe challenge refs.
- Generated Site source and browser-visible JSON do not include MDK access
  token, wallet mnemonic, webhook secret, raw preimage, or payout credentials.

Current #164 contract-stub command:

```bash
curl -i -X POST "$OPENAGENTS_BASE_URL/api/sites/$SITE_ID/commerce/l402/challenges" \
  -H "Authorization: Bearer $OPENAGENTS_AGENT_TOKEN" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: smoke-l402-challenge-1" \
  --data '{
    "paidActionId": "generate-report",
    "method": "POST",
    "path": "/api/actions/generate-report",
    "price": { "amount": 100, "asset": "sats" },
    "spendCap": { "amount": 100, "asset": "sats" },
    "entitlementScope": "action"
  }'
```

Future hosted-MDK command should return an L402 header with a real BOLT11
invoice for pay402-compatible clients, while still logging only redacted refs.

### 2. Capped payment

Use buyer tooling with an explicit cap.

Expected:

- If the invoice amount is above `maxAmountSats`, the agent refuses to pay.
- No payment dispatch happens on over-cap challenge.
- A successful under-cap payment returns a proof/preimage to the client, but
  OpenAgents product surface stores only redacted payment evidence and receipt refs.

Reference `pay402` behavior:

```ts
await pay402(url, {
  maxAmountSats: 100,
  idempotencyKey: 'smoke-pay402-1',
})
```

Reference agent-wallet behavior:

```bash
npx @moneydevkit/agent-wallet@latest send "$BOLT11_INVOICE"
```

Do not paste `$BOLT11_INVOICE` or the returned preimage into issue comments or
tracked docs.

### 3. Paid retry

Retry the protected action with the L402 credential and proof.

Expected:

- The same method/path/action binding is enforced.
- The same amount/currency is enforced.
- The entitlement is granted only for the intended scope.
- If service delivery fails before settlement, deferred settlement permits a
  paid retry until the action succeeds.
- After successful settlement, a second redemption attempt is treated as
  replay and rejected or mapped to the existing idempotent receipt.

Current #164 contract-stub redemption command:

```bash
curl -i -X POST "$OPENAGENTS_BASE_URL/api/sites/$SITE_ID/commerce/l402/redemptions" \
  -H "Authorization: Bearer $OPENAGENTS_AGENT_TOKEN" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: smoke-l402-redemption-1" \
  --data '{
    "challengeId": "site_l402_challenge_site_mdk_smoke_smoke-l402-challenge-1",
    "challengeExpiresAt": "2099-01-01T00:00:00.000Z",
    "credentialId": "site_l402_credential_smoke_1",
    "paidActionId": "generate-report",
    "method": "POST",
    "path": "/api/actions/generate-report",
    "price": { "amount": 100, "asset": "sats" },
    "entitlementScope": "action",
    "paymentProofRef": "mdk_payment_proof_smoke1234"
  }'
```

### 4. Entitlement grant

Inspect OpenAgents product surface's internal receipt/projection state.

Expected:

- Challenge state, payment evidence, entitlement state, and public projection
  are separate records.
- Entitlement is scoped to the Site/action/product/account declared in the
  manifest.
- Public projection says paid/unlocked only through safe refs; it does not
  reveal invoice, preimage, mnemonic, payment hash, or provider payout data.
- Payment evidence does not imply provider payout eligibility.

### 5. Token cache behavior

Run the same action twice with the same idempotency key and then with a new key.

Expected:

- Same idempotency key returns the same logical checkout/challenge/redemption
  receipt or resumes the same pending payment.
- New key creates a distinct challenge.
- Cached credential cannot be replayed against a different method, path, price,
  currency, Site, or action.
- Expired credential must request a fresh challenge.

### 6. Stale challenge expiration

Redeem a challenge after expiry.

Expected:

- OpenAgents product surface rejects it as stale.
- No entitlement is granted.
- No accepted-work or provider payout state is created.
- The agent receives clear instructions to request a fresh challenge.

Current #164 contract-stub stale command:

```bash
curl -i -X POST "$OPENAGENTS_BASE_URL/api/sites/$SITE_ID/commerce/l402/redemptions" \
  -H "Authorization: Bearer $OPENAGENTS_AGENT_TOKEN" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: smoke-l402-stale-1" \
  --data '{
    "challengeId": "site_l402_challenge_site_mdk_smoke_smoke-l402-challenge-1",
    "challengeExpiresAt": "2000-01-01T00:00:00.000Z",
    "credentialId": "site_l402_credential_smoke_1",
    "paidActionId": "generate-report",
    "method": "POST",
    "path": "/api/actions/generate-report",
    "price": { "amount": 100, "asset": "sats" },
    "entitlementScope": "action",
    "paymentProofRef": "mdk_payment_proof_smoke1234"
  }'
```

## Hosted MDK Versus Self-Hosted `mdkd`

Default for Autopilot Sites should be hosted MDK through OpenAgents product surface.

Use hosted MDK when:

- the Site is generated, static, or public;
- OpenAgents is operating the payment boundary;
- customer-owned merchant credentials are not reviewed and bound;
- public proof and customer dashboards must stay redacted;
- the goal is buyer payment, entitlement, and receipt evidence.

Consider self-hosted `mdkd` only when:

- there is a named operator responsible for the node;
- the secret-binding mode has explicit review and rotation;
- logs and public projections are proven to redact raw invoices/preimages where
  needed;
- the Site can tolerate local node downtime and recovery procedures;
- there is a clear reason hosted MDK is insufficient.

Self-hosted `mdkd` must not be introduced as the default Site builder behavior.

## Pass Criteria

The smoke passes only when all of these are true:

- unpaid challenge is correct and redacted;
- over-cap payment is refused before payment dispatch;
- under-cap sandbox/signet payment succeeds;
- paid retry unlocks the exact action;
- stale challenge is rejected;
- same idempotency key is replay-safe;
- different key creates a distinct payment flow;
- generated Site source contains no MDK access token, wallet mnemonic, webhook
  secret, raw invoice, raw preimage, private key, payment hash, or payout
  credential;
- OpenAgents product surface records payment evidence, entitlement, accepted work, provider payout
  eligibility, and settlement as separate states.

## Follow-Up Implementation Work

This runbook should drive the next implementation slices:

- D1 ledgers for checkout intents, L402 challenges, redemptions, entitlements,
  and payment events.
- Hosted MDK invoice/client adapter for Cloudflare Workers.
- One-shot credential consumption and idempotent replay handling.
- Sandbox/signet integration test that runs the full unpaid challenge -> pay ->
  retry -> entitlement flow.
- Agent-facing docs once the internal smoke passes.
