# MoneyDevKit Local Source Audit For OpenAgents product surface

Date: 2026-06-07
Repository: `OpenAgentsInc/openagents`

## Source Set

This follow-up inspected the local MoneyDevKit source under
`/Users/christopherdavid/work/projects/moneydevkit/repos`.

| Repo | Revision | Relevant role |
| --- | --- | --- |
| `mdk-checkout` | `ff64215` | Current checkout SDK source. Contains `@moneydevkit/core` `0.19.0-beta.0` and local `@moneydevkit/api-contract` `0.1.36`. |
| `mdkd` | `9ffea5f` | Native daemon/node source for invoice creation, balance, invoice webhooks, and payout behavior. |
| `mdk-examples` | `25a7de4` | Next.js demo wiring around the checkout SDK. |
| `agent-skills` | `67031bd` | Agent-facing MDK skill references. |
| `api-contract` | `ee0da7e` | Older standalone contract package, version `0.1.22`; useful historical context, but not the current checkout SDK contract. |

The important divergence is that `mdk-checkout/packages/api-contract` is newer
than the standalone `api-contract` repo. OpenAgents product surface should model the current SDK
from `mdk-checkout`, not the older standalone contract checkout/product files.

## Checkout Source Shape

The Next.js and Replit packages are thin adapters over `@moneydevkit/core`.
Their route exports and Express router both forward to
`packages/core/src/route.ts`. OpenAgents product surface should port the core route/client behavior
into Effect services and schemas instead of installing a Next.js route.

The core route accepts JSON bodies with `handler`, `route`, or `target`. The
checkout handlers use:

- `create_checkout` with `{ params }`;
- `get_checkout` with `{ checkoutId }`;
- `confirm_checkout` with `{ confirm }`.

The SDK browser client posts to `/api/mdk` by default, adds the
`x-moneydevkit-csrf-token` header, and expects JSON responses shaped as
`{ data: ... }`. The route also allows server-to-server calls to CSRF routes
when `x-moneydevkit-webhook-secret` matches `MDK_ACCESS_TOKEN`.

OpenAgents product surface already has its own Site commerce route boundary. The Effect port should
map Site checkout intents into the same MDK checkout semantics, but it should
not expose raw MDK route payloads or credential material to the browser.

## Live Checkout Constraint

`@moneydevkit/core` checkout creation is not just an HTTP call. The live SDK
path:

1. validates `MDK_ACCESS_TOKEN` and `MDK_MNEMONIC`;
2. derives a node ID with the native `@moneydevkit/lightning-js` package;
3. calls `checkout.create` on `https://moneydevkit.com/rpc` with `x-api-key`;
4. calls `checkout.mintInvoice` when the checkout is confirmed;
5. relies on the merchant `/api/mdk` webhook/control path to spin up a native
   node session and service mdk.com's WS control calls.

That native `lightning-js` dependency is not Cloudflare Worker compatible.
For OpenAgents product surface, #434 must therefore choose one of these live paths:

- use a pure hosted/platform MDK API that does not require OpenAgents product surface's Worker to
  derive node IDs or host a native node session; or
- delegate the MDK node/control route to a Node-capable sidecar/function and
  keep OpenAgents product surface's Worker as the typed Site commerce authority.

Do not import `@moneydevkit/lightning-js` or port `createMoneyDevKitNode` into
Worker code.

## Product Creation Finding

The current checkout repo's local API contract has product CRUD inputs that
include an app binding (`appId`) in the newer package shape, while the older
standalone `api-contract` repo does not. The authenticated MCP product tool
available in this Codex session also did not expose an `appId` parameter.

This version/schema mismatch explains why repo-side demo checkout work should
continue to use the amount-checkout catalog mapping until a real MDK dashboard
product ID exists. Product-mode checkout can be added once the dashboard or a
fixed MCP/API path returns a stable product ID for the `OpenAgents product surface` app.

## Webhook Distinction

There are multiple webhook concepts in the MDK source:

- `mdk-checkout/packages/core/src/route.ts` authenticates `webhook` and
  `webhooks` with `x-moneydevkit-webhook-secret`, compared against
  `MDK_ACCESS_TOKEN`.
- `mdk-checkout/packages/core/src/handlers/webhooks.ts` treats those events as
  SDK/node-control events such as `incoming-payment` and subscription events.
  The `incoming-payment` path dials mdk.com's WS control plane and starts a
  native node loop.
- `mdkd/src/daemon/webhook/dispatcher.rs` sends per-invoice daemon webhooks
  with `X-MDK-Signature` and `X-MDK-Timestamp`.
- The live dashboard webhook docs describe `checkout.completed` style events
  using Standard Webhooks headers (`webhook-id`, `webhook-timestamp`,
  `webhook-signature`) and an `MDK_WEBHOOK_SECRET`.

#434 should not collapse those into one "MDK webhook". It must explicitly
select which event source OpenAgents product surface configures, verify that exact signature scheme,
and store only redacted event refs and digests.

## Payout And Balance Source Shape

The server-only payout helpers are in `mdk-checkout/packages/core/src/server.ts`.
They are better candidates for an Effect service port because they intentionally
do not import `@moneydevkit/lightning-js`.

The relevant operations are:

- `programmaticPayout({ amountSats?, destination, idempotencyKey })`;
- `waitForPayoutResult({ idempotencyKey } | { paymentId }, timeoutMs?)`;
- `getBalance()`.

All use `MDK_ACCESS_TOKEN` as an app-scoped API key, call mdk.com's oRPC
endpoint with `x-api-key`, classify retryable and terminal errors, and require
stable idempotency keys. The source rejects browser runtime calls, invalid
destinations, missing or mismatched amounts, and missing access tokens before
dispatch.

Balance is not manufactured by code. In `mdkd` tests, the merchant wallet gets
spendable Lightning balance by creating an invoice, having a separate payer
node pay it, waiting for settlement, and then reading `/getbalance`. For OpenAgents product surface
smokes, funding the isolated treasury test wallet remains an operator or
funding-source step. Any repo code should only record readiness and enforce
redaction.

## OpenAgents product surface Issue Impact

#434 should build the Effect/Worker checkout client from the MDK core and
current API-contract semantics, but it must not add the stock Next.js `/api/mdk`
route or native Lightning runtime to the Worker.

#435 should use OpenAgents product surface's Site commerce API and clean return state. The stock
React checkout appends `checkout-id` to the success URL; OpenAgents product surface's invariant
requires server-side consumption and clean first-party routes instead.

#431 and #436 should keep using isolated wallet setup and funding prerequisites.
The source-backed initial-balance path is an inbound payment to the treasury
test wallet, not a fake balance setter. The preview-only fake `pay_invoice`
path in the SDK is not a production funding path.

#437 should continue to wait for verified buyer payment/order state, payout
authority gates, public-safe receipt surfaces, and real two-wallet movement
evidence before bridging product payments into payout intents.
