# Agent Credit Testing on Staging

Date: 2026-06-19

Step-by-step for an agent (or operator) to exercise the credits loop on the
isolated `openagents-staging` Worker:

1. acquire a spendable balance,
2. spend it on inference,
3. observe the balance decrement, with dereferenceable evidence.

Staging is data-isolated from production — see
`2026-06-19-staging-environment-setup.md` for the resource map. Production
(`openagents.com` / `auth.openagents.com`, Worker `openagents-autopilot`) is
untouched by anything here.

## Staging URL

- <https://openagents-staging.openagents.workers.dev>

## What works today vs. the remaining gap

| Loop | Path | Status on staging |
| --- | --- | --- |
| Free allowance (Gemini Flash, zero balance) | agent token → `POST /v1/chat/completions` | **WORKS today** (no credentials beyond a self-registered agent token) |
| Funded balance via operator admin grant | admin token → `POST /api/omni/operator/billing/inference-credit` | **Route deployed**, but needs the **staging `OPENAGENTS_ADMIN_API_TOKEN`** (a Worker secret; the prod admin token is rejected on staging by design) — NEEDS-OWNER |
| Funded balance via #5497 self-serve bridge | browser session → `POST /api/billing/inference-credit` | Blocked: browser sign-in on staging needs the **prod auth issuer** to accept the staging callback (a one-line WIDEN-only allowlist in `index.ts` that only takes effect on a **prod** deploy) — NEEDS-OWNER |
| Spend a funded balance on a premium model | agent token → `POST /v1/chat/completions` (e.g. `claude-*`) | Works once a balance exists AND the owner identity is on the premium allowlist; a zero-balance unclaimed agent correctly gets `403 premium_model_not_allowed` |

The free loop is fully self-serviceable. The funded loop is implemented,
unit-verified against real SQL (asset boundary + idempotency), and deployed to
staging, but the two ways to *fund* a balance both require an owner-held
credential/deploy (see NEEDS-OWNER below).

## Part 1 — Free allowance (works today, no owner action)

A zero-balance agent gets a real Gemini Flash completion. The balance gate's
free-allowance pre-flight admits the request; the metering hook eats the cost
under the owner's Sybil-resistant free pool, so the balance never decrements.

### 1a. Register a fresh staging agent (gets an agent token)

```sh
curl -s -X POST https://openagents-staging.openagents.workers.dev/api/agents/register \
  -H 'content-type: application/json' \
  -d '{"displayName":"Credit Loop Probe '"$(date +%s)"'"}'
```

- Use a **unique** `displayName`; do **not** reuse a `slug`/`externalId` from a
  previous registration.
- The agent token is in the response at `credential.token` (prefix
  `oa_agent_...`). Treat it as a secret — never print or commit it.

```sh
TOKEN='<credential.token from the response>'
```

### 1b. Balance before (expect zero)

```sh
curl -s https://openagents-staging.openagents.workers.dev/api/agents/me/balance \
  -H "Authorization: Bearer $TOKEN"
# => balance.availableMsat: 0, balance.balanceMsat: 0
```

### 1c. Free Gemini completion

```sh
curl -s -X POST https://openagents-staging.openagents.workers.dev/v1/chat/completions \
  -H "Authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"model":"gemini-3.5-flash","messages":[{"role":"user","content":"Reply with exactly the token STAGING_FREE_OK and nothing else."}],"max_tokens":256}'
```

- Free-eligible model ids: `gemini-3.5-flash` (default), `gemini`, plus the
  other ids in `vertex-gemini-adapter.ts` (`gemini-3.5-pro`, `gemini-3.5-flash`,
  `gemini-2.5-pro`). **Do not use `gemini-2.0-flash`** — that id is not served
  by the configured Vertex project and returns a provider `404`.
- Expected: `HTTP 200`, real `usage` token counts, `model: gemini-3.5-flash`,
  and a `chatcmpl_...` id you can use as evidence.

### 1d. Balance after (still zero — free path, no decrement)

```sh
curl -s https://openagents-staging.openagents.workers.dev/api/agents/me/balance \
  -H "Authorization: Bearer $TOKEN"
# => balance.availableMsat: 0  (unchanged — the free pool ate the cost)
```

## Part 2 — Funded balance + paid spend (NEEDS-OWNER to run live)

This is the loop where a **non-free** balance is granted, spent, and the balance
**decrements**. It is implemented and deployed to staging, but funding requires
an owner credential or a prod deploy. Pick one:

