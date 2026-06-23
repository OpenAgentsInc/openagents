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

# Sessions \[Low-cost high-throughput payments]

The `session` intent enables high-frequency, pay-as-you-go payments over unidirectional payment channels. Sessions use the [TIP-1034](https://tips.sh/1034-1) precompile for low cost and high reliability. Clients deposit funds into a channel reserve and sign off-chain vouchers as they consume resources. The server verifies vouchers with fast signature checks—no RPC or blockchain calls—and settles periodically in batches.

Payment sessions reduce payment verification to near constant time, making it possible to meter and bill at the granularity of individual LLM tokens, API calls, or bytes transferred.

:::warning\[Legacy integrations]
`tempo.session` is the current Sessions implementation in `mppx`. The previous contract-backed implementation is Legacy Sessions, also called Sessions v1, and is available as `tempo.sessionLegacy`.
:::

## Which client API should I use?

There are two current Sessions client APIs:

| API | Use it when |
|---|---|
| `tempo({ account, maxDeposit })` | You want a fetch wrapper that handles both one-time charges and Sessions. This expands to `tempo.charge()` plus the current `tempo.session()` client method. |
| `tempo.session({ account, maxDeposit })` | You want to register only the current Sessions client method in `Mppx.create`. |
| `tempo.session.manager({ account, maxDeposit })` | You want direct lifecycle control with `.fetch()`, `.topUp()`, `.close()`, `.sse()`, or `.ws()`. Use this when your code must explicitly close or top up a channel. |
| `tempo.sessionLegacy` / `tempo.sessionLegacy.method()` | You still need compatibility with contract-backed Sessions v1. Do not use this for new integrations. |

For browser reloads or app restarts, pass a [`sessionStore`](/sdk/typescript/client/Method.tempo.session-manager#sessionstore) to `tempo.session.manager()`. Servers can pair this with [`bootstrap: true`](/sdk/typescript/server/Method.tempo.session#with-same-route-bootstrap) so clients lazily recover a previous channel from the same protected route before opening a new one.

## Why Sessions matter in MPP

Traditional payment rails target human purchase flows: a buyer decides, pays, and receives goods. Usage-based billing—the model that powers cloud infrastructure, LLM APIs, and metered services—requires something fundamentally different. It needs payment verification that can keep pace with the service itself.

Consider an LLM API: a single inference request can generate hundreds of tokens over several seconds. Each token has a known cost, but the total cost isn't known when the request begins. Standard billing models handle this by accumulating usage and charging after the fact, introducing credit risk, reconciliation complexity, and billing disputes. Prepaid credit systems require the client to guess consumption upfront and lose unused funds.

Sessions solve this by making payment a continuous, inline part of the HTTP request. The client signs a cumulative voucher for each increment of service consumed, and the server verifies it in microseconds. The server delays on-chain settlement to whenever it chooses, batching hundreds or thousands of vouchers into a single on-chain transaction. This reduces both the latency and the cost of payment verification to near zero.

## How it works

### Overview

<MermaidDiagram
  chart={`sequenceDiagram
  participant Client
  participant Server
  participant Tempo
  Client->>Tempo: (1) Deposit tokens
  Tempo-->>Client: Session created
  Client->>Server: (2) Open credential
  Note over Server: verify deposit
  Server-->>Client: 200 OK (session established)
  loop Per request
      Client->>Server: (3) Request + voucher
      Note over Server: recover signature
      Server-->>Client: 200 OK + Receipt
  end
  Note over Server: (4) Periodic settlement
  Server->>Tempo: settle(channelId, voucher)
  Client->>Server: (5) Close
  Server->>Tempo: close(channelId, voucher)
  Tempo-->>Client: Refund remaining deposit
`}
/>

A payment session has four phases:

::::steps

### Open

The client deposits funds into a channel reserve through the [TIP-1034 precompile](https://tips.sh/1034-1), creating a payment channel between the client (payer) and server (payee). A unique `channelId` identifies the channel and tracks the deposited stablecoins.

### Session

The client signs vouchers with increasing cumulative amounts as service is consumed. Each voucher authorizes "I have now consumed up to X total." The server verifies the signature, checks that the cumulative amount is higher than the previous voucher, and grants access based on the delta.

### Top up

If the channel runs low on funds, the client tops up the channel without closing it. The session continues uninterrupted.

### Close

Either party can close the channel. The server closes the precompile-backed channel with the highest voucher, settling the final balance on-chain and refunding any unused deposit to the client.

::::

## Session receipts

Session Receipts differ from charge Receipts. The `reference` field contains the payment channel ID (a `bytes32` hash), not a transaction hash. The on-chain settlement transaction hash is only available after closing the channel.

```ts
type SessionReceipt = {
  acceptedCumulative: string
  challengeId: string
  channelId: `0x${string}`
  intent: 'session'
  method: 'tempo'
  reference: string
  spent: string
  status: 'success'
  timestamp: string
  txHash?: `0x${string}`
  units?: number
}
```

| Field | Charge receipt | Session receipt |
|-------|---------------|-----------------|
| `reference` | Transaction hash | Channel ID |
| `status` | `"success"` | `"success"` |
| `method` | `"tempo"` | `"tempo"` |

To get the settlement transaction hash, close the channel via `session.close()` and read the `txHash` field from the returned receipt.

## Settlement

Sessions separate payment verification from on-chain settlement. During a request or stream, the client sends cumulative vouchers and the server records the highest valid voucher it has accepted. Settlement submits that highest voucher to the TIP-1034 precompile, updates the on-chain paid amount, and keeps the channel open for more usage unless the channel is closed.

### Automatic settlement

Use automatic settlement when the server should periodically settle accepted usage while a session remains active. The `settlementSchedule` is server-owned and can trigger by spend amount, metered units, or elapsed time. Clients don't receive the schedule and can't change it.

Automatic settlement is the default operational model for high-volume APIs: the hot path stays off-chain, while the server settles in the background as usage accumulates.

```ts twoslash
import { Mppx, Store, tempo } from 'mppx/server'
import { privateKeyToAccount } from 'viem/accounts'

const account = privateKeyToAccount('0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef')

const mppx = Mppx.create({
  methods: [
    tempo.session({
      account,
      chainId: 4217, // optional; pins Challenges to Tempo mainnet
      currency: '0x20c0000000000000000000000000000000000000', // pathUSD on Tempo
      settlementSchedule: { // [!code hl]
        amount: '10',
        intervalMs: 60_000,
        units: 10_000,
      },
      store: Store.memory(),
    }),
  ],
})
```

Any threshold can trigger settlement. Use `amount` to settle after a token-denominated spend threshold, `units` to settle after metered usage, and `intervalMs` to settle after elapsed time since the previous scheduled settlement.

### Manual settlement

Use manual settlement from an admin workflow, job queue, or close-out process when you want explicit control over timing. `tempo.session.settle` settles one channel by submitting its highest accepted voucher. `tempo.session.settleBatch` repeats that operation for a list of channel IDs.

Manual settlement is useful for end-of-period reconciliation, draining channels before maintenance, or forcing settlement after detecting unusual channel activity.

```ts twoslash
import { Store, tempo } from 'mppx/server'
import { createWalletClient, http } from 'viem'
import { privateKeyToAccount } from 'viem/accounts'
import { tempo as tempoMainnet } from 'viem/chains'

const account = privateKeyToAccount('0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef')
const store = Store.memory()

const client = createWalletClient({
  account,
  chain: tempoMainnet,
  transport: http('https://rpc.tempo.xyz'),
})

const channelId = '0x0000000000000000000000000000000000000000000000000000000000000000'
const txHash = await tempo.session.settle(store, client, channelId, { account }) // [!code hl]
console.log(txHash)
// @log: 0x...
```

Settle multiple channels from the same job with `tempo.session.settleBatch`.

```ts twoslash
import { Store, tempo } from 'mppx/server'
import { createWalletClient, http } from 'viem'
import { privateKeyToAccount } from 'viem/accounts'
import { tempo as tempoMainnet } from 'viem/chains'

const account = privateKeyToAccount('0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef')
const store = Store.memory()

const client = createWalletClient({
  account,
  chain: tempoMainnet,
  transport: http('https://rpc.tempo.xyz'),
})

const channelIds = [
  '0x0000000000000000000000000000000000000000000000000000000000000000',
  '0x1111111111111111111111111111111111111111111111111111111111111111',
] as const

const txHashes = await tempo.session.settleBatch(store, client, channelIds, { account }) // [!code hl]
console.log(txHashes)
// @log: ['0x...', '0x...']
```

## High volume API billing

Sessions match the billing model that high-volume APIs need: pay stablecoin tokens, receive API responses. The granularity of payment matches the granularity of consumption.

A typical flow for a high-volume large language model API:

1. **Client:** opens a channel with a 10 USDC deposit
2. **Client:** sends a prompt to the API
3. **Server:** issues Challenges requesting payment for each chunk (for example, 0.000025 USDC per token)
4. **Client:** signs a voucher for each chunk—the cumulative amount increases by the cost of tokens received
5. **Server:** verifies the voucher signature (~microseconds) and sends the next chunk
6. **Server:** settles on-chain and the client gets the unused deposit back

The server never touches the chain during inference. Payment verification adds microseconds of CPU overhead per chunk, not hundreds of milliseconds of network latency.

:::info\[Why Tempo]
Tempo handles payments at scale and has properties that make it a uniquely good fit for payment sessions:

* **Channel management UX**—Opening, topping up, and closing channels are on-chain operations. Tempo's ~500ms finality and sub-cent fees keep channel lifecycle from becoming a UX bottleneck.
* **Payment lane**—Tempo's 2D nonce system provides dedicated nonce lanes for payment transactions, so channel operations don't block other account activity. This matters for clients that use the same account for payments and other on-chain interactions.
* **High throughput**—When a server settles thousands of channels, Tempo's throughput handles the settlement volume without congestion or fee spikes.
* **Fee sponsorship**—Servers can pay channel management fees on behalf of clients, making the client-side integration purely off-chain after the initial deposit.
* **Enshrined tokens**—TIP-20 tokens are precompile-based, not smart contracts. Token operations are cheaper and more predictable than ERC-20 interactions on other chains.
* **Enshrined Tempo**—[TIP-1034](https://tips.sh/1034-1) makes Sessions a native Tempo precompile at the canonical `0x4D5050…` address, whose prefix spells "MPP". The precompile reduces execution overhead, removes the separate approval flow, and keeps session lifecycle operations in the payment lane under congestion.
  :::

## Integration

<Tabs stateKey="platform">
  <Tab title="Server">
    <div className="space-y-4">
      Use [`tempo.session`](/sdk/typescript/server/Method.tempo.session) to accept Sessions. The server needs an RPC URL for channel open, top-up, settlement, and close operations, plus an atomic store backend for channel state.

      ```ts twoslash
      import { Mppx, Store, tempo } from 'mppx/server'
      import { privateKeyToAccount } from 'viem/accounts'

      const account = privateKeyToAccount('0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef')

      const mppx = Mppx.create({
        methods: [
          tempo.session({
            account, // signs server-side reserve settlement and close transactions
            chainId: 4217, // optional; pins Challenges to Tempo mainnet
            currency: '0x20c0000000000000000000000000000000000000', // pathUSD on Tempo
            store: Store.memory(), // use Redis, Upstash, or Cloudflare for production
          }),
        ],
      })
      ```

      `Store.memory()` works for local development. For multi-instance deployments, use `Store.redis()`, `Store.upstash()`, or `Store.cloudflare()` so channel state is shared across processes.

      Use `mppx.session` in your request handler to meter access:

      ```ts twoslash
      import { Mppx, Store, tempo } from 'mppx/server'
      import { privateKeyToAccount } from 'viem/accounts'

      const account = privateKeyToAccount('0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef')

      const mppx = Mppx.create({
        methods: [
          tempo.session({
            account, // signs server-side reserve settlement and close transactions
            chainId: 4217, // optional; pins Challenges to Tempo mainnet
            currency: '0x20c0000000000000000000000000000000000000', // pathUSD on Tempo
            store: Store.memory(), // use Redis, Upstash, or Cloudflare for production
          }),
        ],
      })
      // ---cut---
      export async function handler(request: Request) {
        const result = await mppx.session({
          amount: '25',
          unitType: 'llm_token',
        })(request)

        if (result.status === 402) return result.challenge

        return result.withReceipt(Response.json({ data: '...' }))
      }
      ```
    </div>
  </Tab>

  <Tab title="Client">
    <div className="space-y-4">
      Use [`tempo`](/sdk/typescript/client/Method.tempo) with `Mppx.create` when the same fetch wrapper should handle one-time charges and Sessions. The `tempo()` helper expands to both `tempo.charge()` and `tempo.session()`, so you don't need to declare Sessions separately.

      <Tabs stateKey="account-source">
        <Tab title="Accounts SDK">
          ```ts twoslash
          import { Mppx, tempo } from 'mppx/client'
          import { Provider } from 'accounts'

          const provider = Provider.create({ mpp: false }) // Avoid double 402 handling; mppx is configured below.
          await provider.request({ method: 'wallet_connect' })

          const { fetch: mppxFetch } = Mppx.create({
            methods: [tempo({
              account: provider.getAccount({ signable: true }),
              getClient: provider.getClient,
              maxDeposit: '1',
            })],
            polyfill: false,
          })

          const response = await mppxFetch('https://api.example.com/v1/chat/completions')
          // Automatically opens the channel reserve and signs vouchers per chunk
          ```
        </Tab>

        <Tab title="viem">
          ```ts twoslash
          import { Mppx, tempo } from 'mppx/client'
          import { privateKeyToAccount } from 'viem/accounts'

          const account = privateKeyToAccount('0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef')

          const { fetch: mppxFetch } = Mppx.create({
            methods: [tempo({ account, maxDeposit: '1' })],
            polyfill: false,
          })

          const response = await mppxFetch('https://api.example.com/v1/chat/completions')
          // Automatically opens the channel reserve and signs vouchers per chunk
          ```
        </Tab>
      </Tabs>

      ### With explicit Sessions

      Register `tempo.session()` when this client should only handle Sessions. Use `tempo.session.manager()` for the standalone lifecycle manager shown in [closing the channel](#closing-the-channel).

      <Tabs stateKey="account-source">
        <Tab title="Accounts SDK">
          ```ts twoslash
          import { Mppx, tempo } from 'mppx/client'
          import { Provider } from 'accounts'

          const provider = Provider.create({ mpp: false }) // Avoid double 402 handling; mppx is configured below.
          await provider.request({ method: 'wallet_connect' })

          const mppx = Mppx.create({
            methods: [
              // [!code hl:start]
              tempo.session({
                account: provider.getAccount({ signable: true }),
                getClient: provider.getClient,
                maxDeposit: '1',
              }),
              // [!code hl:end]
            ],
            polyfill: false,
          })

          const response = await mppx.fetch('https://api.example.com/v1/chat/completions')
          ```
        </Tab>

        <Tab title="viem">
          ```ts twoslash
          import { Mppx, tempo } from 'mppx/client'
          import { privateKeyToAccount } from 'viem/accounts'

          const account = privateKeyToAccount('0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef')

          const mppx = Mppx.create({
            methods: [
              // [!code hl:start]
              tempo.session({
                account,
                maxDeposit: '1',
              }),
              // [!code hl:end]
            ],
            polyfill: false,
          })

          const response = await mppx.fetch('https://api.example.com/v1/chat/completions')
          ```
        </Tab>
      </Tabs>

      ### With multiple methods

      Register multiple methods so the client can handle servers that offer multiple payment methods.

      For example, to accept both charge and payment sessions:

      <Tabs stateKey="account-source">
        <Tab title="Accounts SDK">
          ```ts twoslash
          import { Mppx, tempo } from 'mppx/client'
          import { Provider } from 'accounts'

          const provider = Provider.create({ mpp: false }) // Avoid double 402 handling; mppx is configured below.
          await provider.request({ method: 'wallet_connect' })

          Mppx.create({
            methods: [
              tempo.charge({
                account: provider.getAccount({ signable: true }),
                getClient: provider.getClient,
              }),
              tempo.session({ // [!code hl]
                account: provider.getAccount({ signable: true }), // [!code hl]
                getClient: provider.getClient, // [!code hl]
                maxDeposit: '1', // [!code hl]
              }), // [!code hl]
            ],
          })
          ```
        </Tab>

        <Tab title="viem">
          ```ts twoslash
          import { Mppx, tempo } from 'mppx/client'
          import { privateKeyToAccount } from 'viem/accounts'

          const account = privateKeyToAccount('0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef')

          Mppx.create({
            methods: [
              tempo.charge({ account }),
              tempo.session({ account, maxDeposit: '1' }), // [!code hl]
            ],
          })
          ```
        </Tab>
      </Tabs>

      ### Closing the channel

      Use `tempo.session.manager()` when you want direct lifecycle control. Channels remain open for reuse across requests. Call `session.close()` to settle on-chain and reclaim unspent deposit.

      <Tabs stateKey="account-source">
        <Tab title="Accounts SDK">
          ```ts twoslash
          import { tempo } from 'mppx/client'
          import { Provider } from 'accounts'

          const provider = Provider.create({ mpp: false }) // Avoid double 402 handling; mppx is configured below.
          await provider.request({ method: 'wallet_connect' })

          const session = tempo.session.manager({
            account: provider.getAccount({ signable: true }),
            getClient: provider.getClient,
            maxDeposit: '1',
          })

          const response = await session.fetch('https://api.example.com/v1/chat/completions')
          const receipt = await session.close()
          ```
        </Tab>

        <Tab title="viem">
          ```ts twoslash
          import { tempo } from 'mppx/client'
          import { privateKeyToAccount } from 'viem/accounts'

          const account = privateKeyToAccount('0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef')

          const session = tempo.session.manager({
            account,
            maxDeposit: '1',
          })

          const response = await session.fetch('https://api.example.com/v1/chat/completions')
          const receipt = await session.close()
          ```
        </Tab>
      </Tabs>

      :::warning
      Channels do not close automatically. If you don't call `close()`, the deposit stays reserved until the channel expires, the server closes it, or it is manually closed.
      :::

      See [`tempo.session.manager`](/sdk/typescript/client/Method.tempo.session-manager) for the full session lifecycle API.
    </div>
  </Tab>
</Tabs>

## Migrate from Legacy Sessions

Legacy Sessions, also called Sessions v1, is the contract-backed session flow. Use `tempo.sessionLegacy` only when you need compatibility with clients or servers that haven't moved to the latest implementation.

* Register `tempo.session` on the server for the latest implementation.
* Keep `tempo.sessionLegacy` registered beside `tempo.session` during migration so existing clients keep working.
* Use `tempo()` on the client when the same fetch wrapper should handle charges and Sessions.
* Register `tempo.session()` and `tempo.sessionLegacy.method()` explicitly when the client must support both Sessions implementations.

### Compatibility matrix

| Server methods | Client methods | Result |
|---|---|---|
| `tempo.session()` only | `tempo.session()` or `tempo()` | Current Sessions flow. New integrations should target this. |
| `tempo.sessionLegacy()` only | `tempo.sessionLegacy()` or `tempo.sessionLegacy.method()` | Legacy Sessions v1 flow. Use only until the server migrates. |
| `tempo.session()` only | `tempo.sessionLegacy()` or `tempo.sessionLegacy.method()` | Not compatible. The client cannot answer current Sessions Challenges. |
| `tempo.sessionLegacy()` only | `tempo.session()` or `tempo()` | Not compatible for Sessions. The client cannot answer Legacy Sessions Challenges unless `tempo.sessionLegacy.method()` is also registered. |
| `tempo.session()` and `tempo.sessionLegacy()` | `tempo.session()` and `tempo.sessionLegacy.method()` | Migration mode. Both current and Legacy Sessions Challenges can be handled while clients roll forward. |

Current Sessions Challenges advertise `sessionProtocol: "v2"` in method details and use TIP-1034 reserve channels. Legacy Sessions Challenges use the contract-backed Sessions v1 flow. Channel state is not reusable across implementations; let old channels close or settle under `tempo.sessionLegacy`, and open new channels with `tempo.session`.

### Server

```ts twoslash
import { Mppx, Store, tempo } from 'mppx/server'
import { privateKeyToAccount } from 'viem/accounts'

const account = privateKeyToAccount('0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef')

const mppx = Mppx.create({
  methods: [
    // Keep both registered during migration so current and Legacy Sessions clients work.
    // [!code hl:start]
    tempo.session({
      account,
      chainId: 4217, // optional; pins Challenges to Tempo mainnet
      currency: '0x20c0000000000000000000000000000000000000', // pathUSD on Tempo
      store: Store.memory(),
    }),
    tempo.sessionLegacy({
      account,
      currency: '0x20c0000000000000000000000000000000000000', // pathUSD on Tempo
      store: Store.memory(),
    }),
    // [!code hl:end]
  ],
})
```

### Client

<Tabs stateKey="account-source">
  <Tab title="Accounts SDK">
    ```ts twoslash
    import { Mppx, tempo } from 'mppx/client'
    import { createClient, http } from 'viem'
    import { Provider } from 'accounts'
    import { tempo as tempoMainnet } from 'viem/chains'

    const provider = Provider.create({ mpp: false }) // Avoid double 402 handling; mppx is configured below.
    await provider.request({ method: 'wallet_connect' })

    const { fetch: mppxFetch } = Mppx.create({
      methods: [
        tempo.session({
          account: provider.getAccount({ signable: true }),
          getClient: () =>
            createClient({
              chain: tempoMainnet,
              transport: http('https://rpc.tempo.xyz'),
            }),
          maxDeposit: '1',
        }),
        tempo.sessionLegacy.method({
          account: provider.getAccount({ signable: true }),
          getClient: () =>
            createClient({
              chain: tempoMainnet,
              transport: http('https://rpc.tempo.xyz'),
            }),
          maxDeposit: '1',
        }),
      ],
      polyfill: false,
    })

    const response = await mppxFetch('https://api.example.com/resource')
    ```
  </Tab>

  <Tab title="viem">
    ```ts twoslash
    import { Mppx, tempo } from 'mppx/client'
    import { createClient, http } from 'viem'
    import { privateKeyToAccount } from 'viem/accounts'
    import { tempo as tempoMainnet } from 'viem/chains'

    const account = privateKeyToAccount('0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef')

    const { fetch: mppxFetch } = Mppx.create({
      methods: [
        tempo.session({
          account,
          getClient: () =>
            createClient({
              chain: tempoMainnet,
              transport: http('https://rpc.tempo.xyz'),
            }),
          maxDeposit: '1',
        }),
        tempo.sessionLegacy.method({
          account,
          getClient: () =>
            createClient({
              chain: tempoMainnet,
              transport: http('https://rpc.tempo.xyz'),
            }),
          maxDeposit: '1',
        }),
      ],
      polyfill: false,
    })

    const response = await mppxFetch('https://api.example.com/resource')
    ```
  </Tab>
</Tabs>

## Reserve precompile

Sessions use the [TIP-1034 precompile](https://tips.sh/1034-1) for on-chain deposits, settlement, top-ups, and channel close. The IETF Specification documents the voucher format and HTTP authentication flow.

| Network | Chain ID | Precompile address |
|---|---|---|
| Mainnet | 4217 | [`0x4d50500000000000000000000000000000000000`](https://explore.mainnet.tempo.xyz/address/0x4d50500000000000000000000000000000000000) |
| Testnet (Moderato) | 42431 | [`0x4d50500000000000000000000000000000000000`](https://explore.testnet.tempo.xyz/address/0x4d50500000000000000000000000000000000000) |

## Specification

<Cards>
  <SpecCard to="https://paymentauth.org/draft-tempo-session-00" />
</Cards>
