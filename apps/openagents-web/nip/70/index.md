# NIP-70: Protected events

[All implemented NIPs](/nip) · [Pinned specification](https://github.com/OpenAgentsInc/openagents/blob/main/nips/official/70.md)

A protected event may only be published by its authenticated author, when a relay supports accepting it.

## Implementation

The production relay advertises NIP-70. The Coax relay has AUTH plumbing but not this complete protected-event policy.

## Try the example

This demo reads a frozen public NIP-11 capability document. It does not execute this NIP on a relay.

- **DECLARED:** find this NIP in the frozen production relay information.
- **REMOVED:** read a document whose list omits it.
- **MALFORMED:** refuse a capability list with the wrong JSON type.

The page draws its observations in Coax on WebGPU. Its examples are fixed public fixtures; no key is entered, no event is published, and a successful example grants no authority.

## What remains

AUTH alone does not implement NIP-70. The event author must be bound to the authenticated connection under the protected-event rules.

## Source

- [coax/nostr.md](https://github.com/OpenAgentsInc/openagents/blob/main/coax/nostr.md)
- [coax/lib/nostr/relay_route.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/relay_route.coax)

The page examples are in [nip-guide-observe.coax](https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents-web/nip-guide-observe.coax). Relay-only examples inspect NIP-11 metadata, not relay behavior. The catalog distinguishes native library, partial support and relay declarations.
