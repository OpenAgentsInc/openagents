# `@openagentsinc/mkt-swp`

Shared, framework-neutral MKT-SWP requester contracts. The browser product and
its concrete relay, storage, signing, wallet, and rendering hosts live in
`OpenAgentsInc/bazaar`.

## Production engine boundary

`immortal-browser-abi.ts` is the only supported production engine binding. It
consumes Immortal browser ABI v1 from commit
`d62a4f7c6c34a11d191fe78316fd8d4ce4da1d34` and pins requester contract digest
`bf52fda5f4d349fbbe195e4cff58af59a3930e1ee8ab1f1413b6338ba44fb3a8`.

The adapter rejects imported WASM authority, mismatched metadata, bounds, or
operation inventories, duplicate JSON members, widened responses, invalid
operation results, and concurrent access to the ABI's pointer-free global
buffer. Its Effect service and operation wrappers expose typed failures and
strictly decoded results. Funding remains an external host effect: the engine
returns an exact action descriptor and never receives signer, wallet, relay,
or custody authority.

The earlier `SwapEngine`, `EngineModuleLoader`, `fixtureEngineLayer`, and
`SwapWidgetHost` prototype has been removed. Presentation state now carries
the exact engine-prepared `ImmortalFundingRequest`; there is no alternate
funding authorization contract in this package.

## Discovery boundary

`live-discovery.ts` owns deterministic, transport-neutral Nostr subscription
framing and reduction for signed Provider and Offering heads. It verifies each
event through `@openagentsinc/nip-mkt`, enforces frame and head bounds, applies
replaceable-event ordering, publishes an atomic snapshot only at EOSE, and
projects serviceable MKT-SWP Offerings into `@openagentsinc/mkt-swp-pair`.

The host owns WebSocket lifecycle, reconnect, NIP-42 policy, private gift-wrap
delivery, and relay selection. Those capabilities are not implied by this
package.

## Presentation contracts

`widget-state.ts`, `primary-action.ts`, `compose.ts`, and `view-model.ts` retain
the shared state and primary-action law used by existing renderers. They do
not perform profile verification or authorize funding. New product code should
derive protocol state from `ImmortalRequesterSessionView` returned by the
production ABI.

## Ownership

| Concern                                         | Owner                                  |
| ----------------------------------------------- | -------------------------------------- |
| Browser ABI validation and requester operations | this package                           |
| Deterministic public-head discovery reducer     | this package                           |
| Pair, amount, fee, and rate model               | `@openagentsinc/mkt-swp-pair`          |
| Destination parsing and verification            | `@openagentsinc/mkt-swp-destination`   |
| Local session persistence and migration         | `@openagentsinc/mkt-swp-session-store` |
| Concrete browser product and host effects       | `OpenAgentsInc/bazaar`                 |
| Protocol engine and executable fixtures         | `OpenAgentsInc/immortal`               |

No key material, preimages, wallet action, relay transport, or mainnet claim
lives here.

## Verification

The ordinary package gate uses static fixtures and skips the cross-repository
compiled-WASM test. Reproduce the executable compatibility gate from a clean
Immortal checkout at the pinned revision:

```sh
IMMORTAL_SOURCE_DIR=/path/to/immortal-d62 \
  pnpm --filter @openagentsinc/mkt-swp run test:immortal-integration
```

The script refuses any other Immortal revision, rebuilds the WASM with its
source revision embedded, and runs Order, session create/ingest/restore,
funding preparation, exact-request authorization, and mismatch refusal against
Immortal's upstream fixtures.
