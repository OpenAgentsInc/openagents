# Can OpenAgents Collect Money for Credits Right Now? — End-to-End Audit

Date: 2026-06-19
Scope: `apps/openagents.com` (the `openagents.com` Cloudflare Worker + Foldkit web app)
Type: audit only — no feature code changed.
Base: branched off `origin/main` (`3ad73c26e`).

---

## TL;DR Verdict

**Card payments: YES, the machinery is built and wired end-to-end — but it is
almost certainly NOT collecting money today, because three Worker secrets must
be set in production (`STRIPE_API_KEY`, `STRIPE_WEBHOOK_SIGNING_SECRET`,
`STRIPE_CREDIT_PACKAGES_JSON`). With those set, it can collect real money.**

**Bitcoin/Lightning credit top-up: NO. There is no BTC/Lightning path to buy
credits. Card (Stripe Checkout) only.**

**The credits you can buy do NOT fund the inference gateway.** Buying credits
raises the **USD Autopilot credit ledger** (`billing_ledger_entries`), which
pays for **Autopilot container time + Codex token usage**. The **inference
gateway** (`/v1/chat/completions`) meters a **completely separate** msat ledger
(`agent_balances`) funded only by Lightning tips/rewards/sweeps. The two
ledgers are disconnected, and the inference gateway is flag-OFF in production
(`INFERENCE_GATEWAY_ENABLED: "false"`). So "buy credits -> run inference" is
**not a closed loop** today on two independent counts.

So the honest answer to the owner's framing:

- "Can a human give us a credit card and get usable Autopilot credits?" ->
  **Yes, once the three Stripe secrets are configured in the Worker.** The page,
  routes, Stripe Checkout, webhook fulfillment, and the USD ledger are all real.
- "Do those credits make inference work?" -> **No.** Credits fund Autopilot
  (container/Codex) usage, not the inference gateway. Inference is a separate,
  flag-disabled, Lightning-funded economy.

---

## 1. How do people pay for credits? (the actual flow)

The built flow for a real logged-in human is:

1. User logs in, navigates to **`/billing`** (web app route `BillingRoute`,
   `apps/web/src/route.ts:74,549,705`).
2. The billing page shows the USD balance, rates, ledger history, a coupon box,
   credit packages, an "Add card" (SetupIntent) flow, and an auto-top-up policy
   form. View: `apps/web/src/page/loggedIn/page/billing.ts`; commands:
   `apps/web/src/page/loggedIn/billing/commands.ts`; transitions:
   `apps/web/src/page/loggedIn/billing/transitions.ts`.
3. User clicks "Buy" on a package -> the page POSTs `{ packageId }` to
   **`POST /api/billing/checkout`**
   (`billing/commands.ts:76-82`).
4. The Worker creates a real **Stripe Checkout Session** (`mode: 'payment'`)
   and returns its hosted `checkoutUrl`
   (`stripe-billing.ts:573-648`, route `billing-routes.ts:233-286`).
5. The browser navigates to Stripe's hosted checkout page
   (`globalThis.location.assign(response.checkoutUrl)`, `commands.ts:82`).
6. User pays on Stripe's hosted page.
7. Fulfillment (credit grant) happens on **two idempotent paths**:
   - **Webhook (authoritative):** Stripe -> `POST /api/billing/stripe/webhook`
     -> signature verified with `constructEventAsync` +
     `createSubtleCryptoProvider` -> on `checkout.session.completed` /
     `async_payment_succeeded`, calls `fulfillCheckoutSession`
     (`stripe-billing.ts:1054-1134`).
   - **Return page:** Stripe redirects to
     `GET /api/billing/stripe/checkout-return?session_id=...` -> calls the same
     `fulfillCheckoutSession`, then 303-redirects to a clean `/billing`
     (`billing-routes.ts:537-568`).
8. `fulfillCheckoutSession` verifies `payment_status === 'paid'`, then calls
   **`applyStripeCheckoutCredit`**, which writes ONE positive row into
   `billing_ledger_entries` (source `stripe_checkout`, idempotency key
   `billing:stripe-checkout:<sessionId>`) and reactivates a suspended account
   (`billing.ts:729+`, `stripe-billing.ts:650-733`).
9. The user's USD balance is the derived `SUM(amount_cents)` over
   `billing_ledger_entries` (`billing.ts:310-324`). It now goes up.

There is also a **saved-card + auto-top-up** path (SetupIntent ->
`stripe_saved_payment_methods` -> off-session PaymentIntent when balance drops
below threshold), wired through `/api/billing/stripe/setup-intents`,
`/api/billing/stripe/setup-intents/save`, `/api/billing/auto-top-up-policy`,
and `/api/billing/auto-top-up/run` (`stripe-billing.ts:893-1052`).

