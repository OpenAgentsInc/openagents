# NIP-19: Names people can share

[All implemented NIPs](/nip) · [Pinned specification](https://github.com/OpenAgentsInc/openagents/blob/main/nips/official/19.md)

npub, note, nprofile, nevent and naddr turn keys and references into checksummed strings.

## Implementation

Coax encodes and decodes the NIP-19 forms, including bounded TLV references and relay hints. Project pages use the public forms.

## Try the example

A public example runs through the shared NIP-19 decoder, including checksum and output-size refusals.

- **NPUB**: The original public fixture.
- **CHECKSUM**: A changed fixture that exercises the stated boundary.
- **SHORT OUT**: A size, shape, time or capability boundary.

The page draws its observations in Coax on WebGPU. Its examples are fixed public fixtures; no key is entered, no event is published, and a successful example grants no authority.

## What remains

A checksum catches damaged text. It does not authenticate a person, relay hint or referenced event.

## Source

- [coax/lib/nostr/nip19-core.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/nip19-core.coax)
- [coax/lib/nostr/bech32.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/bech32.coax)
- [apps/openagents-web/project-names.coax](https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents-web/project-names.coax)

## Verification records

- [docs/verification/coax-nostr-bech32-2026-10-28.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/verification/coax-nostr-bech32-2026-10-28.md)

The page examples are in [nip-guide-observe.coax](https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents-web/nip-guide-observe.coax). Relay-only examples inspect NIP-11 metadata, not relay behavior. The catalog distinguishes native library, partial support and relay declarations.
