# moneydevkit: Overview and Reference

**Purpose:** Document [moneydevkit](https://moneydevkit.com) — what it is, how it works, and how it relates to the OpenAgents web app. This doc is based on the [moneydevkit monorepo](https://github.com/moneydevkit) at `/Users/christopherdavid/code/moneydevkit/` and the public docs at [docs.moneydevkit.com](https://docs.moneydevkit.com/).

---

## 1. What Is moneydevkit?

From [docs.moneydevkit.com](https://docs.moneydevkit.com/):

- **Tagline:** “Global payments for any app in less than 5 minutes.”
- **Mechanism:** Uses **Bitcoin (Lightning)** under the hood so payments can be global with little friction. Self-custodial: you control your keys; moneydevkit does not hold funds.
- **Status:** Public beta. Community: Discord; feedback and questions encouraged.
- **Pricing:** 2% per transaction (from product/docs).

**Why Lightning:** Enables anyone with a wallet or app (e.g. Cash App’s large user base) to pay with minimal friction and global reach.

---

## 2. How Does moneydevkit Work?

From [How Does moneydevkit Work?](https://docs.moneydevkit.com/howitworks) and the repo’s [ARCHITECTURE.md](https://github.com/moneydevkit/moneydevkit/blob/main/ARCHITECTURE.md):

moneydevkit uses a **hybrid architecture**:

1. **Hosted API (moneydevkit.com)**  
   Checkout management, product catalog, coordination, optional VSS/Esplora/LSP.
2. **Self-hosted Lightning node (in your app)**  
   Actual payment processing: your app runs a Lightning node (via `ldk-node` / `lightning-js`), creates and receives invoices, and keeps custody of funds.

**Endpoints:**

- Mainnet: `https://moneydevkit.com/rpc`
- Signet (testnet): `https://staging.moneydevkit.com/rpc`

**Auth:** API key via `x-api-key` header. You get the key when you create an account (or via `npx @moneydevkit/create` device-flow OAuth).

**Payment flow (high level):**

1. Your app calls the hosted API to create a checkout session.
2. Your local Lightning node creates an invoice.
3. Your app registers the invoice with the API.
4. Customer pays the invoice on the Lightning Network.
5. Your node receives payment and notifies the API (e.g. webhook).

So: **hosted** = coordination and product/checkout state; **self-hosted** = your node and your keys.

---

## 3. Repo Structure (moneydevkit Monorepo)

The [moneydevkit](https://github.com/moneydevkit) org is represented locally as a single repo with these sub-repos:

| Repo | Role |
|------|------|
| **api-contract** | TypeScript API contract (oRPC). Schemas and RPC definitions for checkout, customer, onboarding, order, products, subscription. Used by client SDKs and the API server. |
| **bitcoin-payment-instructions** | Rust parser for Bitcoin payment instruction encodings (QR, URI, paste). |
| **ldk-node** | Rust Lightning node library (LDK + BDK). Self-custodial node with integrated on-chain wallet; UniFFI bindings for Swift, Kotlin, Python. |
| **lightning-js** | JavaScript/TypeScript bindings for Lightning (backed by Rust). Used so Node/Next.js can run the Lightning node. |
| **mdk-checkout** | Checkout UX and dev tooling. Packages: `@moneydevkit/nextjs`, `@moneydevkit/create`, plus `@moneydevkit/core` (route handlers, Lightning node wiring, checkout components). |
| **mdk-examples** | Example apps. Includes **mdk-nextjs-demo**: Next.js App Router demo (create checkout, hosted checkout page, success page, `/api/mdk`). |
| **rust-lightning** | Core Lightning implementation (LDK). Protocol, BOLTs, runtime-agnostic. |
| **vss-server** | Versioned Storage Service (Rust/Java). Optional server-side backup/sync for non-custodial Lightning wallet state (recovery, multi-device). |

**Grouping (from root README):**

- **Core Lightning:** `rust-lightning`, `ldk-node`
- **Developer tools:** `mdk-checkout`, `mdk-examples`, `api-contract`
- **Infrastructure:** `vss-server`
- **Utilities:** `bitcoin-payment-instructions`, `lightning-js`

---

## 4. API Contract (api-contract)

The API is type-safe via [oRPC](https://orpc.unnoq.com/). Main surfaces:

**Checkout:** create, get, confirm, registerInvoice, paymentReceived, list (including paginated/summary). Inputs: `nodeId`, optional `amount`/`currency`/`products`/`successUrl`/`customer`/`requireCustomerData`/metadata. Checkout statuses: UNCONFIRMED, CONFIRMED, PENDING_PAYMENT, PAYMENT_RECEIVED, EXPIRED.

**Customer:** create, get, update, delete, list (paginated), lookup. Used to attach customer data to checkouts and orders.

**Onboarding:** bootstrap, device auth (start, poll) for CLI/device flow (e.g. `npx @moneydevkit/create`).

**Order:** get, list (paginated). Orders are created when checkouts are paid.

**Products:** create, get, update, delete, list. Product catalog for product-based checkouts; supports fixed and CUSTOM (pay-what-you-want) prices.

**Subscription:** create renewal checkout, get subscription, cancel. For recurring payments.

**MCP contract:** A subset of the API is exposed as an MCP contract (customers, orders, checkouts summary, products) for tool/agent use.

---

## 5. Integrations (from docs.moneydevkit.com)

**Next.js**

- Install: `npm install @moneydevkit/nextjs`
- Credentials: account at [moneydevkit.com](https://moneydevkit.com) or `npx @moneydevkit/create` to mint credentials (device flow).
- Env: `MDK_ACCESS_TOKEN`, `MDK_MNEMONIC` (and optional base URLs, network).
- Usage:
  1. **Create checkout** (client): `useCheckout()` → `createCheckout({ type, title, description, amount, currency, successUrl, … })` → redirect to `checkoutUrl`.
  2. **Checkout page:** `<Checkout id={id} />` in `app/checkout/[id]/page.js`.
  3. **API route:** `export { POST } from "@moneydevkit/nextjs/server/route"` at `app/api/mdk/route.js` (unified endpoint for the SDK).
  4. **Next config:** `withMdkCheckout(nextConfig)` from `@moneydevkit/nextjs/next-plugin`.
- Checkout types: `AMOUNT` (donations, custom amounts) or `PRODUCTS` (product ID from dashboard). Optional customer data and `requireCustomerData`. Success page can use `useCheckoutSuccess()` to verify payment and read metadata.

**Replit**

- [docs.moneydevkit.com](https://docs.moneydevkit.com/) mentions a Replit (Express + Vite) integration so Replit’s agent can set up moneydevkit for you. The repo has `mdk-checkout/packages/replit` (Express server, Checkout component, hooks).

**Troubleshooting**

- Docs include an integrations/troubleshooting section for common setup issues.

---

## 6. Dashboard (moneydevkit.com)

The [moneydevkit.com](https://moneydevkit.com) dashboard (from docs) covers:

- **Products** – Catalog for product checkouts; prices (fixed or CUSTOM).
- **Orders** – Result of completed checkouts.
- **Checkouts** – Sessions and status.
- **Customers** – Customer records linked to checkouts/orders.
- **Payouts** – (Referenced in dashboard overview; exact behavior is product-specific.)

Access and API keys are tied to your account.

---

## 7. Coming from Stripe?

Docs include a “Coming from Stripe?” section to help migrate from traditional processors. Conceptually:

- **Stripe:** Hosted payment processing; Stripe holds and moves funds; you get payouts.
- **moneydevkit:** Hosted coordination (checkouts, products, customers) + **your** Lightning node; you receive funds directly (self-custodial). No third party holding your funds.

So the mental model is “Stripe-like checkout and product/order/customer APIs, but with a self-custodial Lightning backend.”

---

## 8. Configuration (from ARCHITECTURE.md)

```env
# Required
MDK_ACCESS_TOKEN=your_api_key_from_account
MDK_MNEMONIC=your_wallet_seed_phrase

# Optional (defaults to hosted services)
MDK_API_BASE_URL=https://moneydevkit.com/rpc
MDK_VSS_URL=https://vss.moneydevkit.com/vss
MDK_ESPLORA_URL=https://esplora.moneydevkit.com/api
MDK_NETWORK=mainnet   # or signet
```

Your app must run a Lightning node (via `lightning-js` / `ldk-node`) and persist its state (local storage or optional VSS).

---

## 9. AI Agents (from ARCHITECTURE.md)

The architecture doc states that AI agents **can** use moneydevkit:

- **API:** Create checkouts, manage products, query status with an access token.
- **Node:** If the agent’s environment runs the Lightning node, it can create invoices, process payments, handle webhooks.
- **Caveats:** Account and credentials (including mnemonic) must be set up and stored securely; node state must be persisted; network access (and optionally LSP) required.

Use cases mentioned: e-commerce bots, payment links, invoice generation, subscriptions, refunds, monitoring.

---

## 10. Relevance to OpenAgents apps/web

- **apps/web** today: Astro + Convex + Cloudflare Pages. No Lightning node, no payment processing.
- **moneydevkit** assumes: Node.js (or similar) runtime that can run `lightning-js` (native bindings) and a long-lived Lightning node process. The unified “MDK” route (`POST /api/mdk`) runs server-side and talks to both the hosted API and the local node.

**Implications:**

- **Cloudflare Pages:** Standard Pages is static/SSR and does not run a persistent Node process or native addons. So you **cannot** run the full moneydevkit stack (Lightning node + `/api/mdk`) directly on Pages in the same way as the Next.js or Replit examples.
- **Options if we want moneydevkit-style payments:**
  1. **Separate Node service:** Run a Node (or Rust) service (e.g. on a VPS, Fly, Railway) that runs the Lightning node and exposes the MDK API; apps/web calls that service to create checkouts and redirect users to the hosted checkout page (or an iframe/embed if supported).
  2. **Use moneydevkit hosted checkout only:** If moneydevkit ever offers a “hosted node” or proxy mode where the API also creates/registers invoices (no self-hosted node), apps/web could call that API from a serverless/edge function and only render the checkout UI. Today the design is “your node, their API,” so this would be a product change on their side.
  3. **Link-out:** Add a “Pay with Lightning (moneydevkit)” flow that redirects to a separate app (e.g. Next.js) that is fully configured with moneydevkit; after payment, redirect back to apps/web with a token or query params. No Lightning node on our stack.

This doc is a reference for product and architecture only; it does not prescribe an implementation. For feasibility of embedding a full checkout (including node) in apps/web, see also [BREEZ_SPARK_DEMO_FEASIBILITY.md](./BREEZ_SPARK_DEMO_FEASIBILITY.md) (browser WASM vs server-side node).

---

## 11. Links

- **Product:** [moneydevkit.com](https://moneydevkit.com)
- **Docs:** [docs.moneydevkit.com](https://docs.moneydevkit.com/)
- **GitHub:** [github.com/moneydevkit](https://github.com/moneydevkit)
- **Next.js package:** `@moneydevkit/nextjs`
- **Create CLI:** `npx @moneydevkit/create`
- **Next.js demo:** [mdk-nextjs-demo (Vercel)](https://mdk-nextjs-demo-brown.vercel.app)
