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

# Tempo subscription \[Recurring billing]

The `subscription` intent enables recurring stablecoin payments on Tempo with reusable access authorization.

Use subscriptions when access has a fixed price per billing period: paid plans, premium API tiers, recurring MCP tool access, and usage bundles that renew on a schedule.

## Why subscriptions matter

Charges work well for one-time purchases. Sessions work well when usage changes inside a request. Subscriptions cover the third common pattern: the client authorizes recurring access once, and the server bills each period without asking the client to sign every request.

A Tempo subscription uses a key authorization. The client authorizes a scoped access key to transfer a fixed amount of a specific TIP-20 token to a specific recipient once per period until `subscriptionExpires`. The server stores the active subscription record and returns Receipts on later requests without another Credential while the current period is paid.

## Choosing a payment method

| | **Charge** | **Session** | **Subscription** <Badge variant="info">New</Badge> |
|---|---|---|---|
| **Pattern** | One-time payment | Pay-as-you-go usage | Recurring access |
| **Client action** | Sign each paid transfer | Open channel and sign vouchers | Authorize an access key once |
| **Server hot path** | Verify and broadcast transfer | Verify voucher signatures | Resolve active subscription |
| **Best for** | Single API calls and purchases | LLM tokens, bytes, streamed usage | Plans, recurring API access, memberships |
| **Renewal** | None | Top up channel as needed | Bill each day or week |

## Flow

<MermaidDiagram
  chart={`sequenceDiagram
  participant Client
  participant Server
  participant Tempo
  Client->>Server: Protected request
  Server-->>Client: 402 subscription Challenge
  Note over Client: Authorize access key
  Client->>Server: Retry with Credential
  Server->>Tempo: Transfer first period
  Tempo-->>Server: Transaction hash
  Server-->>Client: 200 OK + Receipt
  Client->>Server: Later request
  Server-->>Client: 200 OK + Receipt
`}
/>

## Activation

The first request activates the subscription. The server resolves the request to a stable lookup key, such as `user:123:plan:pro`, and includes an access key in the Challenge. The client signs a `keyAuthorization` Credential that binds:

* `amount`
* `currency`
* `periodCount`
* `periodUnit`
* `recipient`
* `subscriptionExpires`
* `accessKey`

The server verifies the Credential, charges the first period, stores a `SubscriptionRecord`, and returns a Receipt with a `subscriptionId`.

## Access reuse

After activation, future requests do not need another Credential while the subscription is active and current. The server calls `resolve`, finds the subscription record for the route or user, validates that it still matches the request terms, and returns a Receipt.

<MermaidDiagram
  chart={`sequenceDiagram
  participant Client
  participant Server
  participant Store
  Client->>Server: Request protected resource
  Server->>Store: Lookup active subscription by resolved key
  Store-->>Server: SubscriptionRecord
  Server->>Server: Check expiry, request binding, paid period
  Server-->>Client: 200 OK + Receipt
`}
/>

## Renewals

When the next billing period starts, the server renews the subscription before granting access. The SDK uses an atomic store lock so concurrent requests do not charge the same period twice. If one request is already renewing, another request receives `409` with `Retry-After: 1`.

<MermaidDiagram
  chart={`sequenceDiagram
  participant RequestA
  participant RequestB
  participant Store
  participant Tempo
  RequestA->>Store: Lock renewal period
  RequestB->>Store: Try same renewal
  Store-->>RequestB: In flight
  RequestA->>Tempo: Transfer period payment
  Tempo-->>RequestA: Transaction hash
  RequestA->>Store: Commit renewed record
  RequestA-->>RequestA: 200 OK + Receipt
  RequestB-->>RequestB: 409 Retry-After
`}
/>

You can renew in the request path with `renew`, or run renewals from a background worker with [`tempo.renewSubscription`](/sdk/typescript/server/Method.tempo.renewSubscription).

## Cancellation

Cancel a Tempo subscription by marking its stored `SubscriptionRecord` with `canceledAt`. `mppx` treats records with `canceledAt` or `revokedAt` as inactive, so later protected requests return a new `402` Challenge instead of reusing or renewing the old subscription.

The recommended client flow is to call your cancellation endpoint first, then optionally revoke the Tempo access key as a backstop. Server cancellation controls product access. Access-key revocation blocks future on-chain renewal attempts, but it doesn't update the merchant's stored subscription record by itself.

```ts twoslash
import { Store } from 'mppx/server'
import { Subscription } from 'mppx/tempo'

const store = Store.memory()
const subscriptions = Subscription.fromStore(store)

export async function cancelSubscription(userId: string) {
  const subscription = await subscriptions.getByKey(`user:${userId}:plan:pro`)
  if (!subscription) return false

  await subscriptions.put({
    ...subscription,
    canceledAt: new Date().toISOString(),
  })

  return true
}
```

