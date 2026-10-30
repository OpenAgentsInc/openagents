# NIP-42: Authenticate to a relay

[All implemented NIPs](/nip) · [Pinned specification](https://github.com/OpenAgentsInc/openagents/blob/main/nips/official/42.md)

A relay sends a fresh challenge. The client signs a kind 22242 event bound to that relay and connection.

## Implementation

Coax handles challenges, canonical relay URLs, event templates, signature checks, connection generations and AUTH routing.

## Try the example

The shared challenge/template functions run on public fixtures. No key is entered and no AUTH is sent.

- **CHALLENGE**: The original public fixture.
- **STALE**: A changed fixture that exercises the stated boundary.
- **BAD RELAY**: A size, shape, time or capability boundary.

The page draws its observations in Coax on WebGPU. Its examples are fixed public fixtures; no key is entered, no event is published, and a successful example grants no authority.

## What remains

The demo builds an unsigned fixture template. It does not sign or send an AUTH event. Real keys remain behind the review gate.

## Source

- [coax/lib/nostr/nip42.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/nip42.coax)
- [coax/lib/nostr/nip42-event.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/nip42-event.coax)
- [coax/lib/nostr/relay_route.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/relay_route.coax)

## Verification records

- [docs/verification/2026-10-29-nostr-nip42.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/verification/2026-10-29-nostr-nip42.md)

The page examples are in [nip-guide-observe.coax](https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents-web/nip-guide-observe.coax). Relay-only examples inspect NIP-11 metadata, not relay behavior. The catalog distinguishes native library, partial support and relay declarations.
