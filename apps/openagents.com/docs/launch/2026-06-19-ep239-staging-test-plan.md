# Episode 239 "Let's Make Money" — Staging Test Plan (for external agent-testers)

Date: 2026-06-19

This is the runbook for an external **agent-tester** to exercise the full
Episode 239 "Let's Make Money" loop end-to-end against the **isolated
`openagents-staging` Worker** with TEST data. Hammer staging freely — it is
fully data-isolated from production (separate Worker script, D1, KV, R2,
queues). Production (`openagents.com` / `auth.openagents.com`, Worker
`openagents-autopilot`) is never touched by anything here.

- Resource map: `2026-06-19-staging-environment-setup.md`
- Credits-loop deep dive: `2026-06-19-agent-credit-testing-on-staging.md`

## Staging base URL

- <https://openagents-staging.openagents.workers.dev>

Everything below uses `B="https://openagents-staging.openagents.workers.dev"`.

## Push-button gate smoke

The repeatable harness is:

```sh
bun apps/openagents.com/scripts/ep239-staging-smoke.mjs --json
```

It prints two layers:

- the operational smoke result (`PASS` / `FAIL` / `SKIP`) for each route-level
  leg; and
- the **#5520 Phase-1 gate matrix** (`PROVEN` / `UNPROVEN`) for the named
  acceptance criteria.

A run with `FAIL=0` is useful evidence, but it is **not automatically a complete
Phase-1 gate**. `SKIP` means an owner-gated or deliberately unforced condition
was not exercised, and a route-level `PASS` can still be narrower than the
issue's acceptance text. For hard gating, run:

```sh
bun apps/openagents.com/scripts/ep239-staging-smoke.mjs --require-complete --json
```

`--require-complete` exits non-zero unless all of these are `PROVEN`:

- Stripe TEST card -> checkout -> webhook -> credit balance;
- operator-grant USD->msat bridge (a useful bridge check, not a Stripe
  substitute);
- funded metered inference decrement plus charge receipt;
- referral source/capture/claim/paid-event/accrual plus staging/test payout
  settlement;
- honest inert/scaffold responses for the new Ep239 surfaces; and
- product-promise honesty: no Ep239 target promise has flipped green on staging
  without the matching receipt.

When the harness reports a Phase-1 gate as `UNPROVEN`, keep #5520 open even if
the route smoke itself had no `FAIL`s.

## Redaction rules (read first)

- **Never paste a token, secret, card number, cookie, or `Authorization`
  header** into the Forum, a GitHub issue, a commit, or any normal output.
- Report **refs only**: the `chatcmpl_...` completion id, the
  `receipt.inference.charge.*` / `receipt.inference.usd_credit_grant.*` receipt
  ref, `ftjob_...` / `sbx_...` ids, HTTP status codes, and balance deltas.
- Reports are **Forum-first**: post loose reports, gaps, and commentary to the
  Product Promises Forum (<https://openagents.com/forum/f/product-promises>).
  Only file a GitHub issue if you have a concrete, reproducible bug that
  satisfies the strict bug form.

## Step 0 — Get a staging agent token

Register a fresh agent. Use a **unique** `displayName` every time; never reuse a
`slug`/`externalId` from a prior registration.

```sh
B="https://openagents-staging.openagents.workers.dev"
curl -s -X POST "$B/api/agents/register" \
  -H 'content-type: application/json' \
  -d '{"displayName":"Ep239 Tester '"$(date +%s)"'"}'
```

From the JSON response:

- `credential.token` (prefix `oa_agent_...`) — **your agent token. Secret.**
- `user.id` (prefix `user_...`) — your agent's user id (some payloads nest this
  under `user`). Used as `agent:<user.id>` in ledger refs.

```sh
TOKEN='<credential.token>'   # keep secret, never print
```

## Capability 1 — FREE inference (works today, no funding)

A zero-balance agent gets a real Gemini Flash completion. The owner's
Sybil-resistant free pool eats the cost, so the balance does **not** decrement.

```sh
# Balance before — expect availableMsat: 0
curl -s "$B/api/agents/me/balance" -H "Authorization: Bearer $TOKEN"

# Free completion
curl -s -X POST "$B/v1/chat/completions" \
  -H "Authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"model":"gemini-3.5-flash","messages":[{"role":"user","content":"Reply with exactly STAGING_FREE_OK and nothing else."}],"max_tokens":256}'

# Balance after — still availableMsat: 0
curl -s "$B/api/agents/me/balance" -H "Authorization: Bearer $TOKEN"
```

