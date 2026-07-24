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

## Owned relay load proof (`SARAH-NR-03`)

`@openagentsinc/sarah/relay-load-proof` measures publish and subscribe rate and
latency against a local `nostr-effect` `startTestRelay` host, a package mock,
or a remote `RELAY_URL`.

```sh
pnpm --dir packages/sarah run test -- src/relay-load-proof/relay-load-proof.test.ts
pnpm --dir packages/sarah run load-proof
RELAY_URL=wss://relay.openagents.com pnpm --dir packages/sarah run load-proof
```

Runbook: `docs/ops/2026-07-24-owned-nostr-relay-deploy.md`.
Pin: `nostr-effect` `77073343c68f159f3dea80ddbe9e9896b1f052f2`
(`nostr-effect/relay/node`, `…/postgres`).


## Nostr memory (`SARAH-NR-07`)

`@openagentsinc/sarah/nostr-memory` holds template builders for NIP-AE engrams
(kind `30174`), NIP-RS read state (kind `30078`), and NIP-ER reminders (kind
`30300`). Encryption uses an injectable cipher port. Production injects
NIP-44. Tests use a reversible fixture cipher.

Contract pointer: `docs/omega/2026-07-24-sarah-memory-runtime.md`.

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

## Community work units (`SARAH-CW-03`)

`@openagentsinc/sarah/community` holds pure types and templates that decompose
one Sarah tick into many bounded community work units. Each unit has a narrow
grant (target, allowed actions, budget, expiration, idempotency identity). No
unit carries a Sarah grant. An expired grant is refused, not extended.

```sh
pnpm --dir packages/sarah test -- src/community/work-units.test.ts
```

Source: `packages/sarah/src/community/work-units.ts`.
Spec: `docs/omega/2026-07-24-sarah-workroom-mvp-spec.md` §33 and §38.2.
Issue: OpenAgentsInc/openagents#9225.

## Experience points (`SARAH-CW-06`)

`@openagentsinc/sarah/xp` holds the community workroom experience namespace
`com.openagents.xp`, fixed point table, NIP-32 award templates (kind `1985`),
NIP-85 rank projection pure functions (kind `30382`), and NIP-58 badge
definition/award/profile templates (kinds `30009` / `8` / `10008`).

Experience is recognition of accepted work. It is not currency. Do not call it
"earnings". Settlement for a paid room stays deferred (`SARAH-CW-07` / #9230).

```sh
pnpm --dir packages/sarah test -- src/xp/xp.test.ts
```

Spec: `docs/omega/2026-07-24-sarah-workroom-mvp-spec.md` §35.

## Community journey proof (`SARAH-CW-09`)

`@openagentsinc/sarah/community-journey` simulates the automatable
outside-developer community workroom journey with mocks and emits
`openagents.sarah.community_journey_receipt.v1`. It does not require a real
outside developer or a live relay. Human invite, pane, and confirmation steps
remain residual.

```sh
pnpm --dir packages/sarah test
pnpm --dir packages/sarah run generate:community-journey-receipt -- --out fixtures/sarah-community-journey/receipt.simulated.json
node fixtures/sarah-community-journey/validate.mjs
```

Proof document: `docs/omega/2026-07-24-sarah-community-journey-proof.md`.
