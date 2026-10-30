# NIP-34: Git repositories and issues

[All implemented NIPs](/nip) · [Pinned specification](https://github.com/OpenAgentsInc/openagents/blob/main/nips/official/34.md)

Repository announcements, issues and status events let development work travel as signed Nostr records.

## Implementation

OpenAgents publishes its work on Nostr. Coax has typed readers for repository, issue and status records, plus OpenAgents project and ordering records.

## Try the example

Synthetic repository tags run through the shared typed reader. It does not verify an event signature.

- **REPOSITORY**: The original public fixture.
- **NO ID**: A changed fixture that exercises the stated boundary.
- **WRONG KIND**: A size, shape, time or capability boundary.

The page draws its observations in Coax on WebGPU. Its examples are fixed public fixtures; no key is entered, no event is published, and a successful example grants no authority.

## What remains

Typed extraction and signature verification are separate. Project scope, allowed authors and complete live folding remain consumer responsibilities.

## Source

- [coax/lib/nostr/nip34.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/nip34.coax)
- [coax/lib/nostr/nip34-event.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/nip34-event.coax)
- [apps/openagents-web/project-fold.coax](https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents-web/project-fold.coax)

The page examples are in [nip-guide-observe.coax](https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents-web/nip-guide-observe.coax). Relay-only examples inspect NIP-11 metadata, not relay behavior. The catalog distinguishes native library, partial support and relay declarations.
