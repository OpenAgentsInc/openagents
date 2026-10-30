# NIP-09: Deletion requests

[All implemented NIPs](/nip) · [Pinned specification](https://github.com/OpenAgentsInc/openagents/blob/main/nips/official/09.md)

Authors can ask relays to delete their own events. A deletion request is a signed event, not a command from any reader.

## Implementation

The production relay advertises NIP-09. Its Coax replacement does not yet implement deletion and tombstones.

## Try the example

This demo reads a frozen public NIP-11 capability document. It does not execute this NIP on a relay.

- **DECLARED:** find this NIP in the frozen production relay information.
- **REMOVED:** read a document whose list omits it.
- **MALFORMED:** refuse a capability list with the wrong JSON type.

The page draws its observations in Coax on WebGPU. Its examples are fixed public fixtures; no key is entered, no event is published, and a successful example grants no authority.

## What remains

Coax needs authorized deletion, durable tombstones and recovery that cannot resurrect deleted events.

## Source

- [coax/nostr.md](https://github.com/OpenAgentsInc/openagents/blob/main/coax/nostr.md)
- [coax/lib/nostr/store_core.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/store_core.coax)

The page examples are in [nip-guide-observe.coax](https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents-web/nip-guide-observe.coax). Relay-only examples inspect NIP-11 metadata, not relay behavior. The catalog distinguishes native library, partial support and relay declarations.
