# @openagents/nip90

Thin OpenAgents workspace surface for NIP-90 Data Vending Machine protocol
helpers.

The implementation is intentionally re-exported from the sibling
`../nostr-effect` checkout:

```ts
export * from "../../../../nostr-effect/src/core/Nip90.js"
```

Do not rebuild Nostr event, tag, kind, or validation primitives in this package.
Extend `nostr-effect` first, then expose the shared surface here for OpenAgents
apps that need a workspace package import.

## Contract

This package covers protocol-only behavior:

- job request kinds `5000`-`5999`
- result kinds `6000`-`6999`
- feedback kind `7000`
- OpenAgents labor-market request kinds `5934` code task, `5935` review, and
  `5936` document work, with result kinds `6934`-`6936`
- NIP-DS dataset listing kind `30404`
- NIP-DS dataset offer kind `30406`
- DS-DVM dataset access request/result kinds `5960`/`6960`
- `i`, `param`, `output`, `relays`, `bid`, `amount`, and `bolt11` tags
- labor input refs, acceptance criteria, compliant-usage policy refs, expected
  artifact descriptors, and result artifact refs
- dataset `d`, `title`, `x`, `published_at`, `delivery`, `price`,
  `payment`, and linked listing/offer `a` tags
- feedback statuses `payment-required`, `processing`, `success`, `error`, and
  `partial`
- Effect Schema-backed event validation, typed malformed-event errors, and
  SHA-256 digest verification for delivered bundles

Historical contract reference:

- `f5919c766^:crates/nostr/core/src/nip90/`
- `f5919c766^:crates/nostr/nips/DS.md`

## NIP-DS CLI

Use the script when an agent needs to turn a public-safe redacted bundle into a
dataset listing and offer:

```bash
bun apps/openagents.com/scripts/nip-ds.ts draft \
  --file ./bundle.json \
  --title "Redacted conversation bundle" \
  --d redacted-conversation-bundle \
  --price-sats 50
```

For the integrated scoped-relay proof:

```bash
bun apps/openagents.com/scripts/nip-ds.ts smoke \
  --relay https://openagents-market-relay.openagents.workers.dev
```

The script signs and publishes a `30404` listing, `30406` offer, `5960` access
request, and `6960` access result, then reads them back and verifies the
delivered bundle digest. It does not move sats or publish private datasets.

## Verification

```bash
bun run --cwd packages/nip90 typecheck
bun run --cwd packages/nip90 test
bun test apps/pylon/tests/nip90-import.test.ts
cd apps/openagents.com/workers/api && bunx vitest run src/nip90-import.test.ts
```