This matches the design in `docs/2026-06-04-stripe-effect-service-audit.md`
exactly — the audit's "future implementation" is now the shipped code.

---

## 2. Have we built the page? Where is it? How does it work?

**Yes.** Customer-facing billing/credit page:

- **Route:** `/billing` (`BillingRoute`, `apps/web/src/route.ts:74`, router at
  `:549`, registered at `:705`). Reachable by any logged-in user.
- **View file:** `apps/web/src/page/loggedIn/page/billing.ts`
- **Logic:** `apps/web/src/page/loggedIn/billing/commands.ts` (effects /
  fetches) and `.../billing/transitions.ts` (state).
- **It is REAL, not presentational.** The "Buy" command does a real POST to
  `/api/billing/checkout` and redirects the browser to the Stripe-hosted
  checkout URL it gets back. It is not a mock.

Note on `apps/web/src/ui/credits-panel.ts`: this is a separate, **reusable
balance/cost-preview component** authored for the workroom page (#4977/#4985).
Its own header comment says it is intentionally **not** exported from
`ui/index.ts` yet. It is presentational (pure Foldkit builders, no fetch). It is
**not** the billing page — do not confuse the two. The billing page above is the
live purchase surface.

---

## 3. Is it actually live and able to collect money TODAY? (brutally honest)

**The code is production-grade and fully wired. Whether it collects money TODAY
hinges entirely on three Worker secrets being set in the live environment.**

The Stripe config (`stripe-billing.ts:478-508`, `decodeStripeConfig`) hard-requires:

- `STRIPE_API_KEY`
- `STRIPE_WEBHOOK_SIGNING_SECRET`
- `STRIPE_CREDIT_PACKAGES_JSON` (the credit packages -> Stripe Price IDs map)
- `OPENAGENTS_APP_URL` (already set: `https://openagents.com`,
  `workers/api/wrangler.jsonc:43`)

These are **secrets**, so by design they are **not** in `wrangler.jsonc`
(only `OPENAGENTS_APP_URL` is). They would be set via `wrangler secret put`.
**This audit cannot read production secrets, so it cannot prove they are set.**
The behavior is graceful when they are absent:

- If unconfigured, `decodeStripeConfig` fails with `StripeConfigError`, and
  `/api/billing/checkout` returns **HTTP 503 `stripe_unconfigured`** with the
  message **"Card checkout is not available yet."** (`billing-routes.ts:31-40`).

So the strong signal: **if those secrets are not yet provisioned, every "Buy"
click returns 503 and no money is collected.** Given there is no evidence in the
repo of the packages/price IDs being configured and the prior audit treats
"initial secret-key launch period" as future debt, the most likely current
state is **unconfigured (503) until an operator sets the three secrets.**

What IS unambiguously live and correct regardless:

- Routes are registered (`omni-routes.ts:81-135`, wired in `index.ts:6466,
  7466, 7505`).
- Webhook signature verification is real (raw body + Stripe subtle-crypto).
- Fulfillment is idempotent on both webhook + return paths.
- The ledger is append-only and balance is derived, not a mutable counter.
- Test coverage exists (`billing-routes.test.ts`, `billing.test.ts`,
  `site-payment-*`), and the design audit's acceptance criteria are met.

This is **not demo/placeholder code.** The only thing standing between it and
collecting money is configuration (live Stripe account, Price IDs, webhook
endpoint registered in the Stripe dashboard, three Worker secrets).

---

## 4. Does a purchased credit balance flow to inference? (the loop)

**No — on two independent counts.**

### 4a. Two separate ledgers, no bridge

There are two unrelated balance systems:

| | USD Autopilot credits | Inference / agent payments |
|---|---|---|
| Table | `billing_ledger_entries` | `agent_balances` |
| Unit | USD cents | millisatoshis (msat) |
| Keyed by | `user_id` | `actor_ref` (e.g. `agent:<id>`) |
| Credited by | Stripe checkout, coupon, trial grant, operator adjust | Lightning tips / rewards / sweeps / buffer funding / adjustment (`payments-ledger.ts:12-18`) |
| Debited by | Autopilot **container time** + **Codex token** usage (`billing.ts:1174,1267`) | **inference gateway** metering (`inference/metering-hook.ts`) |

The Stripe purchase path (`applyStripeCheckoutCredit`) writes **only** to
`billing_ledger_entries`. Nothing — no cron, no bridge, no conversion —
ever reads `billing_ledger_entries` and credits `agent_balances`. The
`agent_balances` pay-in types are `tip | sweep | buffer_funding | reward |
adjustment` — there is **no `stripe`/USD pay-in type**. So a card purchase can
never raise the balance the inference gateway gates on
(`index.ts:8655-8661` reads `agent_balances.availableMsat`;
`metering-hook.ts` debits the same).

### 4b. The inference gateway is OFF in production

`workers/api/wrangler.jsonc:40` sets `INFERENCE_GATEWAY_ENABLED: "false"`.
With the flag off, `/v1/chat/completions` never reaches the balance gate or the
metering hook at all (`index.ts:8637-8642`). So even within the msat economy,
inference is not live and not billing.

**Net:** "buy credits -> run inference" is not a closed loop. Today, buying
credits (once Stripe is configured) gives you spendable **Autopilot
(container/Codex)** credit, not inference credit.

---

## 5. Bitcoin/Lightning top-up for credits?

**No.** The `/billing` page and `billing.ts`/`billing-routes.ts` contain no
Lightning/BOLT11/BOLT12/invoice/sats path for buying USD credits — card
(Stripe Checkout) only. (Lightning exists elsewhere in the system —
`agent_balances`, tips, MDK payouts — but that is the separate agent/msat
economy in section 4, not a human credit-purchase rail.)

---

## End-to-End Flow (and the broken links)

```
USER WANTS CREDITS
  -- /billing page (LIVE)                       apps/web/.../page/loggedIn/page/billing.ts
       -- click Buy -> POST /api/billing/checkout (LIVE route)   billing-routes.ts:233
            -- Stripe Checkout Session created  ---.  stripe-billing.ts:573
               (needs STRIPE_API_KEY +             |
                STRIPE_CREDIT_PACKAGES_JSON)       |  <-- BLOCKER A: secrets unset -> 503
            -- browser -> Stripe hosted page  -----'  commands.ts:82
                 -- user pays
                      -- webhook /api/billing/stripe/webhook (LIVE, idempotent)
                         (needs STRIPE_WEBHOOK_SIGNING_SECRET) <-- BLOCKER A
                         + return page /api/billing/stripe/checkout-return (LIVE)
                           -- fulfillCheckoutSession (LIVE)     stripe-billing.ts:650
                                -- applyStripeCheckoutCredit -> billing_ledger_entries (USD)  billing.ts:729
                                     -- USABLE FOR: Autopilot container time + Codex tokens  [YES]
                                     -- USABLE FOR: inference gateway  [NO]
                                          <-- BLOCKER B: no bridge USD->agent_balances (msat)
                                          <-- BLOCKER C: INFERENCE_GATEWAY_ENABLED=false
```

---

## Can-We-Collect-Money-Now Verdict

- **Collect money for Autopilot (container/Codex) credits via card:**
  **YES — pending only operator configuration of the three Stripe secrets +
  registering the webhook in the Stripe dashboard.** No code work required.
- **Collect money via Bitcoin/Lightning for credits:** **NO** — not built.
- **Sell inference for money (card-funded):** **NO** — separate msat ledger, no
  bridge, gateway flag off.

To turn ON card collection today (no code, operator steps):

1. Create/confirm a live Stripe account; create credit-package **Prices** in
   the Stripe dashboard.
2. `wrangler secret put STRIPE_API_KEY` (live `sk_`/`rk_` key).
3. `wrangler secret put STRIPE_CREDIT_PACKAGES_JSON` — JSON array of
   `{ id, label, amountCents, priceId }` matching the dashboard Price IDs and
   the package IDs the billing page offers (default `starter`).
4. Register the webhook endpoint
   `https://openagents.com/api/billing/stripe/webhook` in the Stripe dashboard
   (events: `checkout.session.completed`,
   `checkout.session.async_payment_succeeded`,
   `checkout.session.async_payment_failed`); copy its signing secret.
5. `wrangler secret put STRIPE_WEBHOOK_SIGNING_SECRET`.
6. Redeploy the Worker; smoke-buy one package end-to-end with a real/test card;
   confirm a `stripe_checkout` row lands in `billing_ledger_entries` and the
   `/billing` balance rises.

---

## Prioritized Gap List (to collect money now)

**P0 — turn on card collection (config only, no code):**
1. Set `STRIPE_API_KEY`, `STRIPE_WEBHOOK_SIGNING_SECRET`,
   `STRIPE_CREDIT_PACKAGES_JSON` as Worker secrets; create Stripe Prices;
   register the webhook endpoint. (Steps above.) Until then, checkout returns
   503 `stripe_unconfigured` and **no money is collected.**
2. Verify the billing page's offered package IDs match
   `STRIPE_CREDIT_PACKAGES_JSON` (default fallback is `starter`,
   `billing-routes.ts:255`); a mismatch yields `Unknown credit package`.

**P1 — close the credits->inference loop (if inference is to be sold):**
3. Decide the funding model and **build the bridge** from a paid USD credit
   (`billing_ledger_entries`) to the inference msat balance (`agent_balances`),
   OR unify inference metering onto the USD ledger. Today there is no path; a
   USD purchase can never fund `/v1/chat/completions`.
4. Add a USD->msat conversion + idempotent credit (new pay-in type or a sync job)
   so `applyStripeCheckoutCredit` (or a downstream worker) raises
   `agent_balances` for the buying account.
5. Flip `INFERENCE_GATEWAY_ENABLED` to `true` (and wire a real provider adapter)
   so the gateway actually meters; currently `false`
   (`wrangler.jsonc:40`).
6. Wire the currently-open abuse/fair-share/spend-cap gates
   (`index.ts:8667-8676`, left no-op) before charging real money for inference.

**P2 — Bitcoin/Lightning credit top-up (if desired):**
7. Build a BTC/Lightning credit-purchase rail for the human `/billing` page
   (e.g. BOLT12 offer -> on-pay credit to `billing_ledger_entries` or
   `agent_balances`). None exists today; card-only.

**P3 — operability:**
8. Add an operator reconciliation job for abandoned/unpaid checkout sessions
   (design-audit "Idempotency And Reconciliation" called for it; the
   `stripe_checkout_sessions` table exists but the reconcile sweep is not
   evidenced).
9. Wire chargeback/refund credit clawback off the Stripe dispute/refund webhook
   (`clawbackInferenceCredits` referenced at `index.ts:8673` but not wired).

---

## Honest Scope: Built+Live vs Demo/Config-Gated/Absent

**Genuinely built and production-grade (code-complete):**
- `/billing` page with real checkout, coupon, saved-card, auto-top-up.
- `/api/billing/*` routes: `summary`, `coupons/redeem`, `checkout`,
  `stripe/setup-intents(/save)`, `stripe/webhook`, `stripe/checkout-return`,
  `auto-top-up-policy`, `auto-top-up/run`.
- Stripe Effect services (config/client/customer/checkout/webhook/credit),
  idempotent fulfillment on both webhook + return paths, append-only derived
  USD ledger, suspension/reactivation, test coverage.

**Built but config-gated (will 503 until secrets set):**
- Live card collection — depends on `STRIPE_API_KEY`,
  `STRIPE_WEBHOOK_SIGNING_SECRET`, `STRIPE_CREDIT_PACKAGES_JSON` (not in repo;
  must be Worker secrets + Stripe dashboard Prices/webhook). Most likely
  **not yet collecting money** as of this audit.

**Disconnected / flag-off (so credits don't reach inference):**
- Inference gateway `/v1/chat/completions` — `INFERENCE_GATEWAY_ENABLED=false`,
  meters a separate `agent_balances` (msat) ledger; abuse/spend-cap gates and
  refund clawback unwired.
- No bridge from purchased USD credits to `agent_balances`.

**Absent:**
- Any Bitcoin/Lightning rail to BUY credits (card-only).

---

## Key Files

- `apps/openagents.com/workers/api/src/stripe-billing.ts` — Stripe services, checkout, webhook, fulfillment, auto-top-up.
- `apps/openagents.com/workers/api/src/billing-routes.ts` — `/api/billing/*` HTTP handlers (incl. 503 `stripe_unconfigured`).
- `apps/openagents.com/workers/api/src/billing.ts` — USD ledger (`billing_ledger_entries`), summary, debits (container/codex), `applyStripeCheckoutCredit`.
- `apps/openagents.com/workers/api/src/omni-routes.ts` — route path dispatch for `/api/billing/*`.
- `apps/openagents.com/workers/api/src/index.ts` — handler wiring (`:6466`, `:7466`, `:7505`); inference gateway wiring (`:8637-8677`).
- `apps/openagents.com/workers/api/wrangler.jsonc` — `OPENAGENTS_APP_URL` (`:43`), `INFERENCE_GATEWAY_ENABLED:"false"` (`:40`); Stripe secrets intentionally absent.
- `apps/openagents.com/apps/web/src/route.ts` — `/billing` route (`:74,549,705`).
- `apps/openagents.com/apps/web/src/page/loggedIn/page/billing.ts` + `apps/web/src/page/loggedIn/billing/{commands,transitions}.ts` — billing page UI + checkout command.
- `apps/openagents.com/apps/web/src/ui/credits-panel.ts` — reusable balance/cost-preview component (NOT the billing page; not yet exported).
- `apps/openagents.com/workers/api/src/payments-ledger.ts` — separate `agent_balances` (msat) ledger + pay-in types (no Stripe/USD type).
- `apps/openagents.com/workers/api/src/inference/metering-hook.ts` — inference metering, debits `agent_balances`.
- `apps/openagents.com/docs/2026-06-04-stripe-effect-service-audit.md` — the design this code implements.
