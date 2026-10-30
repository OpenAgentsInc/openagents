# NIP-40: Event expiration

[All implemented NIPs](/nip) · [Pinned specification](https://github.com/OpenAgentsInc/openagents/blob/main/nips/official/40.md)

An expiration tag limits how long a relay should serve an event. The exact expiry second is already expired.

## Implementation

The Coax relay checks expiration during admission and queries. Its store retains expired replacement heads so older versions do not return.

## Try the example

The shared relay expiration predicate runs at explicit fixture times. No event is published or deleted.

- **BEFORE**: The original public fixture.
- **AT EXPIRY**: A changed fixture that exercises the stated boundary.
- **NO EXPIRY**: A size, shape, time or capability boundary.

The page draws its observations in Coax on WebGPU. Its examples are fixed public fixtures; no key is entered, no event is published, and a successful example grants no authority.

## What remains

Expiration is not a deletion guarantee, and a relay cannot erase copies held elsewhere.

## Source

- [coax/lib/nostr/relay_server.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/relay_server.coax)
- [coax/lib/nostr/store_core.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/store_core.coax)

## Verification records

- [docs/verification/2026-10-29-nostr-store-r1.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/verification/2026-10-29-nostr-store-r1.md)

The page examples are in [nip-guide-observe.coax](https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents-web/nip-guide-observe.coax). Relay-only examples inspect NIP-11 metadata, not relay behavior. The catalog distinguishes native library, partial support and relay declarations.