Keep the canceled record for audit and reconciliation. If the client subscribes again, activation creates a new `subscriptionId` for the same resolved lookup key.

On Tempo, clients that know the authorized access key can revoke it from the payer account:

```ts twoslash
import { createClient, http } from 'viem'
import { tempo } from 'viem/chains'
import { privateKeyToAccount } from 'viem/accounts'
import { Actions } from 'viem/tempo'

const client = createClient({
  account: privateKeyToAccount(
    '0x0000000000000000000000000000000000000000000000000000000000000001', // your account
  ),
  chain: tempo,
  transport: http(),
})

await Actions.accessKey.revokeSync(client, {
  accessKey: '0x70997970C51812dc3A010C7d01b50e0d17dc79C8',
})
```

## Receipts

Subscription Receipts confirm activation or renewal. The `reference` field is the Tempo transaction hash for the period payment.

| Field | Description |
|---|---|
| `externalId` | Optional app-defined reference |
| `method` | Always `"tempo"` |
| `reference` | Tempo transaction hash |
| `status` | Always `"success"` |
| `subscriptionId` | Server-issued subscription identifier |
| `timestamp` | Receipt timestamp |

## Integration

### Server

Register `tempo.subscription()` explicitly. The `tempo.common()` helper registers charge and session intents, but it doesn't register subscriptions.

```ts twoslash
import { Mppx, Store, tempo } from 'mppx/server'

const store = Store.memory()

const mppx = Mppx.create({
  methods: [
    tempo.subscription({
      amount: '1.00',
      currency: '0x20c0000000000000000000000000000000000000',
      periodCount: '1',
      periodUnit: 'week',
      recipient: '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266',
      resolve: async ({ input }) => {
        const userId = input.headers.get('X-User-Id')
        return userId ? { key: `user:${userId}:plan:pro` } : null
      },
      store,
      subscriptionExpires: new Date('2027-01-01T00:00:00.000Z'),
    }),
  ],
})

export async function handler(request: Request) {
  const result = await mppx.tempo.subscription({})(request)

  if (result.status === 402) return result.challenge

  const response = result.withReceipt(Response.json({ plan: 'pro' }))
  console.log(response.status)
  // @log: 200
  return response
}
```

:::warning
Use a durable atomic store such as Redis, Upstash, or Cloudflare KV for production. `Store.memory()` is for local development.
:::

### Client

Register `tempo.subscription()` on the client. The SDK signs the access-key authorization and retries the request after the server returns a subscription Challenge.

<Tabs stateKey="account-source">
  <Tab title="Accounts SDK">
    ```ts twoslash
    import { Mppx, tempo } from 'mppx/client'
    import { Provider } from 'accounts'

    const provider = Provider.create({ mpp: false }) // Avoid double 402 handling; mppx is configured below.
    await provider.request({ method: 'wallet_connect' })

    Mppx.create({
      methods: [tempo.subscription({
          account: provider.getAccount({ signable: true }),
          getClient: provider.getClient,
        })],
    })

    const response = await fetch('https://api.example.com/pro')
    console.log(response.status)
    // @log: 200
    ```
  </Tab>

  <Tab title="viem">
    ```ts twoslash
    import { Mppx, tempo } from 'mppx/client'
    import { privateKeyToAccount } from 'viem/accounts'

    const account = privateKeyToAccount('0xabc…123')

    Mppx.create({
      methods: [tempo.subscription({ account })],
    })

    const response = await fetch('https://api.example.com/pro')
    console.log(response.status)
    // @log: 200
    ```
  </Tab>
</Tabs>

## Advanced options

### Custom activation

Pass `activate` when your application owns settlement and record creation. The SDK still verifies the `keyAuthorization` Credential and validates the returned Receipt and subscription record.

### Custom access keys

Pass `accessKey` or return `accessKey` from `resolve` when you want to use an existing access key. Omit it for the recommended path: the server generates and stores one access key per resolved subscription key.

### Background renewal

Use [`tempo.renewSubscription`](/sdk/typescript/server/Method.tempo.renewSubscription) from a cron job when you want billing to happen before the next user request.

## Related

<Cards>
  <Card title="Build a subscription-gated API" description="Add recurring access to an API route" to="/guides/subscription-payments" />

  <Card title="Subscription intent" description="Understand the method-agnostic recurring payment intent" to="/intents/subscription" />

  <Card title="Server API reference" description="Configure activation, reuse, and renewal" to="/sdk/typescript/server/Method.tempo.subscription" />

  <Card title="Client API reference" description="Sign subscription key authorizations" to="/sdk/typescript/client/Method.tempo.subscription" />
</Cards>