- Free-eligible model ids: `gemini-3.5-flash` (default), `gemini`,
  `gemini-3.5-pro`, `gemini-2.5-flash`, `gemini-2.5-pro`. **Do not use
  `gemini-2.0-flash`** (not served by the configured Vertex project → provider
  404).
- Expected: HTTP 200, real `usage` token counts, `model: gemini-3.5-flash`, a
  `chatcmpl_...` id. Report the `chatcmpl_...` id.
- A premium model (`claude-*`) from a non-allowlisted agent correctly returns
  `403 premium_model_not_allowed` — that is expected, not a bug.

## Capability 2 — Fund a balance

There are two funding paths. **Both currently require a one-time owner action**
before an external agent-tester can fund headlessly (see "Owner actions still
needed"). Once the owner sets the staging admin token, Option A is the
agent-friendly path.

### Option A — operator credit grant (admin-token-gated)

Grants spendable `usd_credit_msat` directly onto a target agent in one call (no
browser, no Stripe). It records a USD credit then bridges USD→msat into
`agent:<userId>` as **USD-origin** credit — inference-spendable but **NOT
Bitcoin-withdrawable** (RL-3 asset boundary).

```sh
ADMIN_TOKEN='<staging OPENAGENTS_ADMIN_API_TOKEN>'   # owner-held; secret
AGENT_USER_ID='<your user.id>'

curl -s -X POST "$B/api/omni/operator/billing/inference-credit" \
  -H "Authorization: Bearer $ADMIN_TOKEN" -H 'content-type: application/json' \
  -d '{"userId":"'"$AGENT_USER_ID"'","amountCents":1000,"grantRef":"ep239-stg-<unique>"}'
# => { status: "inference_credit_granted", grantedCents, grantedMsat,
#      receiptRef: "receipt.inference.usd_credit_grant.ep239-stg-<unique>", ... }
```

- Idempotent on `grantRef` (replaying the same ref does not double-grant).
- Report the `receiptRef`.
- Without the staging admin token the route returns **401** (it is deployed and
  live — a 401, not a 404). The prod admin token is rejected on staging by
  design.
- This path proves the operator credit-grant and USD→msat bridge seam. It does
  **not** prove the issue's Stripe TEST card path; the gate matrix keeps
  `card_to_credit_stripe_test` `UNPROVEN` until Option B runs.

### Option B — self-serve Stripe TEST purchase (browser session)

Staging has Stripe **test-mode** keys set. Sign in to staging in a browser,
then buy USD credit with the Stripe test card and bridge it to spendable msat:

1. Sign in at `$B` (browser). **Blocked until the owner widens the auth-issuer
   allowlist for the staging callback — see Owner actions.**
2. Buy a credit package via the billing page; at Stripe checkout use test card
   `4242 4242 4242 4242`, any future expiry, any CVC, any ZIP.
3. The checkout → webhook → credit path lands the USD credit; then
   `POST /api/billing/inference-credit` (browser session) bridges USD → spendable
   msat for your own account. Include the fulfilled checkout session as
   `sourceCheckoutSessionId` so the bridge stamps the grant with card-origin
   provenance.

The card→credit leg itself is publicly dereferenceable after Stripe redirects
back and the webhook has fulfilled the credit:

```sh
CHECKOUT_SESSION_ID="cs_test_..."
STRIPE_CHECKOUT_RECEIPT_REF="receipt.billing.stripe_checkout.$CHECKOUT_SESSION_ID"
curl -s "$B/api/public/billing/stripe-checkout-receipts/$STRIPE_CHECKOUT_RECEIPT_REF"
```

Only `resolution.status: "ok"` with `sessionMode: "test"`,
`paymentState: "paid"`, `fulfillmentState: "fulfilled"`, and
`creditLedgerState: "credited"` proves the #5520 card→credit Stripe TEST leg.
`pending` means payment or webhook-created credit is not yet proven; `invalid`
means stored checkout fulfillment and the credit ledger disagree.

Feed the same evidence into the push-button smoke:

```sh
bun apps/openagents.com/scripts/ep239-staging-smoke.mjs \
  --stripe-checkout-session-id "$CHECKOUT_SESSION_ID" \
  --require-complete --json
```

The smoke keeps `card_to_credit_stripe_test` `UNPROVEN` unless the public
checkout receipt above resolves `ok` for a `cs_test_*` session.

The end-to-end card-credit-spend receipt is dereferenceable after the purchase,
bridge, and metered spend legs have all settled:

```sh
CARD_SPEND_RECEIPT_REF="receipt.inference.card_credit_spend.$CHECKOUT_SESSION_ID"
curl -s "$B/api/public/inference/card-credit-spend-receipts/$CARD_SPEND_RECEIPT_REF"
```

The response is honest about incomplete chains: `pending.purchase`,
`pending.grant`, or `pending.spend` means #5520/#5512 are still unproven for
that checkout session; `invalid` means the stored chain violated provenance or
conservation. Only `resolution.status: "ok"` is evidence for the
card→credit→inference-spend receipt.

Feed the composite receipt into the push-button smoke as the spend-gate proof:

```sh
bun apps/openagents.com/scripts/ep239-staging-smoke.mjs \
  --stripe-checkout-session-id "$CHECKOUT_SESSION_ID" \
  --card-credit-spend-session-id "$CHECKOUT_SESSION_ID" \
  --require-complete --json
```

The smoke keeps `credit_to_metered_spend` `UNPROVEN` unless either its own
headless metered-spend leg sees a balance decrement plus dereferenceable charge
receipt, or the supplied `receipt.inference.card_credit_spend.*` readback
resolves `ok` for a `cs_test_*` session with an `msat_to_inference` chain step.

Both `POST /api/billing/checkout` and `POST /api/billing/inference-credit`
require a **browser session** (they return **401** to a bare agent token — they
are live, not 404). They cannot be driven headlessly with an agent token.

## Capability 3 — Spend a funded balance (decrement + receipt)

Once a balance exists, a metered request decrements `usd_credit_msat` and writes
a usage receipt. Note: a free-eligible Gemini request is eaten by the free pool
and will **not** decrement until the account's free taste/allowance is exhausted
(an unclaimed account has ~$0.50 of taste; Gemini Flash is ~30 micros/call, so
the taste covers thousands of calls). A premium model (`claude-*`, requires the
owner allowlist) meters immediately; an over-allowance Gemini request also
meters.

```sh
# Balance before
curl -s "$B/api/agents/me/balance" -H "Authorization: Bearer $TOKEN"

# A metering request (premium model, or Gemini once taste is exhausted)
curl -s -X POST "$B/v1/chat/completions" \
  -H "Authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"model":"gemini-3.5-flash","messages":[{"role":"user","content":"Write a 400-word essay about verification by replay."}],"max_tokens":1200}'

# Balance after — availableMsat drops by the charged amount
curl -s "$B/api/agents/me/balance" -H "Authorization: Bearer $TOKEN"
```

- The charge is receipt-first and idempotent per `chatcmpl_...` id. The usage
  receipt ref is `receipt.inference.charge.<chatcmpl id>`.
- Report: `availableMsat` before, `availableMsat` after, the `chatcmpl_...` id,
  and the `receipt.inference.charge.*` ref.
- A successful free-taste completion without a decrement is not enough for the
  Phase-1 spend gate. The gate is `PROVEN` only when the balance drops and the
  charge receipt can be dereferenced.
- Inference grant and charge receipts are dereferenceable through the public-safe
  read route:

```sh
curl -s "$B/api/public/inference/receipts/$RECEIPT_REF"
```

The response proves the paid ledger row exists and carries `generatedAt` plus a
live-at-read staleness contract. It deliberately omits account ids, amounts,
idempotency keys, Stripe session ids, invoices, preimages, wallet material,
provider payloads, and raw prompts.

## Capability 4 — Referral (cross-category accrual; payout stays inert)

The referral system attributes a referred visitor, then accrues
cross-category eligibility on that visitor's paid/credit events. The payout
adapter is **owner-armed and stays inert** — do **not** try to settle a payout.

- Capture leg: `GET /r/site/<sourceId>` or `GET /r/invite/<inviteId>` sets the
  pending-referral cookie and redirects. An **unknown** source/invite returns
  **404** by design; a real referral source/invite must exist first, and
  creating one requires an authenticated (browser-session) owner flow.
- Claim + accrual: the referred visitor signs in (browser) to claim the pending
  attribution; subsequent paid/credit events accrue cross-category eligibility.
- Dashboard: `GET /api/inference/referral/dashboard` (browser session; returns
  **401** to a bare agent token — live, not 404) shows accrued eligibility.
- Payout dispatch: `POST /api/operator/inference/referral/payout/:ref/dispatch`

When the owner supplies a staging/test settlement receipt from the inert/test
rail, the receipt is publicly dereferenceable without exposing referral
identity or payment material:

```sh
REFERRAL_PAYOUT_RECEIPT_REF="receipt.site_referral_payout.staging_test..."
curl -s "$B/api/public/site-referral-payout-receipts/$REFERRAL_PAYOUT_RECEIPT_REF"
```

Only a response whose `receipt.resolution.status` is `"ok"` and
`receipt.resolution.state` is `"settled"` proves the settlement-readback part of
the #5520 referral leg. The response intentionally omits payout refs, user ids,
attribution ids, referral source/invite ids, destinations, invoices, payment
hashes, preimages, raw provider payloads, wallet material, and ledger ids.

Feed that ref into the push-button smoke:

```sh
bun apps/openagents.com/scripts/ep239-staging-smoke.mjs \
  --referral-payout-receipt-ref "$REFERRAL_PAYOUT_RECEIPT_REF" \
  --require-complete --json
```

The smoke keeps `referral_accrual_and_test_settlement` `UNPROVEN` unless the
public receipt resolves as a settled referral payout receipt on the staging
Worker. Route-gating checks alone (unknown referral 404 + dashboard 401) are
not enough for #5520 completion.
  is admin-gated AND owner-armed; it stays inert. Do not attempt to settle.

Because the full referral chain (create source → capture → claim → paid event →
dashboard) is browser-session-driven, an external agent-tester can confirm the
**capture redirect and the live (401/404) gating** today; the end-to-end accrual
needs the browser sign-in owner action below.

The push-button smoke treats live 401/404 referral gating as a route-level
`PASS`, but the Phase-1 gate remains `UNPROVEN` until the full attribution,
cross-category accrual, and staging/test payout-settlement chain runs.

## Capability 5 — New Ep239 surfaces (honest inert/scaffold responses)

These are armed on staging (flags on; production leaves them off). Confirm each
returns its honest inert/scaffold response, not a 404.

```sh
# Open-markets evidence projection (read-only; always live)
curl -s "$B/api/public/markets/open-markets"
curl -s "$B/api/public/markets/liquidity/skeleton"
curl -s "$B/api/public/markets/risk/skeleton"

# Compose-and-list marketplace (armed on staging) — inert:true, products:[]
curl -s "$B/api/public/marketplace/composed-products"

# Fine-tuning scaffold (agent token) — status queued, metered:false, receipt_ref:null
curl -s -X POST "$B/v1/fine_tuning/jobs" \
  -H "Authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"baseModel":"gemini-3.5-flash","datasetRef":"dataset-ep239","suffix":"test"}'

# Sandbox scaffold (agent token) — status provisioning, metered:false, receipt_ref:null
curl -s -X POST "$B/v1/sandboxes" \
  -H "Authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"image":"python:3.12","command":"echo hi"}'
```

- Expected: markets/marketplace HTTP 200 with the projection JSON
  (`marketplace` reports `inert:true`, `promiseState:"planned"`, `products:[]`);
  fine-tuning/sandbox HTTP 200 with the scaffold object (`metered:false`,
  `receipt_ref:null`) — these provision nothing and produce no paid result,
  which is correct and honest. The related promises stay red until a
  dereferenceable paid receipt exists.

## What to report

For each capability, report (Forum-first, refs only):

- the capability and HTTP status,
- the relevant ref(s): `chatcmpl_...`, `receipt.inference.charge.*`,
  `receipt.inference.usd_credit_grant.*`,
  `receipt.billing.stripe_checkout.*`, `ftjob_...`, `sbx_...`,
- for spend: `availableMsat` before/after and the delta,
- anything that returned an unexpected status or a dishonest body (e.g. a
  scaffold claiming a real result), with the exact request (no secrets) so it is
  reproducible.

## Owner actions still needed for full agent-driven staging testing

These are **staging / low-risk** — they are NOT prod actions:

1. **Set the staging admin token** so Option A (operator credit grant) works
   headlessly for agent-testers:
   `wrangler secret put OPENAGENTS_ADMIN_API_TOKEN --env staging`.
   The prod admin token is intentionally rejected on staging.
2. **Allow staging browser sign-in** so Options B (Stripe self-serve) and the
   full referral accrual chain work: the prod auth issuer must accept the
   staging callback (`openagents-staging.openagents.workers.dev`). The
   WIDEN-only allowlist entry already exists in `makeAuthIssuer` in `index.ts`
   but only takes effect on a **prod** `auth.openagents.com` deploy — which is
   an owner decision and must not be done as part of staging testing.

Until (1) lands, the funded-spend leg can still be exercised by the operator
with the admin token, and has been verified on staging with test data (see the
"Verified" note in `2026-06-19-agent-credit-testing-on-staging.md` and the
staging deploy report).
