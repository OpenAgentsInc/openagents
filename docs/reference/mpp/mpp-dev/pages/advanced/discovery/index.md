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

# Discovery \[Let clients automatically discover your API's pricing]

## Overview

MPP's discovery system lets clients and agents learn what your endpoints cost before making a request. You serve a standard [OpenAPI 3.1](https://spec.openapis.org/oas/v3.1.0) document at `/openapi.json` with `x-payment-info` extensions that advertise one or more payment offers for each paid operation. Registries aggregate these documents so agents can find paid APIs automatically, and provide value-added services like reputation, search, and analytics.

<EcosystemDiagram />

:::info\[Discovery is advisory]
Discovery documents are informational hints. The runtime `402` Challenge remains the authoritative source of payment terms. Clients use discovery for display and planning, but defer to the Challenge for actual payment.
:::

## Registries

Registries aggregate discovery documents from multiple services, making it easy for clients and agents to find paid APIs.

| Registry | Description | How to add |
|----------|-------------|------------|
| [MPPScan](https://mppscan.com) | Public registry of MPP-enabled services with search and analytics | [Manually register](https://www.mppscan.com/register) in one click |
| [MPP Services directory](https://mpp.dev/services) | Curated list of live services on mpp.dev | [Submit a PR](https://github.com/tempoxyz/mpp) to add your service |

Agents can query the curated services directory over MCP at
`https://mpp.dev/mcp/services`. The server is read-only and exposes tools to
list services, rank services for an agent task, inspect endpoint offers, get
usage recipes, look up services by payment recipient, inspect available filters,
and fetch advisory OpenAPI summaries.

### Services MCP

The services MCP server is the agent-facing discovery surface for the curated
MPP directory:

```text
https://mpp.dev/mcp/services
```

Use it when an agent needs to:

* rank paid APIs by task, category, integration, or payment method
* turn a selected service into a usage recipe with endpoint candidates
* compare endpoint-level payment offers before constructing a request
* identify which services publish offers for a payment recipient from a `402`
  Challenge
* inspect catalog facets before narrowing a search
* fetch a live OpenAPI summary or registry-derived endpoint view

```json
{
  "mcpServers": {
    "mpp-services": {
      "url": "https://mpp.dev/mcp/services"
    }
  }
}
```

The MCP server is advisory and read-only. After discovery, clients call the
target service directly and treat the runtime `402` Challenge as authoritative.

For agent setup, MCP Inspector smoke tests, example prompts, and recipes, see
[Discover MPP services on Tempo docs](https://docs.tempo.xyz/guide/machine-payments/discover-services).

## Quick start

The `mppx` SDK generates discovery documents from your route configuration. Add `discovery()` to your server and it serves `/openapi.json` automatically.

```ts [server.ts]
import { Hono } from 'hono'
import { Mppx, discovery } from 'mppx/hono'
import { tempo } from 'mppx/server'

const app = new Hono()

const mppx = Mppx.create({
  methods: [
    tempo.charge({
      currency: '0x20c0000000000000000000000000000000000000',
      recipient: '0x...',
      testnet: true,
    }),
  ],
  secretKey: process.env.MPP_SECRET_KEY,
})

app.get('/v1/fortune', mppx.charge({ amount: '0.01' }), (c) => c.json({ fortune: 'You will be rich' }))

// [!code hl:start]
discovery(app, mppx, {
  auto: true,
  info: { title: 'Fortune API', version: '1.0.0' },
})
// [!code hl:end]
```

This generates a `GET /openapi.json` endpoint with canonical `x-payment-info.offers[]` entries on each paid route.

### Express

```ts [server.ts]
import express from 'express'
import { Mppx, discovery } from 'mppx/express'
import { tempo } from 'mppx/server'

const app = express()

const mppx = Mppx.create({
  methods: [
    tempo.charge({
      currency: '0x20c0000000000000000000000000000000000000',
      recipient: '0x...',
      testnet: true,
    }),
  ],
  secretKey: process.env.MPP_SECRET_KEY,
})

const pay = mppx.charge({ amount: '0.01' })
app.get('/v1/fortune', pay, (req, res) => res.json({ fortune: 'You will be rich' }))

// [!code hl:start]
discovery(app, mppx, {
  info: { title: 'Fortune API', version: '1.0.0' },
  routes: [{ handler: pay, method: 'get', path: '/v1/fortune' }],
})
// [!code hl:end]
```

### Next.js

In Next.js, `discovery()` returns a route handler you export from an API route.

```ts [app/openapi.json/route.ts]
import { discovery } from 'mppx/nextjs'
import { mppx, pay } from '../fortune/route'

// [!code hl:start]
export const GET = discovery(mppx, {
  info: { title: 'Fortune API', version: '1.0.0' },
  routes: [{ handler: pay, method: 'get', path: '/api/fortune' }],
})
// [!code hl:end]
```

## How it works

Your server exposes a `GET /openapi.json` endpoint that returns an OpenAPI document. Paid operations include an `x-payment-info` extension with one or more payment offers, and the document root can include `x-service-info` for service-level metadata.

```json [/openapi.json]
{
  "openapi": "3.1.0",
  "info": { "title": "My API", "version": "1.0.0" },
  "x-service-info": {
    "categories": ["ai"],
    "docs": {
      "homepage": "https://example.com",
      "apiReference": "https://example.com/docs",
      "llms": "/llms.txt"
    }
  },
  "paths": {
    "/v1/generate": {
      "post": {
        // [!code hl:start]
        "x-payment-info": {
          "offers": [
            {
              "amount": "1000000",
              "currency": "0x20c0000000000000000000000000000000000001",
              "description": "Generate text with Tempo",
              "intent": "charge",
              "method": "tempo"
            },
            {
              "amount": "100",
              "currency": "usd",
              "description": "Generate text with Stripe",
              "intent": "charge",
              "method": "stripe"
            }
          ]
        },
        // [!code hl:end]
        "responses": {
          "200": { "description": "Successful response" },
          "402": { "description": "Payment Required" }
        }
      }
    },
    "/v1/models": {
      "get": {
        "responses": {
          "200": { "description": "Successful response" }
        }
      }
    }
  }
}
```

### `x-payment-info`

Add this extension to any operation that requires payment. Prefer the canonical multi-offer shape:

| Field | Type | Description |
|-------|------|-------------|
| `offers` | `Offer[]` | Ordered list of payment offers the client can choose from |

### `offers[]`

| Field | Type | Description |
|-------|------|-------------|
| `amount` | `string \| null` | Payment amount in base units |
| `currency` | `string` | Currency code or token address |
| `description` | `string` | Human-readable description of the charge |
| `intent` | `string` | Payment intent (`charge` or `session`) |
| `method` | `string` | Payment method identifier (`tempo`, `stripe`) |

:::note\[Compatibility shorthand]
The flat single-offer form is still valid as shorthand for older emitters, but use it only for backward compatibility. New documents write the same data under `offers[]`.

```json [openapi.json]
{
  // [!code hl:start]
  "x-payment-info": {
    "amount": "1000000",
    "currency": "0x20c0000000000000000000000000000000000001",
    "description": "Generate text with Tempo",
    "intent": "charge",
    "method": "tempo"
  }
  // [!code hl:end]
}
```

:::

### `x-service-info`

Optional root-level metadata about the service:

| Field | Type | Description |
|-------|------|-------------|
| `categories` | `string[]` | Free-form service categories (for example, `ai`, `payments`) |
| `docs.homepage` | `string` | Link to the service homepage |
| `docs.apiReference` | `string` | Link to API documentation |
| `docs.llms` | `string` | Link to an `llms.txt` file for AI consumption |

## Build manually

You can author a discovery document by hand following the [discovery specification](https://paymentauth.org/draft-payment-discovery-00.html). The document is a standard OpenAPI 3.1 file with two extensions:

::::steps

### Create the OpenAPI skeleton

Start with a standard OpenAPI 3.1 document:

```json [openapi.json]
{
  "openapi": "3.1.0",
  "info": {
    "title": "My API",
    "version": "1.0.0"
  },
  "paths": {}
}
```

### Add `x-payment-info` to paid operations

For each endpoint that requires payment, add the `x-payment-info` extension with an `offers` array. Add more objects to `offers[]` when the client can choose between alternative payment methods or currencies. Amounts are in base units (for example, `1000000` for $1.00 with 6 decimals).

```json [openapi.json]
{
  "paths": {
    "/v1/generate": {
      "post": {
        "summary": "Generate text",
        // [!code hl:start]
        "x-payment-info": {
          "offers": [
            {
              "amount": "1000000",
              "currency": "0x20c0000000000000000000000000000000000001",
              "intent": "charge",
              "method": "tempo"
            }
          ]
        },
        // [!code hl:end]
        "responses": {
          "200": { "description": "Successful response" },
          "402": { "description": "Payment Required" }
        }
      }
    }
  }
}
```

:::warning\[Include a 402 response]
Operations with `x-payment-info` must include a `402` response in the `responses` object. Validators flag this as an error if missing.
:::

### Add `x-service-info` (optional)

Add service-level metadata to the document root:

```json [openapi.json]
{
  // [!code hl:start]
  "x-service-info": {
    "categories": ["ai", "text-generation"],
    "docs": {
      "homepage": "https://example.com",
      "apiReference": "https://example.com/docs/api",
      "llms": "https://example.com/llms.txt"
    }
  }
  // [!code hl:end]
}
```

### Serve at `/openapi.json`

Serve the document at `GET /openapi.json` with appropriate caching:

```http
HTTP/1.1 200 OK
Content-Type: application/json
Cache-Control: public, max-age=300
```

::::

## CLI

Generate a static discovery document from a config module:

```bash [terminal]
$ npx mppx discover generate ./discovery.config.ts
```

Validate an existing discovery document from a file or URL:

```bash [terminal]
$ npx mppx discover validate https://example.com/openapi.json
```

## Validation

Common validation issues:

| Issue | Severity | Description |
|-------|----------|-------------|
| Missing `402` response | Error | Operations with `x-payment-info` must include a `402` response |
| Invalid amount format | Error | Each `offers[].amount` value must be a non-negative integer string |
| Missing `requestBody` | Warning | `POST`/`PUT`/`PATCH` operations without a `requestBody` definition |
| Invalid URI in docs | Error | `docs` links must be valid URIs or absolute paths |

## Specification

<Cards>
  <SpecCard to="https://paymentauth.org/draft-payment-discovery-00" />
</Cards>
