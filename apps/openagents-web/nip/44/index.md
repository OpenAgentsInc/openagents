# NIP-44: Version 2 encryption

[All implemented NIPs](/nip) · [Pinned specification](https://github.com/OpenAgentsInc/openagents/blob/main/nips/official/44.md)

A conversation key derives message keys. The receiver checks the MAC before releasing decrypted plaintext.

## Implementation

Coax implements NIP-44 v2 payload encryption and decryption, and conversation-key derivation from validated x-only ECDH.

## Try the example

The shared decryptor reads a pinned upstream public vector. Changed or truncated ciphertext must refuse.

- **VECTOR**: The original public fixture.
- **BAD MAC**: A changed fixture that exercises the stated boundary.
- **TRUNCATED**: A size, shape, time or capability boundary.

The page draws its observations in Coax on WebGPU. Its examples are fixed public fixtures; no key is entered, no event is published, and a successful example grants no authority.

## What remains

These public test vectors are not permission to enter real keys. Production secret use still needs audit, target evidence and independent review.

## Source

- [coax/lib/nostr/nip44.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/nip44.coax)
- [coax/lib/nostr/nip44-key.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/nostr/nip44-key.coax)

## Verification records

- [docs/verification/2026-10-30-nip44-conversation-key.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/verification/2026-10-30-nip44-conversation-key.md)

The page examples are in [nip-guide-observe.coax](https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents-web/nip-guide-observe.coax). Relay-only examples inspect NIP-11 metadata, not relay behavior. The catalog distinguishes native library, partial support and relay declarations.
