# Billing Page Reachability — How a User Buys Credits

Date: 2026-06-19

This documents the real end-to-end "how a user buys credits" path for the
`openagents.com` Worker, what works today, and the exact remaining gap for the
isolated staging environment (`openagents-staging.openagents.workers.dev`).

It accompanies the catalog-driven billing-page fix on branch
`fix-billing-page-reachable`.

## End-to-end "buy credits" path

1. **Sign in.** The user authenticates so the browser holds a session cookie.
   The billing page lives under the logged-in shell. (See "Sign-in on staging"
   for the exact provider and the staging gap.)
2. **Navigate to Billing.** The logged-in sidebar links "Billing" to
   `billingRouter()` → the billing page renders from `model.auth.billing`.
3. **Pick a package.** The page renders one buy button per **server-configured**
   credit package. Clicking POSTs that package's real catalog `id` to
   `POST /api/billing/checkout`.
4. **Stripe Checkout.** `handleBillingCheckoutApi` →
   `StripeCheckoutService.createCreditCheckout` creates a Stripe Checkout
   Session for the package `priceId` and returns its hosted `checkoutUrl`; the
   browser is redirected there. The user pays on Stripe.
5. **Return + webhook.** On success Stripe redirects to
   `GET /api/billing/stripe/checkout-return?session_id=...`
   (`handleBillingStripeCheckoutReturnApi`), which fulfills the session and
   303-redirects to `/billing`. Independently, Stripe calls
   `POST /api/billing/stripe/webhook` (`handleBillingStripeWebhookApi`); on
   `checkout.session.completed` it runs `fulfillCheckoutSession`, which credits
   the user's USD balance via `applyStripeCheckoutCredit` (idempotent per
   session). Either path applies the credit; the webhook is the authoritative
   one.
6. **Credit visible.** The next billing summary read shows the new
   `balanceCents` / `balanceFormatted`.

Server billing code: `workers/api/src/billing-routes.ts`,
`workers/api/src/stripe-billing.ts`, `workers/api/src/billing.ts`. Web billing
page: `apps/web/src/page/loggedIn/page/billing.ts` (+ `billing/commands.ts`,
`billing/transitions.ts`).

## What this branch fixed: catalog-driven packages (gap #1)

**Before:** `apps/web/src/page/loggedIn/page/billing.ts` hardcoded three
packages (`starter` / `builder` / `team`) with their own dollar amounts. The
server catalog is configured via `STRIPE_CREDIT_PACKAGES_JSON` (validated in
`workers/api/src/stripe-billing.ts`) and on staging holds `credits_10` /
`credits_50`. A buy click sent a `packageId` the server did not recognize, and
`createCreditCheckout` rejected it (`Unknown credit package.`). There was also a
fallback that defaulted a missing `packageId` to `'starter'`, which likewise
does not exist.

**After:** the billing page renders its purchasable packages **from the server
catalog**, so the UI can never offer or send an id the checkout endpoint will
reject.

- The billing summary now carries a `packages` projection of the server catalog
  (`id`, `label`, `amountCents`, `amountFormatted`, `currency`). The `id` is the
  real catalog id `/api/billing/checkout` accepts.
  - `BillingSummary.packages` + `BillingCreditPackageDisplay` +
    `withBillingCreditPackages(...)` in `workers/api/src/billing.ts`.
  - `readBillingCreditPackages(env)` in `workers/api/src/stripe-billing.ts`
    projects `STRIPE_CREDIT_PACKAGES_JSON` into display items. It is
    **catalog-only** (no Stripe API key required) and **best-effort** (missing
    or invalid catalog → empty list), so a billing summary read never fails on
    catalog shape, and an unconfigured environment simply shows no purchasable
    packages.
- Every browser-facing summary attaches the catalog: the authenticated
  bootstrap (`readAuthenticatedPageContext` in `workers/api/src/index.ts`) and
  all billing-route responses (summary, checkout, coupon, auto-top-up policy and
  run) via `readBillingSummaryWithPackages(...)` /
  `withBillingCreditPackages(...)` in `workers/api/src/billing-routes.ts`. This
  keeps the buy buttons stable after any billing action replaces
  `auth.billing`.
- `/api/billing/checkout` no longer defaults a missing `packageId`; it returns
  `400 package_required`.
- The web schema (`apps/web/src/domain/session.ts`) gains
  `BillingCreditPackage` and a required `BillingSummary.packages`; the page maps
  `billing.packages` to buy buttons.

Tests: `workers/api/src/billing-routes.test.ts` covers catalog rendering from
`STRIPE_CREDIT_PACKAGES_JSON`, the empty-catalog (unconfigured) case, and the
`package_required` rejection.

