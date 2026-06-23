# @openagentsinc/nip90

Thin OpenAgents workspace surface for NIP-90 Data Vending Machine protocol
helpers.

The implementation is intentionally re-exported from the `nostr-effect`
package:

```ts
export * from "nostr-effect/nip90"
export * from "./lbr.js"
```

Do not rebuild Nostr event, tag, kind, or validation primitives in this package.
Extend `nostr-effect` first, then expose the shared surface here for OpenAgents
apps that need a workspace package import.

The `./lbr` export is an OpenAgents-specific NIP-LBR wrapper over the shared
labor primitives. It keeps the relay payload ref-only for the labor request,
quote, acceptance, and result lifecycle described in `docs/nips/LBR.md`.

## Contract

This package covers protocol-only behavior:

- job request kinds `5000`-`5999`
- result kinds `6000`-`6999`
- feedback kind `7000`
- OpenAgents labor-market request kinds `5934` code task, `5935` review, and
  `5936` document work, with result kinds `6934`-`6936`
- NIP-LBR agentic-coding request/result helpers for `5934`/`6934`, quote and
  acceptance feedback on `7000`, and decode-time rejection of raw prompts,
  private paths, credentials, and payment material
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
- NIP-LBR labor-market **closeout receipt** (`./lbr-closeout`): composes one
  complete labor lifecycle (request `5934`, quote `7000`, acceptance `7000`,
  result `6934`) into a single content-addressed, public-safe
  `LbrLaborCloseout`. It re-decodes each event through the `./lbr` ref/payment
  guards, checks the four events describe one job (consistent `requestId`,
  consistent requester/provider parties, quote within budget), and hashes a
  canonical projection of the public-safe refs. `verifyLbrLaborCloseoutDigest`
  re-derives the digest and `receiptRef` so any reader can confirm the receipt
  dereferences the exact lifecycle that produced it. This is the labor market's
  dereferenceable-receipt rung; it moves no sats and grants no settlement
  authority.

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
  --relay https://relay.openagents.com
```

The script signs and publishes a `30404` listing, `30406` offer, `5960` access
request, and `6960` access result, then reads them back and verifies the
delivered bundle digest. It does not move sats or publish private datasets.

## NIP-LBR closeout proof

The labor-market equivalent of the NIP-DS proof. It signs a complete LBR
lifecycle, composes the four accepted events into one content-addressed
closeout receipt, and re-derives the digest to prove the receipt dereferences:

```bash
# Offline (default): build + sign + bind + dereference, no network.
bun packages/nip90/scripts/lbr-closeout-proof.ts

# Relay smoke (optional): also publish the four events to a scoped market relay
# and read them back, proving the lifecycle is dippable before the closeout
# binds it.
bun packages/nip90/scripts/lbr-closeout-proof.ts --relay https://relay.openagents.com
```

It signs a `5934` request, two `7000` feedback events (quote + acceptance), and
a `6934` result, then prints the closeout receipt with `closeoutDereferenced:
true`. It moves no sats, opens no escrow, and grants no settlement authority — a
live paid labor run is the owner gate.

## Verification

```bash
bun run --cwd packages/nip90 typecheck
bun run --cwd packages/nip90 test
bun run --cwd apps/nostr-relay test
bun test apps/pylon/tests/nip90-import.test.ts
cd apps/openagents.com/workers/api && bunx vitest run src/nip90-import.test.ts
```
