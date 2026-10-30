# NIP-77: Efficient synchronization

[All implemented NIPs](/nip) · [Pinned specification](https://github.com/OpenAgentsInc/openagents/blob/main/nips/official/77.md)

Negentropy helps peers find differences between event sets without downloading the whole shared history.

## Implementation

The production relay advertises NIP-77. The Coax replacement roadmap includes a separate synchronization implementation.

## Try the example

This demo reads a frozen public NIP-11 capability document. It does not execute this NIP on a relay.

- **DECLARED:** find this NIP in the frozen production relay information.
- **REMOVED:** read a document whose list omits it.
- **MALFORMED:** refuse a capability list with the wrong JSON type.

The page draws its observations in Coax on WebGPU. Its examples are fixed public fixtures; no key is entered, no event is published, and a successful example grants no authority.

## What remains

The shared Coax stack does not yet provide NIP-77 reconciliation. EOSE and ordinary pagination are different mechanisms.

## Source

- [coax/nostr.md](https://github.com/OpenAgentsInc/openagents/blob/main/coax/nostr.md)

The page examples are in [nip-guide-observe.coax](https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents-web/nip-guide-observe.coax). Relay-only examples inspect NIP-11 metadata, not relay behavior. The catalog distinguishes native library, partial support and relay declarations.
