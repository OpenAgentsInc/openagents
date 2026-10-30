# NIP-22: Comments with explicit scope

[All implemented NIPs](/nip) · [Pinned specification](https://github.com/OpenAgentsInc/openagents/blob/main/nips/official/22.md)

A comment names its root and parent. Uppercase scope tags and lowercase parent tags make that relationship explicit.

## Implementation

Coax has a typed reader for kind 1111 comments and the task-plan, claim and revision tags OpenAgents uses with them.

## Try the example

Synthetic comment tags run through the shared typed reader. Scope shape is not signature or permission.

- **COMMENT**: The original public fixture.
- **NO PARENT**: A changed fixture that exercises the stated boundary.
- **WRONG KIND**: A size, shape, time or capability boundary.

The page draws its observations in Coax on WebGPU. Its examples are fixed public fixtures; no key is entered, no event is published, and a successful example grants no authority.

## What remains

The typed reader reads structure. Callers still verify the original signature and decide whose comments count.

## Source

- [coax/lib/nostr/nip22.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/nip22.coax)
- [coax/lib/nostr/nip34-event.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/nip34-event.coax)

The page examples are in [nip-guide-observe.coax](https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents-web/nip-guide-observe.coax). Relay-only examples inspect NIP-11 metadata, not relay behavior. The catalog distinguishes native library, partial support and relay declarations.
