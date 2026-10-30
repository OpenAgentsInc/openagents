# NIP-65: Relay list metadata

[All implemented NIPs](/nip) · [Pinned specification](https://github.com/OpenAgentsInc/openagents/blob/main/nips/official/65.md)

A user can publish the relays they read from and write to, so peers know where to look.

## Implementation

The production relay advertises NIP-65. Pylon planning includes publishing its selected relays as a NIP-65 list.

## Try the example

This demo reads a frozen public NIP-11 capability document. It does not execute this NIP on a relay.

- **DECLARED:** find this NIP in the frozen production relay information.
- **REMOVED:** read a document whose list omits it.
- **MALFORMED:** refuse a capability list with the wrong JSON type.

The page draws its observations in Coax on WebGPU. Its examples are fixed public fixtures; no key is entered, no event is published, and a successful example grants no authority.

## What remains

The capability declaration does not establish a complete Coax outbox-discovery or multi-relay synchronization client.

## Source

- [psionic/lib/pylon/](https://github.com/OpenAgentsInc/openagents/tree/main/psionic/lib/pylon/)
- [coax/nostr.md](https://github.com/OpenAgentsInc/openagents/blob/main/coax/nostr.md)

The page examples are in [nip-guide-observe.coax](https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents-web/nip-guide-observe.coax). Relay-only examples inspect NIP-11 metadata, not relay behavior. The catalog distinguishes native library, partial support and relay declarations.
