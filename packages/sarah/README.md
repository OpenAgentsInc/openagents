# `@openagentsinc/sarah`

Shared Sarah owner-orchestrator identity, mobile/API projections, capability
directory, cited business-context model, and the compiled runtime authority
profile. Sarah is a capability inside supported OpenAgents apps—not a restored
standalone app or public `/sarah` route.

## Nostr identity (`SARAH-NR-04`)

`@openagentsinc/sarah/nostr-identity` holds the sealed signer, Secret Manager
custody mount (`SARAH_NOSTR_IDENTITY_SECRET` → `sarah-nostr-identity-secret`),
NIP-OA/AA helpers, and lifecycle transitions for `principal.sarah`.

Contract: `docs/omega/2026-07-24-sarah-nostr-identity-contract.md`.

## Nostr turn ladder (`SARAH-NR-05`)

`@openagentsinc/sarah/nostr-turn` holds the durable kind-44300 / live NIP-AO
publish path and the relay-primary consumer.

## Nostr migration (`SARAH-NR-08`)

`@openagentsinc/sarah/nostr-migration` holds the stage machine
(`shadow` | `cutover` | `retirement`), `thread.sarah.<digest>` ↔
`sarah.<digest>` mapping, public-safe drift comparison, export/rollback
manifests, and `SARAH_NOSTR_RECORD_MODE=khala|shadow|nostr` resolution
(legacy `SARAH_NOSTR_SHADOW_PUBLISH=1` still maps to `shadow`).

Note: `docs/omega/2026-07-24-sarah-nostr-cutover.md`.
Production default stays `khala`. Cutover is operator-gated.

## Nostr journey proof (`SARAH-NR-09`)

`@openagentsinc/sarah/nostr-journey` simulates the automatable Nostr-backed
Sarah journey steps with mocks and emits
`openagents.sarah.nostr_journey_receipt.v1`. It does not require a signed Omega
install. Human install/bind/UI steps remain residual.

```sh
pnpm --dir packages/sarah test
pnpm --dir packages/sarah run generate:journey-receipt -- --out fixtures/sarah-nostr-journey/receipt.simulated.json
node fixtures/sarah-nostr-journey/validate.mjs
```

Proof document: `docs/omega/2026-07-24-sarah-nostr-journey-proof.md`.
