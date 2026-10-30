# NIP-17: Private messages

[All implemented NIPs](/nip) · [Pinned specification](https://github.com/OpenAgentsInc/openagents/blob/main/nips/official/17.md)

Private messages compose signed and encrypted envelopes. Relays transport ciphertext rather than a public plaintext conversation.

## Implementation

The production relay advertises NIP-17. The browser encrypted-deck path already uses the existing NIP-44 and gift-wrap plumbing.

## Try the example

This demo reads a frozen public NIP-11 capability document. It does not execute this NIP on a relay.

- **DECLARED:** find this NIP in the frozen production relay information.
- **REMOVED:** read a document whose list omits it.
- **MALFORMED:** refuse a capability list with the wrong JSON type.

The page draws its observations in Coax on WebGPU. Its examples are fixed public fixtures; no key is entered, no event is published, and a successful example grants no authority.

## What remains

A complete shared Coax private-message client and its NIP-59 composition remain unfinished.

## Source

- [coax/web/unlock/](https://github.com/OpenAgentsInc/openagents/tree/main/coax/web/unlock/)
- [coax/nostr.md](https://github.com/OpenAgentsInc/openagents/blob/main/coax/nostr.md)

The page examples are in [nip-guide-observe.coax](https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents-web/nip-guide-observe.coax). Relay-only examples inspect NIP-11 metadata, not relay behavior. The catalog distinguishes native library, partial support and relay declarations.
