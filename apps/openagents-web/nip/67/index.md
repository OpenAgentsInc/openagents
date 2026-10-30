# NIP-67: Relay pagination metadata

[All implemented NIPs](/nip) · [Pinned specification](https://github.com/OpenAgentsInc/openagents/blob/main/nips/official/67.md)

A relay can attach metadata to EOSE so a client can distinguish more available history from a finished page.

## Implementation

The production relay advertises NIP-67 and sends trailing EOSE metadata. Coax message.coax accepts bounded trailing EOSE values.

## Try the example

This demo reads a frozen public NIP-11 capability document. It does not execute this NIP on a relay.

- **DECLARED:** find this NIP in the frozen production relay information.
- **REMOVED:** read a document whose list omits it.
- **MALFORMED:** refuse a capability list with the wrong JSON type.

The page draws its observations in Coax on WebGPU. Its examples are fixed public fixtures; no key is entered, no event is published, and a successful example grants no authority.

## What remains

Parsing metadata is not proof of complete history. Catch-up still needs explicit limits, ties, retries and missing-record handling.

## Source

- [coax/lib/nostr/message.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/message.coax)
- [apps/openagents-web/project-live.coax](https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents-web/project-live.coax)

## Verification records

- [docs/verification/2026-10-30-nostr-messages.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/verification/2026-10-30-nostr-messages.md)

The page examples are in [nip-guide-observe.coax](https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents-web/nip-guide-observe.coax). Relay-only examples inspect NIP-11 metadata, not relay behavior. The catalog distinguishes native library, partial support and relay declarations.
