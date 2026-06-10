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
- `i`, `param`, `output`, `relays`, `bid`, `amount`, and `bolt11` tags
- feedback statuses `payment-required`, `processing`, `success`, `error`, and
  `partial`
- Effect Schema-backed event validation and typed malformed-event errors

Historical contract reference:

- `f5919c766^:crates/nostr/core/src/nip90/`

## Verification

```bash
bun run --cwd packages/nip90 typecheck
bun run --cwd packages/nip90 test
bun test apps/pylon/tests/nip90-import.test.ts
cd apps/openagents.com/workers/api && bunx vitest run src/nip90-import.test.ts
```
