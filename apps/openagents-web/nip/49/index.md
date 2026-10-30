# NIP-49: Encrypted key backup foundations

[All implemented NIPs](/nip) · [Pinned specification](https://github.com/OpenAgentsInc/openagents/blob/main/nips/official/49.md)

Password-protected key backup needs a costly KDF, a complete encrypted format, and careful memory and key handling.

## Implementation

Coax has a resumable scrypt foundation for NIP-49, bounded workspaces and independent vector checks. This page exercises its buffer-admission helper.

## Try the example

The existing scrypt buffer-admission helper checks public fixture addresses and sizes. No password is derived.

- **FITS**: The original public fixture.
- **BAD COST**: A changed fixture that exercises the stated boundary.
- **SHORT MEMORY**: A size, shape, time or capability boundary.

The page draws its observations in Coax on WebGPU. Its examples are fixed public fixtures; no key is entered, no event is published, and a successful example grants no authority.

## What remains

The complete ncryptsec backup format, normalization, secret erasure and production approval are not established. This is a partial foundation.

## Source

- [coax/lib/crypto/scrypt_nip49.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/crypto/scrypt_nip49.coax)
- [coax/lib/crypto/scrypt_admission.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/crypto/scrypt_admission.coax)

## Verification records

- [docs/verification/2026-10-29-scrypt-nip49.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/verification/2026-10-29-scrypt-nip49.md)

The page examples are in [nip-guide-observe.coax](https://github.com/OpenAgentsInc/openagents/blob/main/apps/openagents-web/nip-guide-observe.coax). Relay-only examples inspect NIP-11 metadata, not relay behavior. The catalog distinguishes native library, partial support and relay declarations.
