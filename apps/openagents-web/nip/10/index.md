# NIP-10: Replies and threads

[All implemented NIPs](/nip) · [Pinned specification](https://github.com/OpenAgentsInc/openagents/blob/main/nips/official/10.md)

An e tag can mark a root, reply or mention. These markers describe the thread rather than granting authority.

## Implementation

Coax reads the root, reply and mention markers of standard event tags.

## Try the example

A public synthetic tag fixture runs through the shared standard-tag reader. No event signature is checked here.

- **ROOT**: The original public fixture.
- **BAD MARKER**: A changed fixture that exercises the stated boundary.
- **BAD ID**: A size, shape, time or capability boundary.

The page draws its observations in Coax on WebGPU. Its examples are fixed public fixtures; no key is entered, no event is published, and a successful example grants no authority.

## What remains

Reading a reference does not verify the referenced event or implement an entire social client.

## Source

- [coax/lib/nostr/tags.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/tags.coax)

## Verification records

- [docs/verification/2026-10-30-nostr-tags-metadata.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/verification/2026-10-30-nostr-tags-metadata.md)

The page examples are in [nip-guide-observe.coax](https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents-web/nip-guide-observe.coax). Relay-only examples inspect NIP-11 metadata, not relay behavior. The catalog distinguishes native library, partial support and relay declarations.