## Sign-in on staging (gap #2)

### Provider

Browser login is backed by **OpenAuth** (`@openauthjs/openauth`), delegating to
a GitHub OAuth provider and an email one-time-code provider. The real GitHub
OAuth secret is held by the OpenAuth **issuer** at `https://auth.openagents.com`
— not by the app Worker. So the staging var
`GITHUB_CLIENT_SECRET = "staging-unconfigured"` is **not** the blocker: the
staging Worker delegates login to the prod issuer (its OpenAuth client fetches
the issuer at `OPENAUTH_ISSUER_URL = https://auth.openagents.com`, which on
staging is a different hostname than the request, so it calls the real prod
issuer Worker).

Relevant code in `workers/api/src/index.ts`: login start `handleLoginStart`
(builds `redirectUri = ${getAppOrigin(env)}/auth/callback`), callback
`handleAuthCallback`, `makeAuthClient` / `makeIssuerAwareFetch`
(`getIssuerOrigin`), and the issuer's redirect-URI allowlist inside
`makeAuthIssuer`.

### Why staging sign-in does not complete on staging today

Two `env.staging` settings in `workers/api/wrangler.jsonc` point auth at
production:

1. `OPENAGENTS_APP_URL = "https://openagents.com"` (not the staging host). The
   login `redirect_uri` is built from this, so after GitHub/email auth the user
   is redirected to **`https://openagents.com/auth/callback`** and the session
   cookie is set for production — not for
   `openagents-staging.openagents.workers.dev`.
2. The prod issuer's redirect-URI **allowlist** (in `makeAuthIssuer`,
   `workers/api/src/index.ts`) only permits `openagents.com`,
   `auth.openagents.com`, `localhost`, and `127.0.0.1`. The staging host is not
   on it, so even a staging-host `redirect_uri` would be rejected by the prod
   issuer.

Net effect: a user who clicks sign-in on the staging Worker is bounced through
the prod issuer and lands authenticated on **production**, never on the staging
origin. The billing page itself is reachable and correct once a session exists;
the only thing missing on staging is a way to obtain that session **on the
staging origin**.

### Precise owner action to enable staging sign-in

These touch auth config / the issuer allowlist, so they are intentionally **not**
applied on this branch (owner review required). They are owner-providable config
changes, not a redesign:

1. **App URL (config var, owner-editable):** set the staging
   `OPENAGENTS_APP_URL` to `https://openagents-staging.openagents.workers.dev`
   in the `env.staging` block of `workers/api/wrangler.jsonc`, so the login
   `redirect_uri` returns to staging.
2. **Issuer allowlist (one-line code change in `index.ts`):** add
   `openagents-staging.openagents.workers.dev` to the `allow` hostname check in
   `makeAuthIssuer` so the prod issuer accepts the staging callback. (This is
   the only code change, and it widens — not weakens — the allowlist; prod
   hosts are unchanged.)
3. **GitHub OAuth app (owner, GitHub side):** the GitHub OAuth app (client id
   `Ov23lirHI1DWTzZ1zT1u`) is owned by the issuer; no client-secret change is
   needed for staging. The user-facing OAuth redirect remains the issuer's own
   callback, so no new GitHub authorized-callback entry is strictly required for
   the delegated flow. If a fully independent staging issuer is ever desired,
   that is a separate decision (separate issuer deployment + its own GitHub
   secret), not required here.

No production auth behavior is changed by documenting this; do not commit the
real GitHub secret or weaken the prod allowlist.

### Verifying the buy flow on staging before sign-in is enabled

Until staging sign-in lands, the catalog-driven billing fix is verifiable on
staging two ways:

- **API directly:** `GET /api/billing/summary` with a valid staging session
  cookie returns `billing.packages` equal to the staging
  `STRIPE_CREDIT_PACKAGES_JSON` catalog (`credits_10`, `credits_50`), and
  `POST /api/billing/checkout` with one of those ids returns a Stripe **TEST**
  Checkout URL (staging uses the "OA Payments Test" Stripe test keys).
- **Production:** the same flow works end-to-end on `openagents.com`, where
  sign-in already completes.

## Status summary

| Item | Status |
| --- | --- |
| Billing page renders packages from the server catalog | Fixed (this branch) |
| Buy button sends a catalog id the server accepts | Fixed (this branch) |
| Checkout → Stripe → webhook → credit | Works (prod; staging on TEST keys) |
| Billing page reachable once signed in | Works |
| Sign-in completes on the staging origin | Blocked — owner action above |
