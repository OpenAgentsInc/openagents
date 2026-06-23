<!--
Sitemap:
- [Machine Payments Protocol](/overview): MPP standardizes HTTP 402 for machine-to-machine payments. Learn how agents, apps, and services exchange payments in the same HTTP request.
- [Frequently asked questions](/faq): Answers to common questions about MPP—payment methods, settlement, pricing, security, and how the protocol compares to API keys and subscriptions.
- [Build with an LLM](/guides/building-with-an-llm): Use llms-full.txt to give your agent complete MPP context.
- [Quickstart](/quickstart/): Get started with MPP in minutes. Protect your API with payments, connect your agent, or integrate your app with MPP-enabled services.
- [Add payments to your API](/quickstart/server): Add payment-gated access to your API with mppx. Accept stablecoins, cards, and Bitcoin in a few lines of code using the MPP server SDK.
- [Use with agents](/quickstart/agent): Connect your coding agent to MPP-enabled services. Set up a wallet to handle payment flows automatically.
- [Use with your app](/quickstart/client): Handle payment-gated resources in your app. Use the mppx client SDK to intercept 402 responses, pay, and retry—all automatically.
- [Accept one-time payments](/guides/one-time-payments): Charge per API call with MPP. Accept pay-per-request payments from agents, apps, and users—no API keys or subscriptions required.
- [Accept pay-as-you-go payments](/guides/pay-as-you-go): Build a payment-gated API with session-based billing using mppx payment channels. Charge per request with near-zero latency overhead.
- [Accept streamed payments](/guides/streamed-payments): Accept streamed payments over Server-Sent Events with mppx. Bill per token in real time using Tempo payment channels for LLM inference APIs.
- [Create and manage subscriptions](/guides/subscription-payments): Build a subscription-gated API with MPP. Let clients authorize recurring stablecoin payments and reuse access across requests.
- [Use MPP with x402](/guides/use-mpp-with-x402): Use MPP with x402 to support exact stablecoin flows, multi-method payments, sessions, and IETF standardization.
- [Accept card payments](/guides/accept-card-payments): Accept card payments via Stripe on your MPP-enabled API. Charge Visa, Mastercard, and other card networks—no stablecoin wallet required.
- [Accept split payments](/guides/split-payments): Split a single charge across multiple recipients in one atomic transaction. Route platform fees, referral bounties, and revenue shares with mppx.
- [Accept multiple payment methods](/guides/multiple-payment-methods): Accept Tempo stablecoins, Stripe cards, and Lightning Bitcoin on a single API endpoint. Serve a multi-method 402 Challenge and let clients choose.
- [Create a payment link](/guides/payment-links): Create a payment link for any API endpoint. Share it anywhere—users pay directly from the page, no integration required.
- [Monetize your MCP server](/guides/monetize-mcp-server): Add payments to your MCP server. Charge per tool call with stablecoins—no API keys or billing portals required
- [Proxy an existing service](/guides/proxy-existing-service): Put a payment gate in front of any API without changing its code. Use the mppx Proxy SDK to charge for upstream access.
- [Protocol overview](/protocol/): The Machine Payments Protocol standardizes HTTP 402 with an extensible challenge–credential–receipt flow that works with any payment network.
- [HTTP 402 Payment Required](/protocol/http-402): HTTP 402 Payment Required signals that a resource requires payment. Learn when and how MPP servers return 402 with a WWW-Authenticate Challenge.
- [Challenges](/protocol/challenges): Create MPP Challenges that tell clients what a resource costs, which method to use, and when the payment request expires.
- [Credentials](/protocol/credentials): Verify MPP Credentials from clients and bind payment proofs to the original server-issued Challenge.
- [Payment receipts and verification](/protocol/receipts): Receipts confirm successful payment in MPP. Return them in the Payment-Receipt header so clients can verify that the server accepted their Credential.
- [Transports](/protocol/transports/): Map MPP Challenges, Credentials, and Receipts to HTTP headers, JSON-RPC messages, and WebSocket frames.
- [HTTP transport](/protocol/transports/http): The HTTP transport maps MPP payment flows to standard HTTP headers—WWW-Authenticate for Challenges, Authorization for Credentials, and Payment-Receipt.
- [MCP and JSON-RPC transport](/protocol/transports/mcp): Use the MCP transport to require payment for JSON-RPC tool calls while preserving MPP Challenges, Credentials, and Receipts.
- [WebSocket transport](/protocol/transports/websocket): The WebSocket transport streams paid data over a persistent connection, with in-band voucher top-ups and JSON message framing.
- [Discovery](/advanced/discovery): Advertise your API's payment terms with an OpenAPI discovery document so clients and agents know what endpoints cost before making requests.
- [Identity](/advanced/identity): Use MPP Credentials for access control, rate limiting, and multi-step workflows—without requiring payment.
- [Refunds](/advanced/refunds): Return funds to clients after a charge, or let sessions refund unused deposits automatically.
- [Security](/advanced/security): Protect MPP server secrets and payment credentials. Keep MPP_SECRET_KEY server-side, never log it, and rotate it safely.
- [Payment methods](/payment-methods/): Compare MPP payment methods and choose the right rails for your API, app, or agent workflow.
- [Charge intent for one-time payments](/intents/charge): Charge intent defines one-time payments in MPP. Use it when a client pays once for a request before receiving the resource.
- [The subscription intent for recurring payments](/intents/subscription): The subscription intent defines recurring fixed payments in MPP. Use it when access renews across billing periods.
- [Tempo stablecoin payments](/payment-methods/tempo/): Use Tempo payment methods in MPP for stablecoin charges and low-cost payment sessions.
- [Tempo charge](/payment-methods/tempo/charge): Accept one-time stablecoin payments on Tempo with signed TIP-20 token transfers.
- [Sessions](/payment-methods/tempo/session): Accept pay-as-you-go stablecoin payments with Sessions.
- [Tempo subscription](/payment-methods/tempo/subscription): Accept recurring stablecoin payments on Tempo with subscriptions backed by scoped access keys.
- [EVM payment method](/payment-methods/evm/): Use EVM payment methods in MPP to accept stablecoin payments and run x402 exact flows inline.
- [EVM charge payment method](/payment-methods/evm/charge): Accept one-time EVM stablecoin payments with MPP and inline x402 exact compatibility.
- [Stripe payment method](/payment-methods/stripe/): Use Stripe payment methods in MPP to accept cards, wallets, and other Stripe-supported payment methods.
- [Stripe charge](/payment-methods/stripe/charge): Accept one-time Stripe payments in MPP with Shared Payment Tokens and browser payment flows.
- [Card payment method](/payment-methods/card/): Use card payments in MPP to accept traditional payment methods with encrypted network tokens.
- [Card charge](/payment-methods/card/charge): Accept one-time card payments in MPP using encrypted network tokens and card-compatible payment flows.
- [Lightning](/payment-methods/lightning/): Use Lightning payment methods in MPP for Bitcoin charges and prepaid session access.
- [Lightning charge](/payment-methods/lightning/charge): Accept one-time Bitcoin payments over Lightning with BOLT11 invoices and MPP charge flows.
- [Lightning session](/payment-methods/lightning/session): Accept pay-as-you-go Lightning payments with prepaid sessions and per-request billing.
- [Solana](/payment-methods/solana/): Use Solana payment methods in MPP to accept SOL and SPL token payments.
- [Solana charge](/payment-methods/solana/charge): Accept one-time Solana payments in MPP with signed transactions or confirmed signatures.
- [Stellar SEP-41 token payments](/payment-methods/stellar/): Use Stellar payment methods in MPP to accept SEP-41 token payments and channel-based sessions.
- [Stellar charge](/payment-methods/stellar/charge): Accept one-time Stellar token payments in MPP using SEP-41 assets and server-side verification.
- [Channel](/payment-methods/stellar/session): Accept high-frequency Stellar payments with one-way payment channels and pay-as-you-go billing.
- [Monad](/payment-methods/monad/): Use Monad payment methods in MPP for ERC-20 token payments with push and pull settlement modes.
- [Monad charge](/payment-methods/monad/charge): Accept one-time Monad payments with ERC-20 transfers or ERC-3009 authorizations.
- [RedotPay payment method](/payment-methods/redotpay/): Accept MPP payments using RedotPay balance or stablecoin rails.
- [RedotPay charge](/payment-methods/redotpay/charge): One-time payments with the RedotPay payment method.
- [Custom payment methods](/payment-methods/custom): Build a custom MPP payment method with your own request schema, Credential format, and server verification logic.
- [SDKs and client libraries](/sdk/): Official MPP SDKs in TypeScript, Python, Rust, Go, and Ruby, plus community SDKs in other languages.
- [SDK features](/sdk/features): Feature parity across TypeScript, Python, Rust, and Ruby MPP SDKs.
- [Getting started](/sdk/typescript/): Use the mppx TypeScript SDK to build MPP clients, servers, middleware, and payment-aware fetch flows.
- [evm client method](/sdk/typescript/client/Method.evm): Sign EVM charge Credentials
- [evm.charge client method](/sdk/typescript/client/Method.evm.charge): Sign EVM charge Credentials
- [tempo client method](/sdk/typescript/client/Method.tempo): Register Tempo charge and session support in an MPP TypeScript client.
- [Method.tempo.charge](/sdk/typescript/client/Method.tempo.charge): One-time payments
- [tempo.session](/sdk/typescript/client/Method.tempo.session): Sessions client method
- [tempo.session.manager](/sdk/typescript/client/Method.tempo.session-manager): Sessions manager
- [Method.tempo.subscription](/sdk/typescript/client/Method.tempo.subscription): Recurring stablecoin payments
- [stripe client method](/sdk/typescript/client/Method.stripe): Register Stripe charge support in an MPP TypeScript client.
- [Method.stripe.charge](/sdk/typescript/client/Method.stripe.charge): One-time payments via Shared Payment Tokens
- [Mppx.create](/sdk/typescript/client/Mppx.create): Create a payment-aware fetch client
- [Mppx.restore](/sdk/typescript/client/Mppx.restore): Restore the original global fetch
- [Fetch.from](/sdk/typescript/client/Fetch.from): Wrap fetch with automatic MPP payments without changing global fetch.
- [Fetch.polyfill](/sdk/typescript/client/Fetch.polyfill): Install a global fetch wrapper that handles MPP payments automatically.
- [Fetch.restore](/sdk/typescript/client/Fetch.restore): Restore the original fetch after installing the MPP fetch polyfill.
- [Transport.from](/sdk/typescript/client/Transport.from): Create a custom transport
- [Transport.http](/sdk/typescript/client/Transport.http): HTTP transport for payments
- [Transport.mcp](/sdk/typescript/client/Transport.mcp): MCP transport for payments
- [McpClient.wrap](/sdk/typescript/client/McpClient.wrap): Payment-aware MCP client
- [evm server method](/sdk/typescript/server/Method.evm): Create EVM charge Challenges
- [evm.charge server method](/sdk/typescript/server/Method.evm.charge): One-time EVM payments
- [tempo server method](/sdk/typescript/server/Method.tempo): Register Tempo charge and session support in an MPP TypeScript server.
- [Method.tempo.charge](/sdk/typescript/server/Method.tempo.charge): One-time stablecoin payments
- [Method.tempo.session](/sdk/typescript/server/Method.tempo.session): Sessions server method
- [Method.tempo.subscription](/sdk/typescript/server/Method.tempo.subscription): Recurring stablecoin payments
- [stripe](/sdk/typescript/server/Method.stripe): Register all Stripe intents
- [Method.stripe.charge](/sdk/typescript/server/Method.stripe.charge): One-time payments via Shared Payment Tokens
- [Mppx.compose](/sdk/typescript/server/Mppx.compose): Present multiple payment options
- [Mppx.create](/sdk/typescript/server/Mppx.create): Create a server-side payment handler
- [Mppx.toNodeListener](/sdk/typescript/server/Mppx.toNodeListener): Adapt payments for Node.js HTTP
- [mppx.verifyCredential](/sdk/typescript/server/Mppx.verifyCredential): Verify MPP Credentials directly in custom transports and background flows.
- [Transport.from](/sdk/typescript/server/Transport.from): Create a custom transport
- [Transport.http](/sdk/typescript/server/Transport.http): HTTP server-side transport
- [Transport.mcp](/sdk/typescript/server/Transport.mcp): Raw JSON-RPC MCP transport
- [Transport.mcpSdk](/sdk/typescript/server/Transport.mcpSdk): MCP SDK server-side transport
- [tempo.Ws.serve](/sdk/typescript/server/Ws.serve): WebSocket session payments
- [Method.tempo.renewSubscription](/sdk/typescript/server/Method.tempo.renewSubscription): Renew subscriptions outside requests
- [Response.requirePayment](/sdk/typescript/server/Response.requirePayment): Create a 402 response
- [Request.toNodeListener](/sdk/typescript/server/Request.toNodeListener): Convert Fetch handlers to Node.js
- [Elysia payment middleware](/sdk/typescript/middlewares/elysia): Protect Elysia routes with MPP payment middleware.
- [Express payment middleware](/sdk/typescript/middlewares/express): Protect Express routes with MPP payment middleware.
- [Hono payment middleware](/sdk/typescript/middlewares/hono): Protect Hono routes with MPP payment middleware.
- [Next.js payment middleware](/sdk/typescript/middlewares/nextjs): Protect Next.js route handlers with MPP payment middleware.
- [Paid API proxy server](/sdk/typescript/proxy): Put MPP payments in front of existing upstream APIs.
- [BodyDigest.compute](/sdk/typescript/core/BodyDigest.compute): Compute a body digest hash
- [BodyDigest.verify](/sdk/typescript/core/BodyDigest.verify): Verify a body digest hash
- [Challenge.deserialize](/sdk/typescript/core/Challenge.deserialize): Deserialize a Challenge from a header
- [Challenge.from](/sdk/typescript/core/Challenge.from): Create a new Challenge
- [Challenge.fromHeaders](/sdk/typescript/core/Challenge.fromHeaders): Extract a Challenge from Headers
- [Challenge.fromMethod](/sdk/typescript/core/Challenge.fromMethod): Create a Challenge from a method
- [Challenge.fromResponse](/sdk/typescript/core/Challenge.fromResponse): Extract a Challenge from a Response
- [Challenge.meta](/sdk/typescript/core/Challenge.meta): Extract correlation data from a Challenge
- [Challenge.serialize](/sdk/typescript/core/Challenge.serialize): Serialize a Challenge to a header
- [Challenge.verify](/sdk/typescript/core/Challenge.verify): Verify a Challenge HMAC
- [Credential.deserialize](/sdk/typescript/core/Credential.deserialize): Deserialize a Credential from a header
- [Credential.from](/sdk/typescript/core/Credential.from): Create a new Credential
- [Credential.fromRequest](/sdk/typescript/core/Credential.fromRequest): Extract a Credential from a Request
- [Credential.serialize](/sdk/typescript/core/Credential.serialize): Serialize a Credential to a header
- [Expires utility functions](/sdk/typescript/core/Expires): Use Expires helpers to set relative expiration timestamps for MPP Challenges and payment requests.
- [Method.from](/sdk/typescript/core/Method.from): Create a payment method definition
- [Method.toClient](/sdk/typescript/core/Method.toClient): Extend a method with client logic
- [Method.toServer](/sdk/typescript/core/Method.toServer): Extend a method with server verification
- [PaymentRequest.deserialize](/sdk/typescript/core/PaymentRequest.deserialize): Deserialize a payment request
- [PaymentRequest.from](/sdk/typescript/core/PaymentRequest.from): Create a payment request
- [PaymentRequest.serialize](/sdk/typescript/core/PaymentRequest.serialize): Serialize a payment request to a string
- [Receipt.deserialize](/sdk/typescript/core/Receipt.deserialize): Deserialize a Receipt from a header
- [Receipt.from](/sdk/typescript/core/Receipt.from): Create a new Receipt
- [Receipt.fromResponse](/sdk/typescript/core/Receipt.fromResponse): Extract a Receipt from a Response
- [Receipt.serialize](/sdk/typescript/core/Receipt.serialize): Serialize a Receipt to a string
- [Html.init](/sdk/typescript/Html.init): Build custom payment UIs for browser-based 402 flows
- [Custom HTML](/sdk/typescript/html/custom): Add payment link support to a custom payment method with Html.init and Method.toServer
- [CLI Reference](/sdk/typescript/cli): Use the mppx CLI to make paid HTTP requests from the terminal with automatic MPP payment handling.
- [Python SDK](/sdk/python/): Use the Python SDK to build MPP clients and servers with typed Challenge, Credential, and Receipt primitives.
- [Core Types](/sdk/python/core): Use Python core types for MPP Challenges, Credentials, Receipts, payment requests, and verification flows.
- [Python MPP client](/sdk/python/client): Use the Python client to handle HTTP 402 responses, pay with supported methods, and retry requests automatically.
- [Server](/sdk/python/server): Use the Python server SDK to protect FastAPI endpoints with MPP payment requirements and verification.
- [Rust SDK for MPP](/sdk/rust/): Use the Rust SDK to build MPP clients and servers with typed Challenge, Credential, and Receipt primitives.
- [Core types](/sdk/rust/core): Use Rust core types for MPP Challenges, Credentials, Receipts, payment requests, and verification flows.
- [Client](/sdk/rust/client): Use the Rust client to handle HTTP 402 responses, pay with supported methods, and retry requests automatically.
- [Server](/sdk/rust/server): Use the Rust server SDK to protect Axum endpoints with MPP payment requirements and verification.
- [Go SDK](/sdk/go/): Use the Go SDK to build MPP clients and servers with typed Challenge, Credential, and Receipt primitives.
- [Core types](/sdk/go/core): Use Go core types for MPP Challenges, Credentials, Receipts, payment requests, and verification flows.
- [Client](/sdk/go/client): Use the Go client to handle HTTP 402 responses, pay with supported methods, and retry requests automatically.
- [Server](/sdk/go/server): Use the Go server SDK to protect HTTP endpoints with MPP payment requirements and verification.
- [Ruby SDK](/sdk/ruby/): Use the Ruby SDK to build MPP clients and servers with typed Challenge, Credential, and Receipt primitives.
- [Core Types](/sdk/ruby/core): Use Ruby core types for MPP Challenges, Credentials, Receipts, payment requests, and verification flows.
- [Client](/sdk/ruby/client): Use the Ruby client to handle HTTP 402 responses, pay with supported methods, and retry requests automatically.
- [Server](/sdk/ruby/server): Use the Ruby server SDK to protect Rack endpoints with MPP payment requirements and verification.
- [Wallets](/tools/wallet): Agent wallets for MPP -- enable your agent to pay for services.
- [Agentic payments](/use-cases/agentic-payments): Learn how coding agents pay for APIs autonomously with MPP. No API keys, no signup forms—agents handle payments inline via HTTP 402.
- [API monetization](/use-cases/api-monetization): Monetize your API with per-request payments using HTTP 402. No API keys, subscriptions, or billing dashboards required.
- [Micropayments](/use-cases/micropayments): MPP makes micropayments viable with stablecoin settlement and off-chain payment sessions—no minimum transaction size, no fixed per-transaction fees.
- [Extensions](/extensions): Community-built tools and integrations for MPP
- [Brand assets and guidelines](/brand): Download official MPP logos, wordmarks, and brand assets. Guidelines for using the Machine Payments Protocol brand in your project or integration.
- [MPP — Machine Payments Protocol](/index): Charge for API requests, tool calls, and content with HTTP 402 payments co-developed by Tempo and Stripe.
- [Page Not Found](/404)
- [MPP vs x402](/mpp-vs-x402): Compare MPP vs x402 for HTTP 402 payments. Learn the protocol differences, supported payment methods, session support, and when to choose each approach.
- [Method.from](/sdk/typescript/Method.from): Create a payment method from a definition
-->

