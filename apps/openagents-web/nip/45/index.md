# NIP-45: Count matching events

[All implemented NIPs](/nip) · [Pinned specification](https://github.com/OpenAgentsInc/openagents/blob/main/nips/official/45.md)

A COUNT request asks a relay how many events match a filter without downloading every event.

## Implementation

The production relay advertises NIP-45. The Coax R1 relay currently reports COUNT as unsupported.

## Try the example

This demo reads a frozen public NIP-11 capability document. It does not execute this NIP on a relay.

- **DECLARED:** find this NIP in the frozen production relay information.
- **REMOVED:** read a document whose list omits it.
- **MALFORMED:** refuse a capability list with the wrong JSON type.

The page draws its observations in Coax on WebGPU. Its examples are fixed public fixtures; no key is entered, no event is published, and a successful example grants no authority.

## What remains

COUNT responses and their limits need a Coax implementation before that relay advertises this NIP.

## Source

- [coax/lib/nostr/message_server.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/message_server.coax)
- [coax/lib/nostr/relay_server.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/relay_server.coax)

The page examples are in [nip-guide-observe.coax](https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents-web/nip-guide-observe.coax). Relay-only examples inspect NIP-11 metadata, not relay behavior. The catalog distinguishes native library, partial support and relay declarations.
