<picture>
  <source media="(prefers-color-scheme: dark)" srcset=".github/logo-dark.svg">
  <source media="(prefers-color-scheme: light)" srcset=".github/logo-light.svg">
  <img alt="mppx" src=".github/logo-light.svg" width="100%" height="100px">
</picture>

<p></p>

<p align="center"><b>TypeScript SDK for the <a href="https://mpp.dev">Machine Payments Protocol</a></b></p>

<p align="center">
  <a href="https://mpp.dev/sdk/typescript">Documentation</a> · <a href="#install">Install</a> · <a href="#quick-start">Quick Start</a> · <a href="#examples">Examples</a> · <a href="#cli">CLI</a> · <a href="#payments-proxy">Payments Proxy</a> · <a href="https://github.com/tempoxyz/mpp-specs">Protocol</a>
</p>

<p align="center">
  <a href="https://www.npmjs.com/package/mppx">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/npm/v/mppx?colorA=21262d&colorB=21262d&style=flat">
      <img src="https://img.shields.io/npm/v/mppx?colorA=f6f8fa&colorB=f6f8fa&style=flat" alt="Version">
    </picture>
  </a>
  <a href="https://github.com/wevm/mppx/blob/main/LICENSE">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://img.shields.io/npm/l/mppx?colorA=21262d&colorB=21262d&style=flat">
      <img src="https://img.shields.io/npm/l/mppx?colorA=f6f8fa&colorB=f6f8fa&style=flat" alt="MIT License">
    </picture>
  </a>
</p>

---

## Documentation

Full documentation, API reference, and guides are available at **[mpp.dev/sdk/typescript](https://mpp.dev/sdk/typescript)**.

## Install

```bash
npm i mppx
```

## Quick Start

### Server

```ts
import { Mppx, tempo } from 'mppx/server'

const mppx = Mppx.create({
  methods: [
    tempo({
      currency: '0x20c0000000000000000000000000000000000000',
      recipient: '0x742d35Cc6634c0532925a3b844bC9e7595F8fE00',
    }),
  ],
  secretKey: process.env.MPP_SECRET_KEY!,
})

export async function handler(request: Request) {
  const response = await mppx.charge({ amount: '1' })(request)

  if (response.status === 402) return response.challenge

  return response.withReceipt(Response.json({ data: '...' }))
}
```

Generate `MPP_SECRET_KEY` with at least 32 bytes, for example: `openssl rand -base64 32`.

### Client

```ts
import { privateKeyToAccount } from 'viem/accounts'
import { Mppx, tempo } from 'mppx/client'

Mppx.create({
  methods: [tempo({ account: privateKeyToAccount('0x...') })],
})

// Global fetch now handles 402 automatically
const res = await fetch('https://mpp.dev/api/ping/paid')
```

## Examples

| Example                                                | Description                                          |
| ------------------------------------------------------ | ---------------------------------------------------- |
| [charge](./examples/charge/)                           | Payment-gated photo generation API                   |
| [charge-wagmi](./examples/charge-wagmi/)               | Payment-gated charge with Wagmi + React              |
| [session/multi-fetch](./examples/session/multi-fetch/) | Multiple paid requests over a single payment channel |
| [session/sse](./examples/session/sse/)                 | Pay-per-token LLM streaming with SSE                 |
| [stripe](./examples/stripe/)                           | Stripe SPT charge with automatic client              |

```bash
npx gitpick wevm/mppx/examples/charge
```

## CLI

`mppx` includes a basic CLI for making HTTP requests with automatic payment handling.

```bash
# create account - stored in keychain, autofunded on testnet
mppx account create

# make request - automatic payment handling, curl-like api
mppx example.com
```

You can also install globally to use the `mppx` CLI from anywhere:

```bash
npm i -g mppx
```

## Payments Proxy

`mppx` exports a `Proxy` server handler so that you can create or define a 402-protected payments proxy for any API.

```ts
import { openai, stripe, Proxy } from 'mppx/proxy'
import { Mppx, tempo } from 'mppx/server'

const mppx = Mppx.create({
  methods: [tempo()],
  secretKey: process.env.MPP_SECRET_KEY!,
})

const proxy = Proxy.create({
  services: [
    openai({
      apiKey: 'sk-...',
      routes: {
        'POST /v1/chat/completions': mppx.charge({ amount: '0.05' }),
        'POST /v1/completions': mppx.tempo.session({
          amount: '0.0001',
          unitType: 'token',
        }),
        'GET /v1/models': true,
      },
    }),
    stripe({
      apiKey: 'sk-...',
      routes: {
        'POST /v1/charges': mppx.charge({ amount: '0.01' }),
        'GET /v1/customers/:id': true,
      },
    }),
  ],
})

createServer(proxy.listener) // Node.js
Bun.serve(proxy) // Bun
Deno.serve(proxy.fetch) // Deno
app.use(proxy.listener) // Express
app.all('*', (c) => proxy.fetch(c.req.raw)) // Hono
app.all('*', (c) => proxy.fetch(c.request)) // Elysia
export const GET = proxy.fetch // Next.js
export const POST = proxy.fetch // Next.js
```

This exposes the following routes:

| Route                              | Pricing                       |
| ---------------------------------- | ----------------------------- |
| `POST /openai/v1/chat/completions` | charge **$0.005**             |
| `POST /openai/v1/completions`      | session **$0.0001 per token** |
| `GET /openai/v1/models`            | free                          |
| `POST /stripe/v1/charges`          | charge **$0.01**              |
| `GET /stripe/v1/customers/:id`     | free                          |

## Protocol

Built on the ["Payment" HTTP Authentication Scheme](https://datatracker.ietf.org/doc/draft-ryan-httpauth-payment/). See [mpp-specs](https://github.com/tempoxyz/mpp-specs) for the full specification.

## License

MIT
