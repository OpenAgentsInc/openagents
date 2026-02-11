# @openagentsinc/lightning-effect

Effect-first Lightning + L402 contracts, errors, services, and base layers for OpenAgents and external consumers.

## What This Package Provides

- Typed contracts for invoices, L402 challenges, credentials, and spend policy.
- Tagged errors for policy, challenge parsing, payment, and credential state.
- Service tags for invoice payment, credential cache, spend policy, and L402 authorization.
- Reusable layers:
  - `CredentialCacheInMemoryLayer`
  - `makeSpendPolicyLayer(...)`
  - `L402ClientLiveLayer`
- Demo adapter:
  - `makeInvoicePayerDemoLayer(...)`

This package is app-agnostic and has no dependency on `apps/*`.

## Install (local monorepo)

```json
{
  "dependencies": {
    "@openagentsinc/lightning-effect": "file:../../packages/lightning-effect"
  }
}
```

## API Surface

- Root export: `@openagentsinc/lightning-effect`
- Subpath exports:
  - `@openagentsinc/lightning-effect/contracts`
  - `@openagentsinc/lightning-effect/errors`
  - `@openagentsinc/lightning-effect/services`
  - `@openagentsinc/lightning-effect/layers`
  - `@openagentsinc/lightning-effect/adapters`
  - `@openagentsinc/lightning-effect/l402`

## Example: Parse Challenge + Build Authorization Header

```ts
import { Effect } from "effect"
import { parseChallengeHeader, buildAuthorizationHeader } from "@openagentsinc/lightning-effect/l402"

const program = Effect.gen(function* () {
  const challenge = yield* parseChallengeHeader(
    'L402 invoice="lnbc...", macaroon="AgED...", amount_msats=2500',
  )

  const header = buildAuthorizationHeader({
    host: "api.example.com",
    macaroon: challenge.macaroon,
    preimageHex: "00".repeat(32),
    amountMsats: challenge.amountMsats ?? 2500,
    issuedAtMs: 1_700_000_000_000,
  })

  return { challenge, header }
})
```

## Example: Full Authorization Flow

```ts
import { Effect, Layer } from "effect"
import {
  L402ClientService,
  L402ClientLiveLayer,
  CredentialCacheInMemoryLayer,
  makeSpendPolicyLayer,
  makeInvoicePayerDemoLayer,
} from "@openagentsinc/lightning-effect"

const live = Layer.mergeAll(
  CredentialCacheInMemoryLayer,
  makeSpendPolicyLayer({
    defaultMaxSpendMsats: 100_000,
    allowedHosts: ["api.example.com"],
    blockedHosts: [],
  }),
  makeInvoicePayerDemoLayer({
    fixedAmountMsats: 2500,
    fixedPaidAtMs: 1_700_000_000_000,
  }),
  L402ClientLiveLayer,
)

const program = Effect.gen(function* () {
  const client = yield* L402ClientService

  const first = yield* client.authorizeRequest({
    url: "https://api.example.com/premium",
    maxSpendMsats: 10_000,
    challengeHeader: 'L402 invoice="lnbc...", macaroon="AgED...", amount_msats=2500',
  })

  const second = yield* client.authorizeRequest({
    url: "https://api.example.com/premium",
    maxSpendMsats: 10_000,
  })

  return { first, second }
}).pipe(Effect.provide(live))
```

## Scripts

- `npm run typecheck`
- `npm test`
- `npm run test:watch`
- `npm run effect:patch`
