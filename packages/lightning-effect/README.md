# @openagentsinc/lightning-effect

Effect-first Lightning + L402 contracts, errors, services, and base layers for OpenAgents and external consumers.

## What This Package Provides

- Typed contracts for invoices, L402 challenges, credentials, and spend policy.
- Typed seller/paywall contracts for hosted L402 infrastructure:
  - `PaywallDefinition`, `PaywallPolicy`, `PaywallRouteBinding`
  - `L402ChallengeIssueRequest` / `L402ChallengeIssueResult`
  - `L402AuthorizationVerificationResult`
  - `SettlementRecord`, `PayoutInstruction`, `GatewayDeploymentSnapshot`
- Tagged errors for policy, challenge parsing, payment, and credential state.
- Service tags for invoice payment, credential cache, spend policy, and L402 authorization.
- Service tags for seller-side orchestration:
  - `PaywallRegistryService`
  - `GatewayConfigCompilerService`
  - `InvoiceIssuerService`
  - `SettlementIngestService`
  - `SellerPolicyService`
- Reusable layers:
  - `CredentialCacheInMemoryLayer`
  - `makeSpendPolicyLayer(...)`
  - `L402ClientLiveLayer`
- Deterministic seller test layers/adapters:
  - `makePaywallRegistryInMemoryLayer(...)`
  - `makeGatewayConfigCompilerDeterministicLayer(...)`
  - `makeInvoiceIssuerDeterministicLayer(...)`
  - `makeSettlementIngestInMemoryLayer(...)`
  - `makeSellerPolicyDeterministicLayer(...)`
  - `makeSellerDeterministicLayer(...)`
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

## Example: Seller-Side Deterministic Layer

```ts
import { Effect } from "effect"
import {
  GatewayConfigCompilerService,
  PaywallRegistryService,
  makeSellerDeterministicLayer,
} from "@openagentsinc/lightning-effect"

const layer = makeSellerDeterministicLayer()

const program = Effect.gen(function* () {
  const registry = yield* PaywallRegistryService
  const compiler = yield* GatewayConfigCompilerService

  const paywall = yield* registry.upsert({
    paywallId: "paywall_ep212",
    ownerId: "user_123",
    name: "Premium Feed",
    status: "active",
    createdAtMs: 1_700_000_000_000,
    updatedAtMs: 1_700_000_000_000,
    route: {
      paywallId: "paywall_ep212",
      hostPattern: "api.example.com",
      pathPattern: "^/premium",
      upstreamUrl: "https://upstream.example.com/premium",
      priority: 10,
    },
    policy: {
      paywallId: "paywall_ep212",
      pricingMode: "fixed_msats",
      fixedAmountMsats: 2_500,
      allowedBuyerHosts: [],
      blockedBuyerHosts: [],
      killSwitch: false,
    },
  })

  return yield* compiler.compilePaywalls([paywall])
}).pipe(Effect.provide(layer))
```

## Migration Note

- Buyer-side APIs remain backward compatible.
- New seller-side contracts/services are additive and available through the same root and subpath exports.
- `@openagentsinc/lightning-effect` remains app-agnostic (no `apps/*` imports).

## Scripts

- `npm run typecheck`
- `npm test`
- `npm run test:contracts`
- `npm run test:watch`
- `npm run effect:patch`