### Option A — Operator admin grant (preferred for an agent test)

Admin-token-gated. Grants spendable `usd_credit_msat` directly onto any target
agent in one call (no browser, no Stripe). It does both #5497 halves: (1) a USD
credit to the target's `billing_ledger_entries`, then (2) the USD→msat bridge
into `agent:<userId>` as **USD-origin** credit — inference-spendable but **NOT
Bitcoin-withdrawable** (RL-3 asset boundary, enforced by
`fundInferenceFromCredit`).

```sh
# Needs the STAGING OPENAGENTS_ADMIN_API_TOKEN (a Worker secret).
ADMIN_TOKEN='<staging OPENAGENTS_ADMIN_API_TOKEN>'
AGENT_USER_ID='<user_... from the agent registration>'

curl -s -X POST https://openagents-staging.openagents.workers.dev/api/omni/operator/billing/inference-credit \
  -H "Authorization: Bearer $ADMIN_TOKEN" -H 'content-type: application/json' \
  -d '{"userId":"'"$AGENT_USER_ID"'","amountCents":1000,"grantRef":"staging-credit-test-1"}'
# => { status: "inference_credit_granted", grantedCents: 1000, grantedMsat: <n>, receiptRef: "receipt.inference.usd_credit_grant.staging-credit-test-1", ... }
```

- Idempotent on `grantRef`: replaying the same ref does not double-grant.
- `receiptRef` is the dereferenceable evidence for the grant.

### Option B — Self-serve #5497 bridge (browser)

Sign in to staging in a browser, buy/hold USD credit (test card `4242 4242 4242
4242`, any future expiry / any CVC / any ZIP, against the staging Stripe keys),
then `POST /api/billing/inference-credit` to bridge USD → spendable msat for your
own account. Requires the prod-issuer allowlist deploy below to sign in on
staging.

### 2c. Spend the funded balance and watch it decrement

After funding, point the agent token at a model that actually meters. A premium
model (`claude-*`) requires the owner identity to be on the premium allowlist;
an over-free-allowance Gemini request also meters. Then:

```sh
curl -s https://openagents-staging.openagents.workers.dev/api/agents/me/balance \
  -H "Authorization: Bearer $TOKEN"
# => availableMsat drops by the charged amount; recentActivity shows the spend.
```

Capture `availableMsat` before and after plus the completion's `chatcmpl_...` id
and the spend's receipt/activity ref as evidence.

## NEEDS-OWNER

1. **Staging admin token** — to run Part 2 Option A, set/share the staging
   Worker secret `OPENAGENTS_ADMIN_API_TOKEN` for `openagents-staging`
   (`wrangler secret put OPENAGENTS_ADMIN_API_TOKEN --env staging`). The prod
   admin token is intentionally rejected on staging.
2. **Prod auth-issuer deploy** — to run Part 2 Option B (browser sign-in on
   staging), the prod issuer must accept the staging callback. The WIDEN-only
   allowlist entry (`openagents-staging.openagents.workers.dev`) is already in
   `makeAuthIssuer` in `index.ts`, but it only takes effect when the **prod**
   Worker (`auth.openagents.com`) is deployed. **Do not prod-deploy without
   explicit owner approval.** Regression coverage lives in
   `workers/api/src/auth-email-otp-hardening.test.ts`, which pins the exact
   staging callback host and rejects sibling/random Worker hosts.

## Verified on 2026-06-19 (this work)

- Part 1 free loop: a zero-balance staging agent
  (`user_121631a7-bbec-4ae8-b1c8-87432c4c9a7c`) got `STAGING_FREE_OK` from
  `gemini-3.5-flash` (`chatcmpl_e705677324854ac9afade68ea1d57eb7`,
  `usage.total_tokens: 164`); balance stayed `availableMsat: 0` before and after.
- The free-allowance pre-flight bypass admits a zero-balance free-eligible
  request (no false `402`), and correctly does **not** apply to a non-free model
  (`claude-opus-4-8` → `403 premium_model_not_allowed`, not a free grant).
- Part 2 operator route is deployed on staging (`401` without the staging admin
  token) and **not** on prod yet (`404`); funded live spend is gated on the
  NEEDS-OWNER items above. The grant→spendable→asset-boundary→idempotent
  behavior is covered by the real-SQL unit test in
  `workers/api/src/operator-billing-routes.test.ts`.
