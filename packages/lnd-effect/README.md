# @openagentsinc/lnd-effect

Effect-first LND contracts and service interfaces for OpenAgents and external consumers.

## What This Package Provides

- Typed contracts for core LND node/wallet, invoices, payments, and RPC transport primitives.
- Tagged errors for deterministic decode, transport, auth, and response parsing boundaries.
- Service tags for LND API boundaries:
  - `LndNodeService`
  - `LndWalletService`
  - `LndInvoiceService`
  - `LndPaymentService`
  - `LndTransportService`
- Deterministic adapters/layers for tests and local scaffolding.
- REST-first transport layer with typed error mapping.

This package is app-agnostic and has no dependency on `apps/*`.

## Relationship to `@openagentsinc/lightning-effect`

- `@openagentsinc/lightning-effect` models L402 buyer/seller workflows and payment orchestration.
- `@openagentsinc/lnd-effect` models the LND node/RPC boundary.
- `lightning-effect` adapters can consume `lnd-effect` services for concrete LND-backed execution.

## Install (local monorepo)

```json
{
  "dependencies": {
    "@openagentsinc/lnd-effect": "file:../../packages/lnd-effect"
  }
}
```

## API Surface Map

- Root export: `@openagentsinc/lnd-effect`
- Subpath exports:
  - `@openagentsinc/lnd-effect/contracts`
  - `@openagentsinc/lnd-effect/errors`
  - `@openagentsinc/lnd-effect/services`
  - `@openagentsinc/lnd-effect/layers`
  - `@openagentsinc/lnd-effect/adapters`

## Example: Read Node Info With Deterministic Layer

```ts
import { Effect } from "effect"
import {
  LndNodeService,
  makeLndNodeDeterministicLayer,
} from "@openagentsinc/lnd-effect"

const program = Effect.gen(function* () {
  const lnd = yield* LndNodeService
  return yield* lnd.getNodeInfo()
}).pipe(Effect.provide(makeLndNodeDeterministicLayer()))
```

## Example: REST Transport Layer

```ts
import { Effect } from "effect"
import { LndTransportService, makeLndRestTransportLayer } from "@openagentsinc/lnd-effect"

const program = Effect.gen(function* () {
  const transport = yield* LndTransportService
  return yield* transport.send({
    method: "GET",
    path: "/v1/getinfo",
  })
}).pipe(
  Effect.provide(
    makeLndRestTransportLayer({
      endpoint: "https://localhost:8080",
      macaroonHex: "deadbeef",
    }),
  ),
)
```

## Migration Note

- This package introduces LND-specific contracts/services as a separate reusable boundary.
- `@openagentsinc/lightning-effect` remains the L402 domain package.
- Future integration should wire `lightning-effect` payment adapters to `lnd-effect` services rather than embedding LND specifics in app code.

## Scripts

- `npm run typecheck`
- `npm test`
- `npm run test:contracts`
- `npm run test:watch`
- `npm run effect:patch`
