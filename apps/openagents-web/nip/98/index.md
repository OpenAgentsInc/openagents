# NIP-98: Authenticate an HTTP request

[All implemented NIPs](/nip) · [Pinned specification](https://github.com/OpenAgentsInc/openagents/blob/main/nips/official/98.md)

A signed kind 27235 event can bind an HTTP method, URL, timestamp and optional body digest to a request.

## Implementation

Coax has tag extraction and a public request-field observer. The existing service and production relay advertise HTTP-auth capability.

## Try the example

The existing observer compares public fixture fields. Matching fields are not authenticated HTTP access.

- **MATCH**: The original public fixture.
- **METHOD**: A changed fixture that exercises the stated boundary.
- **EXPIRED**: A size, shape, time or capability boundary.

The page draws its observations in Coax on WebGPU. Its examples are fixed public fixtures; no key is entered, no event is published, and a successful example grants no authority.

## What remains

The demo compares supplied fields only. It does not verify an event, reserve replay IDs, authorize a request or create a session.

## Source

- [coax/lib/nostr/nip98-tags.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/nip98-tags.coax)
- [coax/lib/nostr/nip98-observe.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/nip98-observe.coax)

## Verification records

- [docs/verification/2026-10-29-nip98-observation.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/verification/2026-10-29-nip98-observation.md)

The page examples are in [nip-guide-observe.coax](https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents-web/nip-guide-observe.coax). Relay-only examples inspect NIP-11 metadata, not relay behavior. The catalog distinguishes native library, partial support and relay declarations.
