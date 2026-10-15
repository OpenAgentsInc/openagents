# Paying for Pro in Bitcoin, through MoneyDevKit (plan)

Status: **plan, 2026-10-13.** The owner wants to collect Bitcoin for Pro and
credits with MoneyDevKit (MDK), which we already run. This sits next to the
Stripe card path in [README.md](README.md); it doesn't replace it.

## LND compatibility check (2026-10-15, #11754)

Pro stays on MDK. Its public [checkout contract](https://github.com/moneydevkit/api-contract/blob/ee0da7e839ee2feff64cc81501c61d64f361c6eb/src/contracts/checkout.ts)
requires both `nodeId` and `scid` for `registerInvoice`. The newer
`mintInvoice` flow delegates to the node holding the app's connection.
Neither contract proves that MDK accepts an independently issued LND invoice
or that it emits `subscription.created` for one. A staging checkout with an
MDK app credential, an LND invoice, and a signed subscription webhook is still
required before enabling that path. This is an unverified integration, not a
confirmed refusal by MDK. Agent credits use the separate direct pay-front path
in [the LND runbook](../payments/lnd-host.md#receiving-agent-credit-purchases).

## What we already have

- **The central receiver** (`e834410db9`, #10185). This is
  `openagents wallet serve` (`crates/wallet`) on mainnet as the
  `openagents-pay` unit, run per `docs/deployment/pay-host.md`.
  - It uses MDK's `ldk-node` fork and **MDK's LSPS4 just-in-time
    liquidity** (#9829/#9830): no channels to fund, no inbound to buy.
  - The seed lives in Secret Manager, with encrypted hourly backups.
  - x402 payments already land here. `docs/payments/2026-10-02-central-receive-and-splits.md`
    explains why this, not the older `MdkTreasuryContainer`, is the
    receiver. The LSP takes about 2% of a forwarded payment.
- **MDK's platform API**, as used by MDK's own Rust daemon `mdkd`
  (`projects/moneydevkit/repos/mdkd`, `src/mdk/mdk_api/client.rs`), comes
  down to three calls:
  - `create_checkout` (products, plus a customer with our `externalId`);
  - `register_invoice`;
  - `payment_received`.
- **MDK's hosted side** provides:
  - a checkout page;
  - **recurring products** (monthly/quarterly/yearly);
  - renewal reminder emails 7, 4 and 1 days before the period ends;
  - a 3-day grace period;
  - signed webhooks (`checkout.completed`, `subscription.created`,
    `subscription.renewed`, `subscription.canceled`; Standard Webhooks
    HMAC, `whsec_` secret).

MDK's own SDKs are Next.js and Express. We don't need them: we're the same
shape as `mdkd`, a Rust node that talks to the MDK platform.

## The plan

### Pro over Lightning

1. **An MDK product** "OpenAgents Pro": recurring, monthly, priced in USD
   ($20). It's paid in sats at the rate at checkout.
2. **Subscribe with Bitcoin** (Settings → Plan, the pricing page, and
   `openagents plan upgrade --bitcoin`).
   - The account service calls MDK `create_checkout` with the Pro product
     and `customer.externalId = <workspace id>`, plus the email for
     reminders.
   - The person pays the invoice on MDK's checkout page from any Lightning
     wallet, including Cash App.
3. **Our receiver takes the payment.**
   - The invoice is issued by `crates/wallet` on `openagents-pay` and
     registered with MDK (`register_invoice`).
   - It's reported paid with `payment_received`, the way `mdkd` does it.
   - The money lands on our own node (self-custodial); MDK never holds it.
4. **MDK webhooks move the billing book.** This is the same rule as Stripe:
   nothing is granted because a browser came back. A new
   `POST /v1/billing/mdk-webhook` checks the signature and the timestamp,
   then:
   - `subscription.created` starts Pro: a paid month, written into the
     environment meter like Stripe's `invoice.paid`;
   - `subscription.renewed` records the next paid month;
   - `subscription.canceled` ends Pro at the end of the paid month.
   - `past_due` (no payment by the end of the period) is MDK's 3-day grace,
     during which Pro stays on.
5. **One billing book.**
   - `tenancy::billing` records the rail (`stripe` or `lightning`) for each
     subscription, along with the USD price and the sats actually paid.
   - Entitlements don't care which rail paid.
   - A person can switch rails at renewal.

### Credits over Lightning

- The same receiver takes **credit top-ups**:
  - an MDK one-time checkout, or a plain invoice from `openagents wallet`;
  - credited to the workspace ledger once the payment settles.
- This is the "credits by card or Bitcoin" line in the spec. Usage stays
  Bitcoin-payable on any plan.

### Agents

- **This is the path that makes Pro fully self-serve for agents.**
  - An agent with a Lightning wallet can pay the Pro checkout with no card
    and no browser:
    - our own `openagents wallet`, or MDK's agent wallet CLI;
    - for example `openagents plan upgrade --bitcoin --json`, which gives
      back the invoice, and the agent pays it.
  - Renewal works the same way: the agent pays the renewal invoice when
    reminded.
- **Per-call usage stays on x402** (L402-compatible) through the same
  receiver, which is already live.

## The free trial on Bitcoin

- Lightning is push-only, with no saved payment method. So "card up front,
  charged after 3 days" doesn't exist here.
- **Recommendation:** the 3-day trial is offered on the card path only.
  Paying in Bitcoin starts Pro right away.
  - The trial's job is to remove the commitment of a card. Someone paying
    sats has already chosen to pay.
  - A no-payment Lightning trial would need a separate abuse defence (one per
    linked GitHub account, small trial limits). That can come later if
    Bitcoin sign-ups ask for it.
- **Cancelling** is just not paying the next invoice. There are no refunds
  for a started Lightning month: Lightning refunds need the payer to hand us
  an invoice, and that isn't worth building now.

## Grace periods, aligned

MDK's grace period is fixed at 3 days, so **use 3 days for both rails.**
That replaces the spec's proposed 7 for Stripe, and gives one rule
everywhere: "If a payment is missed, Pro stays on for 3 days."

## What we can drive from code

Checked against MDK's published API contract (`@moneydevkit/api-contract`
0.1.36):

- **With the app's access token (`MDK_ACCESS_TOKEN`), which our server
  holds:**
  - `checkout.create` / `get` / `confirm`, including checkouts for a
    recurring product, which is how a subscription starts;
  - `registerInvoice`, `paymentReceived`, and L402 `redeem` / `check`;
  - `products.list`.
  - Everything a subscribe flow needs at run time.
- **With the account MCP** (`https://mcp.moneydevkit.com/mcp/account/`,
  after the owner signs in to MoneyDevKit once):
  - products `create` / `update` / `delete` / `list`;
  - customers `create` / `get` / `update` / `delete` / `list`;
  - orders and checkouts `list` / `get`.
  - An agent can set up the Pro product and look up customers this way.
- **Not exposed in either API:** reading or cancelling a subscription
  (`subscription.get`, `cancel` and `createRenewalCheckout` exist in the
  contract but not in the routers above).
  - Subscription state comes from the webhooks.
  - Cancelling happens from the customer's renewal email or the MDK
    dashboard.
  - For our own "Cancel" button, we stop treating the subscription as
    renewing on our side and simply don't send a renewal, then mark it
    cancelled when MDK's `subscription.canceled` arrives after the grace
    period.
- **The app credentials** come from MDK's device sign-in (the `onboarding`
  contract: start, poll, bootstrap), which the owner approves once in the
  browser.

## Owner steps (NEEDS_OWNER)

1. **Sign in to the MDK account MCP once.** An agent then creates the app's
   "OpenAgents Pro" monthly $20 product itself. If you'd rather not, create
   it in the dashboard.
2. **Put the platform access token and the webhook signing secret** into
   Secret Manager.
3. **Add the webhook endpoint** `https://openagents.com/v1/billing/mdk-webhook`
   in the MDK dashboard.

Keep these separate from `MDK_TREASURY_*` (the old treasury container). Never
run a local wallet on the production seed.

## Build order (issues to file)

1. **The MDK platform client** in `crates/wallet`:
   - `create_checkout`, `register_invoice` and `payment_received`;
   - modelled on `mdkd`, with tests against a stub MDK API;
   - checkouts issued by the `openagents-pay` receiver.
2. **`POST /v1/billing/mdk-webhook`:**
   - Standard Webhooks verification;
   - the three subscription events and `checkout.completed`;
   - the billing book with the payment rail recorded, and the environment
     meter;
   - replay-safe.
3. **Subscribe with Bitcoin** on Settings → Plan and the pricing page, plus
   `openagents plan upgrade --bitcoin [--json]`.
4. **Credit top-ups over Lightning** into the workspace ledger.
5. **One regtest/signet end-to-end:** checkout, then pay, then the webhook,
   then Pro on. After that, the first real mainnet subscription, which is
   an owner step.

## Open questions

1. **Is the Bitcoin price exactly $20 in sats at checkout,** or a small
   Bitcoin discount (e.g. $18), since the LSP fee is about 2% against card
   fees of about 3% + 30¢?
2. **Should Bitcoin get an annual plan from day one,** since MDK supports
   yearly? One payment a year suits push payments well.
3. **Should paying for Pro in Bitcoin count towards the launch bonus rules**
   in `docs/payments/2026-10-02-central-receive-and-splits.md`?