# Stripe charge \[One-time payments using Shared Payment Tokens]

The Stripe implementation of the [charge](/intents/charge) intent.

The client creates a [Shared Payment Token (SPT)](https://docs.stripe.com/agentic-commerce/concepts/shared-payment-tokens) and sends it as a Credential. The server creates a Stripe `PaymentIntent` using the SPT, and settlement completes through Stripe's payment rails.

Use this method for single API calls, content access, or one-off purchases where you want to accept cards, wallets, or other Stripe-supported payment methods.

## Server

Use `stripe.charge` to require a one-time Stripe payment before returning a response. The method handles Challenge generation, Credential verification, PaymentIntent creation, and Receipt generation.

You can provide either a `client` (a pre-configured Stripe SDK instance) or a raw `secretKey`. Using `client` is recommended — it lets you configure retries, API version, and other options on the Stripe instance you control.

### With Stripe SDK client (recommended)

```ts twoslash

import Stripe from 'stripe'
import { Mppx, stripe } from 'mppx/server'

const stripeClient = new Stripe(process.env.STRIPE_SECRET_KEY!)

const mppx = Mppx.create({
  methods: [
    stripe.charge({
      client: stripeClient, // [!code hl]
      networkId: 'internal',
      paymentMethodTypes: ['card'],
    }),
  ],
})

export async function handler(request: Request) {
  const result = await mppx.charge({
    amount: '1',
    currency: 'usd',
    decimals: 2,
    description: 'Premium API access',
  })(request)

  if (result.status === 402) return result.challenge

  return result.withReceipt(Response.json({ data: '...' }))
}
```

### With secret key

If you don't need to customize the Stripe SDK instance, pass a `secretKey` directly and mppx makes raw API calls to Stripe.

```ts twoslash
import { Mppx, stripe } from 'mppx/server'

const mppx = Mppx.create({
  methods: [
    stripe.charge({
      secretKey: process.env.STRIPE_SECRET_KEY!, // [!code hl]
      networkId: 'internal',
      paymentMethodTypes: ['card'],
    }),
  ],
})
```

### With metadata

Include `metadata` in the `stripe.charge` configuration to forward key-value pairs to Stripe. The metadata appears in the Challenge and attaches to the Stripe `PaymentIntent`.

```ts twoslash

import Stripe from 'stripe'
import { Mppx, stripe } from 'mppx/server'

const stripeClient = new Stripe(process.env.STRIPE_SECRET_KEY!)

const mppx = Mppx.create({
  methods: [
    stripe.charge({
      client: stripeClient,
      metadata: { plan: 'pro' }, // [!code hl]
      networkId: 'internal',
      paymentMethodTypes: ['card'],
    }),
  ],
})
```

### With multiple payment method types

Allow multiple payment methods, like cards and Link, by specifying them in `paymentMethodTypes`.

```ts twoslash

import Stripe from 'stripe'
import { Mppx, stripe } from 'mppx/server'

const stripeClient = new Stripe(process.env.STRIPE_SECRET_KEY!)

const mppx = Mppx.create({
  methods: [
    stripe.charge({
      client: stripeClient,
      networkId: 'internal',
      paymentMethodTypes: ['card', 'link'], // [!code hl]
    }),
  ],
})
```

### Payment links

Set `html` on the method to render a Stripe Elements payment form when a browser visits the endpoint.

```ts twoslash

import Stripe from 'stripe'
import { Mppx, stripe } from 'mppx/server'

const stripeClient = new Stripe(process.env.STRIPE_SECRET_KEY!)

const mppx = Mppx.create({
  methods: [
    stripe.charge({
      client: stripeClient,
      html: { // [!code hl]
        createTokenUrl: '/api/create-spt', // [!code hl]
        publishableKey: process.env.STRIPE_PUBLISHABLE_KEY!, // [!code hl]
      }, // [!code hl]
      networkId: 'internal',
      paymentMethodTypes: ['card'],
    }),
  ],
})
```

### html.createTokenUrl

* **Type:** `string`

A same-origin URL on your server that accepts a `POST` with `{ paymentMethod, amount, currency, expiresAt }` and returns `{ spt: string }`. This is the same endpoint used by the [client-side `createToken` callback](#client).

### html.publishableKey

* **Type:** `string`

Your Stripe publishable key (`pk_live_...` or `pk_test_...`), embedded in the payment page for Stripe.js initialization.

Programmatic clients with `Authorization` headers are unaffected.

See the [payment links guide](/guides/payment-links) for a full walkthrough and live demo.

### Server parameters

| Parameter | Type | Required | Description |
| --- | --- | --- | --- |
| `client` | `StripeClient` | One of `client` or `secretKey` | Pre-configured Stripe SDK instance (`new Stripe(...)`) |
| `secretKey` | `string` | One of `client` or `secretKey` | Stripe secret API key (mppx makes raw API calls) |
| `metadata` | `Record<string, string>` | Optional | Key-value pairs forwarded to Stripe |
| `networkId` | `string` | Required | Stripe [Business Network](https://docs.stripe.com/get-started/account/profile) profile ID |
| `paymentMethodTypes` | `string[]` | Required | Allowed Stripe payment method types |

## Client

::::tip\[For agents]
The [Link CLI](/tools/wallet#link-cli) handles `stripe.charge` end-to-end—`link-cli mpp pay` parses the 402 Challenge, creates an SPT from the user's Link wallet, and retries the request with the Credential. No code required.
::::

Use `stripe` with `Mppx.create` to automatically handle `402` responses. The client parses the Challenge, creates an SPT through the `createToken` callback, and retries with the Credential.

SPT creation requires a Stripe secret key, so the client accepts a `createToken` callback that proxies through a server endpoint. You can optionally pass a `client` (a Stripe.js instance from `@stripe/stripe-js`) which is forwarded to the `createToken` callback for use with Elements.

### Simple (known payment method)

If you already have a payment method ID (for example a test card or a stored method), pass it as `paymentMethod` and mppx handles the full 402 → SPT → retry flow automatically.

```ts twoslash

import { loadStripe } from '@stripe/stripe-js'
import { Mppx, stripe } from 'mppx/client'

const stripeJs = (await loadStripe('pk_test_...'))!

Mppx.create({
  methods: [
    stripe({
      client: stripeJs,
      createToken: async (params) => {
        const res = await fetch('/api/create-spt', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(params),
        })
        if (!res.ok) throw new Error('Failed to create SPT')
        return (await res.json()).spt
      },
      paymentMethod: 'pm_card_visa', // [!code hl]
    }),
  ],
})

// fetch() now handles 402 → credential → retry automatically
const response = await fetch('https://api.example.com/resource')
// @log: Response { status: 200, ... }
```

### With Stripe Elements

For interactive payment collection, use `onChallenge` to render Stripe Elements when a 402 is received. The user enters card details, you create a payment method, then pass it to `createCredential`.

```ts twoslash

import { loadStripe } from '@stripe/stripe-js'
import { Receipt } from 'mppx'
import { Mppx, stripe } from 'mppx/client'

const stripeJs = (await loadStripe('pk_test_...'))!

const mppx = Mppx.create({
  methods: [
    stripe.charge({
      client: stripeJs,
      createToken: async ({ amount, currency, expiresAt, metadata, networkId, paymentMethod }) => {
        const response = await fetch('/api/create-spt', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ paymentMethod, amount, currency, networkId, expiresAt, metadata }),
        })
        if (!response.ok) throw new Error('Failed to create SPT')
        return (await response.json()).spt
      },
    }),
  ],
  onChallenge: async (challenge, { createCredential }) => {
    // Extract payment method types from the challenge
    const methodDetails = challenge.request.methodDetails as
      | { paymentMethodTypes?: string[] }
      | undefined
    const paymentMethodTypes = methodDetails?.paymentMethodTypes ?? ['card']

    // Create Stripe Elements for payment collection
    const elements = stripeJs.elements({
      mode: 'payment',
      amount: Number(challenge.request.amount),
      currency: challenge.request.currency as string,
      paymentMethodTypes,
      paymentMethodCreation: 'manual',
    })

    // Mount the payment element (you'd mount this to a DOM container)
    const paymentElement = elements.create('payment')
    paymentElement.mount('#payment-element')

    // After user submits the form:
    await elements.submit()
    const { paymentMethod } = await stripeJs.createPaymentMethod({ elements })

    // Create credential with the collected payment method
    return createCredential({ paymentMethod: paymentMethod!.id })
  },
  polyfill: false,
})

const response = await mppx.fetch('/api/resource')
const receipt = Receipt.fromResponse(response)
```

## SPT creation proxy endpoint

The `createToken` callback proxies through your own server because SPT creation requires a Stripe secret key.

:::warning\[Security: server-side authorization]
The server **must** derive SPT parameters (amount, currency, expiry, limits) itself rather than accepting them from the client. A thin proxy that forwards client-supplied parameters effectively delegates payment authorization to an untrusted client.

Send only:

* An authenticated session (cookie or bearer token)
* A server-known resource identifier (for example, `orderId`, `quoteId`, `toolCallId`)

The server then looks up the approved amount, currency, recipient, expiry, and rate/spend limits from its own records.
:::

```ts
// Example: server derives all SPT parameters from a known order
export async function POST(request: Request) {
  // 1. Authenticate the caller (session cookie, bearer token, etc.)
  const session = await getSession(request)
  if (!session) return Response.json({ error: 'Unauthorized' }, { status: 401 })

  // 2. Accept only a server-known resource identifier from the client
  const { orderId, paymentMethod } = await request.json()

  // 3. Look up the authorized payment parameters server-side
  const order = await db.orders.get(orderId)
  if (!order) return Response.json({ error: 'Order not found' }, { status: 404 })
  if (order.userId !== session.userId)
    return Response.json({ error: 'Forbidden' }, { status: 403 })

  // 4. Server derives SPT parameters — the client never specifies amount/currency/expiry
  const body = new URLSearchParams({
    payment_method: paymentMethod,
    'usage_limits[currency]': order.currency,
    'usage_limits[max_amount]': order.amount.toString(),
    'usage_limits[expires_at]': Math.floor(
      (Date.now() + 5 * 60 * 1000) / 1000,
    ).toString(),
  })

  const response = await fetch(
    'https://api.stripe.com/v1/test_helpers/shared_payment/granted_tokens',
    {
      method: 'POST',
      headers: {
        Authorization: `Basic ${btoa(`${process.env.STRIPE_SECRET_KEY}:`)}`,
        'Content-Type': 'application/x-www-form-urlencoded',
      },
      body,
    },
  )

  if (!response.ok) {
    const error = await response.json()
    return Response.json({ error: error.error.message }, { status: 400 })
  }

  const { id: spt } = await response.json()
  return Response.json({ spt })
}
```

:::info
The `test_helpers/shared_payment/granted_tokens` endpoint is for testing. In production, SPTs are created through the agent-side `issued_tokens` API.
:::

### Client parameters

| Parameter | Type | Required | Description |
| --- | --- | --- | --- |
| `client` | `StripeJs` | Optional | Stripe.js instance from `@stripe/stripe-js` — forwarded to `createToken` for use with Elements |
| `createToken` | `(params) => Promise<string>` | Required | Callback to create an SPT (proxied through a server endpoint) |
| `externalId` | `string` | Optional | Client reference ID included in the Credential payload |
| `paymentMethod` | `string` | Optional | Default Stripe payment method ID (overridden by `context.paymentMethod`) |

### `createToken` callback parameters

The `createToken` callback receives a single object with the following fields:

| Field | Type | Description |
| --- | --- | --- |
| `amount` | `string` | Payment amount in smallest currency unit |
| `challenge` | `Challenge` | The parsed Challenge from the server |
| `client` | `StripeJs \| undefined` | Stripe.js instance, if provided to `stripe.charge()` |
| `currency` | `string` | Three-letter ISO currency code |
| `expiresAt` | `number` | SPT expiration as a Unix timestamp (seconds) |
| `metadata` | `Record<string, string>` | Optional metadata from the Challenge |
| `networkId` | `string \| undefined` | Stripe Business Network profile ID |
| `paymentMethod` | `string \| undefined` | Stripe payment method ID |

## Request fields

The Challenge request includes the base charge fields plus Stripe method details.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `amount` | `string` | Required | Amount in the smallest currency unit |
| `currency` | `string` | Required | ISO currency code |
| `decimals` | `number` | Required | Number of decimal places in the amount (for example, `2` for cents) |
| `description` | `string` | Optional | Human-readable payment description |
| `expires` | `string` | Optional | ISO 8601 expiration timestamp (defaults to 5 minutes) |
| `externalId` | `string` | Optional | Merchant reference ID |
| `methodDetails.metadata` | `Record<string, string>` | Optional | Metadata forwarded to Stripe |
| `methodDetails.networkId` | `string` | Required | Stripe Business Network profile ID |
| `methodDetails.paymentMethodTypes` | `string[]` | Required | Allowed Stripe payment method types |

## Credential payload

The Credential payload contains the SPT and an optional client reference ID.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `externalId` | `string` | Optional | Client reference ID |
| `spt` | `string` | Required | Shared Payment Token ID (starts with `spt_`) |

## Specification

<Cards>
  <SpecCard to="https://paymentauth.org/draft-stripe-charge-00" />
</Cards>
