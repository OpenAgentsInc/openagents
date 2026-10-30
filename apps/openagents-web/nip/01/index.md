# NIP-01: Events and relay messages

[All implemented NIPs](/nip) · [Pinned specification](https://github.com/OpenAgentsInc/openagents/blob/main/nips/official/01.md)

The signed event is the common language of Nostr. Parse it, recompute its id, then check its signature.

## Implementation

Coax reads and verifies events, matches filters, reads standard tags and metadata, and builds signed events. The NIP-01 page runs five real verification cases.

## Try the example

The captured event and altered copies run through the shared event reader and signature verifier.

- **REAL (1):** the captured event verifies.
- **ID (2):** an altered id refuses at the hash comparison.
- **TEXT (3):** changed content refuses at the hash comparison.
- **SIG (4):** a changed signature refuses at BIP-340.
- **KIND (5):** kind 99999 refuses while parsing.
- **PARTS (6):** inspect what is implemented and where it runs.

The page draws its observations in Coax on WebGPU. Its examples are fixed public fixtures; no key is entered, no event is published, and a successful example grants no authority.

## The event verification demo

The original /nip/01 pipeline remains: REAL verifies a captured kind 30617 event; ID and TEXT refuse at the id check; SIG refuses at BIP-340; KIND refuses during parsing. Select with keys 1 to 5 or the buttons. PARTS (6) lists the implementation. Signing, typed tags, metadata and message codecs are now on main. The Coax signer uses a fixed-work self-check, but production keys still await review. The project pages read live relay traffic but do not yet locally verify every kept event.

## What remains

Live project pages still trust the configured relay. Production signing keys wait for the secret audit and independent review.

## Source

- [coax/lib/nostr/event.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/event.coax)
- [coax/lib/nostr/message.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/message.coax)
- [coax/lib/nostr/event_sign.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/event_sign.coax)

## Verification records

- [docs/verification/2026-10-30-nostr-messages.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/verification/2026-10-30-nostr-messages.md)

The page examples are in [nip01-verify.coax](https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents-web/nip01-verify.coax). Relay-only examples inspect NIP-11 metadata, not relay behavior. The catalog distinguishes native library, partial support and relay declarations.
