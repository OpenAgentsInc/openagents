# NIP-11: Relay information

[All implemented NIPs](/nip) · [Pinned specification](https://github.com/OpenAgentsInc/openagents/blob/main/nips/official/11.md)

Before subscribing, a client can ask a relay which protocols and limits it declares.

## Implementation

The Coax relay serves its own NIP-11 document. The production relay publishes a separate capability document.

## Try the example

This demo reads a frozen public NIP-11 capability document. It does not execute this NIP on a relay.

- **DECLARED:** find this NIP in the frozen production relay information.
- **REMOVED:** read a document whose list omits it.
- **MALFORMED:** refuse a capability list with the wrong JSON type.

The page draws its observations in Coax on WebGPU. Its examples are fixed public fixtures; no key is entered, no event is published, and a successful example grants no authority.

## What remains

A supported_nips list is a declaration, not a conformance proof. Production and Coax relay capabilities differ.

## Source

- [coax/lib/nostr/relay_route.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/relay_route.coax)
- [coax/lib/nostr/relay_server.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/relay_server.coax)

## Verification records

- [docs/verification/2026-10-29-coax-relay-r1.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/verification/2026-10-29-coax-relay-r1.md)

The page examples are in [nip-guide-observe.coax](https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents-web/nip-guide-observe.coax). Relay-only examples inspect NIP-11 metadata, not relay behavior. The catalog distinguishes native library, partial support and relay declarations.
